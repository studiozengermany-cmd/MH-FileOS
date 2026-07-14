//! Bounded, cancellable, read-only filesystem observation for MH FileOS.
//!
//! The concrete scanner streams observations directly to the application-owned sink, traverses
//! with a bounded iterative depth-first stack, and never owns filesystem mutation or UI behavior.

#![forbid(unsafe_code)]

use std::fs::{self, ReadDir};
use std::io;
use std::path::{Path, PathBuf};

use fileos_app::{
    CancellationToken, ScanFailure, ScanFailureCode, ScanPort, ScanProgress, ScanReport,
    ScanRequest, ScanSink,
};
use fileos_domain::{
    EntryKind, ObservedEntry, ResourceLimits, ResourceUsage, ScanCounters, ScanIssue,
    ScanIssueCode, ScanIssueCounts, ScanPhase, ScanRunStatus,
};
use fileos_platform_windows::{MetadataSnapshot, metadata_snapshot_no_follow};

/// Stateless concrete implementation of the application read-only scan port.
#[derive(Debug, Default)]
pub struct ReadOnlyScanner;

impl ReadOnlyScanner {
    /// Creates a scanner with no retained per-run state.
    pub const fn new() -> Self {
        Self
    }

    fn scan_with_hook(
        &self,
        request: &ScanRequest,
        cancellation: &CancellationToken,
        sink: &mut dyn ScanSink,
        before_metadata: &mut dyn FnMut(&Path),
    ) -> Result<ScanReport, ScanFailure> {
        let mut run = ScanState::new(request.limits());
        request
            .limits()
            .validate()
            .map_err(|_| run.invalid_limits_failure())?;
        if cancellation.is_cancelled() {
            return run.cancelled_report(sink);
        }

        let canonical_root = validate_root(request.root(), &run)?;
        if cancellation.is_cancelled() {
            return run.cancelled_report(sink);
        }

        let root_entries = fs::read_dir(&canonical_root)
            .map_err(|error| root_failure(&run, ScanPhase::DirectoryEnumeration, &error))?;
        let mut stack = vec![DirectoryFrame {
            relative_path: None,
            depth: 0,
            entries: root_entries,
        }];
        run.observe_open_directories(stack.len())?;
        run.observe_depth(0)?;

        while !stack.is_empty() {
            if cancellation.is_cancelled() {
                return run.cancelled_report(sink);
            }

            let (parent_relative_path, parent_depth, next) = {
                let frame = stack.last_mut().ok_or_else(|| run.internal_failure())?;
                (
                    frame.relative_path.clone(),
                    frame.depth,
                    frame.entries.next(),
                )
            };

            let directory_entry = match next {
                Some(Ok(entry)) => entry,
                Some(Err(error)) => {
                    run.stream_io_issue(
                        sink,
                        ScanPhase::DirectoryEnumeration,
                        parent_relative_path,
                        &error,
                    )?;
                    continue;
                }
                None => {
                    stack.pop();
                    continue;
                }
            };

            if cancellation.is_cancelled() {
                return run.cancelled_report(sink);
            }

            let path = directory_entry.path();
            let relative_path = relative_path(&canonical_root, &path)
                .ok_or_else(|| run.scope_failure(ScanPhase::DirectoryEnumeration))?;
            before_metadata(&relative_path);
            run.observe_metadata_activity()?;
            let snapshot = match metadata_snapshot_no_follow(&path) {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    run.stream_io_issue(
                        sink,
                        ScanPhase::MetadataRead,
                        Some(relative_path),
                        &error,
                    )?;
                    continue;
                }
            };

            let kind = entry_kind(&snapshot);
            let byte_len = (kind == EntryKind::File).then(|| snapshot.byte_len());
            let observation = ObservedEntry::new(
                relative_path.clone(),
                kind,
                byte_len,
                snapshot.modified(),
                snapshot.is_readonly(),
            )
            .map_err(|_| run.internal_failure())?;
            run.stream_observation(sink, observation)?;

            if kind == EntryKind::Other {
                run.stream_issue(
                    sink,
                    ScanIssue::new(
                        ScanIssueCode::UnsupportedEntryType,
                        ScanPhase::MetadataRead,
                        Some(relative_path),
                        false,
                        None,
                    )
                    .map_err(|_| run.internal_failure())?,
                )?;
                continue;
            }
            if kind != EntryKind::Directory {
                continue;
            }

            let child_depth = parent_depth.saturating_add(1);
            let depth_exceeded = child_depth > request.limits().max_depth().get();
            let handles_exceeded = stack.len() >= request.limits().open_directories().get();
            if depth_exceeded {
                run.stream_issue(
                    sink,
                    ScanIssue::new(
                        ScanIssueCode::DepthLimitReached,
                        ScanPhase::DirectoryEnumeration,
                        Some(relative_path),
                        false,
                        None,
                    )
                    .map_err(|_| run.internal_failure())?,
                )?;
                continue;
            }
            if handles_exceeded {
                run.stream_issue(
                    sink,
                    ScanIssue::new(
                        ScanIssueCode::OpenDirectoryLimitReached,
                        ScanPhase::DirectoryEnumeration,
                        Some(relative_path),
                        false,
                        None,
                    )
                    .map_err(|_| run.internal_failure())?,
                )?;
                continue;
            }

            if cancellation.is_cancelled() {
                return run.cancelled_report(sink);
            }

            run.observe_metadata_activity()?;
            let current = match metadata_snapshot_no_follow(&path) {
                Ok(current) => current,
                Err(error) => {
                    run.stream_io_issue(
                        sink,
                        ScanPhase::DirectoryEnumeration,
                        Some(relative_path),
                        &error,
                    )?;
                    continue;
                }
            };
            if current.entry_type().is_reparse_point() {
                run.stream_issue(
                    sink,
                    ScanIssue::new(
                        ScanIssueCode::DirectoryReadFailed,
                        ScanPhase::DirectoryEnumeration,
                        Some(relative_path),
                        true,
                        None,
                    )
                    .map_err(|_| run.internal_failure())?,
                )?;
                continue;
            }
            if !current.entry_type().is_directory() {
                run.stream_issue(
                    sink,
                    ScanIssue::new(
                        ScanIssueCode::Disappeared,
                        ScanPhase::DirectoryEnumeration,
                        Some(relative_path),
                        true,
                        None,
                    )
                    .map_err(|_| run.internal_failure())?,
                )?;
                continue;
            }

            let child_entries = match fs::read_dir(&path) {
                Ok(entries) => entries,
                Err(error) => {
                    run.stream_io_issue(
                        sink,
                        ScanPhase::DirectoryEnumeration,
                        Some(relative_path),
                        &error,
                    )?;
                    continue;
                }
            };
            stack.push(DirectoryFrame {
                relative_path: Some(relative_path),
                depth: child_depth,
                entries: child_entries,
            });
            run.observe_open_directories(stack.len())?;
            run.observe_depth(child_depth)?;
        }

        run.completed_report(sink)
    }
}

impl ScanPort for ReadOnlyScanner {
    fn scan(
        &self,
        request: &ScanRequest,
        cancellation: &CancellationToken,
        sink: &mut dyn ScanSink,
    ) -> Result<ScanReport, ScanFailure> {
        self.scan_with_hook(request, cancellation, sink, &mut |_| {})
    }
}

struct DirectoryFrame {
    relative_path: Option<PathBuf>,
    depth: usize,
    entries: ReadDir,
}

struct ScanState {
    limits: ResourceLimits,
    counters: ScanCounters,
    issue_counts: ScanIssueCounts,
    resource_usage: ResourceUsage,
    last_progress_entry_count: u64,
}

impl ScanState {
    fn new(limits: ResourceLimits) -> Self {
        Self {
            limits,
            counters: ScanCounters::default(),
            issue_counts: ScanIssueCounts::default(),
            resource_usage: ResourceUsage::default(),
            last_progress_entry_count: 0,
        }
    }

    fn observe_open_directories(&mut self, open: usize) -> Result<(), ScanFailure> {
        self.resource_usage
            .observe_open_directories(open, self.limits)
            .map_err(|_| self.resource_failure())
    }

    fn observe_depth(&mut self, depth: usize) -> Result<(), ScanFailure> {
        self.resource_usage
            .observe_depth(depth, self.limits)
            .map_err(|_| self.resource_failure())
    }

    fn observe_metadata_activity(&mut self) -> Result<(), ScanFailure> {
        self.resource_usage
            .observe_metadata_queue(1, self.limits)
            .map_err(|_| self.resource_failure())?;
        self.resource_usage
            .observe_metadata_workers(1, self.limits)
            .map_err(|_| self.resource_failure())
    }

    fn stream_observation(
        &mut self,
        sink: &mut dyn ScanSink,
        observation: ObservedEntry,
    ) -> Result<(), ScanFailure> {
        sink.observe(observation.clone())
            .map_err(|error| self.sink_failure(error.retryable()))?;
        self.counters.record_entry(&observation);
        self.maybe_emit_progress(sink)
    }

    fn stream_issue(
        &mut self,
        sink: &mut dyn ScanSink,
        issue: ScanIssue,
    ) -> Result<(), ScanFailure> {
        sink.issue(issue.clone())
            .map_err(|error| self.sink_failure(error.retryable()))?;
        self.counters.record_issue();
        self.issue_counts.record(issue.code());
        Ok(())
    }

    fn stream_io_issue(
        &mut self,
        sink: &mut dyn ScanSink,
        phase: ScanPhase,
        relative_path: Option<PathBuf>,
        error: &io::Error,
    ) -> Result<(), ScanFailure> {
        let issue = ScanIssue::new(
            issue_code(phase, error),
            phase,
            relative_path,
            is_retryable(error),
            error.raw_os_error(),
        )
        .map_err(|_| self.internal_failure())?;
        self.stream_issue(sink, issue)
    }

    fn maybe_emit_progress(&mut self, sink: &mut dyn ScanSink) -> Result<(), ScanFailure> {
        let total_entries = self.counters.total_entries();
        let cadence = u64::try_from(self.limits.progress_every_entries().get())
            .map_err(|_| self.resource_failure())?;
        if total_entries.saturating_sub(self.last_progress_entry_count) < cadence {
            return Ok(());
        }
        self.emit_progress(sink, ScanRunStatus::Running)
    }

    fn emit_progress(
        &mut self,
        sink: &mut dyn ScanSink,
        status: ScanRunStatus,
    ) -> Result<(), ScanFailure> {
        self.resource_usage.record_progress_event();
        let progress = ScanProgress::new(status, self.counters, self.resource_usage)
            .map_err(|_| self.internal_failure())?;
        sink.progress(progress)
            .map_err(|error| self.sink_failure(error.retryable()))?;
        self.last_progress_entry_count = self.counters.total_entries();
        Ok(())
    }

    fn emit_final_progress(
        &mut self,
        sink: &mut dyn ScanSink,
        status: ScanRunStatus,
    ) -> Result<(), ScanFailure> {
        if self.counters.total_entries() != self.last_progress_entry_count
            || self.resource_usage.progress_events_emitted() == 0
        {
            self.emit_progress(sink, status)?;
        }
        Ok(())
    }

    fn cancelled_report(&mut self, sink: &mut dyn ScanSink) -> Result<ScanReport, ScanFailure> {
        self.emit_final_progress(sink, ScanRunStatus::Cancelling)?;
        self.report(ScanRunStatus::Cancelled)
    }

    fn completed_report(&mut self, sink: &mut dyn ScanSink) -> Result<ScanReport, ScanFailure> {
        self.emit_final_progress(sink, ScanRunStatus::Running)?;
        let status = if self.issue_counts.total() == 0 {
            ScanRunStatus::Completed
        } else {
            ScanRunStatus::CompletedWithIssues
        };
        self.report(status)
    }

    fn report(&self, status: ScanRunStatus) -> Result<ScanReport, ScanFailure> {
        ScanReport::new(
            status,
            self.counters,
            self.issue_counts,
            self.limits,
            self.resource_usage,
        )
        .map_err(|_| self.internal_failure())
    }

    fn resource_failure(&self) -> ScanFailure {
        ScanFailure::new(
            ScanFailureCode::ResourceLimitExceeded,
            ScanPhase::Finalization,
            false,
            None,
            self.counters,
            self.resource_usage,
        )
    }

    fn invalid_limits_failure(&self) -> ScanFailure {
        ScanFailure::new(
            ScanFailureCode::ResourceLimitExceeded,
            ScanPhase::ScopeValidation,
            false,
            None,
            self.counters,
            self.resource_usage,
        )
    }

    fn sink_failure(&self, retryable: bool) -> ScanFailure {
        ScanFailure::new(
            ScanFailureCode::SinkRejected,
            ScanPhase::SinkDelivery,
            retryable,
            None,
            self.counters,
            self.resource_usage,
        )
    }

    fn scope_failure(&self, phase: ScanPhase) -> ScanFailure {
        ScanFailure::new(
            ScanFailureCode::ScopeViolation,
            phase,
            false,
            None,
            self.counters,
            self.resource_usage,
        )
    }

    fn internal_failure(&self) -> ScanFailure {
        ScanFailure::new(
            ScanFailureCode::InternalInvariant,
            ScanPhase::Finalization,
            false,
            None,
            self.counters,
            self.resource_usage,
        )
    }
}

fn validate_root(root: &Path, state: &ScanState) -> Result<PathBuf, ScanFailure> {
    let supplied = metadata_snapshot_no_follow(root)
        .map_err(|error| root_failure(state, ScanPhase::ScopeValidation, &error))?;
    if supplied.entry_type().is_reparse_point() {
        return Err(state.scope_failure(ScanPhase::ScopeValidation));
    }
    if !supplied.entry_type().is_directory() {
        return Err(ScanFailure::new(
            ScanFailureCode::InvalidRoot,
            ScanPhase::ScopeValidation,
            false,
            None,
            state.counters,
            state.resource_usage,
        ));
    }

    let canonical = fs::canonicalize(root)
        .map_err(|error| root_failure(state, ScanPhase::ScopeValidation, &error))?;
    let canonical_snapshot = metadata_snapshot_no_follow(&canonical)
        .map_err(|error| root_failure(state, ScanPhase::ScopeValidation, &error))?;
    if canonical_snapshot.entry_type().is_reparse_point()
        || !canonical_snapshot.entry_type().is_directory()
    {
        return Err(state.scope_failure(ScanPhase::ScopeValidation));
    }
    Ok(canonical)
}

fn relative_path(root: &Path, path: &Path) -> Option<PathBuf> {
    let relative = path.strip_prefix(root).ok()?;
    if relative.as_os_str().is_empty() {
        None
    } else {
        Some(relative.to_path_buf())
    }
}

fn entry_kind(snapshot: &MetadataSnapshot) -> EntryKind {
    let entry_type = snapshot.entry_type();
    if entry_type.is_reparse_point() {
        EntryKind::ReparsePoint
    } else if entry_type.is_directory() {
        EntryKind::Directory
    } else if entry_type.is_file() {
        EntryKind::File
    } else {
        EntryKind::Other
    }
}

fn issue_code(phase: ScanPhase, error: &io::Error) -> ScanIssueCode {
    match error.kind() {
        io::ErrorKind::PermissionDenied => ScanIssueCode::AccessDenied,
        io::ErrorKind::NotFound => ScanIssueCode::Disappeared,
        _ if phase == ScanPhase::DirectoryEnumeration => ScanIssueCode::DirectoryReadFailed,
        _ if phase == ScanPhase::MetadataRead => ScanIssueCode::MetadataUnavailable,
        _ => ScanIssueCode::IoOther,
    }
}

fn is_retryable(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::PermissionDenied
            | io::ErrorKind::NotFound
            | io::ErrorKind::Interrupted
            | io::ErrorKind::WouldBlock
            | io::ErrorKind::TimedOut
    )
}

fn root_failure(state: &ScanState, phase: ScanPhase, error: &io::Error) -> ScanFailure {
    let code = match error.kind() {
        io::ErrorKind::PermissionDenied => ScanFailureCode::RootAccessDenied,
        io::ErrorKind::NotFound => ScanFailureCode::RootUnavailable,
        _ => ScanFailureCode::ScopeViolation,
    };
    ScanFailure::new(
        code,
        phase,
        is_retryable(error),
        error.raw_os_error(),
        state.counters,
        state.resource_usage,
    )
}

#[cfg(test)]
mod tests;

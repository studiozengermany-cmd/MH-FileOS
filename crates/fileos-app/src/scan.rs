use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use fileos_domain::{
    ObservedEntry, ResourceLimits, ResourceUsage, ScanCounters, ScanIssue, ScanIssueCode,
    ScanIssueCounts, ScanPhase, ScanRunStatus,
};

/// Schema version emitted by [`ScanReport::to_json`].
pub const SCAN_SUMMARY_SCHEMA_VERSION: u32 = 1;

/// Cloneable cooperative cancellation signal shared by a scan owner and bounded workers.
#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    /// Creates a token in the active state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Requests cancellation and reports whether this call changed the token state.
    pub fn cancel(&self) -> bool {
        !self.cancelled.swap(true, Ordering::AcqRel)
    }

    /// Reports whether cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

/// Error returned when a scan request does not identify an explicit absolute root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvalidScanRequest {
    /// The supplied root was empty.
    EmptyRoot,
    /// The supplied root was relative and therefore depended on process state.
    RootMustBeAbsolute {
        /// Rejected local root; redact it before diagnostic export.
        root: PathBuf,
    },
}

impl fmt::Display for InvalidScanRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyRoot => formatter.write_str("scan root must not be empty"),
            Self::RootMustBeAbsolute { .. } => {
                formatter.write_str("scan root must be an explicit absolute path")
            }
        }
    }
}

impl Error for InvalidScanRequest {}

/// Validated input for one bounded read-only scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanRequest {
    root: PathBuf,
    limits: ResourceLimits,
}

impl ScanRequest {
    /// Creates a request without performing filesystem access.
    ///
    /// The concrete scanner must still validate canonical scope, directory kind, and reparse
    /// evidence immediately before traversal.
    pub fn new(root: PathBuf, limits: ResourceLimits) -> Result<Self, InvalidScanRequest> {
        if root.as_os_str().is_empty() {
            return Err(InvalidScanRequest::EmptyRoot);
        }
        if !root.is_absolute() {
            return Err(InvalidScanRequest::RootMustBeAbsolute { root });
        }
        Ok(Self { root, limits })
    }

    /// Returns the explicit local root, which must be redacted before diagnostic export.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns the scan resource bounds.
    pub const fn limits(&self) -> ResourceLimits {
        self.limits
    }
}

/// Error returned when aggregate progress is assigned a terminal status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidScanProgress {
    status: ScanRunStatus,
}

impl InvalidScanProgress {
    /// Returns the rejected terminal status.
    pub const fn status(self) -> ScanRunStatus {
        self.status
    }
}

impl fmt::Display for InvalidScanProgress {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("progress snapshot requires a non-terminal scan status")
    }
}

impl Error for InvalidScanProgress {}

/// Coalesced in-process progress evidence; never one event per filesystem entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanProgress {
    status: ScanRunStatus,
    counters: ScanCounters,
    resource_usage: ResourceUsage,
}

impl ScanProgress {
    /// Creates a non-terminal progress snapshot.
    pub fn new(
        status: ScanRunStatus,
        counters: ScanCounters,
        resource_usage: ResourceUsage,
    ) -> Result<Self, InvalidScanProgress> {
        if status.is_terminal() {
            return Err(InvalidScanProgress { status });
        }
        Ok(Self {
            status,
            counters,
            resource_usage,
        })
    }

    /// Returns the current non-terminal lifecycle state.
    pub const fn status(self) -> ScanRunStatus {
        self.status
    }

    /// Returns aggregate counters at this checkpoint.
    pub const fn counters(self) -> ScanCounters {
        self.counters
    }

    /// Returns resource high-water evidence at this checkpoint.
    pub const fn resource_usage(self) -> ResourceUsage {
        self.resource_usage
    }
}

/// Error returned when a terminal report would misrepresent scan truth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidScanReport {
    /// Reports accept only completed, qualified-partial, or cancelled states.
    NonReportableStatus {
        /// Rejected lifecycle status.
        status: ScanRunStatus,
    },
    /// `completed` cannot hide partial issues.
    CompletedHasIssues,
    /// `completed_with_issues` requires at least one partial issue.
    PartialStatusHasNoIssues,
    /// The aggregate counter and bounded per-code counts disagree.
    IssueCountMismatch {
        /// Total recorded by scan counters.
        counters: u64,
        /// Total derived from typed per-code counts.
        by_code: u64,
    },
}

impl fmt::Display for InvalidScanReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonReportableStatus { .. } => {
                formatter.write_str("scan report requires a reportable terminal status")
            }
            Self::CompletedHasIssues => {
                formatter.write_str("completed scan report cannot contain partial issues")
            }
            Self::PartialStatusHasNoIssues => formatter
                .write_str("completed-with-issues report requires at least one partial issue"),
            Self::IssueCountMismatch { .. } => {
                formatter.write_str("scan issue counters do not match typed issue totals")
            }
        }
    }
}

impl Error for InvalidScanReport {}

/// Qualified terminal output of one read-only scan.
///
/// Per-entry observations and issues are streamed through [`ScanSink`]. This type retains only
/// fixed-size aggregate counts and cannot grow with the scanned dataset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanReport {
    status: ScanRunStatus,
    counters: ScanCounters,
    issue_counts: ScanIssueCounts,
    resource_limits: ResourceLimits,
    resource_usage: ResourceUsage,
}

impl ScanReport {
    /// Creates a terminal report and rejects false-complete or inconsistent issue evidence.
    pub fn new(
        status: ScanRunStatus,
        counters: ScanCounters,
        issue_counts: ScanIssueCounts,
        resource_limits: ResourceLimits,
        resource_usage: ResourceUsage,
    ) -> Result<Self, InvalidScanReport> {
        if !matches!(
            status,
            ScanRunStatus::Completed
                | ScanRunStatus::CompletedWithIssues
                | ScanRunStatus::Cancelled
        ) {
            return Err(InvalidScanReport::NonReportableStatus { status });
        }

        let total_by_code = issue_counts.total();
        if counters.partial_issues() != total_by_code {
            return Err(InvalidScanReport::IssueCountMismatch {
                counters: counters.partial_issues(),
                by_code: total_by_code,
            });
        }
        if status == ScanRunStatus::Completed && total_by_code != 0 {
            return Err(InvalidScanReport::CompletedHasIssues);
        }
        if status == ScanRunStatus::CompletedWithIssues && total_by_code == 0 {
            return Err(InvalidScanReport::PartialStatusHasNoIssues);
        }

        Ok(Self {
            status,
            counters,
            issue_counts,
            resource_limits,
            resource_usage,
        })
    }

    /// Returns the qualified terminal status.
    pub const fn status(&self) -> ScanRunStatus {
        self.status
    }

    /// Returns aggregate entry counters.
    pub const fn counters(&self) -> ScanCounters {
        self.counters
    }

    /// Returns bounded per-code issue counts.
    pub const fn issue_counts(&self) -> ScanIssueCounts {
        self.issue_counts
    }

    /// Returns configured resource limits.
    pub const fn resource_limits(&self) -> ResourceLimits {
        self.resource_limits
    }

    /// Returns measured resource high-water evidence.
    pub const fn resource_usage(&self) -> ResourceUsage {
        self.resource_usage
    }

    /// Serializes a deterministic, path-free machine-readable summary.
    ///
    /// Fixed schema v1 excludes root paths, entry names, timestamps, duration, generated run IDs,
    /// and platform error messages, so equal scan facts produce byte-identical output.
    pub fn to_json(&self) -> String {
        let counters = self.counters;
        let limits = self.resource_limits;
        let usage = self.resource_usage;
        let mut json = format!(
            "{{\"schema_version\":{SCAN_SUMMARY_SCHEMA_VERSION},\"status\":\"{}\",\"counters\":{{\"files\":{},\"directories\":{},\"reparse_points_skipped\":{},\"other_entries\":{},\"bytes\":{},\"partial_issues\":{}}},\"issues_by_code\":[",
            self.status.as_code(),
            counters.files(),
            counters.directories(),
            counters.reparse_points_skipped(),
            counters.other_entries(),
            counters.bytes(),
            counters.partial_issues(),
        );

        for (index, code) in ScanIssueCode::ALL.into_iter().enumerate() {
            if index != 0 {
                json.push(',');
            }
            json.push_str(&format!(
                "{{\"code\":\"{}\",\"count\":{}}}",
                code.as_code(),
                self.issue_counts.count(code)
            ));
        }

        json.push_str(&format!(
            "],\"resource_limits\":{{\"metadata_workers\":{},\"metadata_queue_capacity\":{},\"open_directories\":{},\"max_depth\":{},\"progress_every_entries\":{}}},\"resource_usage\":{{\"metadata_queue_high_water\":{},\"metadata_workers_high_water\":{},\"open_directories_high_water\":{},\"max_depth_observed\":{},\"progress_events_emitted\":{}}}}}\n",
            limits.metadata_workers(),
            limits.metadata_queue_capacity(),
            limits.open_directories(),
            limits.max_depth(),
            limits.progress_every_entries(),
            usage.metadata_queue_high_water(),
            usage.metadata_workers_high_water(),
            usage.open_directories_high_water(),
            usage.max_depth_observed(),
            usage.progress_events_emitted(),
        ));
        json
    }
}

/// Stable classification for a fatal scan failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanFailureCode {
    /// Root validation rejected the supplied path or entry kind.
    InvalidRoot,
    /// The explicit root was unavailable or disappeared.
    RootUnavailable,
    /// The operating system denied access to the explicit root.
    RootAccessDenied,
    /// Canonical or reparse evidence could not prove scope confinement.
    ScopeViolation,
    /// A measured pipeline resource exceeded its configured bound.
    ResourceLimitExceeded,
    /// The streaming consumer rejected an event.
    SinkRejected,
    /// A bounded worker could not start, join, or return a result.
    WorkerFailed,
    /// An invariant failed without a safe external classification.
    InternalInvariant,
}

impl ScanFailureCode {
    /// Returns the stable path-free code.
    pub const fn as_code(self) -> &'static str {
        match self {
            Self::InvalidRoot => "invalid_root",
            Self::RootUnavailable => "root_unavailable",
            Self::RootAccessDenied => "root_access_denied",
            Self::ScopeViolation => "scope_violation",
            Self::ResourceLimitExceeded => "resource_limit_exceeded",
            Self::SinkRejected => "sink_rejected",
            Self::WorkerFailed => "worker_failed",
            Self::InternalInvariant => "internal_invariant",
        }
    }

    /// Returns a path-free user-safe explanation.
    pub const fn user_safe_message(self) -> &'static str {
        match self {
            Self::InvalidRoot => "The selected scan root is not supported.",
            Self::RootUnavailable => "The selected scan root is unavailable.",
            Self::RootAccessDenied => "Access to the selected scan root was denied.",
            Self::ScopeViolation => "The scan stopped because its scope could not be proven.",
            Self::ResourceLimitExceeded => {
                "The scan stopped after exceeding a configured resource limit."
            }
            Self::SinkRejected => "The scan result consumer could not continue.",
            Self::WorkerFailed => "A scan worker stopped unexpectedly.",
            Self::InternalInvariant => "The scan stopped because an internal check failed.",
        }
    }
}

/// Fatal failure returned instead of a qualified terminal report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanFailure {
    code: ScanFailureCode,
    phase: ScanPhase,
    retryable: bool,
    raw_os_code: Option<i32>,
    partial_counters: ScanCounters,
    resource_usage: ResourceUsage,
}

impl ScanFailure {
    /// Creates a typed fatal failure with aggregate evidence gathered before the stop.
    pub const fn new(
        code: ScanFailureCode,
        phase: ScanPhase,
        retryable: bool,
        raw_os_code: Option<i32>,
        partial_counters: ScanCounters,
        resource_usage: ResourceUsage,
    ) -> Self {
        Self {
            code,
            phase,
            retryable,
            raw_os_code,
            partial_counters,
            resource_usage,
        }
    }

    /// Returns the fatal failure code.
    pub const fn code(self) -> ScanFailureCode {
        self.code
    }

    /// Returns the phase in which the scan stopped.
    pub const fn phase(self) -> ScanPhase {
        self.phase
    }

    /// Reports whether a later retry after external change may succeed.
    pub const fn retryable(self) -> bool {
        self.retryable
    }

    /// Returns the optional platform code without a platform-generated message.
    pub const fn raw_os_code(self) -> Option<i32> {
        self.raw_os_code
    }

    /// Returns counters observed before the fatal stop.
    pub const fn partial_counters(self) -> ScanCounters {
        self.partial_counters
    }

    /// Returns resource evidence observed before the fatal stop.
    pub const fn resource_usage(self) -> ResourceUsage {
        self.resource_usage
    }
}

impl fmt::Display for ScanFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code.user_safe_message())
    }
}

impl Error for ScanFailure {}

/// Stable classification for a streaming-consumer error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanSinkErrorCode {
    /// A bounded consumer channel closed.
    ChannelClosed,
    /// The configured consumer was temporarily unavailable.
    ConsumerUnavailable,
    /// The consumer rejected data because its own invariant failed.
    ConsumerInvariant,
}

impl ScanSinkErrorCode {
    /// Returns the stable machine-readable code.
    pub const fn as_code(self) -> &'static str {
        match self {
            Self::ChannelClosed => "channel_closed",
            Self::ConsumerUnavailable => "consumer_unavailable",
            Self::ConsumerInvariant => "consumer_invariant",
        }
    }
}

/// Typed error returned by a streaming scan consumer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanSinkError {
    code: ScanSinkErrorCode,
    retryable: bool,
}

impl ScanSinkError {
    /// Creates a sink error without raw path or consumer payload text.
    pub const fn new(code: ScanSinkErrorCode, retryable: bool) -> Self {
        Self { code, retryable }
    }

    /// Returns the stable sink error code.
    pub const fn code(self) -> ScanSinkErrorCode {
        self.code
    }

    /// Reports whether a later scan may use the consumer successfully.
    pub const fn retryable(self) -> bool {
        self.retryable
    }
}

impl fmt::Display for ScanSinkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("scan result consumer could not accept an event")
    }
}

impl Error for ScanSinkError {}

/// Streaming consumer for observations, typed partial issues, and coalesced progress.
///
/// Implementations must apply backpressure or return a typed error instead of buffering an
/// unbounded number of entries or issues.
pub trait ScanSink {
    /// Accepts one validated scope-relative observation.
    fn observe(&mut self, entry: ObservedEntry) -> Result<(), ScanSinkError>;

    /// Accepts one typed entry-level partial issue.
    fn issue(&mut self, issue: ScanIssue) -> Result<(), ScanSinkError>;

    /// Accepts one coalesced non-terminal progress snapshot.
    fn progress(&mut self, progress: ScanProgress) -> Result<(), ScanSinkError>;
}

/// Consumer-owned application port implemented by a concrete bounded scanner.
pub trait ScanPort {
    /// Runs one read-only scan and returns either a qualified report or typed fatal failure.
    fn scan(
        &self,
        request: &ScanRequest,
        cancellation: &CancellationToken,
        sink: &mut dyn ScanSink,
    ) -> Result<ScanReport, ScanFailure>;
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;
    use std::path::PathBuf;

    use fileos_domain::{
        EntryKind, ObservedEntry, ResourceLimits, ResourceUsage, ScanCounters, ScanIssueCode,
        ScanIssueCounts, ScanRunStatus,
    };

    use super::{
        CancellationToken, InvalidScanReport, InvalidScanRequest, ScanProgress, ScanReport,
        ScanRequest,
    };

    fn non_zero(value: usize) -> NonZeroUsize {
        NonZeroUsize::new(value).expect("test value must be non-zero")
    }

    fn limits() -> ResourceLimits {
        ResourceLimits::new(
            non_zero(2),
            non_zero(8),
            non_zero(4),
            non_zero(16),
            non_zero(32),
        )
    }

    #[test]
    fn cancellation_is_shared_and_idempotent() {
        let owner = CancellationToken::new();
        let worker = owner.clone();
        assert!(!worker.is_cancelled());
        assert!(owner.cancel());
        assert!(worker.is_cancelled());
        assert!(!worker.cancel());
    }

    #[test]
    fn request_rejects_process_relative_root_without_filesystem_access() {
        let result = ScanRequest::new(PathBuf::from("fixtures/sandbox"), limits());
        assert!(matches!(
            result,
            Err(InvalidScanRequest::RootMustBeAbsolute { .. })
        ));
    }

    #[test]
    fn progress_rejects_terminal_status() {
        let progress = ScanProgress::new(
            ScanRunStatus::Completed,
            ScanCounters::default(),
            ResourceUsage::default(),
        );
        assert!(progress.is_err());
    }

    #[test]
    fn report_rejects_false_complete_and_count_mismatch() {
        let mut counters = ScanCounters::default();
        counters.record_issue();
        let mut issues = ScanIssueCounts::default();
        issues.record(ScanIssueCode::AccessDenied);

        let false_complete = ScanReport::new(
            ScanRunStatus::Completed,
            counters,
            issues,
            limits(),
            ResourceUsage::default(),
        );
        assert_eq!(false_complete, Err(InvalidScanReport::CompletedHasIssues));

        let mismatch = ScanReport::new(
            ScanRunStatus::Cancelled,
            ScanCounters::default(),
            issues,
            limits(),
            ResourceUsage::default(),
        );
        assert!(matches!(
            mismatch,
            Err(InvalidScanReport::IssueCountMismatch {
                counters: 0,
                by_code: 1
            })
        ));
    }

    #[test]
    fn summary_json_is_deterministic_and_path_free() {
        let root = std::env::temp_dir().join("private-fixture-name");
        let request = ScanRequest::new(root, limits()).expect("absolute temp root");
        let entry = ObservedEntry::new(
            PathBuf::from("input/file.bin"),
            EntryKind::File,
            Some(42),
            None,
            false,
        )
        .expect("valid observation");
        let mut counters = ScanCounters::default();
        counters.record_entry(&entry);
        let mut usage = ResourceUsage::default();
        usage
            .observe_metadata_queue(2, limits())
            .expect("bounded queue observation");
        usage.record_progress_event();
        let report = ScanReport::new(
            ScanRunStatus::Completed,
            counters,
            ScanIssueCounts::default(),
            request.limits(),
            usage,
        )
        .expect("truthful completed report");

        let first = report.to_json();
        let second = report.to_json();
        assert_eq!(first, second);
        assert!(!first.contains("private-fixture-name"));
        assert!(!first.contains("file.bin"));
        assert!(first.contains("\"schema_version\":1"));
        assert!(first.contains("\"metadata_queue_high_water\":2"));
    }
}

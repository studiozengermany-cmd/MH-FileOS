use std::error::Error;
use std::fmt;
use std::num::NonZeroUsize;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

/// Lifecycle state of one read-only scan run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanRunStatus {
    /// The run has been accepted but enumeration has not started.
    Queued,
    /// The scanner is enumerating or observing metadata.
    Running,
    /// Cancellation was observed and the scanner is draining to a safe terminal point.
    Cancelling,
    /// Every requested entry was processed without a partial issue.
    Completed,
    /// The run reached its root boundary but one or more entries could not be observed.
    CompletedWithIssues,
    /// The run stopped at a cancellation safe point.
    Cancelled,
    /// A fatal error prevented the run from producing a qualified terminal report.
    Failed,
}

impl ScanRunStatus {
    /// Returns the stable machine-readable code for this status.
    pub const fn as_code(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Cancelling => "cancelling",
            Self::Completed => "completed",
            Self::CompletedWithIssues => "completed_with_issues",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }

    /// Reports whether this status is terminal.
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::CompletedWithIssues | Self::Cancelled | Self::Failed
        )
    }

    /// Reports whether a direct lifecycle transition is allowed.
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Queued, Self::Running | Self::Cancelled | Self::Failed)
                | (
                    Self::Running,
                    Self::Cancelling | Self::Completed | Self::CompletedWithIssues | Self::Failed
                )
                | (Self::Cancelling, Self::Cancelled | Self::Failed)
        )
    }
}

/// Kind of one filesystem entry observed without following it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// A regular file.
    File,
    /// A directory that is not a reparse point.
    Directory,
    /// A symlink, junction, mount point, or other reparse entry.
    ReparsePoint,
    /// A filesystem entry not represented by another variant.
    Other,
}

impl EntryKind {
    /// Returns the stable machine-readable code for this kind.
    pub const fn as_code(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Directory => "directory",
            Self::ReparsePoint => "reparse_point",
            Self::Other => "other",
        }
    }
}

/// Scan stage in which a partial issue was observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanPhase {
    /// Validating the explicit root and its scope boundary.
    ScopeValidation,
    /// Enumerating entries in a directory.
    DirectoryEnumeration,
    /// Reading no-follow metadata for one entry.
    MetadataRead,
    /// Delivering a validated observation to the consumer.
    SinkDelivery,
    /// A cancellation safe point.
    Cancellation,
    /// Producing the terminal report.
    Finalization,
}

impl ScanPhase {
    /// Returns the stable machine-readable code for this phase.
    pub const fn as_code(self) -> &'static str {
        match self {
            Self::ScopeValidation => "scope_validation",
            Self::DirectoryEnumeration => "directory_enumeration",
            Self::MetadataRead => "metadata_read",
            Self::SinkDelivery => "sink_delivery",
            Self::Cancellation => "cancellation",
            Self::Finalization => "finalization",
        }
    }
}

/// Stable classification for an entry-level scan issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScanIssueCode {
    /// The operating system denied access to an entry or subtree.
    AccessDenied,
    /// An entry disappeared between enumeration and observation.
    Disappeared,
    /// A directory iterator could not be opened or continued.
    DirectoryReadFailed,
    /// Metadata could not be read for one enumerated entry.
    MetadataUnavailable,
    /// The host returned an entry kind unsupported by this scan version.
    UnsupportedEntryType,
    /// Traversal stopped at the configured maximum depth.
    DepthLimitReached,
    /// Traversal stopped before opening another directory at the configured handle bound.
    OpenDirectoryLimitReached,
    /// An entry-level I/O failure did not map to a more specific code.
    IoOther,
}

impl ScanIssueCode {
    /// Every issue code in deterministic serialization order.
    pub const ALL: [Self; 8] = [
        Self::AccessDenied,
        Self::Disappeared,
        Self::DirectoryReadFailed,
        Self::MetadataUnavailable,
        Self::UnsupportedEntryType,
        Self::DepthLimitReached,
        Self::OpenDirectoryLimitReached,
        Self::IoOther,
    ];

    /// Returns the stable machine-readable code used for deterministic grouping.
    pub const fn as_code(self) -> &'static str {
        match self {
            Self::AccessDenied => "access_denied",
            Self::Disappeared => "disappeared",
            Self::DirectoryReadFailed => "directory_read_failed",
            Self::MetadataUnavailable => "metadata_unavailable",
            Self::UnsupportedEntryType => "unsupported_entry_type",
            Self::DepthLimitReached => "depth_limit_reached",
            Self::OpenDirectoryLimitReached => "open_directory_limit_reached",
            Self::IoOther => "io_other",
        }
    }

    /// Returns a path-free explanation suitable for a local user-facing adapter.
    pub const fn user_safe_message(self) -> &'static str {
        match self {
            Self::AccessDenied => "Access was denied for part of the scan.",
            Self::Disappeared => "An entry changed or disappeared during the scan.",
            Self::DirectoryReadFailed => "A directory could not be fully enumerated.",
            Self::MetadataUnavailable => "Metadata could not be read for an entry.",
            Self::UnsupportedEntryType => "An unsupported filesystem entry was skipped.",
            Self::DepthLimitReached => "A subtree exceeded the configured scan depth.",
            Self::OpenDirectoryLimitReached => {
                "A subtree exceeded the configured open-directory limit."
            }
            Self::IoOther => "An entry could not be observed because of an I/O error.",
        }
    }
}

/// Bounded aggregate counts for every stable partial-issue code.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScanIssueCounts {
    access_denied: u64,
    disappeared: u64,
    directory_read_failed: u64,
    metadata_unavailable: u64,
    unsupported_entry_type: u64,
    depth_limit_reached: u64,
    open_directory_limit_reached: u64,
    io_other: u64,
}

impl ScanIssueCounts {
    /// Records one issue using saturating arithmetic.
    pub fn record(&mut self, code: ScanIssueCode) {
        let counter = match code {
            ScanIssueCode::AccessDenied => &mut self.access_denied,
            ScanIssueCode::Disappeared => &mut self.disappeared,
            ScanIssueCode::DirectoryReadFailed => &mut self.directory_read_failed,
            ScanIssueCode::MetadataUnavailable => &mut self.metadata_unavailable,
            ScanIssueCode::UnsupportedEntryType => &mut self.unsupported_entry_type,
            ScanIssueCode::DepthLimitReached => &mut self.depth_limit_reached,
            ScanIssueCode::OpenDirectoryLimitReached => &mut self.open_directory_limit_reached,
            ScanIssueCode::IoOther => &mut self.io_other,
        };
        *counter = counter.saturating_add(1);
    }

    /// Returns the count for one stable issue code.
    pub const fn count(self, code: ScanIssueCode) -> u64 {
        match code {
            ScanIssueCode::AccessDenied => self.access_denied,
            ScanIssueCode::Disappeared => self.disappeared,
            ScanIssueCode::DirectoryReadFailed => self.directory_read_failed,
            ScanIssueCode::MetadataUnavailable => self.metadata_unavailable,
            ScanIssueCode::UnsupportedEntryType => self.unsupported_entry_type,
            ScanIssueCode::DepthLimitReached => self.depth_limit_reached,
            ScanIssueCode::OpenDirectoryLimitReached => self.open_directory_limit_reached,
            ScanIssueCode::IoOther => self.io_other,
        }
    }

    /// Returns the total issue count using saturating arithmetic.
    pub const fn total(self) -> u64 {
        self.access_denied
            .saturating_add(self.disappeared)
            .saturating_add(self.directory_read_failed)
            .saturating_add(self.metadata_unavailable)
            .saturating_add(self.unsupported_entry_type)
            .saturating_add(self.depth_limit_reached)
            .saturating_add(self.open_directory_limit_reached)
            .saturating_add(self.io_other)
    }
}

/// Error returned when an entry path is not a non-empty traversal-free relative path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidRelativeScanPath {
    path: PathBuf,
}

impl InvalidRelativeScanPath {
    /// Returns the rejected local in-process path value.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl fmt::Display for InvalidRelativeScanPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("scan entry path must be non-empty, relative, and traversal-free")
    }
}

impl Error for InvalidRelativeScanPath {}

/// One typed partial issue that does not by itself fail the entire scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanIssue {
    code: ScanIssueCode,
    phase: ScanPhase,
    relative_path: Option<PathBuf>,
    retryable: bool,
    raw_os_code: Option<i32>,
}

impl ScanIssue {
    /// Creates a partial issue and validates any local relative-path context.
    pub fn new(
        code: ScanIssueCode,
        phase: ScanPhase,
        relative_path: Option<PathBuf>,
        retryable: bool,
        raw_os_code: Option<i32>,
    ) -> Result<Self, InvalidRelativeScanPath> {
        if let Some(path) = relative_path.as_deref() {
            validate_relative_path(path)?;
        }
        Ok(Self {
            code,
            phase,
            relative_path,
            retryable,
            raw_os_code,
        })
    }

    /// Returns the stable issue code.
    pub const fn code(&self) -> ScanIssueCode {
        self.code
    }

    /// Returns the stage in which the issue occurred.
    pub const fn phase(&self) -> ScanPhase {
        self.phase
    }

    /// Returns local relative-path context, which must be redacted before diagnostics export.
    pub fn relative_path(&self) -> Option<&Path> {
        self.relative_path.as_deref()
    }

    /// Reports whether retrying after an external state change may succeed.
    pub const fn retryable(&self) -> bool {
        self.retryable
    }

    /// Returns the optional platform error code without a platform-generated message.
    pub const fn raw_os_code(&self) -> Option<i32> {
        self.raw_os_code
    }
}

/// Error returned when an observed entry violates kind/metadata invariants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvalidObservedEntry {
    /// The observation path was not a safe relative path.
    InvalidRelativePath(InvalidRelativeScanPath),
    /// A regular file observation omitted its byte length.
    FileSizeMissing,
    /// A non-file observation carried a file byte length.
    NonFileSizePresent {
        /// The entry kind that cannot carry a file length.
        kind: EntryKind,
    },
}

impl fmt::Display for InvalidObservedEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRelativePath(error) => error.fmt(formatter),
            Self::FileSizeMissing => formatter.write_str("regular file observation requires size"),
            Self::NonFileSizePresent { .. } => {
                formatter.write_str("non-file observation cannot carry file size")
            }
        }
    }
}

impl Error for InvalidObservedEntry {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidRelativePath(error) => Some(error),
            Self::FileSizeMissing | Self::NonFileSizePresent { .. } => None,
        }
    }
}

/// Immutable metadata observed for one scope-relative entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedEntry {
    relative_path: PathBuf,
    kind: EntryKind,
    byte_len: Option<u64>,
    modified: Option<SystemTime>,
    read_only: bool,
}

impl ObservedEntry {
    /// Creates an observation while enforcing path and kind/size invariants.
    pub fn new(
        relative_path: PathBuf,
        kind: EntryKind,
        byte_len: Option<u64>,
        modified: Option<SystemTime>,
        read_only: bool,
    ) -> Result<Self, InvalidObservedEntry> {
        validate_relative_path(&relative_path)
            .map_err(InvalidObservedEntry::InvalidRelativePath)?;
        match (kind, byte_len) {
            (EntryKind::File, None) => return Err(InvalidObservedEntry::FileSizeMissing),
            (EntryKind::File, Some(_)) | (_, None) => {}
            (_, Some(_)) => return Err(InvalidObservedEntry::NonFileSizePresent { kind }),
        }
        Ok(Self {
            relative_path,
            kind,
            byte_len,
            modified,
            read_only,
        })
    }

    /// Returns the traversal-free path relative to the explicit scan root.
    pub fn relative_path(&self) -> &Path {
        &self.relative_path
    }

    /// Returns the no-follow entry kind.
    pub const fn kind(&self) -> EntryKind {
        self.kind
    }

    /// Returns byte length for a regular file and `None` for other entry kinds.
    pub const fn byte_len(&self) -> Option<u64> {
        self.byte_len
    }

    /// Returns the observed modification time when the platform supplied one.
    pub const fn modified(&self) -> Option<SystemTime> {
        self.modified
    }

    /// Returns the observed read-only attribute.
    pub const fn read_only(&self) -> bool {
        self.read_only
    }
}

/// Aggregate counts observed during one scan without retaining every entry in memory.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScanCounters {
    files: u64,
    directories: u64,
    reparse_points_skipped: u64,
    other_entries: u64,
    bytes: u64,
    partial_issues: u64,
}

impl ScanCounters {
    /// Records one validated observation using saturating arithmetic.
    pub fn record_entry(&mut self, entry: &ObservedEntry) {
        match entry.kind() {
            EntryKind::File => {
                self.files = self.files.saturating_add(1);
                self.bytes = self.bytes.saturating_add(entry.byte_len().unwrap_or(0));
            }
            EntryKind::Directory => self.directories = self.directories.saturating_add(1),
            EntryKind::ReparsePoint => {
                self.reparse_points_skipped = self.reparse_points_skipped.saturating_add(1);
            }
            EntryKind::Other => self.other_entries = self.other_entries.saturating_add(1),
        }
    }

    /// Records one entry-level partial issue using saturating arithmetic.
    pub fn record_issue(&mut self) {
        self.partial_issues = self.partial_issues.saturating_add(1);
    }

    /// Returns the number of regular files observed.
    pub const fn files(self) -> u64 {
        self.files
    }

    /// Returns the number of non-reparse directories observed below the root.
    pub const fn directories(self) -> u64 {
        self.directories
    }

    /// Returns the number of reparse entries observed and deliberately not followed.
    pub const fn reparse_points_skipped(self) -> u64 {
        self.reparse_points_skipped
    }

    /// Returns the number of other entry kinds observed.
    pub const fn other_entries(self) -> u64 {
        self.other_entries
    }

    /// Returns the sum of observed regular-file byte lengths.
    pub const fn bytes(self) -> u64 {
        self.bytes
    }

    /// Returns the number of accumulated entry-level issues.
    pub const fn partial_issues(self) -> u64 {
        self.partial_issues
    }

    /// Returns the aggregate number of observed entries using saturating arithmetic.
    pub const fn total_entries(self) -> u64 {
        self.files
            .saturating_add(self.directories)
            .saturating_add(self.reparse_points_skipped)
            .saturating_add(self.other_entries)
    }
}

/// Configured hard bounds for one scan pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceLimits {
    metadata_workers: NonZeroUsize,
    metadata_queue_capacity: NonZeroUsize,
    open_directories: NonZeroUsize,
    max_depth: NonZeroUsize,
    progress_every_entries: NonZeroUsize,
}

impl ResourceLimits {
    /// Largest approved metadata worker count for the M4 read-only scanner.
    pub const MAX_METADATA_WORKERS: usize = 64;
    /// Largest approved pending metadata queue for the M4 read-only scanner.
    pub const MAX_METADATA_QUEUE_CAPACITY: usize = 65_536;
    /// Largest approved number of simultaneously open directory iterators.
    pub const MAX_OPEN_DIRECTORIES: usize = 256;
    /// Largest approved traversal depth below an explicit root.
    pub const MAX_DEPTH: usize = 1_024;
    /// Largest approved entry interval between aggregate progress events.
    pub const MAX_PROGRESS_EVERY_ENTRIES: usize = 1_000_000;

    /// Creates an explicit set of non-zero scan resource bounds.
    pub const fn new(
        metadata_workers: NonZeroUsize,
        metadata_queue_capacity: NonZeroUsize,
        open_directories: NonZeroUsize,
        max_depth: NonZeroUsize,
        progress_every_entries: NonZeroUsize,
    ) -> Self {
        Self {
            metadata_workers,
            metadata_queue_capacity,
            open_directories,
            max_depth,
            progress_every_entries,
        }
    }

    /// Validates every configured value against the approved M4 upper bounds.
    ///
    /// Callers must validate before allocating a queue, opening a directory, or starting a worker.
    pub const fn validate(self) -> Result<(), InvalidResourceLimits> {
        if self.metadata_workers.get() > Self::MAX_METADATA_WORKERS {
            return Err(InvalidResourceLimits::new(
                ResourceLimitField::MetadataWorkers,
                self.metadata_workers.get(),
                Self::MAX_METADATA_WORKERS,
            ));
        }
        if self.metadata_queue_capacity.get() > Self::MAX_METADATA_QUEUE_CAPACITY {
            return Err(InvalidResourceLimits::new(
                ResourceLimitField::MetadataQueueCapacity,
                self.metadata_queue_capacity.get(),
                Self::MAX_METADATA_QUEUE_CAPACITY,
            ));
        }
        if self.open_directories.get() > Self::MAX_OPEN_DIRECTORIES {
            return Err(InvalidResourceLimits::new(
                ResourceLimitField::OpenDirectories,
                self.open_directories.get(),
                Self::MAX_OPEN_DIRECTORIES,
            ));
        }
        if self.max_depth.get() > Self::MAX_DEPTH {
            return Err(InvalidResourceLimits::new(
                ResourceLimitField::MaxDepth,
                self.max_depth.get(),
                Self::MAX_DEPTH,
            ));
        }
        if self.progress_every_entries.get() > Self::MAX_PROGRESS_EVERY_ENTRIES {
            return Err(InvalidResourceLimits::new(
                ResourceLimitField::ProgressEveryEntries,
                self.progress_every_entries.get(),
                Self::MAX_PROGRESS_EVERY_ENTRIES,
            ));
        }
        Ok(())
    }

    /// Returns the maximum metadata worker count.
    pub const fn metadata_workers(self) -> NonZeroUsize {
        self.metadata_workers
    }

    /// Returns the maximum pending metadata jobs.
    pub const fn metadata_queue_capacity(self) -> NonZeroUsize {
        self.metadata_queue_capacity
    }

    /// Returns the maximum simultaneously open directory iterators.
    pub const fn open_directories(self) -> NonZeroUsize {
        self.open_directories
    }

    /// Returns the maximum traversed depth below the root.
    pub const fn max_depth(self) -> NonZeroUsize {
        self.max_depth
    }

    /// Returns the minimum observation count between aggregate progress events.
    pub const fn progress_every_entries(self) -> NonZeroUsize {
        self.progress_every_entries
    }
}

/// One configured scan-limit field that failed approved-bound validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceLimitField {
    /// Metadata worker count.
    MetadataWorkers,
    /// Pending metadata queue capacity.
    MetadataQueueCapacity,
    /// Simultaneously open directory iterators.
    OpenDirectories,
    /// Traversal depth below the explicit root.
    MaxDepth,
    /// Entry interval between aggregate progress events.
    ProgressEveryEntries,
}

impl ResourceLimitField {
    /// Returns the stable machine-readable field code.
    pub const fn as_code(self) -> &'static str {
        match self {
            Self::MetadataWorkers => "metadata_workers",
            Self::MetadataQueueCapacity => "metadata_queue_capacity",
            Self::OpenDirectories => "open_directories",
            Self::MaxDepth => "max_depth",
            Self::ProgressEveryEntries => "progress_every_entries",
        }
    }
}

/// Typed failure returned when a non-zero scan limit exceeds its approved maximum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidResourceLimits {
    field: ResourceLimitField,
    observed: usize,
    maximum: usize,
}

impl InvalidResourceLimits {
    const fn new(field: ResourceLimitField, observed: usize, maximum: usize) -> Self {
        Self {
            field,
            observed,
            maximum,
        }
    }

    /// Returns the invalid configuration field.
    pub const fn field(self) -> ResourceLimitField {
        self.field
    }

    /// Returns the rejected configured value.
    pub const fn observed(self) -> usize {
        self.observed
    }

    /// Returns the largest approved value for this field.
    pub const fn maximum(self) -> usize {
        self.maximum
    }
}

impl fmt::Display for InvalidResourceLimits {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "scan limit '{}' value {} exceeds approved maximum {}",
            self.field.as_code(),
            self.observed,
            self.maximum
        )
    }
}

impl Error for InvalidResourceLimits {}

/// Bounded resource whose configured limit was exceeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceKind {
    /// Pending metadata work queue.
    MetadataQueue,
    /// Concurrent metadata worker count.
    MetadataWorkers,
    /// Simultaneously open directory iterators.
    OpenDirectories,
    /// Directory traversal depth.
    TraversalDepth,
}

impl ResourceKind {
    /// Returns the stable machine-readable code for the resource.
    pub const fn as_code(self) -> &'static str {
        match self {
            Self::MetadataQueue => "metadata_queue",
            Self::MetadataWorkers => "metadata_workers",
            Self::OpenDirectories => "open_directories",
            Self::TraversalDepth => "traversal_depth",
        }
    }
}

/// Error proving a scanner attempted to exceed an explicit resource bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceBoundExceeded {
    resource: ResourceKind,
    observed: usize,
    limit: usize,
}

impl ResourceBoundExceeded {
    /// Returns the bounded resource.
    pub const fn resource(self) -> ResourceKind {
        self.resource
    }

    /// Returns the attempted observed value.
    pub const fn observed(self) -> usize {
        self.observed
    }

    /// Returns the configured upper bound.
    pub const fn limit(self) -> usize {
        self.limit
    }
}

impl fmt::Display for ResourceBoundExceeded {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} usage {} exceeded configured limit {}",
            self.resource.as_code(),
            self.observed,
            self.limit
        )
    }
}

impl Error for ResourceBoundExceeded {}

/// Measured high-water evidence for the configured scan resource bounds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ResourceUsage {
    metadata_queue_high_water: usize,
    metadata_workers_high_water: usize,
    open_directories_high_water: usize,
    max_depth_observed: usize,
    progress_events_emitted: u64,
}

impl ResourceUsage {
    /// Records queue depth and rejects a value above the configured capacity.
    pub fn observe_metadata_queue(
        &mut self,
        observed: usize,
        limits: ResourceLimits,
    ) -> Result<(), ResourceBoundExceeded> {
        observe_high_water(
            &mut self.metadata_queue_high_water,
            ResourceKind::MetadataQueue,
            observed,
            limits.metadata_queue_capacity().get(),
        )
    }

    /// Records active workers and rejects a value above the configured worker count.
    pub fn observe_metadata_workers(
        &mut self,
        observed: usize,
        limits: ResourceLimits,
    ) -> Result<(), ResourceBoundExceeded> {
        observe_high_water(
            &mut self.metadata_workers_high_water,
            ResourceKind::MetadataWorkers,
            observed,
            limits.metadata_workers().get(),
        )
    }

    /// Records open directories and rejects a value above the configured handle bound.
    pub fn observe_open_directories(
        &mut self,
        observed: usize,
        limits: ResourceLimits,
    ) -> Result<(), ResourceBoundExceeded> {
        observe_high_water(
            &mut self.open_directories_high_water,
            ResourceKind::OpenDirectories,
            observed,
            limits.open_directories().get(),
        )
    }

    /// Records traversal depth and rejects a value above the configured depth bound.
    pub fn observe_depth(
        &mut self,
        observed: usize,
        limits: ResourceLimits,
    ) -> Result<(), ResourceBoundExceeded> {
        observe_high_water(
            &mut self.max_depth_observed,
            ResourceKind::TraversalDepth,
            observed,
            limits.max_depth().get(),
        )
    }

    /// Records one aggregate progress event using saturating arithmetic.
    pub fn record_progress_event(&mut self) {
        self.progress_events_emitted = self.progress_events_emitted.saturating_add(1);
    }

    /// Returns the maximum pending metadata queue depth.
    pub const fn metadata_queue_high_water(self) -> usize {
        self.metadata_queue_high_water
    }

    /// Returns the maximum active metadata worker count.
    pub const fn metadata_workers_high_water(self) -> usize {
        self.metadata_workers_high_water
    }

    /// Returns the maximum simultaneously open directory iterators.
    pub const fn open_directories_high_water(self) -> usize {
        self.open_directories_high_water
    }

    /// Returns the deepest observed directory level below the root.
    pub const fn max_depth_observed(self) -> usize {
        self.max_depth_observed
    }

    /// Returns the number of aggregate progress events emitted.
    pub const fn progress_events_emitted(self) -> u64 {
        self.progress_events_emitted
    }
}

fn validate_relative_path(path: &Path) -> Result<(), InvalidRelativeScanPath> {
    let valid = !path.as_os_str().is_empty()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)));
    if valid {
        Ok(())
    } else {
        Err(InvalidRelativeScanPath {
            path: path.to_path_buf(),
        })
    }
}

fn observe_high_water(
    high_water: &mut usize,
    resource: ResourceKind,
    observed: usize,
    limit: usize,
) -> Result<(), ResourceBoundExceeded> {
    if observed > limit {
        return Err(ResourceBoundExceeded {
            resource,
            observed,
            limit,
        });
    }
    *high_water = (*high_water).max(observed);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;
    use std::path::PathBuf;

    use super::{
        EntryKind, InvalidObservedEntry, ObservedEntry, ResourceKind, ResourceLimitField,
        ResourceLimits, ResourceUsage, ScanCounters, ScanIssue, ScanIssueCode, ScanIssueCounts,
        ScanPhase, ScanRunStatus,
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
    fn lifecycle_rejects_false_terminal_transitions() {
        assert!(ScanRunStatus::Queued.can_transition_to(ScanRunStatus::Running));
        assert!(ScanRunStatus::Running.can_transition_to(ScanRunStatus::CompletedWithIssues));
        assert!(ScanRunStatus::Running.can_transition_to(ScanRunStatus::Cancelling));
        assert!(ScanRunStatus::Cancelling.can_transition_to(ScanRunStatus::Cancelled));
        assert!(!ScanRunStatus::Running.can_transition_to(ScanRunStatus::Cancelled));
        assert!(!ScanRunStatus::Completed.can_transition_to(ScanRunStatus::Running));
        assert!(ScanRunStatus::CompletedWithIssues.is_terminal());
    }

    #[test]
    fn observation_rejects_escape_and_kind_size_mismatch() {
        let escape = ObservedEntry::new(
            PathBuf::from("../outside.txt"),
            EntryKind::File,
            Some(1),
            None,
            false,
        );
        assert!(matches!(
            escape,
            Err(InvalidObservedEntry::InvalidRelativePath(_))
        ));

        let missing_size = ObservedEntry::new(
            PathBuf::from("inside.txt"),
            EntryKind::File,
            None,
            None,
            false,
        );
        assert_eq!(missing_size, Err(InvalidObservedEntry::FileSizeMissing));

        let directory_size = ObservedEntry::new(
            PathBuf::from("nested"),
            EntryKind::Directory,
            Some(0),
            None,
            false,
        );
        assert!(matches!(
            directory_size,
            Err(InvalidObservedEntry::NonFileSizePresent {
                kind: EntryKind::Directory
            })
        ));
    }

    #[test]
    fn counters_and_resource_usage_are_bounded() {
        let file = ObservedEntry::new(
            PathBuf::from("nested/file.bin"),
            EntryKind::File,
            Some(42),
            None,
            false,
        )
        .expect("valid test observation");
        let mut counters = ScanCounters::default();
        counters.record_entry(&file);
        counters.record_issue();
        assert_eq!(counters.files(), 1);
        assert_eq!(counters.bytes(), 42);
        assert_eq!(counters.partial_issues(), 1);

        let mut usage = ResourceUsage::default();
        usage
            .observe_metadata_queue(8, limits())
            .expect("queue is at its bound");
        let error = usage
            .observe_metadata_queue(9, limits())
            .expect_err("queue above its bound must fail");
        assert_eq!(error.resource(), ResourceKind::MetadataQueue);
        assert_eq!(error.observed(), 9);
        assert_eq!(error.limit(), 8);
        assert_eq!(usage.metadata_queue_high_water(), 8);
    }

    #[test]
    fn resource_limits_reject_unapproved_usize_max_values() {
        let valid = limits();
        let cases = [
            (
                ResourceLimits::new(
                    non_zero(usize::MAX),
                    valid.metadata_queue_capacity(),
                    valid.open_directories(),
                    valid.max_depth(),
                    valid.progress_every_entries(),
                ),
                ResourceLimitField::MetadataWorkers,
                ResourceLimits::MAX_METADATA_WORKERS,
            ),
            (
                ResourceLimits::new(
                    valid.metadata_workers(),
                    non_zero(usize::MAX),
                    valid.open_directories(),
                    valid.max_depth(),
                    valid.progress_every_entries(),
                ),
                ResourceLimitField::MetadataQueueCapacity,
                ResourceLimits::MAX_METADATA_QUEUE_CAPACITY,
            ),
            (
                ResourceLimits::new(
                    valid.metadata_workers(),
                    valid.metadata_queue_capacity(),
                    non_zero(usize::MAX),
                    valid.max_depth(),
                    valid.progress_every_entries(),
                ),
                ResourceLimitField::OpenDirectories,
                ResourceLimits::MAX_OPEN_DIRECTORIES,
            ),
            (
                ResourceLimits::new(
                    valid.metadata_workers(),
                    valid.metadata_queue_capacity(),
                    valid.open_directories(),
                    non_zero(usize::MAX),
                    valid.progress_every_entries(),
                ),
                ResourceLimitField::MaxDepth,
                ResourceLimits::MAX_DEPTH,
            ),
            (
                ResourceLimits::new(
                    valid.metadata_workers(),
                    valid.metadata_queue_capacity(),
                    valid.open_directories(),
                    valid.max_depth(),
                    non_zero(usize::MAX),
                ),
                ResourceLimitField::ProgressEveryEntries,
                ResourceLimits::MAX_PROGRESS_EVERY_ENTRIES,
            ),
        ];

        for (limits, expected_field, expected_maximum) in cases {
            let error = limits
                .validate()
                .expect_err("usize::MAX must exceed every approved resource bound");
            assert_eq!(error.field(), expected_field);
            assert_eq!(error.observed(), usize::MAX);
            assert_eq!(error.maximum(), expected_maximum);
        }
        valid.validate().expect("approved test limits");
    }

    #[test]
    fn partial_issue_validates_relative_path_and_keeps_typed_context() {
        let issue = ScanIssue::new(
            ScanIssueCode::Disappeared,
            ScanPhase::MetadataRead,
            Some(PathBuf::from("input/changed.txt")),
            true,
            Some(2),
        )
        .expect("valid relative issue path");
        assert_eq!(issue.code(), ScanIssueCode::Disappeared);
        assert_eq!(issue.phase(), ScanPhase::MetadataRead);
        assert!(issue.retryable());
        assert_eq!(issue.raw_os_code(), Some(2));

        let escape = ScanIssue::new(
            ScanIssueCode::IoOther,
            ScanPhase::DirectoryEnumeration,
            Some(PathBuf::from("../escape")),
            false,
            None,
        );
        assert!(escape.is_err());

        let mut counts = ScanIssueCounts::default();
        counts.record(ScanIssueCode::Disappeared);
        counts.record(ScanIssueCode::Disappeared);
        counts.record(ScanIssueCode::AccessDenied);
        assert_eq!(counts.count(ScanIssueCode::Disappeared), 2);
        assert_eq!(counts.total(), 3);
    }
}

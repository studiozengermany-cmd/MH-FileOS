use std::path::{Path, PathBuf};

use fileos_domain::{EntryKind, ScanRunStatus};

/// Opaque catalog identifier for a storage volume.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VolumeId(pub(crate) i64);

impl VolumeId {
    /// Returns the local database identity for diagnostics and DTO mapping.
    pub const fn get(self) -> i64 {
        self.0
    }
}

/// Opaque catalog identifier for an explicitly registered scan root.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScanRootId(pub(crate) i64);

impl ScanRootId {
    /// Returns the local database identity for diagnostics and DTO mapping.
    pub const fn get(self) -> i64 {
        self.0
    }
}

/// Opaque catalog identifier for one scan run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScanRunId(pub(crate) i64);

impl ScanRunId {
    /// Returns the local database identity for diagnostics and DTO mapping.
    pub const fn get(self) -> i64 {
        self.0
    }
}

/// Stable identity supplied by a platform adapter or synthetic test scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumeDescriptor {
    pub(crate) identity_kind: String,
    pub(crate) identity_value: Vec<u8>,
    pub(crate) filesystem: Option<String>,
}

impl VolumeDescriptor {
    /// Creates a volume descriptor without inferring identity from a drive letter.
    pub fn new(
        identity_kind: impl Into<String>,
        identity_value: impl Into<Vec<u8>>,
        filesystem: Option<String>,
    ) -> Self {
        Self {
            identity_kind: identity_kind.into(),
            identity_value: identity_value.into(),
            filesystem,
        }
    }
}

/// Explicit scan-root identity and versioned comparison representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanRootDescriptor {
    pub(crate) volume_id: VolumeId,
    pub(crate) display_path: PathBuf,
    pub(crate) comparison_path: Vec<u8>,
    pub(crate) normalization_version: u32,
    pub(crate) policy_version: u32,
}

impl ScanRootDescriptor {
    /// Creates a root with a caller-supplied comparison key.
    pub fn new(
        volume_id: VolumeId,
        display_path: PathBuf,
        comparison_path: Vec<u8>,
        normalization_version: u32,
        policy_version: u32,
    ) -> Self {
        Self {
            volume_id,
            display_path,
            comparison_path,
            normalization_version,
            policy_version,
        }
    }

    /// Creates the exact-byte M5 comparison representation used by synthetic fixtures.
    ///
    /// This does not claim Windows case or normalization semantics. A later platform identity
    /// adapter must supply those semantics with a new normalization version.
    pub fn exact_path(
        volume_id: VolumeId,
        display_path: PathBuf,
        normalization_version: u32,
        policy_version: u32,
    ) -> Self {
        let comparison_path = display_path.as_os_str().as_encoded_bytes().to_vec();
        Self::new(
            volume_id,
            display_path,
            comparison_path,
            normalization_version,
            policy_version,
        )
    }
}

/// Current-presence filter applied to catalog summaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresenceFilter {
    /// Include only entries observed as present in the best-known catalog state.
    Present,
    /// Include only retained entries marked missing by a complete scan.
    Missing,
    /// Include both present and missing rows.
    All,
}

impl PresenceFilter {
    pub(crate) const fn as_code(self) -> &'static str {
        match self {
            Self::Present => "present",
            Self::Missing => "missing",
            Self::All => "all",
        }
    }
}

/// Filters for one aggregate summary query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SummaryQuery {
    /// Optional root constraint.
    pub root_id: Option<ScanRootId>,
    /// Optional M5 category constraint. Category v1 is the observed entry kind.
    pub category: Option<EntryKind>,
    /// Presence state selection.
    pub presence: PresenceFilter,
}

impl SummaryQuery {
    /// Creates a present-only query for one root.
    pub const fn present_for_root(root_id: ScanRootId) -> Self {
        Self {
            root_id: Some(root_id),
            category: None,
            presence: PresenceFilter::Present,
        }
    }
}

/// Aggregate catalog facts for the selected root/category/presence scope.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CatalogSummary {
    entries: u64,
    files: u64,
    directories: u64,
    reparse_points: u64,
    other_entries: u64,
    bytes: u64,
}

impl CatalogSummary {
    pub(crate) fn from_database(values: [i64; 6]) -> Option<Self> {
        Some(Self {
            entries: u64::try_from(values[0]).ok()?,
            files: u64::try_from(values[1]).ok()?,
            directories: u64::try_from(values[2]).ok()?,
            reparse_points: u64::try_from(values[3]).ok()?,
            other_entries: u64::try_from(values[4]).ok()?,
            bytes: u64::try_from(values[5]).ok()?,
        })
    }

    /// Returns the number of selected rows.
    pub const fn entries(self) -> u64 {
        self.entries
    }

    /// Returns the selected regular-file count.
    pub const fn files(self) -> u64 {
        self.files
    }

    /// Returns the selected directory count.
    pub const fn directories(self) -> u64 {
        self.directories
    }

    /// Returns the selected non-followed reparse-point count.
    pub const fn reparse_points(self) -> u64 {
        self.reparse_points
    }

    /// Returns the selected other-entry count.
    pub const fn other_entries(self) -> u64 {
        self.other_entries
    }

    /// Returns the sum of selected regular-file sizes.
    pub const fn bytes(self) -> u64 {
        self.bytes
    }
}

/// Presence state retained for one current catalog entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryPresence {
    /// The entry was observed in a scan and has not been authoritatively marked absent.
    Present,
    /// A later complete scan did not observe the entry.
    Missing,
}

/// Lifecycle evidence for one root-relative catalog row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntryRecord {
    id: i64,
    kind: EntryKind,
    byte_len: Option<u64>,
    presence: EntryPresence,
    first_seen_run_id: ScanRunId,
    last_seen_run_id: ScanRunId,
    missing_since_run_id: Option<ScanRunId>,
}

impl EntryRecord {
    pub(crate) fn new(
        id: i64,
        kind: EntryKind,
        byte_len: Option<u64>,
        presence: EntryPresence,
        first_seen_run_id: ScanRunId,
        last_seen_run_id: ScanRunId,
        missing_since_run_id: Option<ScanRunId>,
    ) -> Self {
        Self {
            id,
            kind,
            byte_len,
            presence,
            first_seen_run_id,
            last_seen_run_id,
            missing_since_run_id,
        }
    }

    /// Returns the stable row identity retained across repeat scans.
    pub const fn id(self) -> i64 {
        self.id
    }

    /// Returns the last observed entry kind.
    pub const fn kind(self) -> EntryKind {
        self.kind
    }

    /// Returns the last observed file size.
    pub const fn byte_len(self) -> Option<u64> {
        self.byte_len
    }

    /// Returns the best-known presence state.
    pub const fn presence(self) -> EntryPresence {
        self.presence
    }

    /// Returns the run that first created this row.
    pub const fn first_seen_run_id(self) -> ScanRunId {
        self.first_seen_run_id
    }

    /// Returns the most recent run that actually observed this path.
    pub const fn last_seen_run_id(self) -> ScanRunId {
        self.last_seen_run_id
    }

    /// Returns the complete run that first marked the row missing.
    pub const fn missing_since_run_id(self) -> Option<ScanRunId> {
        self.missing_since_run_id
    }
}

/// Durable result of finalizing a scan session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanCommit {
    run_id: ScanRunId,
    status: ScanRunStatus,
    absence_reconciled: bool,
    missing_marked: u64,
}

impl ScanCommit {
    pub(crate) const fn new(
        run_id: ScanRunId,
        status: ScanRunStatus,
        absence_reconciled: bool,
        missing_marked: u64,
    ) -> Self {
        Self {
            run_id,
            status,
            absence_reconciled,
            missing_marked,
        }
    }

    /// Returns the finalized run identifier.
    pub const fn run_id(self) -> ScanRunId {
        self.run_id
    }

    /// Returns the qualified scanner status persisted for the run.
    pub const fn status(self) -> ScanRunStatus {
        self.status
    }

    /// Reports whether unseen rows were reconciled as missing.
    pub const fn absence_reconciled(self) -> bool {
        self.absence_reconciled
    }

    /// Returns the number of rows newly marked missing.
    pub const fn missing_marked(self) -> u64 {
        self.missing_marked
    }
}

pub(crate) fn encoded_path(path: &Path) -> Vec<u8> {
    path.as_os_str().as_encoded_bytes().to_vec()
}

pub(crate) const fn entry_kind_code(kind: EntryKind) -> &'static str {
    kind.as_code()
}

pub(crate) fn parse_entry_kind(code: &str) -> Option<EntryKind> {
    match code {
        "file" => Some(EntryKind::File),
        "directory" => Some(EntryKind::Directory),
        "reparse_point" => Some(EntryKind::ReparsePoint),
        "other" => Some(EntryKind::Other),
        _ => None,
    }
}

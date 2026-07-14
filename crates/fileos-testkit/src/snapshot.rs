use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::digest::FixtureDigest;
use crate::error::{Result, TestkitError};
use crate::manifest::{DIGEST_ALGORITHM, json_escape, portable_path};

const SNAPSHOT_SCHEMA_VERSION: u32 = 1;
const SNAPSHOT_BUFFER_BYTES: usize = 64 * 1024;
const MAX_SNAPSHOT_ENTRIES: usize = 100_000;
const MAX_SNAPSHOT_DEPTH: usize = 512;
const MAX_SNAPSHOT_FILE_BYTES: u64 = 256 * 1024 * 1024;

/// Filesystem kind observed without following a symlink or other reparse point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixtureSnapshotEntryKind {
    /// A directory that may be traversed within the scanner root.
    Directory,
    /// A regular file whose bytes contribute deterministic digest evidence.
    File,
    /// A symlink or other platform reparse point that is recorded but never traversed.
    ReparsePoint,
    /// A filesystem object that is neither a regular file nor directory.
    Other,
}

impl FixtureSnapshotEntryKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Directory => "directory",
            Self::File => "file",
            Self::ReparsePoint => "reparse_point",
            Self::Other => "other",
        }
    }
}

/// Read-only evidence for one path relative to a fixture scanner root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureSnapshotEntry {
    relative_path: PathBuf,
    kind: FixtureSnapshotEntryKind,
    size: u64,
    digest: Option<String>,
    read_only: bool,
    modified_unix_nanos: Option<i128>,
    created_unix_nanos: Option<i128>,
    platform_attributes: Option<u32>,
}

impl FixtureSnapshotEntry {
    /// Returns the traversal-free path relative to the captured scanner root.
    pub fn relative_path(&self) -> &Path {
        &self.relative_path
    }

    /// Returns the no-follow filesystem kind.
    pub fn kind(&self) -> FixtureSnapshotEntryKind {
        self.kind
    }

    /// Returns the metadata length reported for this entry.
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Returns deterministic content evidence for regular files only.
    pub fn digest(&self) -> Option<&str> {
        self.digest.as_deref()
    }

    /// Returns whether the host reported the entry as read-only.
    pub fn read_only(&self) -> bool {
        self.read_only
    }

    /// Returns the modification timestamp as signed nanoseconds from the Unix epoch, if exposed.
    pub fn modified_unix_nanos(&self) -> Option<i128> {
        self.modified_unix_nanos
    }

    /// Returns the creation timestamp as signed nanoseconds from the Unix epoch, if exposed.
    pub fn created_unix_nanos(&self) -> Option<i128> {
        self.created_unix_nanos
    }

    /// Returns raw Windows file attributes when available on the current platform.
    pub fn platform_attributes(&self) -> Option<u32> {
        self.platform_attributes
    }
}

/// Stable before/after oracle for one fixture scanner root.
///
/// Equality contains only relative entry evidence. The unique sandbox run path is deliberately
/// absent so callers cannot accidentally treat a machine-local runtime path as logical data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureSnapshot {
    entries: Vec<FixtureSnapshotEntry>,
}

impl FixtureSnapshot {
    /// Returns entries in stable portable relative-path order.
    pub fn entries(&self) -> &[FixtureSnapshotEntry] {
        &self.entries
    }

    /// Serializes snapshot evidence to stable JSON without embedding the runtime root.
    pub fn to_json(&self) -> String {
        let mut json = format!(
            "{{\n  \"schema_version\": {SNAPSHOT_SCHEMA_VERSION},\n  \"digest_algorithm\": \"{DIGEST_ALGORITHM}\",\n  \"entries\": [\n"
        );
        for (index, entry) in self.entries.iter().enumerate() {
            json.push_str("    {\"path\": \"");
            json.push_str(&json_escape(&portable_path(&entry.relative_path)));
            json.push_str("\", \"kind\": \"");
            json.push_str(entry.kind.as_str());
            json.push_str(&format!(
                "\", \"size\": {}, \"digest\": {}, \"read_only\": {}, \"modified_unix_nanos\": {}, \"created_unix_nanos\": {}, \"platform_attributes\": {}}}",
                entry.size,
                json_string_or_null(entry.digest.as_deref()),
                entry.read_only,
                optional_number(entry.modified_unix_nanos),
                optional_number(entry.created_unix_nanos),
                optional_number(entry.platform_attributes)
            ));
            if index + 1 != self.entries.len() {
                json.push(',');
            }
            json.push('\n');
        }
        json.push_str("  ]\n}\n");
        json
    }
}

pub(crate) fn capture_snapshot(root: &Path) -> Result<FixtureSnapshot> {
    let root_metadata = fs::symlink_metadata(root)
        .map_err(|error| TestkitError::io("inspect fixture scan root", root, error))?;
    if !root_metadata.is_dir() || is_reparse(&root_metadata) {
        return Err(TestkitError::ScopeViolation {
            reason: "fixture scan root must be a non-reparse directory",
        });
    }

    let mut pending = vec![(root.to_path_buf(), 0_usize)];
    let mut entries = Vec::new();
    while let Some((directory, depth)) = pending.pop() {
        let directory_metadata = fs::symlink_metadata(&directory).map_err(|error| {
            TestkitError::io("reinspect fixture snapshot directory", &directory, error)
        })?;
        if !directory_metadata.is_dir() || is_reparse(&directory_metadata) {
            return Err(TestkitError::ScopeViolation {
                reason: "fixture snapshot traversal reached a reparse or non-directory entry",
            });
        }
        let children = fs::read_dir(&directory)
            .map_err(|error| TestkitError::io("enumerate fixture snapshot", &directory, error))?;
        for child in children {
            if entries.len() >= MAX_SNAPSHOT_ENTRIES {
                return Err(TestkitError::SnapshotLimitExceeded {
                    resource: "entry count",
                    limit: MAX_SNAPSHOT_ENTRIES as u64,
                });
            }
            let child = child.map_err(|error| {
                TestkitError::io("read fixture snapshot entry", &directory, error)
            })?;
            let path = child.path();
            let relative_path = path
                .strip_prefix(root)
                .map_err(|_| TestkitError::ScopeViolation {
                    reason: "fixture snapshot entry escaped the scanner root",
                })?
                .to_path_buf();
            let metadata = fs::symlink_metadata(&path).map_err(|error| {
                TestkitError::io("inspect fixture snapshot entry", &path, error)
            })?;
            let reparse = is_reparse(&metadata);
            let kind = if reparse {
                FixtureSnapshotEntryKind::ReparsePoint
            } else if metadata.is_dir() {
                FixtureSnapshotEntryKind::Directory
            } else if metadata.is_file() {
                FixtureSnapshotEntryKind::File
            } else {
                FixtureSnapshotEntryKind::Other
            };

            if matches!(kind, FixtureSnapshotEntryKind::Directory) {
                let child_depth = depth.saturating_add(1);
                if child_depth > MAX_SNAPSHOT_DEPTH {
                    return Err(TestkitError::SnapshotLimitExceeded {
                        resource: "directory depth",
                        limit: MAX_SNAPSHOT_DEPTH as u64,
                    });
                }
                pending.push((path.clone(), child_depth));
            }
            let digest = if matches!(kind, FixtureSnapshotEntryKind::File) {
                Some(stream_file_digest(&path, metadata.len())?)
            } else {
                None
            };
            entries.push(FixtureSnapshotEntry {
                relative_path,
                kind,
                size: metadata.len(),
                digest,
                read_only: metadata.permissions().readonly(),
                modified_unix_nanos: timestamp_nanos(metadata.modified().ok()),
                created_unix_nanos: timestamp_nanos(metadata.created().ok()),
                platform_attributes: platform_attributes(&metadata),
            });
        }
    }
    entries.sort_by(|left, right| {
        portable_path(&left.relative_path).cmp(&portable_path(&right.relative_path))
    });
    Ok(FixtureSnapshot { entries })
}

fn stream_file_digest(path: &Path, expected_size: u64) -> Result<String> {
    if expected_size > MAX_SNAPSHOT_FILE_BYTES {
        return Err(TestkitError::SnapshotLimitExceeded {
            resource: "file bytes",
            limit: MAX_SNAPSHOT_FILE_BYTES,
        });
    }
    let mut file = File::open(path)
        .map_err(|error| TestkitError::io("open fixture snapshot file", path, error))?;
    let mut buffer = [0_u8; SNAPSHOT_BUFFER_BYTES];
    let mut observed_size = 0_u64;
    let mut digest = FixtureDigest::new();
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| TestkitError::io("read fixture snapshot file", path, error))?;
        if read == 0 {
            break;
        }
        observed_size = observed_size.saturating_add(read as u64);
        if observed_size > MAX_SNAPSHOT_FILE_BYTES {
            return Err(TestkitError::SnapshotLimitExceeded {
                resource: "file bytes",
                limit: MAX_SNAPSHOT_FILE_BYTES,
            });
        }
        digest.update(&buffer[..read]);
    }
    if observed_size != expected_size {
        return Err(TestkitError::IdentityMismatch {
            artifact: "fixture snapshot file size",
        });
    }
    Ok(digest.finish())
}

fn timestamp_nanos(timestamp: Option<SystemTime>) -> Option<i128> {
    let timestamp = timestamp?;
    match timestamp.duration_since(UNIX_EPOCH) {
        Ok(duration) => i128::try_from(duration.as_nanos()).ok(),
        Err(error) => i128::try_from(error.duration().as_nanos())
            .ok()
            .and_then(i128::checked_neg),
    }
}

fn json_string_or_null(value: Option<&str>) -> String {
    value
        .map(|value| format!("\"{}\"", json_escape(value)))
        .unwrap_or_else(|| String::from("null"))
}

fn optional_number<T: ToString>(value: Option<T>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| String::from("null"))
}

#[cfg(windows)]
fn platform_attributes(metadata: &fs::Metadata) -> Option<u32> {
    use std::os::windows::fs::MetadataExt;

    Some(metadata.file_attributes())
}

#[cfg(not(windows))]
fn platform_attributes(_metadata: &fs::Metadata) -> Option<u32> {
    None
}

#[cfg(windows)]
fn is_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

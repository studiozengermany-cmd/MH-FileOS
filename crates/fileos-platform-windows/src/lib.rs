//! Windows path, identity, volume capability, and filesystem adapter boundary for MH FileOS.
//!
//! This crate exposes read-only platform facts. Product policy, including whether an observed
//! entry should be traversed, does not belong here.

#![forbid(unsafe_code)]

use std::fs;
use std::io;
use std::path::Path;
use std::time::SystemTime;

#[cfg(windows)]
use std::os::windows::fs::MetadataExt;

#[cfg(windows)]
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;

/// No-follow type facts observed for one filesystem entry.
///
/// A reparse point is deliberately not also classified as a traversable file or directory. The
/// caller must make an explicit policy decision before doing anything beyond recording it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EntryTypeFlags {
    file: bool,
    directory: bool,
    reparse_point: bool,
}

impl EntryTypeFlags {
    /// Returns whether the entry was observed as a regular, non-reparse file.
    pub fn is_file(self) -> bool {
        self.file
    }

    /// Returns whether the entry was observed as a non-reparse directory.
    pub fn is_directory(self) -> bool {
        self.directory
    }

    /// Returns whether the entry carries reparse/symlink semantics on this platform.
    pub fn is_reparse_point(self) -> bool {
        self.reparse_point
    }

    fn from_metadata(metadata: &fs::Metadata) -> Self {
        let reparse_point = is_reparse_point(metadata);
        Self {
            file: !reparse_point && metadata.is_file(),
            directory: !reparse_point && metadata.is_dir(),
            reparse_point,
        }
    }
}

/// A read-only, point-in-time metadata observation for one filesystem entry.
///
/// This value is evidence about one completed metadata call. It is not proof that the entry still
/// exists or remains unchanged after the call returns.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetadataSnapshot {
    entry_type: EntryTypeFlags,
    byte_len: u64,
    readonly: bool,
    modified: Option<SystemTime>,
    created: Option<SystemTime>,
    raw_windows_file_attributes: Option<u32>,
}

impl MetadataSnapshot {
    /// Returns the no-follow type facts for the observed entry.
    pub fn entry_type(&self) -> EntryTypeFlags {
        self.entry_type
    }

    /// Returns the byte length reported by the filesystem.
    ///
    /// Directory lengths are platform-defined and must not be treated as content size.
    pub fn byte_len(&self) -> u64 {
        self.byte_len
    }

    /// Returns the read-only permission flag reported by the filesystem.
    pub fn is_readonly(&self) -> bool {
        self.readonly
    }

    /// Returns the last-modified timestamp when the platform exposes it.
    pub fn modified(&self) -> Option<SystemTime> {
        self.modified
    }

    /// Returns the creation timestamp when the platform exposes it.
    pub fn created(&self) -> Option<SystemTime> {
        self.created
    }

    /// Returns raw Windows file attributes on Windows and `None` on other platforms.
    ///
    /// These bits are observation evidence, not a traversal or mutation decision.
    pub fn raw_windows_file_attributes(&self) -> Option<u32> {
        self.raw_windows_file_attributes
    }
}

/// Reads metadata for `path` without following the final symlink or reparse-point component.
///
/// The helper performs no filesystem write and does not read file content. Like all path-based
/// metadata calls, the returned snapshot can become stale immediately because another process may
/// change the filesystem concurrently.
pub fn metadata_snapshot_no_follow(path: impl AsRef<Path>) -> io::Result<MetadataSnapshot> {
    let metadata = fs::symlink_metadata(path)?;
    Ok(MetadataSnapshot {
        entry_type: EntryTypeFlags::from_metadata(&metadata),
        byte_len: metadata.len(),
        readonly: metadata.permissions().readonly(),
        modified: metadata.modified().ok(),
        created: metadata.created().ok(),
        raw_windows_file_attributes: raw_windows_file_attributes(&metadata),
    })
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(windows)]
fn raw_windows_file_attributes(metadata: &fs::Metadata) -> Option<u32> {
    Some(metadata.file_attributes())
}

#[cfg(not(windows))]
fn raw_windows_file_attributes(_metadata: &fs::Metadata) -> Option<u32> {
    None
}

#[cfg(test)]
mod tests {
    use super::metadata_snapshot_no_follow;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn repository_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("platform crate must remain below the repository root")
            .to_path_buf()
    }

    #[test]
    fn regular_repository_file_is_observed_without_mutation() {
        let path = repository_root().join("AGENTS.md");
        let before = fs::metadata(&path).expect("repository identity metadata before observation");

        let snapshot =
            metadata_snapshot_no_follow(&path).expect("read-only repository metadata snapshot");

        let after = fs::metadata(&path).expect("repository identity metadata after observation");
        assert!(snapshot.entry_type().is_file());
        assert!(!snapshot.entry_type().is_directory());
        assert!(!snapshot.entry_type().is_reparse_point());
        assert_eq!(snapshot.byte_len(), before.len());
        assert_eq!(after.len(), before.len());
        assert_eq!(after.modified().ok(), before.modified().ok());
        assert_eq!(
            after.permissions().readonly(),
            before.permissions().readonly()
        );
    }

    #[test]
    fn repository_fixture_directory_is_observed_as_directory() {
        let path = repository_root().join("fixtures");

        let snapshot =
            metadata_snapshot_no_follow(path).expect("read-only fixture directory snapshot");

        assert!(!snapshot.entry_type().is_file());
        assert!(snapshot.entry_type().is_directory());
        assert!(!snapshot.entry_type().is_reparse_point());
    }

    #[cfg(windows)]
    #[test]
    fn windows_snapshot_exposes_raw_file_attributes() {
        let path = repository_root().join("AGENTS.md");

        let snapshot =
            metadata_snapshot_no_follow(path).expect("read-only Windows metadata snapshot");

        assert!(snapshot.raw_windows_file_attributes().is_some());
    }

    #[cfg(not(windows))]
    #[test]
    fn non_windows_snapshot_omits_windows_file_attributes() {
        let path = repository_root().join("AGENTS.md");

        let snapshot =
            metadata_snapshot_no_follow(path).expect("read-only fallback metadata snapshot");

        assert_eq!(snapshot.raw_windows_file_attributes(), None);
    }
}

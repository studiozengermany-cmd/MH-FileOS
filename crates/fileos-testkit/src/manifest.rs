use std::path::{Component, Path, PathBuf};

use crate::digest::fixture_digest;
use crate::error::{Result, TestkitError};

/// Versioned deterministic digest used only for fixture evidence.
///
/// FNV-1a is intentionally lightweight and is not collision-resistant. Production duplicate
/// detection must use the separately approved fingerprint pipeline.
pub const DIGEST_ALGORITHM: &str = "fnv1a64-fixture-v1";
pub(crate) const MANIFEST_SCHEMA_VERSION: u32 = 1;
pub(crate) const GENERATOR_VERSION: &str = "fileos-testkit-m3-v1";
const LARGE_FIXTURE_BYTES: usize = 64 * 1024;

/// A deterministic optional scenario that may depend on host capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionalScenario {
    /// The generator attempts an in-sandbox file symlink and records runtime evidence separately.
    SymlinkIfSupported,
    /// ACL mutation is intentionally not attempted without a separately approved safe adapter.
    PermissionDeniedNotSimulated,
}

impl OptionalScenario {
    fn as_str(self) -> &'static str {
        match self {
            Self::SymlinkIfSupported => "symlink_if_supported",
            Self::PermissionDeniedNotSimulated => "permission_denied_not_simulated",
        }
    }
}

/// The expected kind and metadata of one logical fixture entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixtureEntryKind {
    Directory,
    File {
        size: u64,
        digest: String,
        read_only: bool,
    },
    OptionalSymlink {
        target: PathBuf,
    },
}

/// One ordered, fixture-relative manifest entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureEntry {
    path: PathBuf,
    kind: FixtureEntryKind,
}

impl FixtureEntry {
    /// Returns the traversal-free path relative to the sandbox run root.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the expected entry kind and deterministic file evidence.
    pub fn kind(&self) -> &FixtureEntryKind {
        &self.kind
    }
}

/// Logical fixture evidence that is stable across unique runtime directories.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureManifest {
    seed: u64,
    entries: Vec<FixtureEntry>,
    optional_scenarios: Vec<OptionalScenario>,
}

impl FixtureManifest {
    pub(crate) fn for_seed(seed: u64) -> Result<(Self, Vec<FilePayload>)> {
        let directories = [
            "artifacts",
            "expected",
            "input",
            "input/nested",
            "input/nested/deeper",
            "input/small",
            "input/Đề án 🚀",
            "quarantine",
        ];

        let small = deterministic_bytes(seed ^ 0x51a1_1f1e, 257);
        let large = deterministic_bytes(seed ^ 0x1a26_e5c4, LARGE_FIXTURE_BYTES);
        let unicode =
            format!("MH FileOS synthetic fixture\nseed={seed}\nUnicode: Tiếng Việt — dữ liệu 🚀\n")
                .into_bytes();
        let readonly = b"read-only synthetic fixture\n".to_vec();
        let link_target = b"in-sandbox symlink target\n".to_vec();

        let payloads = vec![
            FilePayload::new("input/empty.txt", Vec::new(), false)?,
            FilePayload::new("input/small/sample.bin", small, false)?,
            FilePayload::new("input/nested/deeper/large.bin", large, false)?,
            FilePayload::new("input/Đề án 🚀/dữ liệu & notes's.txt", unicode, false)?,
            FilePayload::new("input/read-only.txt", readonly, true)?,
            FilePayload::new("input/nested/link-target.txt", link_target, false)?,
        ];

        let mut entries = Vec::with_capacity(directories.len() + payloads.len() + 1);
        for directory in directories {
            entries.push(FixtureEntry {
                path: validate_relative_path(directory)?,
                kind: FixtureEntryKind::Directory,
            });
        }
        for payload in &payloads {
            entries.push(FixtureEntry {
                path: payload.path.clone(),
                kind: FixtureEntryKind::File {
                    size: payload.bytes.len() as u64,
                    digest: fixture_digest(&payload.bytes),
                    read_only: payload.read_only,
                },
            });
        }
        entries.push(FixtureEntry {
            path: validate_relative_path("input/link-inside.txt")?,
            kind: FixtureEntryKind::OptionalSymlink {
                target: validate_relative_path("input/nested/link-target.txt")?,
            },
        });
        entries.sort_by(|left, right| portable_path(&left.path).cmp(&portable_path(&right.path)));

        Ok((
            Self {
                seed,
                entries,
                optional_scenarios: vec![
                    OptionalScenario::SymlinkIfSupported,
                    OptionalScenario::PermissionDeniedNotSimulated,
                ],
            },
            payloads,
        ))
    }

    /// Returns the deterministic generator seed.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Returns manifest entries in stable portable-path order.
    pub fn entries(&self) -> &[FixtureEntry] {
        &self.entries
    }

    /// Returns host-dependent scenarios described without embedding their runtime outcome.
    pub fn optional_scenarios(&self) -> &[OptionalScenario] {
        &self.optional_scenarios
    }

    /// Serializes the logical manifest to stable JSON without an external dependency.
    pub fn to_json(&self) -> String {
        let mut json = String::from("{\n");
        json.push_str(&format!(
            "  \"schema_version\": {MANIFEST_SCHEMA_VERSION},\n  \"generator_version\": \"{GENERATOR_VERSION}\",\n  \"seed\": {},\n  \"disposable\": true,\n  \"digest_algorithm\": \"{DIGEST_ALGORITHM}\",\n",
            self.seed
        ));
        json.push_str("  \"optional_scenarios\": [");
        for (index, scenario) in self.optional_scenarios.iter().enumerate() {
            if index > 0 {
                json.push_str(", ");
            }
            json.push('"');
            json.push_str(scenario.as_str());
            json.push('"');
        }
        json.push_str("],\n  \"entries\": [\n");

        for (index, entry) in self.entries.iter().enumerate() {
            json.push_str("    {\"path\": \"");
            json.push_str(&json_escape(&portable_path(&entry.path)));
            match &entry.kind {
                FixtureEntryKind::Directory => json.push_str("\", \"kind\": \"directory\"}"),
                FixtureEntryKind::File {
                    size,
                    digest,
                    read_only,
                } => json.push_str(&format!(
                    "\", \"kind\": \"file\", \"size\": {size}, \"digest\": \"{digest}\", \"read_only\": {read_only}}}"
                )),
                FixtureEntryKind::OptionalSymlink { target } => {
                    json.push_str("\", \"kind\": \"optional_symlink\", \"target\": \"");
                    json.push_str(&json_escape(&portable_path(target)));
                    json.push_str("\"}");
                }
            }
            if index + 1 != self.entries.len() {
                json.push(',');
            }
            json.push('\n');
        }
        json.push_str("  ]\n}\n");
        json
    }
}

#[derive(Debug)]
pub(crate) struct FilePayload {
    pub(crate) path: PathBuf,
    pub(crate) bytes: Vec<u8>,
    pub(crate) read_only: bool,
}

impl FilePayload {
    fn new(path: &str, bytes: Vec<u8>, read_only: bool) -> Result<Self> {
        Ok(Self {
            path: validate_relative_path(path)?,
            bytes,
            read_only,
        })
    }
}

pub(crate) fn validate_relative_path(path: impl AsRef<Path>) -> Result<PathBuf> {
    let path = path.as_ref();
    let valid = !path.as_os_str().is_empty()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)));
    if !valid {
        return Err(TestkitError::InvalidRelativePath {
            path: path.to_path_buf(),
        });
    }
    Ok(path.to_path_buf())
}

pub(crate) fn portable_path(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

pub(crate) fn json_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                escaped.push_str(&format!("\\u{:04x}", u32::from(character)));
            }
            character => escaped.push(character),
        }
    }
    escaped
}

fn deterministic_bytes(seed: u64, length: usize) -> Vec<u8> {
    let mut state = seed;
    let mut bytes = Vec::with_capacity(length);
    while bytes.len() < length {
        state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^= value >> 31;
        for byte in value.to_le_bytes() {
            if bytes.len() == length {
                break;
            }
            bytes.push(byte);
        }
    }
    bytes
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{FixtureEntryKind, FixtureManifest, validate_relative_path};

    #[test]
    fn same_seed_has_identical_logical_manifest() {
        for seed in [0, 1, 42, 0xfeed_beef, u64::MAX] {
            let (first, _) = FixtureManifest::for_seed(seed).expect("first manifest");
            let (second, _) = FixtureManifest::for_seed(seed).expect("second manifest");
            assert_eq!(first, second);
            assert_eq!(first.to_json(), second.to_json());
        }
    }

    #[test]
    fn different_seeds_change_file_evidence() {
        let (first, _) = FixtureManifest::for_seed(1).expect("first manifest");
        let (second, _) = FixtureManifest::for_seed(2).expect("second manifest");
        assert_ne!(first, second);
    }

    #[test]
    fn manifest_covers_required_portable_cases() {
        let (manifest, _) = FixtureManifest::for_seed(7).expect("manifest");
        let paths = manifest
            .entries()
            .iter()
            .map(|entry| entry.path().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert!(paths.iter().any(|path| path.contains("Đề án 🚀")));
        assert!(paths.iter().any(|path| path.contains(" & ")));
        assert!(paths.iter().any(|path| path.contains("notes's")));
        assert!(manifest.entries().iter().any(|entry| {
            matches!(
                entry.kind(),
                FixtureEntryKind::File {
                    read_only: true,
                    ..
                }
            )
        }));
    }

    #[test]
    fn relative_path_validation_rejects_escape_forms() {
        for invalid in ["", "../escape", "/absolute", "nested/../../escape"] {
            assert!(validate_relative_path(Path::new(invalid)).is_err());
        }
    }

    #[test]
    fn manifest_json_probe_emits_deterministic_document() {
        let (manifest, _) = FixtureManifest::for_seed(42).expect("manifest");
        println!("MH_FILEOS_MANIFEST_JSON_BEGIN");
        print!("{}", manifest.to_json());
        println!("MH_FILEOS_MANIFEST_JSON_END");
    }
}

use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::digest::fixture_digest;
use crate::error::{CleanupFailure, Result, TestkitError};
use crate::manifest::{
    FilePayload, FixtureEntryKind, FixtureManifest, GENERATOR_VERSION, json_escape,
};
use crate::snapshot::{FixtureSnapshot, capture_snapshot};

/// Identity marker required at the root of every disposable test run.
pub const MARKER_FILE_NAME: &str = ".mh-fileos-test-sandbox";
/// Deterministic logical fixture manifest required before fixture population begins.
pub const MANIFEST_FILE_NAME: &str = "manifest.json";

const MARKER_SCHEMA_VERSION: u32 = 1;
const MAX_RUN_ALLOCATION_ATTEMPTS: u64 = 64;
const WINDOWS_ERROR_PRIVILEGE_NOT_HELD: i32 = 1314;
static RUN_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Positively identified repository-owned base for disposable fixture runs.
#[derive(Debug, Clone)]
pub struct SandboxBase {
    repository_root: PathBuf,
    root: PathBuf,
}

impl SandboxBase {
    /// Establishes `fixtures/sandbox/runtime` beneath a positively identified repository root.
    ///
    /// The constructor rejects repository aliases through reparse points and creates each missing
    /// sandbox directory one level at a time before canonical containment is checked again.
    pub fn for_repository(repository_root: impl AsRef<Path>) -> Result<Self> {
        let supplied_root = repository_root.as_ref();
        ensure_not_reparse(supplied_root, "repository root is a reparse point")?;
        let repository_root = canonicalize(supplied_root, "canonicalize repository root")?;
        ensure_repository_identity(&repository_root)?;

        let fixtures = repository_root.join("fixtures");
        ensure_directory_without_reparse(&fixtures, "fixtures directory is missing or unsafe")?;
        let sandbox = fixtures.join("sandbox");
        create_directory_level(&sandbox)?;
        let runtime = sandbox.join("runtime");
        create_directory_level(&runtime)?;

        let root = canonicalize(&runtime, "canonicalize sandbox runtime")?;
        let expected_relative = Path::new("fixtures").join("sandbox").join("runtime");
        let actual_relative =
            root.strip_prefix(&repository_root)
                .map_err(|_| TestkitError::ScopeViolation {
                    reason: "sandbox runtime escaped the canonical repository root",
                })?;
        if actual_relative != expected_relative {
            return Err(TestkitError::ScopeViolation {
                reason: "sandbox runtime did not resolve to fixtures/sandbox/runtime",
            });
        }
        ensure_no_reparse_between(&repository_root, &root)?;

        Ok(Self {
            repository_root,
            root,
        })
    }

    /// Returns the canonical repository root used to establish this capability.
    pub fn repository_root(&self) -> &Path {
        &self.repository_root
    }

    /// Returns the canonical `fixtures/sandbox/runtime` directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Creates and verifies a deterministic logical fixture in a unique runtime directory.
    pub fn create_fixture(&self, seed: u64) -> Result<SandboxRun> {
        self.verify_base_identity()?;
        let (manifest, payloads) = FixtureManifest::for_seed(seed)?;
        let (run_id, run_root, created_unix_millis) = self.allocate_run(seed)?;
        let marker_json = marker_json(&run_id, &run_root, created_unix_millis, seed);
        let manifest_json = manifest.to_json();

        write_new_file(
            &run_root.join(MARKER_FILE_NAME),
            marker_json.as_bytes(),
            "write sandbox marker",
        )?;
        write_new_file(
            &run_root.join(MANIFEST_FILE_NAME),
            manifest_json.as_bytes(),
            "write fixture manifest",
        )?;

        let mut run = SandboxRun {
            base: self.clone(),
            scan_root: run_root.join("input"),
            root: run_root,
            run_id,
            manifest,
            marker_json,
            manifest_json,
            evidence: SandboxRunEvidence {
                symlink_created: false,
                permission_denied_simulated: false,
            },
        };

        let creation_result = run
            .populate(&payloads)
            .and_then(|()| run.verify_generated_files(&payloads));
        if let Err(creation) = creation_result {
            return match run.cleanup() {
                Ok(()) => Err(creation),
                Err(cleanup_failure) => {
                    let (_run, cleanup) = cleanup_failure.into_parts();
                    Err(TestkitError::CreationCleanupFailed {
                        creation: Box::new(creation),
                        cleanup: Box::new(cleanup),
                    })
                }
            };
        }
        Ok(run)
    }

    /// Verifies that an existing directory is one direct, non-reparse child of this sandbox base.
    ///
    /// Marker and manifest identity are verified by [`SandboxRun::verify_cleanup_identity`].
    pub fn validate_cleanup_target(&self, target: impl AsRef<Path>) -> Result<()> {
        self.verify_base_identity()?;
        let target = target.as_ref();
        let metadata = symlink_metadata(target, "inspect cleanup target")?;
        if !metadata.is_dir() || is_reparse(&metadata) {
            return Err(TestkitError::ScopeViolation {
                reason: "cleanup target must be a non-reparse directory",
            });
        }
        let canonical_target = canonicalize(target, "canonicalize cleanup target")?;
        if canonical_target == self.root || canonical_target == self.repository_root {
            return Err(TestkitError::ScopeViolation {
                reason: "cleanup target is a protected repository or sandbox base",
            });
        }
        if canonical_target.parent() != Some(self.root.as_path()) {
            return Err(TestkitError::ScopeViolation {
                reason: "cleanup target must be a direct child of the sandbox runtime",
            });
        }
        if is_home_directory(&canonical_target) || canonical_target.parent().is_none() {
            return Err(TestkitError::ScopeViolation {
                reason: "cleanup target resolved to a protected filesystem location",
            });
        }
        ensure_no_reparse_between(&self.repository_root, &canonical_target)
    }

    fn verify_base_identity(&self) -> Result<()> {
        ensure_repository_identity(&self.repository_root)?;
        ensure_no_reparse_between(&self.repository_root, &self.root)?;
        let current_root = canonicalize(&self.root, "re-canonicalize sandbox runtime")?;
        if current_root != self.root {
            return Err(TestkitError::IdentityMismatch {
                artifact: "sandbox runtime root",
            });
        }
        Ok(())
    }

    fn allocate_run(&self, seed: u64) -> Result<(String, PathBuf, u128)> {
        for _attempt in 0..MAX_RUN_ALLOCATION_ATTEMPTS {
            let duration = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| TestkitError::ClockUnavailable)?;
            let unix_millis = duration.as_millis();
            let counter = RUN_COUNTER.fetch_add(1, Ordering::Relaxed);
            let run_id = format!(
                "run-{seed:016x}-{:x}-{:x}-{counter:016x}",
                process::id(),
                duration.as_nanos()
            );
            let candidate = self.root.join(&run_id);
            match fs::create_dir(&candidate) {
                Ok(()) => {
                    let canonical_candidate =
                        canonicalize(&candidate, "canonicalize allocated sandbox run")?;
                    if canonical_candidate.parent() != Some(self.root.as_path()) {
                        return Err(TestkitError::ScopeViolation {
                            reason: "allocated run escaped the sandbox runtime",
                        });
                    }
                    return Ok((run_id, canonical_candidate, unix_millis));
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(TestkitError::io("allocate sandbox run", candidate, error));
                }
            }
        }
        Err(TestkitError::RunAllocationExhausted)
    }
}

/// Runtime evidence for optional host-dependent fixture cases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SandboxRunEvidence {
    symlink_created: bool,
    permission_denied_simulated: bool,
}

impl SandboxRunEvidence {
    /// Reports whether the optional in-sandbox symlink was created on this host.
    pub fn symlink_created(self) -> bool {
        self.symlink_created
    }

    /// Reports whether a permission-denied fixture was safely simulated.
    pub fn permission_denied_simulated(self) -> bool {
        self.permission_denied_simulated
    }
}

/// Capability representing one uniquely identified disposable fixture run.
#[derive(Debug)]
pub struct SandboxRun {
    base: SandboxBase,
    root: PathBuf,
    scan_root: PathBuf,
    run_id: String,
    manifest: FixtureManifest,
    marker_json: String,
    manifest_json: String,
    evidence: SandboxRunEvidence,
}

impl SandboxRun {
    /// Returns the unique opaque run ID recorded in the marker.
    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    /// Returns the canonical runtime fixture root.
    pub fn path(&self) -> &Path {
        &self.root
    }

    /// Returns the fixture directory approved as the product scanner root.
    pub fn scan_root(&self) -> &Path {
        &self.scan_root
    }

    /// Captures stable, read-only evidence for the scanner root without following reparse points.
    ///
    /// Access time is intentionally excluded because ordinary reads may update it according to
    /// filesystem policy. Relative paths, entry kinds, file bytes, write/creation timestamps and
    /// platform attributes are retained where the host exposes them.
    pub fn snapshot(&self) -> Result<FixtureSnapshot> {
        self.verify_cleanup_identity()?;
        capture_snapshot(&self.scan_root)
    }

    /// Returns the deterministic logical fixture manifest.
    pub fn manifest(&self) -> &FixtureManifest {
        &self.manifest
    }

    /// Returns runtime evidence for optional host-dependent cases.
    pub fn evidence(&self) -> SandboxRunEvidence {
        self.evidence
    }

    /// Revalidates scope, run ID, marker bytes and manifest bytes without mutating the run.
    pub fn verify_cleanup_identity(&self) -> Result<()> {
        self.base.validate_cleanup_target(&self.root)?;
        if self.root.file_name() != Some(OsStr::new(&self.run_id)) {
            return Err(TestkitError::IdentityMismatch {
                artifact: "run directory name",
            });
        }
        verify_exact_file(
            &self.root.join(MARKER_FILE_NAME),
            self.marker_json.as_bytes(),
            "sandbox marker",
        )?;
        verify_exact_file(
            &self.root.join(MANIFEST_FILE_NAME),
            self.manifest_json.as_bytes(),
            "fixture manifest",
        )?;
        Ok(())
    }

    /// Deletes this run after fail-closed identity validation, without following reparse points.
    ///
    /// On failure, [`CleanupFailure`] retains the capability so callers do not need to rebuild one
    /// from a raw path. Successful cleanup consumes the capability.
    pub fn cleanup(self) -> std::result::Result<(), CleanupFailure> {
        if let Err(error) = self.verify_cleanup_identity() {
            return Err(CleanupFailure { error, run: self });
        }
        // The pinned Rust standard library does not follow symbolic links here and its Windows
        // implementation protects against symlink TOCTOU races. The testkit still validates
        // product-specific scope and durable identity above before invoking that primitive.
        if let Err(source) = fs::remove_dir_all(&self.root) {
            let error = TestkitError::io("remove verified sandbox tree", &self.root, source);
            return Err(CleanupFailure { error, run: self });
        }
        Ok(())
    }

    fn populate(&mut self, payloads: &[FilePayload]) -> Result<()> {
        for entry in self.manifest.entries() {
            if matches!(entry.kind(), FixtureEntryKind::Directory) {
                let directory = self.scoped_path(entry.path())?;
                fs::create_dir(&directory).map_err(|error| {
                    TestkitError::io("create fixture directory", directory, error)
                })?;
            }
        }

        for payload in payloads {
            let path = self.scoped_path(&payload.path)?;
            write_new_file(&path, &payload.bytes, "write fixture file")?;
            if payload.read_only {
                let mut permissions = fs::metadata(&path)
                    .map_err(|error| TestkitError::io("read fixture permissions", &path, error))?
                    .permissions();
                permissions.set_readonly(true);
                fs::set_permissions(&path, permissions)
                    .map_err(|error| TestkitError::io("set fixture read-only", path, error))?;
            }
        }

        let target = self.scoped_path(Path::new("input/nested/link-target.txt"))?;
        let link = self.scoped_path(Path::new("input/link-inside.txt"))?;
        self.evidence.symlink_created = try_create_file_symlink(&target, &link)?;
        Ok(())
    }

    fn verify_generated_files(&self, payloads: &[FilePayload]) -> Result<()> {
        for payload in payloads {
            let path = self.scoped_path(&payload.path)?;
            let observed = fs::read(&path)
                .map_err(|error| TestkitError::io("verify fixture file", &path, error))?;
            if observed.len() != payload.bytes.len()
                || fixture_digest(&observed) != fixture_digest(&payload.bytes)
            {
                return Err(TestkitError::IdentityMismatch {
                    artifact: "generated fixture file",
                });
            }
            if payload.read_only {
                let read_only = fs::metadata(&path)
                    .map_err(|error| TestkitError::io("verify read-only metadata", path, error))?
                    .permissions()
                    .readonly();
                if !read_only {
                    return Err(TestkitError::IdentityMismatch {
                        artifact: "read-only fixture metadata",
                    });
                }
            }
        }
        Ok(())
    }

    fn scoped_path(&self, relative: &Path) -> Result<PathBuf> {
        let valid = !relative.as_os_str().is_empty()
            && relative
                .components()
                .all(|component| matches!(component, Component::Normal(_)));
        if !valid {
            return Err(TestkitError::InvalidRelativePath {
                path: relative.to_path_buf(),
            });
        }
        Ok(self.root.join(relative))
    }
}

fn marker_json(run_id: &str, root: &Path, created_unix_millis: u128, seed: u64) -> String {
    format!(
        "{{\n  \"schema_version\": {MARKER_SCHEMA_VERSION},\n  \"run_id\": \"{}\",\n  \"canonical_root\": \"{}\",\n  \"created_unix_millis\": {created_unix_millis},\n  \"generator_version\": \"{GENERATOR_VERSION}\",\n  \"seed\": {seed},\n  \"disposable\": true\n}}\n",
        json_escape(run_id),
        json_escape(&root.to_string_lossy())
    )
}

fn ensure_repository_identity(repository_root: &Path) -> Result<()> {
    if repository_root.parent().is_none() || is_home_directory(repository_root) {
        return Err(TestkitError::InvalidRepository {
            reason: "filesystem roots and home/profile directories are forbidden",
        });
    }
    ensure_regular_identity_file(&repository_root.join("AGENTS.md"), "AGENTS.md")?;
    ensure_regular_identity_file(
        &repository_root.join("fixtures").join("README.md"),
        "fixtures/README.md",
    )
}

fn ensure_regular_identity_file(path: &Path, label: &'static str) -> Result<()> {
    let metadata = symlink_metadata(path, "inspect repository identity file")?;
    if !metadata.is_file() || is_reparse(&metadata) {
        return Err(TestkitError::InvalidRepository { reason: label });
    }
    Ok(())
}

fn ensure_directory_without_reparse(path: &Path, reason: &'static str) -> Result<()> {
    let metadata = symlink_metadata(path, "inspect repository fixture directory")?;
    if !metadata.is_dir() || is_reparse(&metadata) {
        return Err(TestkitError::InvalidRepository { reason });
    }
    Ok(())
}

fn create_directory_level(path: &Path) -> Result<()> {
    match fs::create_dir(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            ensure_directory_without_reparse(path, "sandbox directory is not a safe directory")
        }
        Err(error) => Err(TestkitError::io(
            "create sandbox directory level",
            path,
            error,
        )),
    }
}

fn ensure_no_reparse_between(base: &Path, target: &Path) -> Result<()> {
    let relative = target
        .strip_prefix(base)
        .map_err(|_| TestkitError::ScopeViolation {
            reason: "target is outside the canonical repository root",
        })?;
    ensure_not_reparse(base, "repository root became a reparse point")?;
    let mut current = base.to_path_buf();
    for component in relative.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(TestkitError::ScopeViolation {
                reason: "target contains an unsupported path component",
            });
        }
        current.push(component.as_os_str());
        ensure_not_reparse(&current, "path component is a reparse point")?;
    }
    Ok(())
}

fn ensure_not_reparse(path: &Path, reason: &'static str) -> Result<()> {
    let metadata = symlink_metadata(path, "inspect path reparse metadata")?;
    if is_reparse(&metadata) {
        return Err(TestkitError::ScopeViolation { reason });
    }
    Ok(())
}

fn canonicalize(path: &Path, operation: &'static str) -> Result<PathBuf> {
    fs::canonicalize(path).map_err(|error| TestkitError::io(operation, path, error))
}

fn symlink_metadata(path: &Path, operation: &'static str) -> Result<fs::Metadata> {
    fs::symlink_metadata(path).map_err(|error| TestkitError::io(operation, path, error))
}

fn write_new_file(path: &Path, bytes: &[u8], operation: &'static str) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| TestkitError::io(operation, path, error))?;
    file.write_all(bytes)
        .map_err(|error| TestkitError::io(operation, path, error))?;
    file.sync_all()
        .map_err(|error| TestkitError::io("flush fixture file", path, error))
}

fn verify_exact_file(path: &Path, expected: &[u8], artifact: &'static str) -> Result<()> {
    let observed =
        fs::read(path).map_err(|error| TestkitError::io("read identity file", path, error))?;
    if observed != expected {
        return Err(TestkitError::IdentityMismatch { artifact });
    }
    Ok(())
}

fn is_home_directory(path: &Path) -> bool {
    ["USERPROFILE", "HOME"]
        .into_iter()
        .filter_map(std::env::var_os)
        .filter_map(|home| fs::canonicalize(home).ok())
        .any(|home| home == path)
}

fn try_create_file_symlink(target: &Path, link: &Path) -> Result<bool> {
    let result = create_platform_file_symlink(target, link);
    match result {
        Ok(()) => Ok(true),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::PermissionDenied | io::ErrorKind::Unsupported
            ) || error.raw_os_error() == Some(WINDOWS_ERROR_PRIVILEGE_NOT_HELD) =>
        {
            Ok(false)
        }
        Err(error) => Err(TestkitError::io(
            "create optional in-sandbox symlink",
            link,
            error,
        )),
    }
}

#[cfg(windows)]
fn create_platform_file_symlink(target: &Path, link: &Path) -> io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}

#[cfg(unix)]
fn create_platform_file_symlink(target: &Path, link: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(not(any(windows, unix)))]
fn create_platform_file_symlink(_target: &Path, _link: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "file symlink creation is not supported by this platform adapter",
    ))
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

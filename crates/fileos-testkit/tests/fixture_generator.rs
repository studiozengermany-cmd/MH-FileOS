use std::fs;
use std::path::{Path, PathBuf};

use fileos_testkit::{
    FixtureEntryKind, FixtureSnapshotEntryKind, MANIFEST_FILE_NAME, MARKER_FILE_NAME, SandboxBase,
    TestkitError,
};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("testkit crate must remain below the repository root")
        .to_path_buf()
}

#[test]
fn generated_tree_matches_manifest_and_cleans_up() {
    let base = SandboxBase::for_repository(repository_root()).expect("approved sandbox base");
    let run = base.create_fixture(42).expect("fixture generation");
    let run_root = run.path().to_path_buf();

    assert!(run_root.starts_with(base.root()));
    assert_eq!(run_root.parent(), Some(base.root()));
    assert!(run_root.join(MARKER_FILE_NAME).is_file());
    assert!(run_root.join(MANIFEST_FILE_NAME).is_file());

    for entry in run.manifest().entries() {
        let path = run_root.join(entry.path());
        match entry.kind() {
            FixtureEntryKind::Directory => assert!(path.is_dir(), "{}", path.display()),
            FixtureEntryKind::File {
                size, read_only, ..
            } => {
                let metadata = fs::metadata(&path).expect("fixture file metadata");
                assert!(metadata.is_file(), "{}", path.display());
                assert_eq!(metadata.len(), *size, "{}", path.display());
                if *read_only {
                    assert!(metadata.permissions().readonly(), "{}", path.display());
                }
            }
            FixtureEntryKind::OptionalSymlink { .. } => {
                assert_eq!(
                    fs::symlink_metadata(&path).is_ok(),
                    run.evidence().symlink_created(),
                    "{}",
                    path.display()
                );
            }
        }
    }
    assert!(!run.evidence().permission_denied_simulated());
    run.verify_cleanup_identity()
        .expect("cleanup identity before removal");
    run.cleanup().expect("safe fixture cleanup");
    assert!(!run_root.exists());
}

#[test]
fn bounded_seed_set_is_logically_deterministic() {
    let base = SandboxBase::for_repository(repository_root()).expect("approved sandbox base");

    for seed in [0, 1, 42, 0xfeed_beef, u64::MAX] {
        let first = base.create_fixture(seed).expect("first fixture");
        let second = base.create_fixture(seed).expect("second fixture");
        assert_ne!(first.run_id(), second.run_id());
        assert_eq!(first.manifest(), second.manifest());
        let first_json = fs::read(first.path().join(MANIFEST_FILE_NAME)).expect("first manifest");
        let second_json =
            fs::read(second.path().join(MANIFEST_FILE_NAME)).expect("second manifest");
        assert_eq!(first_json, second_json);
        first.cleanup().expect("first cleanup");
        second.cleanup().expect("second cleanup");
    }
}

#[test]
fn scan_root_snapshot_is_stable_and_runtime_path_free() {
    let base = SandboxBase::for_repository(repository_root()).expect("approved sandbox base");
    let run = base.create_fixture(314).expect("fixture generation");

    assert_eq!(run.scan_root(), run.path().join("input"));
    let first = run.snapshot().expect("first scan-root snapshot");
    let second = run.snapshot().expect("second scan-root snapshot");
    assert_eq!(first, second);
    assert_eq!(first.to_json(), second.to_json());
    assert!(
        !first
            .to_json()
            .contains(&run.path().to_string_lossy().into_owned())
    );
    assert!(first.entries().windows(2).all(|pair| {
        pair[0].relative_path().to_string_lossy() <= pair[1].relative_path().to_string_lossy()
    }));

    run.cleanup().expect("safe fixture cleanup");
}

#[test]
fn scan_root_snapshot_records_but_does_not_follow_optional_symlink() {
    let base = SandboxBase::for_repository(repository_root()).expect("approved sandbox base");
    let run = base.create_fixture(2718).expect("fixture generation");
    let snapshot = run.snapshot().expect("scan-root snapshot");
    let link_path = Path::new("link-inside.txt");
    let link_entries = snapshot
        .entries()
        .iter()
        .filter(|entry| entry.relative_path() == link_path)
        .collect::<Vec<_>>();

    if run.evidence().symlink_created() {
        assert_eq!(link_entries.len(), 1);
        assert_eq!(
            link_entries[0].kind(),
            FixtureSnapshotEntryKind::ReparsePoint
        );
        assert_eq!(link_entries[0].digest(), None);
    } else {
        assert!(link_entries.is_empty());
    }
    assert_eq!(
        snapshot
            .entries()
            .iter()
            .filter(|entry| entry.relative_path() == Path::new("nested/link-target.txt"))
            .count(),
        1
    );

    run.cleanup().expect("safe fixture cleanup");
}

#[test]
fn scan_root_snapshot_detects_same_size_content_change() {
    let base = SandboxBase::for_repository(repository_root()).expect("approved sandbox base");
    let run = base.create_fixture(1_618).expect("fixture generation");
    let target = run.scan_root().join("small/sample.bin");
    let before = run.snapshot().expect("snapshot before scoped mutation");
    let mut bytes = fs::read(&target).expect("read scoped synthetic file");
    assert!(!bytes.is_empty());
    bytes[0] ^= 0xff;
    fs::write(&target, &bytes).expect("write same-size scoped synthetic file");
    let after = run.snapshot().expect("snapshot after scoped mutation");

    assert_ne!(before, after);
    assert_eq!(
        fs::metadata(&target)
            .expect("mutated fixture metadata")
            .len(),
        bytes.len() as u64
    );
    run.cleanup().expect("safe fixture cleanup");
}

#[test]
fn cleanup_scope_rejects_repository_and_sandbox_base() {
    let repository_root = repository_root();
    let base = SandboxBase::for_repository(&repository_root).expect("approved sandbox base");

    assert!(matches!(
        base.validate_cleanup_target(&repository_root),
        Err(TestkitError::ScopeViolation { .. })
    ));
    assert!(matches!(
        base.validate_cleanup_target(base.root()),
        Err(TestkitError::ScopeViolation { .. })
    ));
}

#[test]
fn marker_tamper_fails_closed_and_retains_capability() {
    let base = SandboxBase::for_repository(repository_root()).expect("approved sandbox base");
    let run = base.create_fixture(91).expect("fixture generation");
    let marker_path = run.path().join(MARKER_FILE_NAME);
    let original_marker = fs::read(&marker_path).expect("original marker");
    fs::write(&marker_path, b"tampered\n").expect("tamper marker inside fixture");

    assert!(matches!(
        run.verify_cleanup_identity(),
        Err(TestkitError::IdentityMismatch {
            artifact: "sandbox marker"
        })
    ));
    let failure = run
        .cleanup()
        .expect_err("tampered marker must block cleanup");
    let (run, error) = failure.into_parts();
    assert!(matches!(
        error,
        TestkitError::IdentityMismatch {
            artifact: "sandbox marker"
        }
    ));

    fs::write(&marker_path, original_marker).expect("restore marker inside fixture");
    run.cleanup().expect("cleanup after marker restoration");
}

#[test]
fn manifest_tamper_and_missing_marker_fail_closed() {
    let base = SandboxBase::for_repository(repository_root()).expect("approved sandbox base");
    let run = base.create_fixture(123).expect("fixture generation");
    let manifest_path = run.path().join(MANIFEST_FILE_NAME);
    let marker_path = run.path().join(MARKER_FILE_NAME);
    let original_manifest = fs::read(&manifest_path).expect("original manifest");
    let original_marker = fs::read(&marker_path).expect("original marker");

    fs::write(&manifest_path, b"{}\n").expect("tamper manifest inside fixture");
    assert!(matches!(
        run.verify_cleanup_identity(),
        Err(TestkitError::IdentityMismatch {
            artifact: "fixture manifest"
        })
    ));
    fs::write(&manifest_path, original_manifest).expect("restore fixture manifest");

    fs::remove_file(&marker_path).expect("remove marker inside fixture");
    assert!(matches!(
        run.verify_cleanup_identity(),
        Err(TestkitError::Io { .. })
    ));
    fs::write(&marker_path, original_marker).expect("restore fixture marker");
    run.cleanup().expect("cleanup after identity restoration");
}

#[test]
fn repository_identity_is_required_before_any_sandbox_write() {
    let invalid_root = repository_root().join("fixtures");
    assert!(matches!(
        SandboxBase::for_repository(invalid_root),
        Err(TestkitError::Io { .. }) | Err(TestkitError::InvalidRepository { .. })
    ));
}

#[test]
fn cleanup_never_follows_an_outside_symlink_target() {
    let repository_root = repository_root();
    let base = SandboxBase::for_repository(&repository_root).expect("approved sandbox base");
    let run = base.create_fixture(777).expect("fixture generation");
    let outside_target = repository_root.join("README.md");
    let outside_before = fs::read(&outside_target).expect("outside target before cleanup");
    let injected_link = run.path().join("input/outside-target-link.md");

    if let Err(error) = create_file_symlink_for_test(&outside_target, &injected_link) {
        if matches!(
            error.kind(),
            std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::Unsupported
        ) || error.raw_os_error() == Some(1314)
        {
            run.cleanup().expect("cleanup after unsupported symlink");
            return;
        }
        panic!("unexpected symlink creation failure: {error}");
    }

    run.cleanup()
        .expect("cleanup must unlink without following target");
    let outside_after = fs::read(&outside_target).expect("outside target after cleanup");
    assert_eq!(outside_before, outside_after);
    assert!(outside_target.is_file());
}

#[cfg(windows)]
fn create_file_symlink_for_test(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}

#[cfg(unix)]
fn create_file_symlink_for_test(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(not(any(windows, unix)))]
fn create_file_symlink_for_test(_target: &Path, _link: &Path) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "test symlink creation is unsupported",
    ))
}

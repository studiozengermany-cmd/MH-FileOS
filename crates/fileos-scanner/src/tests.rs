use std::fs;
use std::io;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

use fileos_app::{
    CancellationToken, ScanFailureCode, ScanPort, ScanProgress, ScanRequest, ScanSink,
    ScanSinkError,
};
use fileos_domain::{
    EntryKind, ObservedEntry, ResourceLimits, ScanIssue, ScanIssueCode, ScanRunStatus,
};
use fileos_testkit::{FixtureSnapshotEntryKind, SandboxBase};

use super::{ReadOnlyScanner, issue_code};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("scanner crate must remain below the repository root")
        .to_path_buf()
}

fn non_zero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).expect("test resource limit must be non-zero")
}

fn limits(open_directories: usize, max_depth: usize, progress_every: usize) -> ResourceLimits {
    ResourceLimits::new(
        non_zero(1),
        non_zero(4),
        non_zero(open_directories),
        non_zero(max_depth),
        non_zero(progress_every),
    )
}

#[derive(Default)]
struct CollectingSink {
    entries: Vec<ObservedEntry>,
    issues: Vec<ScanIssue>,
    progress: Vec<ScanProgress>,
}

impl ScanSink for CollectingSink {
    fn observe(&mut self, entry: ObservedEntry) -> Result<(), ScanSinkError> {
        self.entries.push(entry);
        Ok(())
    }

    fn issue(&mut self, issue: ScanIssue) -> Result<(), ScanSinkError> {
        self.issues.push(issue);
        Ok(())
    }

    fn progress(&mut self, progress: ScanProgress) -> Result<(), ScanSinkError> {
        self.progress.push(progress);
        Ok(())
    }
}

struct CancellingSink {
    inner: CollectingSink,
    token: CancellationToken,
    cancel_after_entries: usize,
}

impl ScanSink for CancellingSink {
    fn observe(&mut self, entry: ObservedEntry) -> Result<(), ScanSinkError> {
        self.inner.observe(entry)?;
        if self.inner.entries.len() >= self.cancel_after_entries {
            self.token.cancel();
        }
        Ok(())
    }

    fn issue(&mut self, issue: ScanIssue) -> Result<(), ScanSinkError> {
        self.inner.issue(issue)
    }

    fn progress(&mut self, progress: ScanProgress) -> Result<(), ScanSinkError> {
        self.inner.progress(progress)
    }
}

#[test]
fn complete_scan_matches_snapshot_and_is_read_only() {
    let base = SandboxBase::for_repository(repository_root()).expect("approved sandbox base");
    let run = base.create_fixture(4_004).expect("fixture generation");
    let before = run.snapshot().expect("before snapshot");
    let request = ScanRequest::new(run.scan_root().to_path_buf(), limits(16, 32, 2))
        .expect("absolute fixture scan request");

    let scanner = ReadOnlyScanner::new();
    let mut first_sink = CollectingSink::default();
    let first = scanner
        .scan(&request, &CancellationToken::new(), &mut first_sink)
        .expect("first complete scan");
    assert_eq!(first.status(), ScanRunStatus::Completed);
    assert!(first_sink.issues.is_empty());

    let expected_files = before
        .entries()
        .iter()
        .filter(|entry| entry.kind() == FixtureSnapshotEntryKind::File)
        .count() as u64;
    let expected_directories = before
        .entries()
        .iter()
        .filter(|entry| entry.kind() == FixtureSnapshotEntryKind::Directory)
        .count() as u64;
    let expected_reparse = before
        .entries()
        .iter()
        .filter(|entry| entry.kind() == FixtureSnapshotEntryKind::ReparsePoint)
        .count() as u64;
    let expected_other = before
        .entries()
        .iter()
        .filter(|entry| entry.kind() == FixtureSnapshotEntryKind::Other)
        .count() as u64;
    let expected_bytes = before
        .entries()
        .iter()
        .filter(|entry| entry.kind() == FixtureSnapshotEntryKind::File)
        .map(|entry| entry.size())
        .sum::<u64>();

    let counters = first.counters();
    assert_eq!(counters.files(), expected_files);
    assert_eq!(counters.directories(), expected_directories);
    assert_eq!(counters.reparse_points_skipped(), expected_reparse);
    assert_eq!(counters.other_entries(), expected_other);
    assert_eq!(counters.bytes(), expected_bytes);
    assert_eq!(counters.total_entries(), before.entries().len() as u64);
    assert_eq!(first_sink.entries.len(), before.entries().len());

    let usage = first.resource_usage();
    assert!(usage.metadata_queue_high_water() <= request.limits().metadata_queue_capacity().get());
    assert!(usage.metadata_workers_high_water() <= request.limits().metadata_workers().get());
    assert!(usage.open_directories_high_water() <= request.limits().open_directories().get());
    assert!(usage.max_depth_observed() <= request.limits().max_depth().get());
    assert!(usage.progress_events_emitted() > 0);

    let after = run.snapshot().expect("after snapshot");
    assert_eq!(before, after);

    let mut second_sink = CollectingSink::default();
    let second = scanner
        .scan(&request, &CancellationToken::new(), &mut second_sink)
        .expect("second complete scan");
    assert_eq!(first.to_json(), second.to_json());
    assert_eq!(first_sink.entries, second_sink.entries);
    run.cleanup().expect("safe fixture cleanup");
}

#[test]
fn cancellation_is_terminal_and_never_reported_complete() {
    let base = SandboxBase::for_repository(repository_root()).expect("approved sandbox base");
    let run = base.create_fixture(5_005).expect("fixture generation");
    let before = run.snapshot().expect("before snapshot");
    let request = ScanRequest::new(run.scan_root().to_path_buf(), limits(16, 32, 1))
        .expect("absolute fixture scan request");
    let scanner = ReadOnlyScanner::new();

    let pre_cancelled = CancellationToken::new();
    pre_cancelled.cancel();
    let mut pre_sink = CollectingSink::default();
    let pre_report = scanner
        .scan(&request, &pre_cancelled, &mut pre_sink)
        .expect("pre-cancelled report");
    assert_eq!(pre_report.status(), ScanRunStatus::Cancelled);
    assert!(pre_sink.entries.is_empty());

    let mid_token = CancellationToken::new();
    let mut mid_sink = CancellingSink {
        inner: CollectingSink::default(),
        token: mid_token.clone(),
        cancel_after_entries: 2,
    };
    let mid_report = scanner
        .scan(&request, &mid_token, &mut mid_sink)
        .expect("mid-scan cancellation report");
    assert_eq!(mid_report.status(), ScanRunStatus::Cancelled);
    assert!(mid_sink.inner.entries.len() >= 2);
    assert!(mid_sink.inner.entries.len() < before.entries().len());
    assert_eq!(before, run.snapshot().expect("after cancellation snapshot"));
    run.cleanup().expect("safe fixture cleanup");
}

#[test]
fn open_directory_limit_is_a_truthful_partial_result() {
    let base = SandboxBase::for_repository(repository_root()).expect("approved sandbox base");
    let run = base.create_fixture(6_006).expect("fixture generation");
    let before = run.snapshot().expect("before snapshot");
    let request = ScanRequest::new(run.scan_root().to_path_buf(), limits(1, 32, 4))
        .expect("absolute fixture scan request");
    let mut sink = CollectingSink::default();
    let report = ReadOnlyScanner::new()
        .scan(&request, &CancellationToken::new(), &mut sink)
        .expect("bounded partial scan");

    assert_eq!(report.status(), ScanRunStatus::CompletedWithIssues);
    assert!(
        report
            .issue_counts()
            .count(ScanIssueCode::OpenDirectoryLimitReached)
            > 0
    );
    assert!(
        sink.issues
            .iter()
            .any(|issue| { issue.code() == ScanIssueCode::OpenDirectoryLimitReached })
    );
    assert_eq!(before, run.snapshot().expect("after bounded scan snapshot"));
    run.cleanup().expect("safe fixture cleanup");
}

#[test]
fn invalid_limits_fail_before_filesystem_observation() {
    let base = SandboxBase::for_repository(repository_root()).expect("approved sandbox base");
    let run = base.create_fixture(7_007).expect("fixture generation");
    let before = run.snapshot().expect("before snapshot");
    let invalid = ResourceLimits::new(
        non_zero(usize::MAX),
        non_zero(1),
        non_zero(1),
        non_zero(1),
        non_zero(1),
    );
    let request = ScanRequest::new(run.scan_root().to_path_buf(), invalid)
        .expect("absolute request with deferred resource validation");
    let mut sink = CollectingSink::default();
    let failure = ReadOnlyScanner::new()
        .scan(&request, &CancellationToken::new(), &mut sink)
        .expect_err("invalid limits must fail");

    assert_eq!(failure.code(), ScanFailureCode::ResourceLimitExceeded);
    assert!(sink.entries.is_empty());
    assert_eq!(
        before,
        run.snapshot().expect("after invalid request snapshot")
    );
    run.cleanup().expect("safe fixture cleanup");
}

#[test]
fn disappearing_file_becomes_partial_and_scan_continues() {
    let base = SandboxBase::for_repository(repository_root()).expect("approved sandbox base");
    let run = base.create_fixture(8_008).expect("fixture generation");
    let relative_target = PathBuf::from("small/sample.bin");
    let absolute_target = run.scan_root().join(&relative_target);
    let request = ScanRequest::new(run.scan_root().to_path_buf(), limits(16, 32, 4))
        .expect("absolute fixture scan request");
    let mut removed = false;
    let mut hook = |relative_path: &Path| {
        if !removed && relative_path == relative_target {
            fs::remove_file(&absolute_target).expect("remove scoped race fixture");
            removed = true;
        }
    };
    let mut sink = CollectingSink::default();
    let report = ReadOnlyScanner::new()
        .scan_with_hook(&request, &CancellationToken::new(), &mut sink, &mut hook)
        .expect("scan with deterministic disappearance");

    assert!(removed);
    assert_eq!(report.status(), ScanRunStatus::CompletedWithIssues);
    assert!(sink.issues.iter().any(|issue| {
        issue.code() == ScanIssueCode::Disappeared
            && issue.relative_path() == Some(relative_target.as_path())
    }));
    assert!(report.counters().files() > 0);
    run.cleanup().expect("safe fixture cleanup");
}

#[test]
fn outside_target_symlink_is_observed_without_traversal() {
    let base = SandboxBase::for_repository(repository_root()).expect("approved sandbox base");
    let run = base.create_fixture(9_009).expect("fixture generation");
    let sentinel = run.path().join("expected/outside-sentinel.txt");
    let link = run.scan_root().join("outside-target-link.txt");
    let sentinel_bytes = b"synthetic outside-scope sentinel\n";
    fs::write(&sentinel, sentinel_bytes).expect("write scoped sentinel");

    if let Err(error) = create_file_symlink_for_test(&sentinel, &link) {
        if matches!(
            error.kind(),
            io::ErrorKind::PermissionDenied | io::ErrorKind::Unsupported
        ) || error.raw_os_error() == Some(1314)
        {
            run.cleanup().expect("cleanup after unsupported symlink");
            return;
        }
        panic!("unexpected symlink creation failure: {error}");
    }

    let before = run.snapshot().expect("before symlink scan snapshot");
    let request = ScanRequest::new(run.scan_root().to_path_buf(), limits(16, 32, 4))
        .expect("absolute fixture scan request");
    let mut sink = CollectingSink::default();
    let report = ReadOnlyScanner::new()
        .scan(&request, &CancellationToken::new(), &mut sink)
        .expect("scan with outside-target symlink");

    assert_eq!(report.status(), ScanRunStatus::Completed);
    assert!(report.counters().reparse_points_skipped() >= 1);
    assert!(sink.entries.iter().any(|entry| {
        entry.relative_path() == Path::new("outside-target-link.txt")
            && entry.kind() == EntryKind::ReparsePoint
    }));
    assert!(
        !sink
            .entries
            .iter()
            .any(|entry| { entry.relative_path() == Path::new("outside-sentinel.txt") })
    );
    assert_eq!(
        fs::read(&sentinel).expect("sentinel after scan"),
        sentinel_bytes
    );
    assert_eq!(before, run.snapshot().expect("after symlink scan snapshot"));
    run.cleanup().expect("safe fixture cleanup");
}

#[test]
fn permission_denied_maps_to_typed_retryable_issue_code() {
    let error = io::Error::from(io::ErrorKind::PermissionDenied);
    assert_eq!(
        issue_code(fileos_domain::ScanPhase::MetadataRead, &error),
        ScanIssueCode::AccessDenied
    );
}

#[cfg(windows)]
fn create_file_symlink_for_test(target: &Path, link: &Path) -> io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}

#[cfg(unix)]
fn create_file_symlink_for_test(target: &Path, link: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(not(any(windows, unix)))]
fn create_file_symlink_for_test(_target: &Path, _link: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "test symlink creation is unsupported",
    ))
}

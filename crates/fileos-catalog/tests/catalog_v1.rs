use std::fs;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::time::Duration;

use fileos_app::{CancellationToken, ScanPort, ScanReport, ScanRequest, ScanSink};
use fileos_catalog::{
    Catalog, CatalogErrorCode, CatalogOpenOptions, EntryPresence, PresenceFilter,
    ScanRootDescriptor, ScanRootId, SummaryQuery, VolumeDescriptor,
};
use fileos_domain::{
    EntryKind, ObservedEntry, ResourceLimits, ResourceUsage, ScanCounters, ScanIssue,
    ScanIssueCode, ScanIssueCounts, ScanPhase, ScanRunStatus,
};
use fileos_scanner::ReadOnlyScanner;
use fileos_testkit::{FixtureSnapshot, SandboxBase, SandboxRun};
use rusqlite::{Connection, TransactionBehavior};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("catalog crate must remain below the repository root")
        .to_path_buf()
}

fn non_zero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).expect("test bound must be non-zero")
}

fn limits(open_directories: usize) -> ResourceLimits {
    ResourceLimits::new(
        non_zero(1),
        non_zero(8),
        non_zero(open_directories),
        non_zero(64),
        non_zero(2),
    )
}

fn create_run(seed: u64) -> SandboxRun {
    SandboxBase::for_repository(repository_root())
        .expect("approved sandbox base")
        .create_fixture(seed)
        .expect("fixture generation")
}

fn with_run(seed: u64, test: impl FnOnce(&SandboxRun)) {
    let run = create_run(seed);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| test(&run)));
    run.cleanup().expect("safe fixture cleanup after test body");
    if let Err(payload) = result {
        std::panic::resume_unwind(payload);
    }
}

fn database_path(run: &SandboxRun, name: &str) -> PathBuf {
    run.path().join("artifacts").join(name)
}

fn register_fixture_root(catalog: &mut Catalog, run: &SandboxRun) -> ScanRootId {
    let volume = catalog
        .upsert_volume(&VolumeDescriptor::new(
            "synthetic_fixture",
            run.run_id().as_bytes().to_vec(),
            Some("test-sandbox".to_owned()),
        ))
        .expect("register synthetic volume");
    catalog
        .upsert_scan_root(&ScanRootDescriptor::exact_path(
            volume,
            run.scan_root().to_path_buf(),
            1,
            1,
        ))
        .expect("register fixture root")
}

fn scan_fixture(
    catalog: &mut Catalog,
    root_id: ScanRootId,
    run: &SandboxRun,
    open_directories: usize,
) -> fileos_catalog::ScanCommit {
    let request = ScanRequest::new(run.scan_root().to_path_buf(), limits(open_directories))
        .expect("valid fixture scan request");
    let scanner = ReadOnlyScanner::new();
    let mut session = catalog
        .begin_scan(root_id, non_zero(2))
        .expect("begin catalog scan");
    let report = match scanner.scan(&request, &CancellationToken::new(), &mut session) {
        Ok(report) => report,
        Err(failure) => {
            session.fail(failure).expect("persist fatal scan");
            panic!("fixture scan failed: {}", failure.code().as_code());
        }
    };
    session.finish(&report).expect("finalize catalog scan")
}

fn assert_summary_matches_snapshot(
    catalog: &Catalog,
    root_id: ScanRootId,
    snapshot: &FixtureSnapshot,
) {
    let summary = catalog
        .summary(SummaryQuery::present_for_root(root_id))
        .expect("present root summary");
    assert_eq!(summary.entries(), snapshot.entries().len() as u64);
    assert_eq!(summary.bytes(), snapshot_file_bytes(snapshot));
    assert_eq!(
        summary.files(),
        snapshot
            .entries()
            .iter()
            .filter(|entry| entry.kind() == fileos_testkit::FixtureSnapshotEntryKind::File)
            .count() as u64
    );
    assert_eq!(
        summary.directories(),
        snapshot
            .entries()
            .iter()
            .filter(|entry| entry.kind() == fileos_testkit::FixtureSnapshotEntryKind::Directory)
            .count() as u64
    );
}

#[test]
fn fresh_database_migrates_reopens_and_enforces_connection_settings() {
    let run = create_run(5_001);
    let database = database_path(&run, "fresh.sqlite");

    {
        let mut catalog = Catalog::open(&database, CatalogOpenOptions::default())
            .expect("fresh catalog migration");
        assert_eq!(catalog.schema_version(), 1);
        assert_eq!(catalog.journal_mode(), "wal");
        assert!(catalog.foreign_keys_enabled().expect("foreign-key pragma"));
        assert!(catalog.quick_check().expect("quick check"));
        let root_id = register_fixture_root(&mut catalog, &run);
        assert_eq!(catalog.entry_record_count(root_id).expect("empty root"), 0);
    }

    {
        let catalog = Catalog::open(&database, CatalogOpenOptions::default())
            .expect("reopen migrated catalog");
        assert!(catalog.quick_check().expect("reopen quick check"));
    }

    let raw = Connection::open(&database).expect("raw catalog inspection");
    let user_version: i64 = raw
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("user version");
    let application_id: i64 = raw
        .query_row("PRAGMA application_id", [], |row| row.get(0))
        .expect("application id");
    let table_count: i64 = raw
        .query_row(
            "SELECT COUNT(*) FROM sqlite_schema
             WHERE type = 'table' AND name IN ('volumes', 'scan_roots', 'scan_runs', 'file_entries')",
            [],
            |row| row.get(0),
        )
        .expect("catalog tables");
    assert_eq!(user_version, 1);
    assert_eq!(application_id, 1_296_582_223);
    assert_eq!(table_count, 4);
    drop(raw);

    run.cleanup().expect("safe fixture cleanup");
}

#[test]
fn repeat_scan_converges_and_scanner_preserves_fixture() {
    let run = create_run(5_002);
    let before = run.snapshot().expect("before snapshot");
    let database = database_path(&run, "repeat.sqlite");

    {
        let mut catalog =
            Catalog::open(&database, CatalogOpenOptions::default()).expect("open repeat catalog");
        let root_id = register_fixture_root(&mut catalog, &run);
        let first_commit = scan_fixture(&mut catalog, root_id, &run, 32);
        assert_eq!(first_commit.status(), ScanRunStatus::Completed);
        assert!(first_commit.absence_reconciled());
        assert_eq!(first_commit.missing_marked(), 0);
        assert_summary_matches_snapshot(&catalog, root_id, &before);

        let first_entry = catalog
            .entry_record(root_id, Path::new("empty.txt"))
            .expect("first entry query")
            .expect("empty file row");
        let first_row_count = catalog
            .entry_record_count(root_id)
            .expect("first row count");

        let second_commit = scan_fixture(&mut catalog, root_id, &run, 32);
        assert_eq!(second_commit.status(), ScanRunStatus::Completed);
        let second_entry = catalog
            .entry_record(root_id, Path::new("empty.txt"))
            .expect("second entry query")
            .expect("empty file row after repeat");
        assert_eq!(second_entry.id(), first_entry.id());
        assert_eq!(
            second_entry.first_seen_run_id(),
            first_entry.first_seen_run_id()
        );
        assert_ne!(
            second_entry.last_seen_run_id(),
            first_entry.last_seen_run_id()
        );
        assert_eq!(
            catalog
                .entry_record_count(root_id)
                .expect("repeat row count"),
            first_row_count
        );
        assert_eq!(catalog.scan_run_count(root_id).expect("run history"), 2);

        let file_summary = catalog
            .summary(SummaryQuery {
                root_id: Some(root_id),
                category: Some(EntryKind::File),
                presence: PresenceFilter::Present,
            })
            .expect("file category summary");
        assert_eq!(file_summary.entries(), file_summary.files());
        assert_eq!(file_summary.bytes(), snapshot_file_bytes(&before));
    }

    let after = run.snapshot().expect("after repeat-scan snapshot");
    assert_eq!(after, before);
    run.cleanup().expect("safe fixture cleanup");
}

fn snapshot_file_bytes(snapshot: &FixtureSnapshot) -> u64 {
    snapshot
        .entries()
        .iter()
        .filter(|entry| entry.kind() == fileos_testkit::FixtureSnapshotEntryKind::File)
        .map(fileos_testkit::FixtureSnapshotEntry::size)
        .sum()
}

#[test]
fn complete_scan_marks_missing_and_reappearance_reuses_the_row() {
    let run = create_run(5_003);
    let database = database_path(&run, "missing.sqlite");
    let removed_relative = Path::new("empty.txt");
    let removed_path = run.scan_root().join(removed_relative);

    {
        let mut catalog =
            Catalog::open(&database, CatalogOpenOptions::default()).expect("open missing catalog");
        let root_id = register_fixture_root(&mut catalog, &run);
        scan_fixture(&mut catalog, root_id, &run, 32);
        let original = catalog
            .entry_record(root_id, removed_relative)
            .expect("original row query")
            .expect("original row");
        let retained_count = catalog
            .entry_record_count(root_id)
            .expect("original row count");

        run.verify_cleanup_identity()
            .expect("scope proof before fixture mutation");
        fs::remove_file(&removed_path).expect("remove synthetic fixture file");
        let missing_commit = scan_fixture(&mut catalog, root_id, &run, 32);
        assert_eq!(missing_commit.missing_marked(), 1);
        let missing = catalog
            .entry_record(root_id, removed_relative)
            .expect("missing row query")
            .expect("retained missing row");
        assert_eq!(missing.id(), original.id());
        assert_eq!(missing.presence(), EntryPresence::Missing);
        assert_eq!(missing.last_seen_run_id(), original.last_seen_run_id());
        assert_eq!(
            missing.missing_since_run_id(),
            Some(missing_commit.run_id())
        );
        assert_eq!(
            catalog
                .entry_record_count(root_id)
                .expect("retained row count"),
            retained_count
        );

        fs::write(&removed_path, b"changed").expect("restore synthetic file with new metadata");
        scan_fixture(&mut catalog, root_id, &run, 32);
        let reappeared = catalog
            .entry_record(root_id, removed_relative)
            .expect("reappeared row query")
            .expect("reappeared row");
        assert_eq!(reappeared.id(), original.id());
        assert_eq!(reappeared.presence(), EntryPresence::Present);
        assert_eq!(reappeared.missing_since_run_id(), None);
        assert_eq!(reappeared.byte_len(), Some(7));
    }

    run.cleanup().expect("safe fixture cleanup");
}

#[test]
fn partial_scan_never_marks_unseen_entries_missing() {
    let run = create_run(5_004);
    let database = database_path(&run, "partial.sqlite");

    {
        let mut catalog =
            Catalog::open(&database, CatalogOpenOptions::default()).expect("open partial catalog");
        let root_id = register_fixture_root(&mut catalog, &run);
        scan_fixture(&mut catalog, root_id, &run, 32);
        let nested = PathBuf::from("nested").join("deeper").join("large.bin");
        let before = catalog
            .entry_record(root_id, &nested)
            .expect("nested query")
            .expect("nested row");

        let partial = scan_fixture(&mut catalog, root_id, &run, 1);
        assert_eq!(partial.status(), ScanRunStatus::CompletedWithIssues);
        assert!(!partial.absence_reconciled());
        assert_eq!(partial.missing_marked(), 0);
        let after = catalog
            .entry_record(root_id, &nested)
            .expect("nested query after partial")
            .expect("nested row retained");
        assert_eq!(after.presence(), EntryPresence::Present);
        assert_eq!(after.id(), before.id());

        let request = ScanRequest::new(run.scan_root().to_path_buf(), limits(32))
            .expect("cancelled scan request");
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let scanner = ReadOnlyScanner::new();
        let mut cancelled_session = catalog
            .begin_scan(root_id, non_zero(2))
            .expect("begin cancelled scan");
        let cancelled_report = scanner
            .scan(&request, &cancellation, &mut cancelled_session)
            .expect("qualified cancelled report");
        let cancelled_commit = cancelled_session
            .finish(&cancelled_report)
            .expect("finalize cancelled run");
        assert_eq!(cancelled_commit.status(), ScanRunStatus::Cancelled);
        assert!(!cancelled_commit.absence_reconciled());

        let missing_root_request =
            ScanRequest::new(run.scan_root().join("does-not-exist"), limits(32))
                .expect("absolute unavailable-root request");
        let mut failed_session = catalog
            .begin_scan(root_id, non_zero(2))
            .expect("begin failed scan");
        let failure = scanner
            .scan(
                &missing_root_request,
                &CancellationToken::new(),
                &mut failed_session,
            )
            .expect_err("missing scan root must fail");
        failed_session.fail(failure).expect("persist failed scan");

        let after_terminal_variants = catalog
            .entry_record(root_id, &nested)
            .expect("nested query after cancelled/failed")
            .expect("nested row retained after cancelled/failed");
        assert_eq!(after_terminal_variants.presence(), EntryPresence::Present);
        assert_eq!(after_terminal_variants.id(), before.id());
    }

    run.cleanup().expect("safe fixture cleanup");
}

#[test]
fn newer_foreign_and_corrupt_databases_fail_with_typed_redacted_errors() {
    let run = create_run(5_005);
    run.verify_cleanup_identity()
        .expect("scope proof before database fixtures");

    let newer = database_path(&run, "newer.sqlite");
    let raw = Connection::open(&newer).expect("create newer database");
    raw.execute_batch("PRAGMA user_version = 2;")
        .expect("mark newer schema");
    drop(raw);
    let error = match Catalog::open(&newer, CatalogOpenOptions::default()) {
        Ok(_) => panic!("newer schema must be rejected"),
        Err(error) => error,
    };
    assert_eq!(error.code(), CatalogErrorCode::UnsupportedSchemaVersion);
    assert_eq!(error.migration_version(), Some(2));
    assert!(
        !error
            .to_string()
            .contains(&newer.to_string_lossy().into_owned())
    );

    let foreign = database_path(&run, "foreign.sqlite");
    let raw = Connection::open(&foreign).expect("create foreign database");
    raw.execute_batch("CREATE TABLE unrelated (id INTEGER PRIMARY KEY);")
        .expect("create foreign table");
    drop(raw);
    let error = match Catalog::open(&foreign, CatalogOpenOptions::default()) {
        Ok(_) => panic!("foreign database must be rejected"),
        Err(error) => error,
    };
    assert_eq!(error.code(), CatalogErrorCode::NotCatalog);

    let corrupt = database_path(&run, "corrupt.sqlite");
    fs::write(&corrupt, b"not a sqlite database").expect("create corrupt database bytes");
    let error = match Catalog::open(&corrupt, CatalogOpenOptions::default()) {
        Ok(_) => panic!("corrupt database must be rejected"),
        Err(error) => error,
    };
    assert_eq!(error.code(), CatalogErrorCode::Corrupt);
    assert!(!error.retryable());
    assert!(
        !error
            .to_string()
            .contains(&corrupt.to_string_lossy().into_owned())
    );

    run.cleanup().expect("safe fixture cleanup");
}

#[test]
fn writer_contention_returns_retryable_typed_busy_error() {
    let run = create_run(5_006);
    let database = database_path(&run, "busy.sqlite");
    let root_id = {
        let mut catalog =
            Catalog::open(&database, CatalogOpenOptions::default()).expect("prepare busy catalog");
        register_fixture_root(&mut catalog, &run)
    };

    let mut locker = Connection::open(&database).expect("open locking connection");
    let transaction = locker
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .expect("hold immediate writer transaction");
    let mut contender = Catalog::open(
        &database,
        CatalogOpenOptions::new(Duration::from_millis(25)),
    )
    .expect("open contender");
    let error = match contender.begin_scan(root_id, non_zero(2)) {
        Ok(_) => panic!("writer contention must not begin a scan"),
        Err(error) => error,
    };
    assert_eq!(error.code(), CatalogErrorCode::Busy);
    assert!(error.retryable());
    transaction.rollback().expect("release writer lock");
    drop(contender);
    drop(locker);

    run.cleanup().expect("safe fixture cleanup");
}

#[test]
fn second_active_scan_for_the_same_root_is_rejected() {
    use fileos_app::{ScanFailure, ScanFailureCode};
    use fileos_domain::{ResourceUsage, ScanCounters, ScanPhase};

    let run = create_run(5_008);
    let database = database_path(&run, "active.sqlite");
    let mut owner =
        Catalog::open(&database, CatalogOpenOptions::default()).expect("open active owner");
    let root_id = register_fixture_root(&mut owner, &run);
    let first_session = owner
        .begin_scan(root_id, non_zero(2))
        .expect("begin first active scan");

    let mut contender =
        Catalog::open(&database, CatalogOpenOptions::default()).expect("open active contender");
    let error = match contender.begin_scan(root_id, non_zero(2)) {
        Ok(_) => panic!("a second active scan must be rejected"),
        Err(error) => error,
    };
    assert_eq!(error.code(), CatalogErrorCode::ActiveScanExists);
    assert!(error.retryable());

    let failure = ScanFailure::new(
        ScanFailureCode::InternalInvariant,
        ScanPhase::Finalization,
        false,
        None,
        ScanCounters::default(),
        ResourceUsage::default(),
    );
    first_session.fail(failure).expect("close first active run");
    drop(contender);
    drop(owner);
    run.cleanup().expect("safe fixture cleanup");
}

#[test]
fn out_of_range_observation_survives_sink_mapping_as_typed_catalog_error() {
    use fileos_app::{ScanFailure, ScanFailureCode};
    use fileos_domain::{ResourceUsage, ScanCounters, ScanPhase};

    let run = create_run(5_007);
    let database = database_path(&run, "range.sqlite");

    {
        let mut catalog =
            Catalog::open(&database, CatalogOpenOptions::default()).expect("open range catalog");
        let root_id = register_fixture_root(&mut catalog, &run);
        let mut session = catalog
            .begin_scan(root_id, non_zero(1))
            .expect("begin range scan");
        let entry = ObservedEntry::new(
            PathBuf::from("too-large.bin"),
            EntryKind::File,
            Some(u64::MAX),
            None,
            false,
        )
        .expect("domain accepts u64 file size");
        assert!(session.observe(entry).is_err());
        let failure = ScanFailure::new(
            ScanFailureCode::SinkRejected,
            ScanPhase::SinkDelivery,
            false,
            None,
            ScanCounters::default(),
            ResourceUsage::default(),
        );
        let error = session
            .fail(failure)
            .expect_err("typed catalog error must survive sink adapter");
        assert_eq!(error.code(), CatalogErrorCode::NumericOutOfRange);

        let retry = catalog
            .begin_scan(root_id, non_zero(2))
            .expect("sink failure must not leave the root permanently active");
        retry
            .fail(failure)
            .expect("close retry run after terminalization proof");
    }

    run.cleanup().expect("safe fixture cleanup");
}

#[test]
fn dropped_scan_can_be_explicitly_recovered_after_reopen() {
    with_run(5_009, |run| {
        let database = database_path(run, "recover.sqlite");
        let root_id = {
            let mut catalog = Catalog::open(&database, CatalogOpenOptions::default())
                .expect("open recovery catalog");
            let root_id = register_fixture_root(&mut catalog, run);
            let session = catalog
                .begin_scan(root_id, non_zero(2))
                .expect("begin interrupted scan");
            drop(session);
            let blocked = match catalog.begin_scan(root_id, non_zero(2)) {
                Ok(_) => panic!("unrecovered active scan must remain blocked"),
                Err(error) => error,
            };
            assert_eq!(blocked.code(), CatalogErrorCode::ActiveScanExists);
            root_id
        };

        let mut reopened = Catalog::open(&database, CatalogOpenOptions::default())
            .expect("reopen interrupted catalog");
        assert_eq!(
            reopened
                .recover_interrupted_scans()
                .expect("explicit startup recovery"),
            1
        );
        let commit = scan_fixture(&mut reopened, root_id, run, 32);
        assert_eq!(commit.status(), ScanRunStatus::Completed);
    });
}

#[test]
fn mismatched_completed_report_is_rejected_without_marking_missing() {
    with_run(5_010, |run| {
        let database = database_path(run, "report-mismatch.sqlite");
        let mut catalog = Catalog::open(&database, CatalogOpenOptions::default())
            .expect("open report-mismatch catalog");
        let root_id = register_fixture_root(&mut catalog, run);
        scan_fixture(&mut catalog, root_id, run, 32);
        let retained_path = PathBuf::from("nested").join("deeper").join("large.bin");
        let retained_before = catalog
            .entry_record(root_id, &retained_path)
            .expect("retained query before mismatch")
            .expect("retained row before mismatch");

        let mut session = catalog
            .begin_scan(root_id, non_zero(2))
            .expect("begin mismatched report scan");
        session
            .observe(
                ObservedEntry::new(
                    PathBuf::from("empty.txt"),
                    EntryKind::File,
                    Some(0),
                    None,
                    false,
                )
                .expect("valid observation"),
            )
            .expect("stream one observation");
        let false_completed = ScanReport::new(
            ScanRunStatus::Completed,
            ScanCounters::default(),
            ScanIssueCounts::default(),
            limits(32),
            ResourceUsage::default(),
        )
        .expect("syntactically valid but unrelated report");
        let error = session
            .finish(&false_completed)
            .expect_err("session/report mismatch must fail closed");
        assert_eq!(error.code(), CatalogErrorCode::ScanReportMismatch);

        let retained_after = catalog
            .entry_record(root_id, &retained_path)
            .expect("retained query after mismatch")
            .expect("retained row after mismatch");
        assert_eq!(retained_after.id(), retained_before.id());
        assert_eq!(retained_after.presence(), EntryPresence::Present);
        scan_fixture(&mut catalog, root_id, run, 32);
    });
}

#[test]
fn mismatched_issue_breakdown_is_rejected() {
    with_run(5_014, |run| {
        let database = database_path(run, "issue-mismatch.sqlite");
        let mut catalog = Catalog::open(&database, CatalogOpenOptions::default())
            .expect("open issue-mismatch catalog");
        let root_id = register_fixture_root(&mut catalog, run);
        let mut session = catalog
            .begin_scan(root_id, non_zero(2))
            .expect("begin issue-mismatch scan");
        session
            .issue(
                ScanIssue::new(
                    ScanIssueCode::AccessDenied,
                    ScanPhase::MetadataRead,
                    None,
                    false,
                    None,
                )
                .expect("valid streamed issue"),
            )
            .expect("stream one issue");

        let mut counters = ScanCounters::default();
        counters.record_issue();
        let mut unrelated_issue_counts = ScanIssueCounts::default();
        unrelated_issue_counts.record(ScanIssueCode::Disappeared);
        let report = ScanReport::new(
            ScanRunStatus::CompletedWithIssues,
            counters,
            unrelated_issue_counts,
            limits(32),
            ResourceUsage::default(),
        )
        .expect("internally consistent but unrelated issue report");
        let error = session
            .finish(&report)
            .expect_err("issue breakdown mismatch must fail closed");
        assert_eq!(error.code(), CatalogErrorCode::ScanReportMismatch);
        scan_fixture(&mut catalog, root_id, run, 32);
    });
}

#[test]
fn tampered_durable_observation_count_is_rejected() {
    with_run(5_015, |run| {
        let database = database_path(run, "durable-count-mismatch.sqlite");
        let mut catalog = Catalog::open(&database, CatalogOpenOptions::default())
            .expect("open durable-count catalog");
        let root_id = register_fixture_root(&mut catalog, run);
        let mut session = catalog
            .begin_scan(root_id, non_zero(1))
            .expect("begin durable-count scan");
        let entry = ObservedEntry::new(
            PathBuf::from("one.bin"),
            EntryKind::File,
            Some(1),
            None,
            false,
        )
        .expect("valid observation");
        let mut counters = ScanCounters::default();
        counters.record_entry(&entry);
        session.observe(entry).expect("flush one observation");

        let raw = Connection::open(&database).expect("open raw catalog for count tamper");
        assert_eq!(
            raw.execute(
                "UPDATE scan_runs SET observed_entries = 0 WHERE id = ?1",
                [session.run_id().get()],
            )
            .expect("tamper durable observation count"),
            1
        );
        drop(raw);

        let report = ScanReport::new(
            ScanRunStatus::Completed,
            counters,
            ScanIssueCounts::default(),
            limits(32),
            ResourceUsage::default(),
        )
        .expect("matching streamed report");
        let error = session
            .finish(&report)
            .expect_err("durable observation mismatch must fail closed");
        assert_eq!(error.code(), CatalogErrorCode::ScanReportMismatch);
        scan_fixture(&mut catalog, root_id, run, 32);
    });
}

#[test]
fn malformed_v1_schema_is_rejected_on_open() {
    with_run(5_011, |run| {
        let database = database_path(run, "malformed-v1.sqlite");
        let raw = Connection::open(&database).expect("create malformed v1 database");
        raw.execute_batch(
            "CREATE TABLE volumes (id INTEGER PRIMARY KEY);
             CREATE TABLE scan_roots (id INTEGER PRIMARY KEY);
             CREATE TABLE scan_runs (id INTEGER PRIMARY KEY);
             CREATE TABLE file_entries (id INTEGER PRIMARY KEY);
             PRAGMA application_id = 1296582223;
             PRAGMA user_version = 1;",
        )
        .expect("create four incompatible tables");
        drop(raw);

        let error = match Catalog::open(&database, CatalogOpenOptions::default()) {
            Ok(_) => panic!("malformed v1 schema must not open"),
            Err(error) => error,
        };
        assert_eq!(error.code(), CatalogErrorCode::NotCatalog);
    });
}

#[test]
fn same_named_but_incompatible_index_is_rejected_on_open() {
    with_run(5_013, |run| {
        let database = database_path(run, "incompatible-index.sqlite");
        drop(
            Catalog::open(&database, CatalogOpenOptions::default()).expect("create valid catalog"),
        );

        let raw = Connection::open(&database).expect("open raw catalog");
        raw.execute_batch(
            "DROP INDEX file_entries_summary;
             CREATE INDEX file_entries_summary ON file_entries(kind);",
        )
        .expect("replace index with incompatible same-named definition");
        drop(raw);

        let error = match Catalog::open(&database, CatalogOpenOptions::default()) {
            Ok(_) => panic!("same-named incompatible index must not open"),
            Err(error) => error,
        };
        assert_eq!(error.code(), CatalogErrorCode::NotCatalog);
    });
}

#[test]
fn persisted_foreign_key_violation_is_rejected_on_reopen() {
    with_run(5_012, |run| {
        let database = database_path(run, "foreign-key-violation.sqlite");
        {
            let mut catalog = Catalog::open(&database, CatalogOpenOptions::default())
                .expect("create valid catalog");
            register_fixture_root(&mut catalog, run);
        }

        let raw = Connection::open(&database).expect("open raw catalog with foreign keys off");
        raw.execute_batch("PRAGMA foreign_keys = OFF;")
            .expect("disable foreign keys for corruption injection");
        raw.execute(
            "INSERT INTO scan_runs (
                scan_root_id, status, started_unix_seconds, started_subsec_nanos
             ) VALUES (999999, 'running', 0, 0)",
            [],
        )
        .expect("inject persisted orphan row");
        drop(raw);

        let error = match Catalog::open(&database, CatalogOpenOptions::default()) {
            Ok(_) => panic!("persisted foreign-key violation must not open"),
            Err(error) => error,
        };
        assert_eq!(error.code(), CatalogErrorCode::InvalidPersistedData);
    });
}

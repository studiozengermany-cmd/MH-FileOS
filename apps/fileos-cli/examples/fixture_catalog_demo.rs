use std::error::Error;
use std::fmt;
use std::io::{self, Write};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use fileos_app::{CancellationToken, ScanPort, ScanRequest};
use fileos_catalog::{
    Catalog, CatalogOpenOptions, CatalogSummary, ScanRootDescriptor, SummaryQuery, VolumeDescriptor,
};
use fileos_domain::{ResourceLimits, ScanRunStatus};
use fileos_scanner::ReadOnlyScanner;
use fileos_testkit::{
    FixtureEntryKind, FixtureManifest, FixtureSnapshot, FixtureSnapshotEntryKind, SandboxBase,
    SandboxRun,
};

const DEMO_SEED: u64 = 6_001;
const DEMO_DATABASE_NAME: &str = "m6-vertical-slice.sqlite";
const EXIT_FAILURE: u8 = 1;
const EXIT_IO_ERROR: u8 = 74;

fn main() -> ExitCode {
    let stdout = io::stdout();
    let mut output = stdout.lock();
    match run_fixture_catalog_demo(&mut output) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let stderr = io::stderr();
            let mut errors = stderr.lock();
            let message = format!("{}: {}\n", error.code().as_code(), error);
            if errors.write_all(message.as_bytes()).is_err() {
                ExitCode::from(EXIT_IO_ERROR)
            } else {
                ExitCode::from(EXIT_FAILURE)
            }
        }
    }
}

fn run_fixture_catalog_demo(output: &mut dyn Write) -> Result<(), DemoError> {
    let repository = repository_root()?;
    let base = SandboxBase::for_repository(repository)
        .map_err(|_| DemoError::new(DemoErrorCode::SandboxSetupFailed))?;
    let run = base
        .create_fixture(DEMO_SEED)
        .map_err(|_| DemoError::new(DemoErrorCode::SandboxSetupFailed))?;

    let evidence = build_evidence(&run);
    let cleanup = run.cleanup();
    if cleanup.is_err() {
        return Err(DemoError::new(DemoErrorCode::CleanupFailed));
    }

    let json = evidence?.to_json();
    output
        .write_all(json.as_bytes())
        .map_err(|_| DemoError::new(DemoErrorCode::OutputFailed))
}

fn build_evidence(run: &SandboxRun) -> Result<DemoEvidence, DemoError> {
    let before = run
        .snapshot()
        .map_err(|_| DemoError::new(DemoErrorCode::SnapshotFailed))?;
    let expected =
        ExpectedSummary::from_manifest(run.manifest(), run.evidence().symlink_created())?;
    let snapshot_summary = ExpectedSummary::from_snapshot(&before)?;
    if expected != snapshot_summary {
        return Err(DemoError::new(DemoErrorCode::FixtureEvidenceMismatch));
    }
    let database = run.path().join("artifacts").join(DEMO_DATABASE_NAME);
    let limits = demo_limits()?;
    let request = ScanRequest::new(run.scan_root().to_path_buf(), limits)
        .map_err(|_| DemoError::new(DemoErrorCode::InternalInvariant))?;

    let mut catalog = Catalog::open(&database, CatalogOpenOptions::default())
        .map_err(|_| DemoError::new(DemoErrorCode::CatalogFailed))?;
    let volume_id = catalog
        .upsert_volume(&VolumeDescriptor::new(
            "synthetic_fixture",
            run.run_id().as_bytes().to_vec(),
            Some("test-sandbox".to_owned()),
        ))
        .map_err(|_| DemoError::new(DemoErrorCode::CatalogFailed))?;
    let root_id = catalog
        .upsert_scan_root(&ScanRootDescriptor::exact_path(
            volume_id,
            run.scan_root().to_path_buf(),
            1,
            1,
        ))
        .map_err(|_| DemoError::new(DemoErrorCode::CatalogFailed))?;
    let batch_size =
        NonZeroUsize::new(64).ok_or_else(|| DemoError::new(DemoErrorCode::InternalInvariant))?;
    let mut session = catalog
        .begin_scan(root_id, batch_size)
        .map_err(|_| DemoError::new(DemoErrorCode::CatalogFailed))?;
    let scanner = ReadOnlyScanner::new();
    let report = match scanner.scan(&request, &CancellationToken::new(), &mut session) {
        Ok(report) => report,
        Err(failure) => {
            return if session.fail(failure).is_err() {
                Err(DemoError::new(DemoErrorCode::CatalogFailed))
            } else {
                Err(DemoError::new(DemoErrorCode::ScanFailed))
            };
        }
    };
    let commit = session
        .finish(&report)
        .map_err(|_| DemoError::new(DemoErrorCode::CatalogFailed))?;
    if report.status() != ScanRunStatus::Completed
        || commit.status() != ScanRunStatus::Completed
        || !commit.absence_reconciled()
    {
        return Err(DemoError::new(DemoErrorCode::IncompleteScan));
    }
    drop(catalog);

    let catalog = Catalog::open(&database, CatalogOpenOptions::default())
        .map_err(|_| DemoError::new(DemoErrorCode::CatalogFailed))?;
    if !catalog
        .quick_check()
        .map_err(|_| DemoError::new(DemoErrorCode::CatalogFailed))?
    {
        return Err(DemoError::new(DemoErrorCode::CatalogIntegrityFailed));
    }
    let summary = catalog
        .summary(SummaryQuery::present_for_root(root_id))
        .map_err(|_| DemoError::new(DemoErrorCode::CatalogFailed))?;
    let record_count = catalog
        .entry_record_count(root_id)
        .map_err(|_| DemoError::new(DemoErrorCode::CatalogFailed))?;
    let scan_run_count = catalog
        .scan_run_count(root_id)
        .map_err(|_| DemoError::new(DemoErrorCode::CatalogFailed))?;
    if record_count != expected.entries || scan_run_count != 1 {
        return Err(DemoError::new(DemoErrorCode::CatalogEvidenceMismatch));
    }

    let after = run
        .snapshot()
        .map_err(|_| DemoError::new(DemoErrorCode::SnapshotFailed))?;
    if before != after {
        return Err(DemoError::new(DemoErrorCode::SourceChanged));
    }
    if !expected.matches(summary) {
        return Err(DemoError::new(DemoErrorCode::SummaryMismatch));
    }

    Ok(DemoEvidence::new(summary, catalog.schema_version()))
}

fn repository_root() -> Result<PathBuf, DemoError> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .map(Path::to_path_buf)
        .ok_or_else(|| DemoError::new(DemoErrorCode::InternalInvariant))
}

fn demo_limits() -> Result<ResourceLimits, DemoError> {
    Ok(ResourceLimits::new(
        non_zero(1)?,
        non_zero(64)?,
        non_zero(64)?,
        non_zero(256)?,
        non_zero(128)?,
    ))
}

fn non_zero(value: usize) -> Result<NonZeroUsize, DemoError> {
    NonZeroUsize::new(value).ok_or_else(|| DemoError::new(DemoErrorCode::InternalInvariant))
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ExpectedSummary {
    entries: u64,
    files: u64,
    directories: u64,
    reparse_points: u64,
    other_entries: u64,
    bytes: u64,
}

impl ExpectedSummary {
    fn from_manifest(
        manifest: &FixtureManifest,
        optional_symlink_created: bool,
    ) -> Result<Self, DemoError> {
        let mut expected = Self::default();
        for entry in manifest.entries() {
            let Ok(relative) = entry.path().strip_prefix(Path::new("input")) else {
                continue;
            };
            if relative.as_os_str().is_empty() {
                continue;
            }
            match entry.kind() {
                FixtureEntryKind::Directory => {
                    expected.entries = checked_increment(expected.entries)?;
                    expected.directories = checked_increment(expected.directories)?;
                }
                FixtureEntryKind::File { size, .. } => {
                    expected.entries = checked_increment(expected.entries)?;
                    expected.files = checked_increment(expected.files)?;
                    expected.bytes = expected
                        .bytes
                        .checked_add(*size)
                        .ok_or_else(|| DemoError::new(DemoErrorCode::NumericOverflow))?;
                }
                FixtureEntryKind::OptionalSymlink { .. } if optional_symlink_created => {
                    expected.entries = checked_increment(expected.entries)?;
                    expected.reparse_points = checked_increment(expected.reparse_points)?;
                }
                FixtureEntryKind::OptionalSymlink { .. } => {}
            }
        }
        Ok(expected)
    }

    fn from_snapshot(snapshot: &FixtureSnapshot) -> Result<Self, DemoError> {
        let mut expected = Self::default();
        for entry in snapshot.entries() {
            expected.entries = checked_increment(expected.entries)?;
            match entry.kind() {
                FixtureSnapshotEntryKind::File => {
                    expected.files = checked_increment(expected.files)?;
                    expected.bytes = expected
                        .bytes
                        .checked_add(entry.size())
                        .ok_or_else(|| DemoError::new(DemoErrorCode::NumericOverflow))?;
                }
                FixtureSnapshotEntryKind::Directory => {
                    expected.directories = checked_increment(expected.directories)?;
                }
                FixtureSnapshotEntryKind::ReparsePoint => {
                    expected.reparse_points = checked_increment(expected.reparse_points)?;
                }
                FixtureSnapshotEntryKind::Other => {
                    expected.other_entries = checked_increment(expected.other_entries)?;
                }
            }
        }
        Ok(expected)
    }

    fn matches(self, actual: CatalogSummary) -> bool {
        self.entries == actual.entries()
            && self.files == actual.files()
            && self.directories == actual.directories()
            && self.reparse_points == actual.reparse_points()
            && self.other_entries == actual.other_entries()
            && self.bytes == actual.bytes()
    }
}

fn checked_increment(value: u64) -> Result<u64, DemoError> {
    value
        .checked_add(1)
        .ok_or_else(|| DemoError::new(DemoErrorCode::NumericOverflow))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DemoEvidence {
    summary: CatalogSummary,
    catalog_schema_version: u32,
}

impl DemoEvidence {
    const fn new(summary: CatalogSummary, catalog_schema_version: u32) -> Self {
        Self {
            summary,
            catalog_schema_version,
        }
    }

    fn to_json(self) -> String {
        format!(
            concat!(
                "{{\n",
                "  \"schema_version\": 1,\n",
                "  \"status\": \"verified\",\n",
                "  \"scan_status\": \"completed\",\n",
                "  \"catalog_schema_version\": {},\n",
                "  \"catalog_summary\": {{\"entries\": {}, \"files\": {}, ",
                "\"directories\": {}, \"reparse_points\": {}, ",
                "\"other_entries\": {}, \"bytes\": {}}},\n",
                "  \"catalog_reopened\": true,\n",
                "  \"integrity_check_passed\": true,\n",
                "  \"snapshot_matches_manifest\": true,\n",
                "  \"summary_matches_manifest\": true,\n",
                "  \"source_unchanged\": true,\n",
                "  \"sandbox_cleaned\": true\n",
                "}}\n"
            ),
            self.catalog_schema_version,
            self.summary.entries(),
            self.summary.files(),
            self.summary.directories(),
            self.summary.reparse_points(),
            self.summary.other_entries(),
            self.summary.bytes(),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DemoErrorCode {
    SandboxSetupFailed,
    SnapshotFailed,
    CatalogFailed,
    CatalogIntegrityFailed,
    CatalogEvidenceMismatch,
    FixtureEvidenceMismatch,
    ScanFailed,
    IncompleteScan,
    SummaryMismatch,
    SourceChanged,
    CleanupFailed,
    OutputFailed,
    NumericOverflow,
    InternalInvariant,
}

impl DemoErrorCode {
    const fn as_code(self) -> &'static str {
        match self {
            Self::SandboxSetupFailed => "m6_demo_sandbox_setup_failed",
            Self::SnapshotFailed => "m6_demo_snapshot_failed",
            Self::CatalogFailed => "m6_demo_catalog_failed",
            Self::CatalogIntegrityFailed => "m6_demo_catalog_integrity_failed",
            Self::CatalogEvidenceMismatch => "m6_demo_catalog_evidence_mismatch",
            Self::FixtureEvidenceMismatch => "m6_demo_fixture_evidence_mismatch",
            Self::ScanFailed => "m6_demo_scan_failed",
            Self::IncompleteScan => "m6_demo_incomplete_scan",
            Self::SummaryMismatch => "m6_demo_summary_mismatch",
            Self::SourceChanged => "m6_demo_source_changed",
            Self::CleanupFailed => "m6_demo_cleanup_failed",
            Self::OutputFailed => "m6_demo_output_failed",
            Self::NumericOverflow => "m6_demo_numeric_overflow",
            Self::InternalInvariant => "m6_demo_internal_invariant",
        }
    }

    const fn user_safe_message(self) -> &'static str {
        match self {
            Self::SandboxSetupFailed => "The disposable fixture could not be prepared safely.",
            Self::SnapshotFailed => "Read-only fixture evidence could not be captured.",
            Self::CatalogFailed => "The local demo catalog operation failed.",
            Self::CatalogIntegrityFailed => "The local demo catalog failed its integrity check.",
            Self::CatalogEvidenceMismatch => {
                "The persisted demo catalog evidence was internally inconsistent."
            }
            Self::FixtureEvidenceMismatch => {
                "The fixture snapshot did not match its deterministic manifest."
            }
            Self::ScanFailed => "The read-only fixture scan failed.",
            Self::IncompleteScan => "The demo scan did not complete authoritatively.",
            Self::SummaryMismatch => "The catalog summary did not match the fixture evidence.",
            Self::SourceChanged => "The fixture changed during the read-only workflow.",
            Self::CleanupFailed => "The disposable fixture could not be cleaned up safely.",
            Self::OutputFailed => "The verified demo summary could not be written.",
            Self::NumericOverflow => "The demo evidence exceeded a supported numeric range.",
            Self::InternalInvariant => "The demo encountered an internal invariant failure.",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DemoError {
    code: DemoErrorCode,
}

impl DemoError {
    const fn new(code: DemoErrorCode) -> Self {
        Self { code }
    }

    const fn code(self) -> DemoErrorCode {
        self.code
    }
}

impl fmt::Display for DemoError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code.user_safe_message())
    }
}

impl Error for DemoError {}

#[cfg(test)]
mod tests {
    use std::io::{self, Write};

    use super::{DemoErrorCode, run_fixture_catalog_demo};

    #[test]
    fn fixture_catalog_demo_emits_verified_path_free_summary_and_cleans() {
        let mut output = Vec::new();
        run_fixture_catalog_demo(&mut output).expect("vertical slice must pass");
        let json = String::from_utf8(output).expect("demo output must be UTF-8 JSON");

        assert!(json.contains("\"status\": \"verified\""));
        assert!(json.contains("\"scan_status\": \"completed\""));
        assert!(json.contains("\"files\":"));
        assert!(json.contains("\"directories\":"));
        assert!(json.contains("\"bytes\":"));
        assert!(json.contains("\"snapshot_matches_manifest\": true"));
        assert!(json.contains("\"summary_matches_manifest\": true"));
        assert!(json.contains("\"source_unchanged\": true"));
        assert!(json.contains("\"sandbox_cleaned\": true"));
        assert!(!json.contains(env!("CARGO_MANIFEST_DIR")));
        assert!(!json.contains("run-"));
        assert!(!json.contains('\\'));
        assert!(!json.contains("\"path\""));
        assert!(!json.contains("\"run_id\""));

        println!("MH_FILEOS_M6_JSON_BEGIN");
        print!("{json}");
        println!("MH_FILEOS_M6_JSON_END");
    }

    #[test]
    fn output_failure_is_typed_and_occurs_after_safe_cleanup() {
        let error = run_fixture_catalog_demo(&mut FailingWriter)
            .expect_err("failing output must return a typed error");
        assert_eq!(error.code(), DemoErrorCode::OutputFailed);
        assert_eq!(
            error.to_string(),
            "The verified demo summary could not be written."
        );
    }

    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "synthetic output failure",
            ))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
}

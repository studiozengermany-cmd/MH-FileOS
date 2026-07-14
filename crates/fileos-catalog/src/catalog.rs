use std::num::NonZeroUsize;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use fileos_app::{
    ScanFailure, ScanProgress, ScanReport, ScanSink, ScanSinkError, ScanSinkErrorCode,
};
use fileos_domain::{ObservedEntry, ScanCounters, ScanIssue, ScanIssueCounts, ScanRunStatus};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use crate::error::{CatalogError, CatalogErrorCode, Result};
use crate::migration::{CATALOG_SCHEMA_VERSION, migrate, pragma_i64};
use crate::model::{
    CatalogSummary, EntryPresence, EntryRecord, ScanCommit, ScanRootDescriptor, ScanRootId,
    ScanRunId, SummaryQuery, VolumeDescriptor, VolumeId, encoded_path, entry_kind_code,
    parse_entry_kind,
};

const PATH_ENCODING: &str = "rust_os_str_encoded_bytes_v1";
const DEFAULT_BUSY_TIMEOUT: Duration = Duration::from_millis(500);

/// Largest accepted in-memory observation batch for a catalog scan session.
pub const MAX_CATALOG_BATCH_SIZE: usize = 65_536;

/// Connection settings whose effects are verified when the catalog opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatalogOpenOptions {
    busy_timeout: Duration,
}

impl CatalogOpenOptions {
    /// Creates explicit catalog connection settings.
    pub const fn new(busy_timeout: Duration) -> Self {
        Self { busy_timeout }
    }

    /// Returns the bounded SQLite busy timeout.
    pub const fn busy_timeout(self) -> Duration {
        self.busy_timeout
    }
}

impl Default for CatalogOpenOptions {
    fn default() -> Self {
        Self::new(DEFAULT_BUSY_TIMEOUT)
    }
}

/// SQLite-backed local observation catalog.
///
/// The catalog has no filesystem mutation API. Its rows are historical observations, not proof
/// that the source filesystem still matches a previous scan.
pub struct Catalog {
    connection: Connection,
    journal_mode: String,
}

impl Catalog {
    /// Opens or atomically migrates one caller-selected catalog database.
    pub fn open(path: impl AsRef<Path>, options: CatalogOpenOptions) -> Result<Self> {
        let mut connection = Connection::open(path).map_err(|error| {
            CatalogError::from_sqlite(error, CatalogErrorCode::OpenFailed, None)
        })?;
        connection
            .busy_timeout(options.busy_timeout())
            .map_err(|error| {
                CatalogError::from_sqlite(error, CatalogErrorCode::ConfigurationFailed, None)
            })?;
        connection
            .execute_batch("PRAGMA foreign_keys = ON;")
            .map_err(|error| {
                CatalogError::from_sqlite(error, CatalogErrorCode::ConfigurationFailed, None)
            })?;
        if pragma_i64(&connection, "foreign_keys")? != 1 {
            return Err(CatalogError::new(
                CatalogErrorCode::ConfigurationFailed,
                None,
                false,
            ));
        }

        migrate(&mut connection)?;

        let journal_mode: String = connection
            .query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))
            .map_err(|error| {
                CatalogError::from_sqlite(error, CatalogErrorCode::ConfigurationFailed, None)
            })?;
        if !journal_mode.eq_ignore_ascii_case("wal") {
            return Err(CatalogError::new(
                CatalogErrorCode::ConfigurationFailed,
                None,
                false,
            ));
        }
        Ok(Self {
            connection,
            journal_mode,
        })
    }

    /// Returns the understood schema version after successful open/migration.
    pub const fn schema_version(&self) -> u32 {
        CATALOG_SCHEMA_VERSION
    }

    /// Returns the journal mode verified for this connection.
    pub fn journal_mode(&self) -> &str {
        &self.journal_mode
    }

    /// Verifies that foreign-key enforcement remains enabled on this connection.
    pub fn foreign_keys_enabled(&self) -> Result<bool> {
        Ok(pragma_i64(&self.connection, "foreign_keys")? == 1)
    }

    /// Runs SQLite's bounded quick integrity check.
    pub fn quick_check(&self) -> Result<bool> {
        let result: String = self
            .connection
            .query_row("PRAGMA quick_check", [], |row| row.get(0))
            .map_err(|error| {
                CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, None)
            })?;
        Ok(result == "ok")
    }

    /// Inserts or refreshes one explicitly identified volume.
    pub fn upsert_volume(&mut self, descriptor: &VolumeDescriptor) -> Result<VolumeId> {
        if descriptor.identity_kind.is_empty() || descriptor.identity_value.is_empty() {
            return Err(CatalogError::new(
                CatalogErrorCode::InvalidInput,
                None,
                false,
            ));
        }
        let (now, _) = system_time_parts(SystemTime::now())?;
        let id = self
            .connection
            .query_row(
                "INSERT INTO volumes (
                    identity_kind, identity_value, filesystem,
                    first_seen_unix_seconds, last_seen_unix_seconds
                 ) VALUES (?1, ?2, ?3, ?4, ?4)
                 ON CONFLICT(identity_kind, identity_value) DO UPDATE SET
                    filesystem = excluded.filesystem,
                    last_seen_unix_seconds = excluded.last_seen_unix_seconds
                 RETURNING id",
                params![
                    descriptor.identity_kind,
                    descriptor.identity_value,
                    descriptor.filesystem,
                    now
                ],
                |row| row.get(0),
            )
            .map_err(|error| {
                CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, None)
            })?;
        Ok(VolumeId(id))
    }

    /// Inserts or refreshes one explicit scan root without filesystem access.
    pub fn upsert_scan_root(&mut self, descriptor: &ScanRootDescriptor) -> Result<ScanRootId> {
        if !descriptor.display_path.is_absolute()
            || descriptor.comparison_path.is_empty()
            || descriptor.normalization_version == 0
            || descriptor.policy_version == 0
        {
            return Err(CatalogError::new(
                CatalogErrorCode::InvalidInput,
                None,
                false,
            ));
        }
        let display_path = encoded_path(&descriptor.display_path);
        let normalization_version = i64::from(descriptor.normalization_version);
        let policy_version = i64::from(descriptor.policy_version);
        let (now, _) = system_time_parts(SystemTime::now())?;
        let id = self
            .connection
            .query_row(
                "INSERT INTO scan_roots (
                    volume_id, display_path, comparison_path, path_encoding,
                    normalization_version, policy_version,
                    first_seen_unix_seconds, last_seen_unix_seconds
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
                 ON CONFLICT(volume_id, comparison_path, normalization_version) DO UPDATE SET
                    display_path = excluded.display_path,
                    policy_version = excluded.policy_version,
                    last_seen_unix_seconds = excluded.last_seen_unix_seconds
                 RETURNING id",
                params![
                    descriptor.volume_id.0,
                    display_path,
                    descriptor.comparison_path,
                    PATH_ENCODING,
                    normalization_version,
                    policy_version,
                    now
                ],
                |row| row.get(0),
            )
            .map_err(|error| {
                CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, None)
            })?;
        Ok(ScanRootId(id))
    }

    /// Begins one ordered bounded scan writer for a root.
    pub fn begin_scan(
        &mut self,
        root_id: ScanRootId,
        batch_size: NonZeroUsize,
    ) -> Result<CatalogScanSession<'_>> {
        if batch_size.get() > MAX_CATALOG_BATCH_SIZE {
            return Err(CatalogError::new(
                CatalogErrorCode::InvalidInput,
                None,
                false,
            ));
        }
        let root_exists: i64 = self
            .connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM scan_roots WHERE id = ?1)",
                [root_id.0],
                |row| row.get(0),
            )
            .map_err(|error| {
                CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, None)
            })?;
        if root_exists != 1 {
            return Err(CatalogError::new(
                CatalogErrorCode::InvalidInput,
                None,
                false,
            ));
        }
        let (started_seconds, started_nanos) = system_time_parts(SystemTime::now())?;
        let run_result = self.connection.query_row(
            "INSERT INTO scan_runs (
                    scan_root_id, status, started_unix_seconds, started_subsec_nanos
                 ) VALUES (?1, 'running', ?2, ?3)
                 RETURNING id",
            params![root_id.0, started_seconds, started_nanos],
            |row| row.get(0),
        );
        let run_id = match run_result {
            Ok(run_id) => run_id,
            Err(error) => {
                let mapped =
                    CatalogError::from_sqlite(error, CatalogErrorCode::ConstraintViolation, None);
                let mapped = if mapped.sqlite_extended_code()
                    == Some(rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE)
                {
                    CatalogError::new(CatalogErrorCode::ActiveScanExists, None, true)
                } else {
                    mapped
                };
                return Err(mapped);
            }
        };

        Ok(CatalogScanSession {
            catalog: self,
            root_id,
            run_id: ScanRunId(run_id),
            batch_size: batch_size.get(),
            buffer: Vec::with_capacity(batch_size.get()),
            last_error: None,
            streamed_counters: ScanCounters::default(),
            streamed_issue_counts: ScanIssueCounts::default(),
        })
    }

    /// Marks every nonterminal scan run failed after the caller has established exclusive startup
    /// ownership of this catalog.
    ///
    /// This is an explicit recovery action for sessions lost to process interruption or an
    /// ungraceful owner drop. It never reconciles missing entries.
    pub fn recover_interrupted_scans(&mut self) -> Result<u64> {
        let (ended_seconds, ended_nanos) = system_time_parts(SystemTime::now())?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| {
                CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, None)
            })?;
        let recovered = transaction
            .execute(
                "UPDATE scan_runs SET
                    status = 'failed',
                    ended_unix_seconds = ?1,
                    ended_subsec_nanos = ?2,
                    terminal_error_code = 'interrupted_recovered',
                    absence_reconciled = 0
                 WHERE status IN ('queued', 'running', 'cancelling')",
                params![ended_seconds, ended_nanos],
            )
            .map_err(|error| {
                CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, None)
            })?;
        transaction.commit().map_err(|error| {
            CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, None)
        })?;
        u64::try_from(recovered)
            .map_err(|_| CatalogError::new(CatalogErrorCode::NumericOutOfRange, None, false))
    }

    /// Returns aggregate current or historical facts with optional root/category filters.
    pub fn summary(&self, query: SummaryQuery) -> Result<CatalogSummary> {
        let root_id = query.root_id.map(|id| id.0);
        let category = query.category.map(entry_kind_code);
        let values = self
            .connection
            .query_row(
                "SELECT
                    COUNT(*),
                    COALESCE(SUM(CASE WHEN kind = 'file' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN kind = 'directory' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN kind = 'reparse_point' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN kind = 'other' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN kind = 'file' THEN byte_len ELSE 0 END), 0)
                 FROM file_entries
                 WHERE (?1 IS NULL OR scan_root_id = ?1)
                   AND (?2 IS NULL OR kind = ?2)
                   AND (?3 = 'all' OR lifecycle_state = ?3)",
                params![root_id, category, query.presence.as_code()],
                |row| {
                    Ok([
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ])
                },
            )
            .map_err(|error| {
                CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, None)
            })?;
        CatalogSummary::from_database(values)
            .ok_or_else(|| CatalogError::new(CatalogErrorCode::InvalidPersistedData, None, false))
    }

    /// Returns the retained row count for one root, including missing rows.
    pub fn entry_record_count(&self, root_id: ScanRootId) -> Result<u64> {
        self.count_for_root("file_entries", root_id)
    }

    /// Returns the scan history count for one root.
    pub fn scan_run_count(&self, root_id: ScanRootId) -> Result<u64> {
        self.count_for_root("scan_runs", root_id)
    }

    /// Returns lifecycle evidence for an exact root-relative path key.
    pub fn entry_record(
        &self,
        root_id: ScanRootId,
        relative_path: &Path,
    ) -> Result<Option<EntryRecord>> {
        let key = encoded_path(relative_path);
        let raw = self
            .connection
            .query_row(
                "SELECT id, kind, byte_len, lifecycle_state,
                        first_seen_run_id, last_seen_run_id, missing_since_run_id
                 FROM file_entries
                 WHERE scan_root_id = ?1 AND comparison_relative_path = ?2",
                params![root_id.0, key],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<i64>>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, Option<i64>>(6)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| {
                CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, None)
            })?;

        raw.map(|(id, kind, byte_len, presence, first, last, missing)| {
            let kind = parse_entry_kind(&kind).ok_or_else(|| {
                CatalogError::new(CatalogErrorCode::InvalidPersistedData, None, false)
            })?;
            let byte_len = byte_len
                .map(|value| {
                    u64::try_from(value).map_err(|_| {
                        CatalogError::new(CatalogErrorCode::InvalidPersistedData, None, false)
                    })
                })
                .transpose()?;
            let presence = match presence.as_str() {
                "present" => EntryPresence::Present,
                "missing" => EntryPresence::Missing,
                _ => {
                    return Err(CatalogError::new(
                        CatalogErrorCode::InvalidPersistedData,
                        None,
                        false,
                    ));
                }
            };
            Ok(EntryRecord::new(
                id,
                kind,
                byte_len,
                presence,
                ScanRunId(first),
                ScanRunId(last),
                missing.map(ScanRunId),
            ))
        })
        .transpose()
    }

    fn count_for_root(&self, table: &str, root_id: ScanRootId) -> Result<u64> {
        let sql = match table {
            "file_entries" => "SELECT COUNT(*) FROM file_entries WHERE scan_root_id = ?1",
            "scan_runs" => "SELECT COUNT(*) FROM scan_runs WHERE scan_root_id = ?1",
            _ => {
                return Err(CatalogError::new(
                    CatalogErrorCode::InvalidInput,
                    None,
                    false,
                ));
            }
        };
        let count: i64 = self
            .connection
            .query_row(sql, [root_id.0], |row| row.get(0))
            .map_err(|error| {
                CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, None)
            })?;
        u64::try_from(count)
            .map_err(|_| CatalogError::new(CatalogErrorCode::InvalidPersistedData, None, false))
    }
}

/// Bounded synchronous scan sink that checkpoints observations in short transactions.
pub struct CatalogScanSession<'catalog> {
    catalog: &'catalog mut Catalog,
    root_id: ScanRootId,
    run_id: ScanRunId,
    batch_size: usize,
    buffer: Vec<ObservedEntry>,
    last_error: Option<CatalogError>,
    streamed_counters: ScanCounters,
    streamed_issue_counts: ScanIssueCounts,
}

impl CatalogScanSession<'_> {
    /// Returns the durable scan-run identifier allocated at begin.
    pub const fn run_id(&self) -> ScanRunId {
        self.run_id
    }

    /// Returns the number of observations currently held outside a transaction.
    pub fn buffered_entries(&self) -> usize {
        self.buffer.len()
    }

    /// Finalizes a qualified scanner report and performs conservative absence reconciliation.
    pub fn finish(mut self, report: &ScanReport) -> Result<ScanCommit> {
        if let Some(error) = self.last_error.take() {
            self.buffer.clear();
            self.terminalize_failed("catalog_sink_failed", self.streamed_counters)?;
            return Err(error);
        }
        if let Err(error) = self.flush() {
            self.buffer.clear();
            self.terminalize_failed("catalog_flush_failed", self.streamed_counters)?;
            return Err(error);
        }

        if report.counters() != self.streamed_counters
            || report.issue_counts() != self.streamed_issue_counts
        {
            self.terminalize_failed(
                CatalogErrorCode::ScanReportMismatch.as_code(),
                self.streamed_counters,
            )?;
            return Err(CatalogError::new(
                CatalogErrorCode::ScanReportMismatch,
                None,
                false,
            ));
        }

        let durable_observations: i64 = self
            .catalog
            .connection
            .query_row(
                "SELECT observed_entries FROM scan_runs
                 WHERE id = ?1 AND scan_root_id = ?2 AND status = 'running'",
                params![self.run_id.0, self.root_id.0],
                |row| row.get(0),
            )
            .map_err(|error| {
                CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, None)
            })?;
        let expected_observations = sqlite_u64(self.streamed_counters.total_entries())?;
        if durable_observations != expected_observations {
            self.terminalize_failed(
                CatalogErrorCode::ScanReportMismatch.as_code(),
                self.streamed_counters,
            )?;
            return Err(CatalogError::new(
                CatalogErrorCode::ScanReportMismatch,
                None,
                false,
            ));
        }

        let status = report.status();
        let absence_reconciled = status == ScanRunStatus::Completed;
        let counters = report.counters();
        let files = sqlite_u64(counters.files())?;
        let directories = sqlite_u64(counters.directories())?;
        let reparse_points = sqlite_u64(counters.reparse_points_skipped())?;
        let other_entries = sqlite_u64(counters.other_entries())?;
        let total_bytes = sqlite_u64(counters.bytes())?;
        let partial_issues = sqlite_u64(counters.partial_issues())?;
        let (ended_seconds, ended_nanos) = system_time_parts(SystemTime::now())?;

        let transaction = self
            .catalog
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| {
                CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, None)
            })?;
        let newest_run: Option<i64> = transaction
            .query_row(
                "SELECT MAX(id) FROM scan_runs WHERE scan_root_id = ?1",
                [self.root_id.0],
                |row| row.get(0),
            )
            .map_err(|error| {
                CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, None)
            })?;
        if newest_run != Some(self.run_id.0) {
            return Err(CatalogError::new(
                CatalogErrorCode::InvalidPersistedData,
                None,
                false,
            ));
        }

        let missing_marked = if absence_reconciled {
            transaction
                .execute(
                    "UPDATE file_entries
                     SET lifecycle_state = 'missing', missing_since_run_id = ?1
                     WHERE scan_root_id = ?2
                       AND lifecycle_state = 'present'
                       AND last_seen_run_id <> ?1",
                    params![self.run_id.0, self.root_id.0],
                )
                .map_err(|error| {
                    CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, None)
                })?
        } else {
            0
        };

        let updated = transaction
            .execute(
                "UPDATE scan_runs SET
                    status = ?1,
                    ended_unix_seconds = ?2,
                    ended_subsec_nanos = ?3,
                    files = ?4,
                    directories = ?5,
                    reparse_points = ?6,
                    other_entries = ?7,
                    total_bytes = ?8,
                    partial_issues = ?9,
                    absence_reconciled = ?10
                 WHERE id = ?11 AND scan_root_id = ?12 AND status = 'running'",
                params![
                    status.as_code(),
                    ended_seconds,
                    ended_nanos,
                    files,
                    directories,
                    reparse_points,
                    other_entries,
                    total_bytes,
                    partial_issues,
                    i64::from(absence_reconciled),
                    self.run_id.0,
                    self.root_id.0
                ],
            )
            .map_err(|error| {
                CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, None)
            })?;
        if updated != 1 {
            return Err(CatalogError::new(
                CatalogErrorCode::InvalidPersistedData,
                None,
                false,
            ));
        }
        transaction.commit().map_err(|error| {
            CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, None)
        })?;

        let missing_marked = u64::try_from(missing_marked)
            .map_err(|_| CatalogError::new(CatalogErrorCode::NumericOutOfRange, None, false))?;
        Ok(ScanCommit::new(
            self.run_id,
            status,
            absence_reconciled,
            missing_marked,
        ))
    }

    /// Marks a fatal scanner run failed without inferring absence.
    pub fn fail(mut self, failure: ScanFailure) -> Result<()> {
        self.buffer.clear();
        let original_catalog_error = self.last_error.take();
        let counters = failure.partial_counters();
        self.terminalize_failed(failure.code().as_code(), counters)?;
        if let Some(error) = original_catalog_error {
            return Err(error);
        }
        Ok(())
    }

    fn terminalize_failed(
        &mut self,
        terminal_error_code: &str,
        counters: ScanCounters,
    ) -> Result<()> {
        let (ended_seconds, ended_nanos) = system_time_parts(SystemTime::now())?;
        let converted_counters = sqlite_scan_counters(counters);
        let updated = match converted_counters {
            Ok(
                [
                    files,
                    directories,
                    reparse_points,
                    other_entries,
                    total_bytes,
                    partial_issues,
                ],
            ) => self.catalog.connection.execute(
                "UPDATE scan_runs SET
                        status = 'failed',
                        ended_unix_seconds = ?1,
                        ended_subsec_nanos = ?2,
                        files = ?3,
                        directories = ?4,
                        reparse_points = ?5,
                        other_entries = ?6,
                        total_bytes = ?7,
                        partial_issues = ?8,
                        terminal_error_code = ?9,
                        absence_reconciled = 0
                     WHERE id = ?10 AND scan_root_id = ?11 AND status = 'running'",
                params![
                    ended_seconds,
                    ended_nanos,
                    files,
                    directories,
                    reparse_points,
                    other_entries,
                    total_bytes,
                    partial_issues,
                    terminal_error_code,
                    self.run_id.0,
                    self.root_id.0
                ],
            ),
            Err(_) => self.catalog.connection.execute(
                "UPDATE scan_runs SET
                    status = 'failed',
                    ended_unix_seconds = ?1,
                    ended_subsec_nanos = ?2,
                    terminal_error_code = ?3,
                    absence_reconciled = 0
                 WHERE id = ?4 AND scan_root_id = ?5 AND status = 'running'",
                params![
                    ended_seconds,
                    ended_nanos,
                    terminal_error_code,
                    self.run_id.0,
                    self.root_id.0
                ],
            ),
        }
        .map_err(|error| CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, None))?;
        if updated != 1 {
            return Err(CatalogError::new(
                CatalogErrorCode::InvalidPersistedData,
                None,
                false,
            ));
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let batch_count = sqlite_usize(self.buffer.len())?;
        let transaction = self
            .catalog
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| {
                CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, None)
            })?;
        {
            let mut statement = transaction
                .prepare(
                    "INSERT INTO file_entries (
                        scan_root_id, display_relative_path, comparison_relative_path,
                        path_encoding, kind, byte_len,
                        modified_unix_seconds, modified_subsec_nanos, read_only,
                        lifecycle_state, first_seen_run_id, last_seen_run_id, missing_since_run_id
                     ) VALUES (?1, ?2, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'present', ?9, ?9, NULL)
                     ON CONFLICT(scan_root_id, comparison_relative_path) DO UPDATE SET
                        display_relative_path = excluded.display_relative_path,
                        path_encoding = excluded.path_encoding,
                        kind = excluded.kind,
                        byte_len = excluded.byte_len,
                        modified_unix_seconds = excluded.modified_unix_seconds,
                        modified_subsec_nanos = excluded.modified_subsec_nanos,
                        read_only = excluded.read_only,
                        lifecycle_state = 'present',
                        last_seen_run_id = excluded.last_seen_run_id,
                        missing_since_run_id = NULL",
                )
                .map_err(|error| {
                    CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, None)
                })?;

            for entry in &self.buffer {
                let path = encoded_path(entry.relative_path());
                let byte_len = entry.byte_len().map(sqlite_u64).transpose()?;
                let modified = entry.modified().map(system_time_parts).transpose()?;
                let (modified_seconds, modified_nanos) = match modified {
                    Some((seconds, nanos)) => (Some(seconds), Some(nanos)),
                    None => (None, None),
                };
                statement
                    .execute(params![
                        self.root_id.0,
                        path,
                        PATH_ENCODING,
                        entry_kind_code(entry.kind()),
                        byte_len,
                        modified_seconds,
                        modified_nanos,
                        i64::from(entry.read_only()),
                        self.run_id.0
                    ])
                    .map_err(|error| {
                        CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, None)
                    })?;
            }
        }
        let updated = transaction
            .execute(
                "UPDATE scan_runs
                 SET observed_entries = observed_entries + ?1
                 WHERE id = ?2 AND scan_root_id = ?3 AND status = 'running'",
                params![batch_count, self.run_id.0, self.root_id.0],
            )
            .map_err(|error| {
                CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, None)
            })?;
        if updated != 1 {
            return Err(CatalogError::new(
                CatalogErrorCode::InvalidPersistedData,
                None,
                false,
            ));
        }
        transaction.commit().map_err(|error| {
            CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, None)
        })?;
        self.buffer.clear();
        Ok(())
    }

    fn reject_with(&mut self, error: CatalogError) -> ScanSinkError {
        let retryable = error.retryable();
        let code = if retryable {
            ScanSinkErrorCode::ConsumerUnavailable
        } else {
            ScanSinkErrorCode::ConsumerInvariant
        };
        self.last_error = Some(error);
        ScanSinkError::new(code, retryable)
    }
}

impl ScanSink for CatalogScanSession<'_> {
    fn observe(&mut self, entry: ObservedEntry) -> std::result::Result<(), ScanSinkError> {
        if self.last_error.is_some() {
            return Err(ScanSinkError::new(
                ScanSinkErrorCode::ConsumerInvariant,
                false,
            ));
        }
        self.streamed_counters.record_entry(&entry);
        self.buffer.push(entry);
        if self.buffer.len() >= self.batch_size {
            if let Err(error) = self.flush() {
                return Err(self.reject_with(error));
            }
        }
        Ok(())
    }

    fn issue(&mut self, issue: ScanIssue) -> std::result::Result<(), ScanSinkError> {
        if self.last_error.is_some() {
            return Err(ScanSinkError::new(
                ScanSinkErrorCode::ConsumerInvariant,
                false,
            ));
        }
        self.streamed_counters.record_issue();
        self.streamed_issue_counts.record(issue.code());
        Ok(())
    }

    fn progress(&mut self, _progress: ScanProgress) -> std::result::Result<(), ScanSinkError> {
        if self.last_error.is_some() {
            return Err(ScanSinkError::new(
                ScanSinkErrorCode::ConsumerInvariant,
                false,
            ));
        }
        Ok(())
    }
}

fn sqlite_u64(value: u64) -> Result<i64> {
    i64::try_from(value)
        .map_err(|_| CatalogError::new(CatalogErrorCode::NumericOutOfRange, None, false))
}

fn sqlite_usize(value: usize) -> Result<i64> {
    i64::try_from(value)
        .map_err(|_| CatalogError::new(CatalogErrorCode::NumericOutOfRange, None, false))
}

fn sqlite_scan_counters(counters: ScanCounters) -> Result<[i64; 6]> {
    Ok([
        sqlite_u64(counters.files())?,
        sqlite_u64(counters.directories())?,
        sqlite_u64(counters.reparse_points_skipped())?,
        sqlite_u64(counters.other_entries())?,
        sqlite_u64(counters.bytes())?,
        sqlite_u64(counters.partial_issues())?,
    ])
}

fn system_time_parts(time: SystemTime) -> Result<(i64, i64)> {
    match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => Ok((
            sqlite_u64(duration.as_secs())?,
            i64::from(duration.subsec_nanos()),
        )),
        Err(error) => {
            let duration = error.duration();
            let seconds = sqlite_u64(duration.as_secs())?;
            let nanos = duration.subsec_nanos();
            if nanos == 0 {
                Ok((-seconds, 0))
            } else {
                let seconds = seconds.checked_add(1).ok_or_else(|| {
                    CatalogError::new(CatalogErrorCode::NumericOutOfRange, None, false)
                })?;
                Ok((-seconds, i64::from(1_000_000_000_u32 - nanos)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::system_time_parts;
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn pre_epoch_time_is_encoded_without_wrapping() {
        let time = UNIX_EPOCH - Duration::new(1, 250_000_000);
        assert_eq!(
            system_time_parts(time).expect("valid pre-epoch time"),
            (-2, 750_000_000)
        );
    }
}

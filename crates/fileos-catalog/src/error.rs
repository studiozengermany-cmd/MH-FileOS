use std::error::Error;
use std::fmt;

use rusqlite::{Error as SqliteError, ErrorCode as SqliteErrorCode};

/// Stable machine-readable classification for catalog failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogErrorCode {
    /// SQLite could not open the requested catalog database.
    OpenFailed,
    /// Connection safety settings were not accepted or could not be verified.
    ConfigurationFailed,
    /// The file is not an MH FileOS catalog.
    NotCatalog,
    /// The catalog schema is newer than this application understands.
    UnsupportedSchemaVersion,
    /// A forward migration failed atomically.
    MigrationFailed,
    /// Another writer currently owns the requested database/root lease.
    Busy,
    /// Persisted or supplied data violated a catalog constraint.
    ConstraintViolation,
    /// SQLite reported corrupt or non-database bytes.
    Corrupt,
    /// A query could not be completed.
    QueryFailed,
    /// A numeric value cannot be represented safely by SQLite.
    NumericOutOfRange,
    /// A public catalog input violated a documented invariant.
    InvalidInput,
    /// A scan is already active for the selected root.
    ActiveScanExists,
    /// A terminal report did not match observations streamed by its catalog session.
    ScanReportMismatch,
    /// A persisted value did not match the versioned catalog contract.
    InvalidPersistedData,
}

impl CatalogErrorCode {
    /// Returns the stable external code.
    pub const fn as_code(self) -> &'static str {
        match self {
            Self::OpenFailed => "catalog_open_failed",
            Self::ConfigurationFailed => "catalog_configuration_failed",
            Self::NotCatalog => "not_fileos_catalog",
            Self::UnsupportedSchemaVersion => "unsupported_catalog_schema",
            Self::MigrationFailed => "catalog_migration_failed",
            Self::Busy => "catalog_busy",
            Self::ConstraintViolation => "catalog_constraint_violation",
            Self::Corrupt => "catalog_corrupt",
            Self::QueryFailed => "catalog_query_failed",
            Self::NumericOutOfRange => "catalog_numeric_out_of_range",
            Self::InvalidInput => "catalog_invalid_input",
            Self::ActiveScanExists => "catalog_active_scan_exists",
            Self::ScanReportMismatch => "catalog_scan_report_mismatch",
            Self::InvalidPersistedData => "catalog_invalid_persisted_data",
        }
    }

    /// Returns a path-free and SQL-free explanation suitable for a local client.
    pub const fn user_safe_message(self) -> &'static str {
        match self {
            Self::OpenFailed => "The local catalog database could not be opened.",
            Self::ConfigurationFailed => {
                "The local catalog database could not enable required safety settings."
            }
            Self::NotCatalog => "The selected database is not an MH FileOS catalog.",
            Self::UnsupportedSchemaVersion => {
                "The catalog was created by a newer unsupported application version."
            }
            Self::MigrationFailed => "The catalog schema could not be upgraded safely.",
            Self::Busy => "The local catalog is busy; the operation can be retried.",
            Self::ConstraintViolation => "Catalog data failed a required consistency check.",
            Self::Corrupt => "The local catalog is corrupt or is not a valid database.",
            Self::QueryFailed => "The local catalog query could not be completed.",
            Self::NumericOutOfRange => "A catalog value exceeded the supported numeric range.",
            Self::InvalidInput => "The catalog request contained invalid data.",
            Self::ActiveScanExists => "A scan is already active for the selected root.",
            Self::ScanReportMismatch => {
                "The scan report did not match the observations accepted for this run."
            }
            Self::InvalidPersistedData => {
                "The catalog contains data that this application cannot interpret safely."
            }
        }
    }
}

/// Typed database failure with redacted user-safe presentation.
#[derive(Debug)]
pub struct CatalogError {
    code: CatalogErrorCode,
    migration_version: Option<u32>,
    retryable: bool,
    sqlite_extended_code: Option<i32>,
    source: Option<SqliteError>,
}

impl CatalogError {
    pub(crate) const fn new(
        code: CatalogErrorCode,
        migration_version: Option<u32>,
        retryable: bool,
    ) -> Self {
        Self {
            code,
            migration_version,
            retryable,
            sqlite_extended_code: None,
            source: None,
        }
    }

    pub(crate) fn from_sqlite(
        error: SqliteError,
        fallback: CatalogErrorCode,
        migration_version: Option<u32>,
    ) -> Self {
        let sqlite_code = error.sqlite_error_code();
        let code = match sqlite_code {
            Some(SqliteErrorCode::DatabaseBusy | SqliteErrorCode::DatabaseLocked) => {
                CatalogErrorCode::Busy
            }
            Some(SqliteErrorCode::DatabaseCorrupt | SqliteErrorCode::NotADatabase) => {
                CatalogErrorCode::Corrupt
            }
            Some(SqliteErrorCode::ConstraintViolation) => CatalogErrorCode::ConstraintViolation,
            _ => fallback,
        };
        let retryable = code == CatalogErrorCode::Busy;
        let sqlite_extended_code = match &error {
            SqliteError::SqliteFailure(inner, _) => Some(inner.extended_code),
            _ => None,
        };
        Self {
            code,
            migration_version,
            retryable,
            sqlite_extended_code,
            source: Some(error),
        }
    }

    /// Returns the stable machine-readable error classification.
    pub const fn code(&self) -> CatalogErrorCode {
        self.code
    }

    /// Returns the migration version involved, when applicable.
    pub const fn migration_version(&self) -> Option<u32> {
        self.migration_version
    }

    /// Reports whether retry after a bounded delay or external state change may succeed.
    pub const fn retryable(&self) -> bool {
        self.retryable
    }

    /// Returns SQLite's numeric extended code without exposing its message or SQL text.
    pub const fn sqlite_extended_code(&self) -> Option<i32> {
        self.sqlite_extended_code
    }
}

impl fmt::Display for CatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code.user_safe_message())
    }
}

impl Error for CatalogError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_ref()
            .map(|error| error as &(dyn Error + 'static))
    }
}

pub(crate) type Result<T> = std::result::Result<T, CatalogError>;

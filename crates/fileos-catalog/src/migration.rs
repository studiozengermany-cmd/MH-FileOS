use rusqlite::{Connection, TransactionBehavior};

use crate::error::{CatalogError, CatalogErrorCode, Result};

pub(crate) const CATALOG_SCHEMA_VERSION: u32 = 1;
pub(crate) const CATALOG_APPLICATION_ID: i64 = 1_296_582_223;
const MIGRATION_V1: &str = include_str!("../migrations/0001_catalog_v1.sql");

#[derive(Clone, Copy)]
struct ColumnSpec {
    name: &'static str,
    data_type: &'static str,
    not_null: i64,
    primary_key: i64,
}

const VOLUME_COLUMNS: [ColumnSpec; 6] = [
    column("id", "INTEGER", 0, 1),
    column("identity_kind", "TEXT", 1, 0),
    column("identity_value", "BLOB", 1, 0),
    column("filesystem", "TEXT", 0, 0),
    column("first_seen_unix_seconds", "INTEGER", 1, 0),
    column("last_seen_unix_seconds", "INTEGER", 1, 0),
];

const SCAN_ROOT_COLUMNS: [ColumnSpec; 9] = [
    column("id", "INTEGER", 0, 1),
    column("volume_id", "INTEGER", 1, 0),
    column("display_path", "BLOB", 1, 0),
    column("comparison_path", "BLOB", 1, 0),
    column("path_encoding", "TEXT", 1, 0),
    column("normalization_version", "INTEGER", 1, 0),
    column("policy_version", "INTEGER", 1, 0),
    column("first_seen_unix_seconds", "INTEGER", 1, 0),
    column("last_seen_unix_seconds", "INTEGER", 1, 0),
];

const SCAN_RUN_COLUMNS: [ColumnSpec; 16] = [
    column("id", "INTEGER", 0, 1),
    column("scan_root_id", "INTEGER", 1, 0),
    column("status", "TEXT", 1, 0),
    column("started_unix_seconds", "INTEGER", 1, 0),
    column("started_subsec_nanos", "INTEGER", 1, 0),
    column("ended_unix_seconds", "INTEGER", 0, 0),
    column("ended_subsec_nanos", "INTEGER", 0, 0),
    column("observed_entries", "INTEGER", 1, 0),
    column("files", "INTEGER", 1, 0),
    column("directories", "INTEGER", 1, 0),
    column("reparse_points", "INTEGER", 1, 0),
    column("other_entries", "INTEGER", 1, 0),
    column("total_bytes", "INTEGER", 1, 0),
    column("partial_issues", "INTEGER", 1, 0),
    column("terminal_error_code", "TEXT", 0, 0),
    column("absence_reconciled", "INTEGER", 1, 0),
];

const FILE_ENTRY_COLUMNS: [ColumnSpec; 14] = [
    column("id", "INTEGER", 0, 1),
    column("scan_root_id", "INTEGER", 1, 0),
    column("display_relative_path", "BLOB", 1, 0),
    column("comparison_relative_path", "BLOB", 1, 0),
    column("path_encoding", "TEXT", 1, 0),
    column("kind", "TEXT", 1, 0),
    column("byte_len", "INTEGER", 0, 0),
    column("modified_unix_seconds", "INTEGER", 0, 0),
    column("modified_subsec_nanos", "INTEGER", 0, 0),
    column("read_only", "INTEGER", 1, 0),
    column("lifecycle_state", "TEXT", 1, 0),
    column("first_seen_run_id", "INTEGER", 1, 0),
    column("last_seen_run_id", "INTEGER", 1, 0),
    column("missing_since_run_id", "INTEGER", 0, 0),
];

const fn column(
    name: &'static str,
    data_type: &'static str,
    not_null: i64,
    primary_key: i64,
) -> ColumnSpec {
    ColumnSpec {
        name,
        data_type,
        not_null,
        primary_key,
    }
}

pub(crate) fn migrate(connection: &mut Connection) -> Result<()> {
    let user_version = pragma_i64(connection, "user_version")?;
    let application_id = pragma_i64(connection, "application_id")?;

    if user_version > i64::from(CATALOG_SCHEMA_VERSION) {
        return Err(CatalogError::new(
            CatalogErrorCode::UnsupportedSchemaVersion,
            u32::try_from(user_version).ok(),
            false,
        ));
    }

    match user_version {
        0 => migrate_fresh(connection, application_id),
        1 if application_id == CATALOG_APPLICATION_ID => verify_v1(connection),
        1 => Err(CatalogError::new(
            CatalogErrorCode::NotCatalog,
            Some(CATALOG_SCHEMA_VERSION),
            false,
        )),
        _ => Err(CatalogError::new(
            CatalogErrorCode::UnsupportedSchemaVersion,
            u32::try_from(user_version).ok(),
            false,
        )),
    }
}

fn migrate_fresh(connection: &mut Connection, application_id: i64) -> Result<()> {
    if application_id != 0 && application_id != CATALOG_APPLICATION_ID {
        return Err(CatalogError::new(CatalogErrorCode::NotCatalog, None, false));
    }

    let user_table_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| {
            CatalogError::from_sqlite(error, CatalogErrorCode::MigrationFailed, Some(1))
        })?;
    if user_table_count != 0 {
        return Err(CatalogError::new(CatalogErrorCode::NotCatalog, None, false));
    }

    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| {
            CatalogError::from_sqlite(error, CatalogErrorCode::MigrationFailed, Some(1))
        })?;
    transaction.execute_batch(MIGRATION_V1).map_err(|error| {
        CatalogError::from_sqlite(error, CatalogErrorCode::MigrationFailed, Some(1))
    })?;
    transaction
        .execute_batch(
            "PRAGMA application_id = 1296582223;
             PRAGMA user_version = 1;",
        )
        .map_err(|error| {
            CatalogError::from_sqlite(error, CatalogErrorCode::MigrationFailed, Some(1))
        })?;
    transaction.commit().map_err(|error| {
        CatalogError::from_sqlite(error, CatalogErrorCode::MigrationFailed, Some(1))
    })?;
    verify_v1(connection)
}

fn verify_v1(connection: &Connection) -> Result<()> {
    let table_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_schema
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| {
            CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, Some(1))
        })?;
    if table_count != 4 {
        return Err(incompatible_v1());
    }

    verify_schema_definitions(connection)?;
    verify_columns(connection, "volumes", &VOLUME_COLUMNS)?;
    verify_columns(connection, "scan_roots", &SCAN_ROOT_COLUMNS)?;
    verify_columns(connection, "scan_runs", &SCAN_RUN_COLUMNS)?;
    verify_columns(connection, "file_entries", &FILE_ENTRY_COLUMNS)?;

    let strict_tables: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_list
             WHERE type = 'table'
               AND name IN ('volumes', 'scan_roots', 'scan_runs', 'file_entries')
               AND strict = 1",
            [],
            |row| row.get(0),
        )
        .map_err(|error| {
            CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, Some(1))
        })?;
    if strict_tables != 4 {
        return Err(incompatible_v1());
    }

    let named_indexes: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_schema
             WHERE type = 'index' AND sql IS NOT NULL
               AND name IN (
                   'one_active_scan_per_root',
                   'scan_runs_by_root_started',
                   'file_entries_summary',
                   'file_entries_missing_reconciliation'
               )",
            [],
            |row| row.get(0),
        )
        .map_err(|error| {
            CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, Some(1))
        })?;
    if named_indexes != 4 {
        return Err(incompatible_v1());
    }

    verify_foreign_key_layout(connection)?;
    verify_foreign_key_consistency(connection)?;
    Ok(())
}

fn verify_schema_definitions(connection: &Connection) -> Result<()> {
    let reference = Connection::open_in_memory().map_err(|error| {
        CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, Some(1))
    })?;
    reference.execute_batch(MIGRATION_V1).map_err(|error| {
        CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, Some(1))
    })?;

    if schema_definitions(connection)? != schema_definitions(&reference)? {
        return Err(incompatible_v1());
    }
    Ok(())
}

fn schema_definitions(connection: &Connection) -> Result<Vec<(String, String, String)>> {
    let mut statement = connection
        .prepare(
            "SELECT type, name, sql FROM sqlite_schema
             WHERE name NOT LIKE 'sqlite_%' AND sql IS NOT NULL
             ORDER BY type, name",
        )
        .map_err(|error| {
            CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, Some(1))
        })?;
    let rows = statement
        .query_map([], |row| {
            let sql: String = row.get(2)?;
            Ok((
                row.get(0)?,
                row.get(1)?,
                sql.split_whitespace().collect::<Vec<_>>().join(" "),
            ))
        })
        .map_err(|error| {
            CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, Some(1))
        })?;
    let mut definitions = Vec::new();
    for row in rows {
        definitions.push(row.map_err(|error| {
            CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, Some(1))
        })?);
    }
    Ok(definitions)
}

fn verify_columns(connection: &Connection, table: &str, expected: &[ColumnSpec]) -> Result<()> {
    let sql = match table {
        "volumes" => "PRAGMA table_info(volumes)",
        "scan_roots" => "PRAGMA table_info(scan_roots)",
        "scan_runs" => "PRAGMA table_info(scan_runs)",
        "file_entries" => "PRAGMA table_info(file_entries)",
        _ => return Err(incompatible_v1()),
    };
    let mut statement = connection.prepare(sql).map_err(|error| {
        CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, Some(1))
    })?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(5)?,
            ))
        })
        .map_err(|error| {
            CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, Some(1))
        })?;
    let mut observed = Vec::new();
    for row in rows {
        observed.push(row.map_err(|error| {
            CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, Some(1))
        })?);
    }
    if observed.len() != expected.len()
        || observed.iter().zip(expected).any(
            |((name, data_type, not_null, primary_key), expected)| {
                name != expected.name
                    || data_type != expected.data_type
                    || *not_null != expected.not_null
                    || *primary_key != expected.primary_key
            },
        )
    {
        return Err(incompatible_v1());
    }
    Ok(())
}

fn verify_foreign_key_layout(connection: &Connection) -> Result<()> {
    let expected = [
        ("scan_roots", 1_usize),
        ("scan_runs", 1_usize),
        ("file_entries", 7_usize),
    ];
    for (table, expected_rows) in expected {
        let sql = match table {
            "scan_roots" => "PRAGMA foreign_key_list(scan_roots)",
            "scan_runs" => "PRAGMA foreign_key_list(scan_runs)",
            "file_entries" => "PRAGMA foreign_key_list(file_entries)",
            _ => return Err(incompatible_v1()),
        };
        let mut statement = connection.prepare(sql).map_err(|error| {
            CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, Some(1))
        })?;
        let mut rows = statement.query([]).map_err(|error| {
            CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, Some(1))
        })?;
        let mut count = 0_usize;
        while rows
            .next()
            .map_err(|error| {
                CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, Some(1))
            })?
            .is_some()
        {
            count = count.checked_add(1).ok_or_else(|| {
                CatalogError::new(CatalogErrorCode::NumericOutOfRange, Some(1), false)
            })?;
        }
        if count != expected_rows {
            return Err(incompatible_v1());
        }
    }
    Ok(())
}

fn verify_foreign_key_consistency(connection: &Connection) -> Result<()> {
    let mut statement = connection
        .prepare("PRAGMA foreign_key_check")
        .map_err(|error| {
            CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, Some(1))
        })?;
    let mut rows = statement.query([]).map_err(|error| {
        CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, Some(1))
    })?;
    if rows
        .next()
        .map_err(|error| CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, Some(1)))?
        .is_some()
    {
        return Err(CatalogError::new(
            CatalogErrorCode::InvalidPersistedData,
            Some(CATALOG_SCHEMA_VERSION),
            false,
        ));
    }
    Ok(())
}

const fn incompatible_v1() -> CatalogError {
    CatalogError::new(
        CatalogErrorCode::NotCatalog,
        Some(CATALOG_SCHEMA_VERSION),
        false,
    )
}

pub(crate) fn pragma_i64(connection: &Connection, name: &str) -> Result<i64> {
    let sql = match name {
        "user_version" => "PRAGMA user_version",
        "application_id" => "PRAGMA application_id",
        "foreign_keys" => "PRAGMA foreign_keys",
        _ => {
            return Err(CatalogError::new(
                CatalogErrorCode::InvalidInput,
                None,
                false,
            ));
        }
    };
    connection
        .query_row(sql, [], |row| row.get(0))
        .map_err(|error| CatalogError::from_sqlite(error, CatalogErrorCode::QueryFailed, None))
}

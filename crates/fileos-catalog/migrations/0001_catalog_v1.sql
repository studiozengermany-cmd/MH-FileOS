CREATE TABLE volumes (
    id INTEGER PRIMARY KEY,
    identity_kind TEXT NOT NULL,
    identity_value BLOB NOT NULL,
    filesystem TEXT,
    first_seen_unix_seconds INTEGER NOT NULL,
    last_seen_unix_seconds INTEGER NOT NULL,
    UNIQUE (identity_kind, identity_value),
    CHECK (length(identity_kind) > 0),
    CHECK (length(identity_value) > 0)
) STRICT;

CREATE TABLE scan_roots (
    id INTEGER PRIMARY KEY,
    volume_id INTEGER NOT NULL REFERENCES volumes(id) ON DELETE RESTRICT,
    display_path BLOB NOT NULL,
    comparison_path BLOB NOT NULL,
    path_encoding TEXT NOT NULL,
    normalization_version INTEGER NOT NULL,
    policy_version INTEGER NOT NULL,
    first_seen_unix_seconds INTEGER NOT NULL,
    last_seen_unix_seconds INTEGER NOT NULL,
    UNIQUE (volume_id, comparison_path, normalization_version),
    CHECK (length(display_path) > 0),
    CHECK (length(comparison_path) > 0),
    CHECK (path_encoding = 'rust_os_str_encoded_bytes_v1'),
    CHECK (normalization_version > 0),
    CHECK (policy_version > 0)
) STRICT;

CREATE TABLE scan_runs (
    id INTEGER PRIMARY KEY,
    scan_root_id INTEGER NOT NULL REFERENCES scan_roots(id) ON DELETE RESTRICT,
    status TEXT NOT NULL,
    started_unix_seconds INTEGER NOT NULL,
    started_subsec_nanos INTEGER NOT NULL,
    ended_unix_seconds INTEGER,
    ended_subsec_nanos INTEGER,
    observed_entries INTEGER NOT NULL DEFAULT 0,
    files INTEGER NOT NULL DEFAULT 0,
    directories INTEGER NOT NULL DEFAULT 0,
    reparse_points INTEGER NOT NULL DEFAULT 0,
    other_entries INTEGER NOT NULL DEFAULT 0,
    total_bytes INTEGER NOT NULL DEFAULT 0,
    partial_issues INTEGER NOT NULL DEFAULT 0,
    terminal_error_code TEXT,
    absence_reconciled INTEGER NOT NULL DEFAULT 0,
    UNIQUE (id, scan_root_id),
    CHECK (status IN (
        'queued', 'running', 'cancelling', 'completed',
        'completed_with_issues', 'cancelled', 'failed'
    )),
    CHECK (started_subsec_nanos BETWEEN 0 AND 999999999),
    CHECK (
        (ended_unix_seconds IS NULL AND ended_subsec_nanos IS NULL) OR
        (ended_unix_seconds IS NOT NULL AND ended_subsec_nanos BETWEEN 0 AND 999999999)
    ),
    CHECK (observed_entries >= 0),
    CHECK (files >= 0),
    CHECK (directories >= 0),
    CHECK (reparse_points >= 0),
    CHECK (other_entries >= 0),
    CHECK (total_bytes >= 0),
    CHECK (partial_issues >= 0),
    CHECK (absence_reconciled IN (0, 1)),
    CHECK (absence_reconciled = 0 OR status = 'completed'),
    CHECK (
        (status IN ('queued', 'running', 'cancelling') AND ended_unix_seconds IS NULL) OR
        (status IN ('completed', 'completed_with_issues', 'cancelled', 'failed') AND ended_unix_seconds IS NOT NULL)
    )
) STRICT;

CREATE UNIQUE INDEX one_active_scan_per_root
    ON scan_runs(scan_root_id)
    WHERE status IN ('queued', 'running', 'cancelling');

CREATE INDEX scan_runs_by_root_started
    ON scan_runs(scan_root_id, started_unix_seconds, id);

CREATE TABLE file_entries (
    id INTEGER PRIMARY KEY,
    scan_root_id INTEGER NOT NULL REFERENCES scan_roots(id) ON DELETE RESTRICT,
    display_relative_path BLOB NOT NULL,
    comparison_relative_path BLOB NOT NULL,
    path_encoding TEXT NOT NULL,
    kind TEXT NOT NULL,
    byte_len INTEGER,
    modified_unix_seconds INTEGER,
    modified_subsec_nanos INTEGER,
    read_only INTEGER NOT NULL,
    lifecycle_state TEXT NOT NULL,
    first_seen_run_id INTEGER NOT NULL,
    last_seen_run_id INTEGER NOT NULL,
    missing_since_run_id INTEGER,
    UNIQUE (scan_root_id, comparison_relative_path),
    FOREIGN KEY (first_seen_run_id, scan_root_id)
        REFERENCES scan_runs(id, scan_root_id) ON DELETE RESTRICT,
    FOREIGN KEY (last_seen_run_id, scan_root_id)
        REFERENCES scan_runs(id, scan_root_id) ON DELETE RESTRICT,
    FOREIGN KEY (missing_since_run_id, scan_root_id)
        REFERENCES scan_runs(id, scan_root_id) ON DELETE RESTRICT,
    CHECK (length(display_relative_path) > 0),
    CHECK (length(comparison_relative_path) > 0),
    CHECK (path_encoding = 'rust_os_str_encoded_bytes_v1'),
    CHECK (kind IN ('file', 'directory', 'reparse_point', 'other')),
    CHECK (
        (kind = 'file' AND byte_len IS NOT NULL AND byte_len >= 0) OR
        (kind <> 'file' AND byte_len IS NULL)
    ),
    CHECK (
        (modified_unix_seconds IS NULL AND modified_subsec_nanos IS NULL) OR
        (modified_unix_seconds IS NOT NULL AND modified_subsec_nanos BETWEEN 0 AND 999999999)
    ),
    CHECK (read_only IN (0, 1)),
    CHECK (lifecycle_state IN ('present', 'missing')),
    CHECK (
        (lifecycle_state = 'present' AND missing_since_run_id IS NULL) OR
        (lifecycle_state = 'missing' AND missing_since_run_id IS NOT NULL)
    )
) STRICT;

CREATE INDEX file_entries_summary
    ON file_entries(scan_root_id, lifecycle_state, kind, byte_len);

CREATE INDEX file_entries_missing_reconciliation
    ON file_entries(scan_root_id, lifecycle_state, last_seen_run_id);

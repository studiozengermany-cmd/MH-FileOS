# ADR-002 — SQLite Catalog v1 Dependency and Persistence Boundary

**Status:** Accepted

**Date:** 2026-07-14

**Decision owners:** Product owner and primary implementation agent

## Context

EP-000 Milestone 5 introduces the first durable local catalog. The catalog records read-only scan observations and must support repeat scans without unbounded duplicate rows, retain explicit absence state, reject incompatible schemas, and return typed database errors. It must not become proof that the filesystem still matches an older observation.

Rust's standard library has no SQLite client. Calling an external `sqlite3` executable would introduce an undeclared machine dependency and a process boundary that is unsuitable for typed path-safe application code. Writing direct SQLite FFI would require project-owned `unsafe` code, which the workspace forbids without a separate safety decision and evidence.

## Decision

- Pin `rusqlite` exactly at `0.40.1` with `default-features = false` and the `bundled` feature in `fileos-catalog` only.
- Use the bundled SQLite `3.53.2` supplied through `libsqlite3-sys 0.38.1`; do not rely on a contributor-installed SQLite DLL or development package.
- Do not enable extension loading, SQLCipher, serialization, backup, FTS, time, UUID, or other optional rusqlite features in Catalog v1.
- Keep SQL and rusqlite types inside `fileos-catalog`. Domain and application contracts remain independent from SQLite.
- Use schema version `1`, atomic forward migration, foreign keys on every connection, an explicit busy timeout, and WAL only after the connection verifies that SQLite accepted WAL mode.
- Keep write transactions bounded to in-memory observation batches. Scanner metadata I/O must occur outside database transactions.
- Treat absence conservatively: only a fully `completed` scan may mark unseen entries missing. Cancelled, failed, or completed-with-issues runs retain the previous presence state.
- Persist opaque path identity as bytes scoped by a scan root and keep display text separately. Catalog v1 never reconstructs a filesystem path from database bytes and exposes no filesystem mutation API.

## Rationale

- `rusqlite` provides parameterized SQL, transactions, SQLite result-code access, and a maintained safe Rust API.
- Bundling makes the SQLite runtime deterministic across supported Windows developer and test machines and avoids DLL discovery/version drift.
- Disabling default and unused features limits the dependency and capability surface.
- Conservative absence semantics prevent an access-denied, bounded, cancelled, or interrupted scan from falsely tombstoning entries.
- Stable `(scan_root_id, path_key)` uniqueness makes repeated observations converge while `first_seen_run_id`, `last_seen_run_id`, and `missing_since_run_id` preserve lifecycle evidence without deleting rows.

Upstream references consulted on 2026-07-14:

- [rusqlite repository and bundled-build guidance](https://github.com/rusqlite/rusqlite)
- [rusqlite 0.40.1 manifest](https://github.com/rusqlite/rusqlite/blob/v0.40.1/Cargo.toml)
- [rusqlite MIT license](https://github.com/rusqlite/rusqlite/blob/v0.40.1/LICENSE)
- [SQLite public-domain dedication](https://www.sqlite.org/copyright.html)

## Dependency impact

- Production dependencies include rusqlite, libsqlite3-sys, and their small Rust/build-time transitive set.
- The bundled C source increases clean build time and binary size compared with dynamic linking.
- Security and maintenance require intentional rusqlite/SQLite updates, lockfile review, and rerunning migration/integration tests.
- No network client, telemetry, runtime download, or external database service is introduced.

## Alternatives considered

### Link to a system SQLite library

Rejected for Catalog v1 because supported Windows machines do not provide one consistent SQLite development/runtime installation. This would move version and DLL-search behavior outside repository control.

### Invoke `sqlite3.exe`

Rejected because it requires an external binary, weakens typed error/transaction boundaries, and introduces a process/shell surface that the application does not need.

### Implement direct SQLite FFI

Rejected because it duplicates a mature binding, requires `unsafe`, and creates disproportionate security and maintenance cost.

### Use a different Rust SQLite wrapper

Deferred because common higher-level wrappers either build on rusqlite or add ORM/async dependencies not needed by the single bounded writer design.

## Safety impact

- SI-001: catalog writes only its caller-supplied database and never mutates a scan root.
- SI-002 and SI-013: integration databases live under the testkit run's `artifacts` directory or an explicitly supplied temporary test scope.
- SI-017: migration, reopen, repeat-scan, absence, corruption, and read-only source claims require named test evidence.
- A catalog row remains an observation, not current filesystem truth and not authorization for a destructive operation.

## Compatibility and maintenance

- Schema versions newer than `1` fail closed with a typed `unsupported_schema_version` error.
- Version-zero databases migrate atomically to v1. No legacy production schema exists yet.
- WAL support is verified per database connection; an unsupported journal-mode result is a typed configuration failure rather than a silent downgrade.
- Windows 10/11 CI evidence remains deferred under EP-000; local Windows integration tests are required for this milestone checkpoint.

## Rollback

Revert this ADR, the catalog implementation/migration, dependency manifest and lockfile changes, and M5 integration tests. Runtime test databases are disposable artifacts and are not committed. No rollback touches fixture input or user files.

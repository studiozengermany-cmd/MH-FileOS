# MH FileOS — Safety Traceability

**Status:** Active evidence index
**Current scope:** EP-000 Milestone 6 read-only vertical slice local checkpoint complete
**Last updated:** 2026-07-14

This table maps the safety invariants currently exercised by repository contracts, the deterministic fixture generator, the bounded read-only scanner CLI and SQLite Catalog v1. A missing future implementation is reported as untested rather than inferred from these entries.

| Invariant | Enforcement in current scope | Named tests/evidence | User-visible behavior | Gate | Latest evidence |
|---|---|---|---|---|---|
| SI-001 Read-only means read-only | Scanner exposes observation only; catalog writes only its separate database artifact; CLI composition has no mutation capability | `complete_scan_matches_snapshot_and_is_read_only`; `fixture_scan_emits_path_free_json_and_preserves_source`; `repeat_scan_converges_and_scanner_preserves_fixture`; `fixture_catalog_demo_emits_verified_path_free_summary_and_cleans` | Complete, partial, cancelled and failed scans remain distinct; catalog state is not filesystem truth | M4/M5/M6 | M6 manifest/snapshot/catalog equality and before/after fixture proof |
| SI-002 Scope confinement | Testkit cleanup validates its capability root; scanner validates an absolute root and does not traverse reparse targets | `cleanup_scope_rejects_repository_and_sandbox_base`; `outside_target_symlink_is_observed_without_traversal` | Invalid roots fail; reparse points are counted and skipped | M3/M4 | Local workspace test |
| SI-010 No shell interpolation | Testkit, scanner and CLI pass typed paths to standard-library filesystem APIs; verifier rejects unapproved files/dependencies | Source inspection + Clippy; CLI fixture integration | CLI accepts one absolute path argument without constructing shell commands | Every change | `scripts/verify.ps1` |
| SI-013 Test isolation | Capability-owned `SandboxRun`; catalog databases live under `artifacts`, outside scanner `input`; cleanup does not follow reparse points | marker/manifest tamper tests; `scan_root_snapshot_detects_same_size_content_change`; `fresh_database_migrates_reopens_and_enforces_connection_settings` | Cleanup and verification fail closed instead of widening scope | M3/M4/M5 | Final aggregate gate confirms zero runtime sandbox residue |
| SI-017 Truthful evidence labels | EP-000 separates file, test, untested and inference claims | package/workspace verification; milestone evidence in `PLANS.md` | Reports cannot call scanner/catalog behavior tested before implementation | Every handoff | Current EP-000 evidence |

## M5 residual evidence limits

- Windows file-symlink creation is attempted only when the host permits it; the logical manifest marks the scenario optional and runtime evidence reports whether it was created.
- Permission-denied is covered only by typed error mapping; a real Windows ACL denial fixture still needs a separately approved test design.
- Crash/kill during a migration is not yet tested with a child process; deterministic migration transactions, reopen, corrupt bytes and writer contention are covered locally.
- Interrupted catalog scan runs require an explicit exclusive-startup call to `recover_interrupted_scans`; `dropped_scan_can_be_explicitly_recovered_after_reopen` proves recovery never reconciles absence, while sink-error terminalization is covered separately.
- Authoritative completion is bound to streamed counters, per-code issue counts and the durable observed count before absence reconciliation; counter, issue-breakdown and durable-count mismatch tests fail closed.
- Schema v1 reopen verification compares the complete normalized user schema with the compiled migration, then independently checks columns, STRICT tables, named indexes, foreign-key layout and persisted foreign-key consistency.
- Reopening and explicitly recovering an orphaned fixture capability is not a testkit API; cleanup still requires the in-memory capability returned by successful creation.
- Static reparse fixtures prove no-follow behavior, but standard-library path APIs do not provide evidence against a hostile reparse-swap TOCTOU race.
- Windows volume/file identity and comparison normalization remain deferred; M5 fixture identities and path keys are explicitly synthetic/exact-byte only.
- Million-entry catalog performance, concurrent-reader soak, non-Windows runtime behavior and CI remain untested until their approved gates.
- M6 process-kill recovery is not simulated; an abrupt kill can leave a positively marked orphaned fixture capability, so manual cleanup still requires separate approval and the aggregate gate fails on any residue.

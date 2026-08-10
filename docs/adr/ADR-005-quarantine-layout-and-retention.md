# ADR-005 — Quarantine Layout and Retention Policy

**Status:** Proposed · **Deciders:** TBD · **Safety impact:** HIGH (SI-003, SI-007, SI-009)

## Context
MVP "delete" is never an unlink; it is a journaled move to an app-managed Quarantine
(SAFETY §4.3). Restore must be byte-exact. MVP does not auto-purge.

## Decision
- Per-volume quarantine root `<volume>/.mh-fileos/quarantine/` to keep same-volume atomic rename
  and avoid a cross-volume copy for the common case (SAFETY §4.1).
- Entry path `<quarantine-root>/<operation-id>/<action-id>/<original-basename>`, collision-safe via
  UUIDv7 operation/action ids.
- Sidecar metadata durable BEFORE movement (SI-014): original path (display + comparison),
  quarantine path, reason/plan id, fingerprint (algorithm + digest), created_at, expiry policy,
  restore state, source volume identity.
- Retention: MVP never auto-purges. Cleanup touches ONLY entries verified present with matching
  metadata; it fails closed otherwise (mirrors TEST-STRATEGY §5 fixture contract).
- SI-009: quarantine can never remove the last verified independent copy of a duplicate group.

## Alternatives
- Centralized single-volume quarantine → forces a cross-volume copy + verify for every delete,
  widening the SI-003 surface. Rejected as MVP default (may revisit for cross-volume sources).
- OS Recycle Bin → not app-controlled and not byte-exact-restore guaranteed. Rejected.

## Migration / reversal cost
On-disk layout; a later change needs a quarantine-index migration. Low now (no data yet).

## Test evidence (required before acceptance)
Executor §11.3 quarantine tests, undo §12 restore-to-occupied-path, protected-zone rejection.
Status: [CHƯA KIỂM THỬ].

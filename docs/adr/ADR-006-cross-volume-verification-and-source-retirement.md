# ADR-006 — Cross-Volume Verification and Source-Retirement Protocol

**Status:** Proposed · **Deciders:** TBD · **Safety impact:** CRITICAL (SI-003, SI-005, SI-007, SI-014)

## Context
The originating directive's "2-Phase Atomic Swap" is unsafe across volumes: no atomic rename exists
across filesystems. SAFETY §4.2 mandates copy → verify → promote → retire and forbids retiring the
source before durable verification.

## Decision
Cross-volume move state machine (durable journal intent before each irreversible step, SI-014):

```text
planned → validated → intent_recorded → temp_destination_created → copying → copied
→ destination_verified → destination_promoted → source_retirement_started
→ source_retired → committed
```

Rules:
- Source retirement is IMPOSSIBLE until `destination_verified` is durably recorded (central
  assertion, TEST-STRATEGY §11.2).
- Verification = full content-hash compare (BLAKE3 by default, ADR-004) between the source snapshot
  fingerprint and the promoted destination; optional byte-compare per risk class.
- Retirement = move-to-quarantine (ADR-005), never a direct unlink.
- On any typed I/O error mid-flight: stop, do NOT panic (AGENTS §8), transition to
  `failed_recoverable` / `failed_attention_required`; recovery derives the next safe action from the
  journal + live filesystem observation (SI-007), never from the database alone (SI-015).
- Same-volume stays atomic rename (SAFETY §4.1); no silent fallback to copy/delete unless the plan
  explicitly authorizes a cross-volume-like strategy.

## Alternatives
- Atomic swap everywhere → impossible cross-filesystem; data-loss risk. Rejected.
- Delete-then-copy → violates SI-003. Rejected.

## Migration / reversal cost
Encoded in versioned journal events; a later ordering change needs an event-schema migration and a
recovery-logic review. Contained via versioned events now.

## Test evidence (required before acceptance)
Fault injection at every transition (disk-full, short write, kill-after-copy-before-verify,
kill-after-verify-before-retire, digest mismatch, source-locked-at-retire, volume disconnect).
Status: [CHƯA KIỂM THỬ].

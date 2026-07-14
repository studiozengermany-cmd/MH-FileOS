# MH FileOS — Test Strategy

**Document ID:** TEST-001  
**Status:** Accepted for EP-000 implementation
**Risk level:** P0 data-safety product  
**Last updated:** 2026-07-14

Testing for MH FileOS is not a final polish step. It is the evidence system that permits a filesystem operation to be called safe, correct, recoverable, and performant.

## 1. Objectives

The test system must prove, at the appropriate layer, that:

- scanning is read-only and scope-confined;
- findings and duplicate evidence are correct and explainable;
- plans are deterministic, immutable, and invalidated when stale;
- execution obeys the state machines in `SAFETY-INVARIANTS.md`;
- crashes, cancellation, disk-full, locks, and disconnects fail safely;
- recovery and undo use current filesystem evidence;
- database migration preserves catalog and journal meaning;
- the UI communicates partial failure and risk truthfully;
- resource use is bounded and measured.

## 2. Evidence language

Every completion or review report uses one of these labels:

- **[ĐÃ XÁC MINH BẰNG TEST]** — a named automated or manual test ran and passed.
- **[ĐÃ XÁC MINH BẰNG FILE]** — direct inspection of named files supports the claim.
- **[CHƯA KIỂM THỬ]** — implementation or claim has not been exercised.
- **[SUY LUẬN]** — reasoned conclusion, not direct evidence.

“Should work,” “probably safe,” and “looks fine” are not release evidence.

## 3. Test principles

1. **Isolation:** automated mutation tests operate only inside a newly created test sandbox.
2. **Observable facts:** tests assert filesystem state and journal events, not only return values.
3. **Negative-first safety:** every destructive happy path has stale, conflict, permission, interruption, and verification-failure cases.
4. **Determinism:** seeds, fixture manifests, policies, timestamps, and randomized cases are reproducible.
5. **Fault injection:** failure at every durable transition is a required test technique.
6. **Platform truth:** Windows-specific behavior is tested on Windows filesystems, not inferred from Linux CI.
7. **No production paths:** test helpers reject roots that are not positively identified as disposable sandboxes.
8. **Performance honesty:** benchmarks report hardware, filesystem, dataset, cache state, version, and variance.

## 4. Verification layers

| Layer | Purpose | Runs |
|---|---|---|
| Static checks | formatting, lint, dependency policy, forbidden APIs | every change |
| Unit tests | pure domain transitions and validation | every change |
| Property tests | path/rule/plan invariants over generated cases | every change, bounded |
| Component tests | scanner, catalog, hashing, planner with fixtures/fakes | every change |
| Integration tests | real SQLite and sandbox filesystem | every change |
| Fault/recovery tests | crash points and partial filesystem outcomes | PR subset; full nightly |
| UI component tests | presentation, accessibility, error states | every UI change |
| End-to-end tests | desktop user journeys over fixtures | PR smoke; full nightly |
| Platform matrix | Windows volumes and capabilities | nightly/release |
| Performance/soak | throughput, latency, memory, handles, long-run stability | nightly/release |
| Manual exploratory | OS integration and high-risk UX | release candidate |

Line coverage is a diagnostic, not the target. Coverage gates are risk-based: all safety invariants, transitions, typed errors, and recovery branches require explicit named tests.

## 5. Fixture safety contract

All test files live under a unique run directory created beneath an approved test root such as:

```text
fixtures/sandbox/runtime/<run-id>/
├─ .mh-fileos-test-sandbox
├─ manifest.json
├─ input/
├─ expected/
├─ quarantine/
└─ artifacts/
```

Before a test helper creates, moves, renames, quarantines, restores, or deletes anything, it must prove:

1. the canonical root is beneath the configured sandbox base;
2. the marker file exists and contains the current run ID and schema version;
3. the target is beneath that exact canonical run root;
4. no component used to prove containment traverses a symlink/junction/reparse point;
5. the root is not a filesystem root, home/profile directory, repository parent, or protected zone;
6. the manifest identifies the path as disposable.

Cleanup fails closed if any proof is absent. Cleanup must never broaden its target to recover from an error.

## 6. Fixture catalog

The deterministic generator produces a manifest with seed, entry type, bytes/digest, timestamps, attributes, identity expectations, and intended findings.

Required fixture families:

| Fixture | Cases |
|---|---|
| Basic tree | nested directories, empty directories, common file types |
| Exact duplicates | same bytes/different names, zero-byte files, large files |
| False duplicate candidates | same size/different bytes, same quick hash/different full hash synthetic case |
| Hard links | two entries/same identity, link-count changes |
| Links/reparse | symlink, junction, cycle, target outside scope |
| Path semantics | Unicode normalization, combining marks, emoji, RTL, trailing-dot/space API cases |
| Length and depth | extended-length path, deep tree, maximum configured depth |
| Case behavior | collisions and case-sensitive directory where supported |
| Access problems | unreadable, locked, disappearing, permission denied |
| Mutation conflicts | destination exists, changed source, changed destination, rename race |
| Volume behavior | same-volume, cross-volume, removal/reconnect simulation |
| Database | empty, large, old migration, corrupt copy, interrupted transaction |
| Project zones | `.git`, `node_modules`, build outputs, package metadata |
| Cloud placeholders | offline/placeholder attributes where test environment supports them |

Fixture source content is synthetic. Do not commit personal documents, credentials, private paths, real user databases, or malware samples.

## 7. Unit and property tests

### 7.1 Domain/state machine tests

For every allowed transition, test:

- valid transition reaches the expected state;
- invalid predecessor is rejected;
- retry does not duplicate an irreversible side effect;
- terminal state cannot return to execution;
- partial completion cannot be serialized as full success;
- unknown persisted enum or schema version fails safely.

### 7.2 Path and identity properties

Generate paths and identities to prove:

- display-path formatting does not change comparison identity;
- normalization is idempotent for its version;
- containment cannot be created by string-prefix tricks;
- sibling paths such as `C:\\data` and `C:\\database` are not confused;
- `..`, alternate separators, device paths, UNC paths, and extended prefixes are handled explicitly;
- a drive-letter remount does not satisfy a mismatched `VolumeId`;
- a reparse target cannot escape scope through canonicalization races;
- case rules use actual volume/directory capability.

### 7.3 Rule and planner properties

Prove that:

- identical catalog snapshot + policy yields identical plan ordering/content;
- rule priority has one unambiguous outcome;
- exclusions and protected zones dominate organization rules;
- a plan never contains a target outside approved scope;
- no destination collision exists within one plan;
- every action has complete preconditions and rationale;
- a catalog or policy version change invalidates the affected plan;
- AI text cannot become an executable action without deterministic parsing and validation.

## 8. Scanner tests

Scanner tests assert both returned data and the absence of writes.

Required cases:

- complete small-tree scan;
- million-entry synthetic/virtualized stress dataset;
- cancellation at directory enumeration, metadata batching, and database checkpoint;
- access denied on one subtree while other entries continue;
- file disappears between enumeration and metadata read;
- directory rename during traversal;
- reparse point ignored by default;
- reparse cycle cannot loop;
- outside-scope target is not traversed;
- long/Unicode paths remain round-trippable;
- open file handles remain below the configured bound;
- queue length remains below configured capacity;
- partial errors are visible in final status;
- repeat scan converges without duplicate catalog rows.

Read-only proof includes a before/after manifest comparison of path, identity, bytes, timestamps where observable, and attributes. Expected access-time noise must be documented and controlled rather than ignored broadly.

## 9. Catalog and migration tests

- migrate a new database from version zero;
- upgrade a copy from every supported schema version;
- reject a database newer than the application understands;
- prove foreign-key enforcement on each connection path;
- crash/kill during migration and reopen safely;
- verify WAL/checkpoint behavior and recovery;
- concurrent readers with bounded writer contention;
- idempotent scan upserts and tombstone/absence semantics;
- FTS/search results match canonical queries;
- integrity-check and backup/restore workflows;
- preserve operation events without reordering or loss;
- reject corrupt or semantically inconsistent rows safely.

Migration fixtures are immutable golden inputs. Tests copy them before opening.

## 10. Duplicate-detection tests

Required proofs:

- same size alone never yields exact-duplicate eligibility;
- quick fingerprint alone never yields destructive eligibility;
- full hashes match for byte-identical files across buffer boundaries;
- same size/different content separates correctly;
- zero-byte policy is explicit and safe;
- algorithm/version changes do not reuse incompatible digests;
- source mutation during hashing marks evidence stale;
- hard-linked paths are identified as one physical identity;
- sparse/compressed/encrypted attributes are recorded and handled by policy;
- optional byte verification detects a synthetic digest mismatch;
- duplicate group retains at least one verified independent copy;
- keeper choice is deterministic, explainable, and protected-zone aware.

## 11. Executor tests

No executor test uses real personal data.

### 11.1 Same-volume move/rename

Test success and failure at:

1. precondition refresh;
2. journal intent durable;
3. destination conflict check;
4. native rename attempt;
5. post-operation identity/path verification;
6. final journal event.

Assert that a conflict never overwrites, a stale source never proceeds, retry is idempotent, and reported status matches observed filesystem state.

### 11.2 Cross-volume move

Inject failure before and after every transition:

```text
IntentDurable → Copying → CopyComplete → Verifying → Verified
→ SourceRetireIntent → SourceQuarantined/Retired → Completed
```

Scenarios include:

- destination disk full mid-copy;
- short write or injected I/O error;
- destination already exists;
- source changes during copy;
- destination changes before verification;
- digest verification fails;
- process killed after copy but before verification;
- process killed after verification but before source retirement;
- source locked at retirement;
- volume disconnect and reconnect with different identity;
- journal write fails at each transition.

The central assertion: source retirement is impossible until destination verification is durably recorded.

### 11.3 Quarantine and delete semantics

- quarantine destination is unique and collision-safe;
- metadata required for restore is durable before movement;
- protected-zone item cannot enter the destructive path;
- retention cleanup only touches verified quarantine entries;
- “permanent delete” is absent from MVP or separately gated;
- failed quarantine remains visible and recoverable;
- duplicate cleanup cannot remove every independent verified copy.

## 12. Recovery and undo tests

The test harness kills the process or injects an abrupt adapter failure at every durable state. On restart it verifies:

- the operation is detected as nonterminal;
- observed filesystem facts are reconciled with journal evidence;
- the proposed recovery action is deterministic;
- no overwrite or second source retirement occurs;
- repeated recovery is idempotent;
- ambiguous state stops for user review;
- terminal wording is truthful.

Undo tests cover:

- successful restore to an empty original location;
- original location occupied by a new file;
- source volume absent;
- quarantine item modified or missing;
- permissions changed;
- partial multi-item undo;
- process killed during undo;
- repeated undo request;
- undo itself appears as a fully journaled operation.

## 13. Security tests

Automated checks include:

- path traversal and scope-prefix confusion;
- symlink/junction swap between validation and use;
- device/UNC/extended path inputs;
- command/shell injection characters in file names;
- malicious rule payloads and unknown rule versions;
- oversized IPC payloads and malformed DTOs;
- unauthorized Tauri command invocation;
- release CSP contains no remote script wildcard or unsafe capability drift;
- diagnostics redact file names, full paths, content, and credentials;
- database content cannot cause UI script injection;
- update verification rejects unsigned/mismatched artifacts when updates exist;
- dependency audit/license policy and committed lockfiles.

Security tests do not substitute for core safety tests; both are required.

## 14. UI and accessibility tests

### 14.1 Component tests

- loading, empty, populated, partial-error, cancelled, disconnected, and recovery states;
- risk labels and protected-zone explanations;
- plan preview displays source, destination, reason, conflict policy, and reversibility;
- approval cannot be triggered by an accidental single click for high-risk actions;
- completed/partial/failed labels match typed backend status;
- large virtualized lists preserve selection and keyboard operation;
- Vietnamese and English text expansion does not hide critical controls.

### 14.2 End-to-end journeys

1. Create fixture sandbox → scan → view dashboard.
2. Find exact duplicates → inspect evidence → build plan without executing.
3. Approve sandbox move → observe progress → inspect journal.
4. Inject interruption → restart → recover.
5. Quarantine sandbox duplicate → undo with verification.
6. Try protected/out-of-scope action → receive actionable rejection.

E2E tests assert backend/filesystem truth as well as visible UI.

### 14.3 Accessibility

- keyboard-only critical journeys;
- visible focus;
- semantic names/roles and status announcements;
- contrast and non-color risk communication;
- reduced-motion support;
- 200% text scaling for critical actions;
- screen-reader review on Windows for release candidates.

## 15. Windows/platform matrix

| Environment | PR | Nightly | Release |
|---|---:|---:|---:|
| Windows 11 + NTFS | smoke | full | full |
| Windows 10 supported baseline + NTFS | — | core | full |
| exFAT removable drive | — | core | full |
| NTFS external/removable | — | core | full |
| ReFS, if supported | — | exploratory | documented |
| UNC/network share read-only | — | exploratory | documented |
| OneDrive placeholder tree | — | selected | manual + selected |
| Case-sensitive directory | selected | full | full |
| Non-admin standard account | smoke | full | full |
| Restricted/locked files | selected | full | full |

Mutation support is enabled per capability only after its row meets the relevant release gate. Unsupported combinations are visibly rejected, not treated as generic success.

## 16. Performance and soak testing

### 16.1 Standard datasets

- **S:** 10k entries / 10 GB / shallow tree.
- **M:** 250k entries / 250 GB / mixed depth and duplicates.
- **L:** 1M entries / metadata-dominant synthetic tree.
- **Hash-L:** controlled large-byte corpus for throughput and thermal behavior.
- **Mutation:** 10k planned sandbox operations with injected conflicts.

### 16.2 Metrics

- entries enumerated per second;
- bytes hashed per second by volume type;
- time to first visible result;
- p50/p95 query and UI response latency;
- peak/steady resident memory;
- open handles and queue high-water marks;
- database size, WAL size, checkpoint duration;
- cancellation latency;
- recovery duration and unresolved count;
- CPU, I/O throughput, and UI dropped frames/long tasks.

### 16.3 Benchmark report requirements

Every report states application commit/build, Rust/OS version, CPU, RAM, storage model/filesystem, power mode, dataset manifest/seed, warm/cold cache, run count, median, p95 or spread, and known background load. Compare against a pinned baseline and flag statistically/materially significant regressions.

Soak tests run repeated scan/rescan/watcher cycles and interruption recovery for at least the duration defined by the release plan. They watch memory, handles, database growth, deadlocks, event backlog, and result drift.

## 17. CI gates

### 17.1 Every pull request

1. formatting and generated-file consistency;
2. compile/type check all affected workspaces;
3. lints with warnings denied under agreed policy;
4. unit/property/component tests;
5. SQLite migration/integration tests;
6. fixture sandbox containment check;
7. security/dependency checks;
8. UI component/accessibility smoke where relevant;
9. Windows smoke for filesystem-affecting code;
10. changed safety invariant has named tests and reviewer acknowledgement.

### 17.2 Nightly

- full Windows platform matrix available to the project;
- fault-injection/recovery matrix;
- E2E desktop journeys;
- large dataset performance;
- migration chain from all retained versions;
- dependency/license vulnerability scan;
- soak subset.

### 17.3 Release candidate

- clean full matrix with retained artifacts;
- installer/update/signing verification;
- manual standard-user exploratory pass;
- accessibility pass;
- recovery from every documented nonterminal state;
- performance report against baseline;
- known limitations and unsupported filesystems reviewed;
- safety-invariant traceability reviewed and signed off.

Flaky tests are failures with owners and deadlines. They are not silently retried until green or removed from gates without an approved risk record.

## 18. Safety traceability matrix

Maintain a machine-readable or generated matrix that maps each `SI-xxx` invariant to:

- enforcing module(s);
- unit/property tests;
- integration/fault tests;
- user-visible behavior;
- release gate;
- latest evidence artifact.

A P0 invariant with no test mapping blocks release.

## 19. Test result record template

```markdown
# Verification Record: <change/release>

- Build/commit:
- Environment:
- Scope:
- Fixture manifest/seed:
- Commands executed:
- Passed:
- Failed:
- Skipped and reason:
- Fault points covered:
- Performance comparison:
- Artifacts/logs:
- [ĐÃ XÁC MINH BẰNG TEST]:
- [ĐÃ XÁC MINH BẰNG FILE]:
- [CHƯA KIỂM THỬ]:
- [SUY LUẬN]:
- Residual risks:
```

## 20. Exit criteria by phase

### Sprint 0 / read-only foundation

- fixture containment tests pass;
- scanner performs no writes and reports partial failures;
- catalog migrations and read queries pass;
- resource bounds are measured;
- no executor or destructive API is reachable.

### Planning preview

- deterministic plan/property tests pass;
- protected zones and stale-plan rejection pass;
- UI preview represents every precondition and risk.

### First mutation preview

- same-volume state-machine and fault tests pass fully in sandbox;
- journal/recovery/undo evidence passes;
- manual review confirms truthful UI language;
- feature remains opt-in and clearly marked preview.

### Production release

- every P0 safety invariant has current evidence;
- no unresolved P0/P1 data-loss or recovery defect;
- Windows support matrix and installer pass;
- performance budgets have an honest report;
- rollback/disable strategy is documented and tested.

No schedule or feature pressure can waive a P0 safety gate without an explicit product decision that removes the unsafe capability from the release.

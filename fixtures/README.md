# Fixtures

This directory is the only repository-owned area intended for generated filesystem test data. It must never contain or point to real user documents.

## Safety contract

Mutation and cleanup helpers may act only under:

```text
fixtures/sandbox/runtime/<run-id>/
```

Each run directory must contain `.mh-fileos-test-sandbox` and `manifest.json`. The marker records at minimum:

- schema version;
- unique run ID;
- canonical sandbox root;
- creation time;
- generator version and seed;
- explicit `disposable: true` flag.

Before any write, move, quarantine, restore, or cleanup, the test helper must verify the canonical containment and marker/run ID. It rejects filesystem roots, profile/home directories, repository parents, reparse escapes, missing markers, mismatched manifests, and relative/ambiguous roots.

No helper may “recover” from failed containment by widening the cleanup target.

## Milestone 3 implementation

`fileos-testkit::SandboxBase::for_repository` positively identifies the repository and establishes the canonical runtime base. It returns `SandboxRun` capabilities for unique runs; no public cleanup API accepts an arbitrary path.

The generator writes the marker and deterministic manifest before fixture payloads. Cleanup revalidates canonical direct-child containment, repository identity, marker bytes and manifest bytes, then removes entries without following reparse points. A failed cleanup returns the unconsumed capability together with a typed error.

The M3 manifest uses `fnv1a64-fixture-v1` only as lightweight deterministic fixture evidence. It is not collision-resistant and must never be reused as production exact-duplicate proof.

## Milestone 4 read-only oracle

`SandboxRun::scan_root()` provides the only root used by scanner verification. `SandboxRun::snapshot()` records stable relative paths, entry kinds, file size/content evidence, modified time and observable attributes without following reparse points. Snapshot work is bounded by explicit entry, depth and file-size limits; exceeding a limit returns a typed error instead of silently weakening the comparison.

M4 scanner and CLI tests compare snapshots before and after traversal, exercise cancellation and partial outcomes, and verify that an outside-target symlink is observed but never followed. Snapshot evidence remains a test oracle only; it is not a production catalog or duplicate fingerprint.

## Planned layout

```text
fixtures/
├─ README.md
├─ specs/                       # versioned declarative fixture definitions
├─ golden/                      # immutable DB/migration and output fixtures
└─ sandbox/
   └─ runtime/                  # generated; never committed
      └─ <run-id>/
         ├─ .mh-fileos-test-sandbox
         ├─ manifest.json
         ├─ input/
         ├─ expected/
         ├─ quarantine/
         └─ artifacts/
```

## Required generated cases

- nested and empty directories;
- byte-identical files with different names/extensions;
- same-size files with different content;
- zero-byte and large sparse/regular files where supported;
- hard links;
- Unicode, combining characters, emoji, RTL, long and deep paths;
- case collisions/case-sensitive directories where supported;
- symlinks, junctions, reparse cycles, and targets outside scope;
- files that disappear or change during observation;
- permission-denied and locked files;
- destination conflicts;
- simulated cross-volume interruption and disk-full faults;
- project zones such as `.git`, dependency directories, and build outputs;
- migration databases for every retained schema version.

## Manifest requirements

The generator writes an ordered manifest containing expected path, entry type, byte generator/digest, size, timestamp policy, attributes, link relationship, expected identity behavior, intended findings, and protected-zone expectation. Random generation always records the seed.

Tests compare actual results with the manifest; directory listing order must never be assumed.

## Repository hygiene

- Commit declarative specs and small synthetic golden fixtures only.
- Do not commit generated runtime sandboxes, logs, large corpora, personal paths, credentials, or private documents.
- Large benchmark corpora should be reproducibly generated and identified by manifest hash.
- Golden database fixtures are opened only after copying them into the active run directory.
- Any test artifact retained after a failure remains inside that run’s `artifacts/` directory.

See [Test Strategy](../docs/TEST-STRATEGY.md) and [Safety Invariants](../docs/SAFETY-INVARIANTS.md).

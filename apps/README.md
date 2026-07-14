# Applications

This directory contains user-facing shells and developer tools. Applications are clients of the MH FileOS core; they do not reimplement core policy.

## Applications

### `apps/desktop`

Windows-first Tauri 2 + React + TypeScript desktop application.

Responsibilities:

- guide scope selection and show protected-zone decisions;
- display scan progress, partial errors, findings, and catalog queries;
- present action plans, preconditions, risk, conflicts, and reversibility;
- collect explicit approval;
- show execution/recovery/undo state from typed backend events;
- provide accessible Vietnamese and English presentation.

Forbidden responsibilities:

- direct filesystem or SQLite access from React;
- duplicate proof, protected-zone enforcement, or keeper selection;
- path concatenation for mutation;
- treating UI state as operation truth;
- shelling out with user paths;
- hiding partial failures behind a generic success message.

Target UI structure:

```text
apps/desktop/
├─ src/
│  ├─ app/               # routing, providers, composition
│  ├─ features/          # scope, scan, findings, plans, history, settings
│  ├─ components/        # reusable presentational components
│  ├─ contracts/         # generated/versioned backend DTO client
│  ├─ i18n/              # vi/en resources
│  └─ test/              # UI test utilities
├─ src-tauri/            # thin Tauri adapters and composition root
└─ e2e/
```

Initial screen map:

1. Welcome and safety promise.
2. Scope selection.
3. Scan progress with cancellation and partial-error details.
4. Dashboard.
5. Findings: exact duplicates, large files, empty directories.
6. Plan preview and explicit approval.
7. Operation progress, history, recovery, and undo.
8. Settings, protected zones, diagnostics, and privacy.

Release shell requirements:

- restrictive Content Security Policy;
- minimum Tauri capabilities per window;
- no remote scripts/content;
- debug/dev commands absent from release;
- typed, versioned IPC contracts;
- progress event coalescing;
- keyboard/screen-reader support for critical journeys;
- safe visible states for loading, empty, partial, cancelled, failed, disconnected, and recovery-required.

### `apps/fileos-cli`

A developer/diagnostic CLI introduced before the desktop shell. Its production command remains read-only and accepts exactly one explicit absolute scan root. It supports deterministic verification of scanner behavior without UI ambiguity.

Milestone 4 implements the read-only scanner slice: one explicit absolute root, a path-free deterministic JSON summary, and truthful exit codes for completed, partial, cancelled, usage and fatal outcomes. It does not persist a catalog or expose any mutation command.

Milestone 6 adds a dev-only Cargo example that composes the deterministic fixture generator, scanner and SQLite catalog. The example database stays below the fixture's `artifacts` directory, verifies manifest/snapshot/catalog counts and source immutability, emits path-free JSON, and cleans the capability-owned sandbox before reporting success. It does not expand the production CLI command surface.

The CLI must not become a safety bypass. Future mutation commands, if ever added, use the same planner, approval artifact, executor, protected-zone policy, and journal as the desktop app.

## Contract rule

Backend contracts have an explicit version. A client compiled against an unsupported contract receives a typed incompatibility error rather than silently dropping fields or defaulting an unknown action.

See [Architecture](../docs/ARCHITECTURE.md) and [Test Strategy](../docs/TEST-STRATEGY.md).

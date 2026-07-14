# ADR-000 — Local Toolchain Baseline

**Status:** Accepted

**Date:** 2026-07-14

**Decision owners:** Product owner and primary implementation agent

## Context

EP-000 Milestone 1 requires reproducible Rust, Node.js, and package-manager versions before any workspace or production dependency is introduced. The repository is Windows-first and is currently developed and verified locally; GitHub workflows are intentionally deferred by product-owner direction.

A floating channel such as `stable`, `lts`, or `latest` would allow two machines to build with different compilers or package managers. Installing a newer toolchain during this milestone would also make the baseline depend on an unreviewed machine mutation.

## Decision

- Pin Rust `1.97.0` in `rust-toolchain.toml` with the minimal profile plus the already-installed `rustfmt` and `clippy` components.
- Pin Node.js `24.17.0` in `.node-version`. Node 24 is an LTS line; the exact patch matches the current local machine.
- Select pnpm `10.11.0` and pin it through the root `package.json` `packageManager` and `engines` fields.
- Keep the root package private and dependency-free. A pnpm workspace and application packages belong to later milestones.
- Use `pwsh -NoProfile -File scripts/verify-toolchain.ps1` as the canonical local verification command. The verifier runs the already-installed target-qualified Rust toolchain through `rustup run`, so a missing baseline fails instead of triggering a toolchain install. A future Windows CI workflow must call the same script rather than duplicate version logic.
- Do not invoke Corepack, `rustup update`, an installer, or any dependency-install command as part of this milestone.

## Rationale

- Rust stable matches the architecture contract and avoids nightly-only language or compiler behavior.
- Node 24 is an LTS release line suitable for the later Tauri/React shell. Pinning the installed patch avoids an implicit download.
- pnpm has first-class workspace support and a content-addressed store, which fits the planned monorepo while reducing duplicate package storage.
- Exact pins make toolchain drift visible and reviewable.

Official references consulted on 2026-07-14:

- [Rust 1.97.0 release](https://blog.rust-lang.org/releases/latest/)
- [Node.js release status](https://nodejs.org/en/about/previous-releases)
- [Node.js 24.17.0 LTS release](https://nodejs.org/en/blog/release/v24.17.0)
- [pnpm workspace-oriented package manager](https://pnpm.io/)

## Alternatives considered

### Floating Rust `stable` and Node `lts`

Rejected because builds could change without a repository diff.

### Node.js 26 Current

Rejected for the baseline because it has not entered LTS as of this decision.

### npm as the workspace package manager

Viable and bundled with Node.js, but pnpm is preferred for the planned multi-package workspace and shared content-addressed storage. npm remains present only as part of the Node installation and is not the selected project package manager.

### Installing the newest Node or pnpm patch now

Deferred. Toolchain updates must be explicit, reviewed changes with local verification rather than side effects of reading or building the repository.

## Safety impact

- No user file is scanned or mutated.
- No production dependency, executable, or generated binary is added.
- The verification script reads repository metadata and tool version output only.
- SI-017 applies: version availability is reported as runtime-tested only after the verifier succeeds.
- SI-020 is supported by exact, reviewable toolchain identity; product binary versioning remains a later concern.

## Compatibility and maintenance

- Node `24.17.0` is LTS but is not the newest Node 24 patch available on the decision date. Updating to a later LTS patch requires a deliberate ADR amendment or superseding decision and a passing verifier.
- The local verifier is Windows/PowerShell-first, matching the MVP platform. Cross-platform developer tooling is not promised by this ADR.
- CI remains unverified until the product owner restores GitHub/CI scope.

## Rollback

Revert this ADR together with `.node-version`, `rust-toolchain.toml`, `package.json`, and `scripts/verify-toolchain.ps1`. No dependency cache, workspace artifact, or user data needs recovery because this decision installs nothing and creates no runtime state.

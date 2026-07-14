//! Synthetic fixtures and scope-confined cleanup capabilities for MH FileOS tests.
//!
//! The public API intentionally does not expose cleanup over an arbitrary path. A caller must
//! first establish an approved repository sandbox through [`SandboxBase`], then operate on the
//! resulting [`SandboxRun`] capability. Runtime fixture data is created only beneath
//! `fixtures/sandbox/runtime/<run-id>/`.

#![forbid(unsafe_code)]

mod digest;
mod error;
mod manifest;
mod sandbox;
mod snapshot;

pub use error::{CleanupFailure, Result, TestkitError};
pub use manifest::{
    DIGEST_ALGORITHM, FixtureEntry, FixtureEntryKind, FixtureManifest, OptionalScenario,
};
pub use sandbox::{
    MANIFEST_FILE_NAME, MARKER_FILE_NAME, SandboxBase, SandboxRun, SandboxRunEvidence,
};
pub use snapshot::{FixtureSnapshot, FixtureSnapshotEntry, FixtureSnapshotEntryKind};

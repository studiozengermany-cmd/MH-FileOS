//! Platform-independent domain types, value semantics, and invariant-preserving state
//! transitions for MH FileOS.
//!
//! Milestone 4 introduces the read-only scan observations, lifecycle, partial issues, counters,
//! and bounded-resource evidence used by application ports and infrastructure adapters.
//!
//! EP-007 slice A introduces the immutable action-plan domain: time-ordered action identifiers,
//! typed actions, plan-level structural validation, deterministic canonical serialization, and a
//! content-addressed plan fingerprint. This slice performs no filesystem access; it only models the
//! proposal that a later, gated executor may act on.

#![forbid(unsafe_code)]

mod plan;
mod scan;

pub use plan::{
    ActionId, ActionKind, ActionPlan, ActionPlanBuilder, ChecksumAlgorithm, ExpectedChecksum,
    InvalidActionId, InvalidChecksum, InvalidPlanPath, PlanFingerprint, PlanPath,
    PlanValidationError, PlannedAction, UuidV7Clock, UuidV7Rng, ACTION_PLAN_SCHEMA_VERSION,
};
pub use scan::{
    EntryKind, InvalidObservedEntry, InvalidRelativeScanPath, InvalidResourceLimits, ObservedEntry,
    ResourceBoundExceeded, ResourceKind, ResourceLimitField, ResourceLimits, ResourceUsage,
    ScanCounters, ScanIssue, ScanIssueCode, ScanIssueCounts, ScanPhase, ScanRunStatus,
};

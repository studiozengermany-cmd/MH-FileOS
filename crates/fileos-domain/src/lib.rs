//! Platform-independent domain types, value semantics, and invariant-preserving state
//! transitions for MH FileOS.
//!
//! Milestone 4 introduces the read-only scan observations, lifecycle, partial issues, counters,
//! and bounded-resource evidence used by application ports and infrastructure adapters.

#![forbid(unsafe_code)]

mod scan;

pub use scan::{
    EntryKind, InvalidObservedEntry, InvalidRelativeScanPath, InvalidResourceLimits, ObservedEntry,
    ResourceBoundExceeded, ResourceKind, ResourceLimitField, ResourceLimits, ResourceUsage,
    ScanCounters, ScanIssue, ScanIssueCode, ScanIssueCounts, ScanPhase, ScanRunStatus,
};

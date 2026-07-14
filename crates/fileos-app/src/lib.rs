//! Application orchestration and consumer-owned command/query port boundary for MH FileOS.
//!
//! This crate remains independent of UI types and concrete infrastructure adapters. Milestone 4
//! introduces only read-only scan requests, cancellation, reports, failures, and streaming ports.

#![forbid(unsafe_code)]

mod scan;

pub use scan::{
    CancellationToken, InvalidScanProgress, InvalidScanReport, InvalidScanRequest,
    SCAN_SUMMARY_SCHEMA_VERSION, ScanFailure, ScanFailureCode, ScanPort, ScanProgress, ScanReport,
    ScanRequest, ScanSink, ScanSinkError, ScanSinkErrorCode,
};

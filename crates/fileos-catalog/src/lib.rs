//! SQLite catalog persistence, migration, query, and bounded scan-writer boundary for MH FileOS.
//!
//! Catalog rows are historical observations and never authorize filesystem mutation.

#![forbid(unsafe_code)]

mod catalog;
mod error;
mod migration;
mod model;

pub use catalog::{Catalog, CatalogOpenOptions, CatalogScanSession, MAX_CATALOG_BATCH_SIZE};
pub use error::{CatalogError, CatalogErrorCode};
pub use model::{
    CatalogSummary, EntryPresence, EntryRecord, PresenceFilter, ScanCommit, ScanRootDescriptor,
    ScanRootId, ScanRunId, SummaryQuery, VolumeDescriptor, VolumeId,
};

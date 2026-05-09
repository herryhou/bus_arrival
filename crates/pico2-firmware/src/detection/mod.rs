//! Detection layer for pico2-firmware
//!
//! Provides firmware-specific adaptations of the pipeline detection crate.
//!
//! # Crate Integration
//!
//! - `find_active_stops()` delegates to `pipeline-filter` crate
//! - Probability computation via `pipeline-probability` crate

pub mod filter;
pub mod probability;

// ===== GPS Status =====

/// GPS processing status for phantom arrival detection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpsStatus {
    /// GPS is being processed normally
    Valid,
    /// GPS is being rejected (dr_outage)
    DrOutage,
    /// GPS is off-route (position frozen)
    OffRoute,
}

// Re-export filter function for convenience
pub use filter::find_active_stops;
pub use probability::FirmwareProbabilityEngine;

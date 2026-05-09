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

// ===== Compatibility Functions =====

/// Compute arrival probability (simplified version for control module)
///
/// This is a compatibility wrapper that uses the detection crate's
/// probability computation directly, bypassing the caching layer.
/// Use `FirmwareProbabilityEngine` for cached computation.
pub fn compute_arrival_probability(
    signals: shared::PositionSignals,
    v_cms: shared::SpeedCms,
    stop: &shared::Stop,
    dwell_time_s: u16,
    gps_status: GpsStatus,
) -> u8 {
    use detection::probability;
    use crate::lut::{GAUSSIAN_LUT, LOGISTIC_LUT};

    let gps_status = match gps_status {
        GpsStatus::Valid => probability::GpsStatus::Valid,
        GpsStatus::DrOutage => probability::GpsStatus::DrOutage,
        GpsStatus::OffRoute => probability::GpsStatus::OffRoute,
    };

    probability::compute_arrival_probability(
        signals, v_cms, stop, dwell_time_s, gps_status, &GAUSSIAN_LUT, &LOGISTIC_LUT
    )
}

/// Compute arrival probability with adaptive weights (compatibility wrapper)
pub fn compute_arrival_probability_adaptive(
    signals: shared::PositionSignals,
    v_cms: shared::SpeedCms,
    stop: &shared::Stop,
    dwell_time_s: u16,
    gps_status: GpsStatus,
    next_stop: Option<&shared::Stop>,
) -> u8 {
    use detection::probability;
    use crate::lut::{GAUSSIAN_LUT, LOGISTIC_LUT};

    let gps_status = match gps_status {
        GpsStatus::Valid => probability::GpsStatus::Valid,
        GpsStatus::DrOutage => probability::GpsStatus::DrOutage,
        GpsStatus::OffRoute => probability::GpsStatus::OffRoute,
    };

    probability::compute_arrival_probability_adaptive(
        signals, v_cms, stop, dwell_time_s, gps_status, &GAUSSIAN_LUT, &LOGISTIC_LUT, next_stop
    )
}

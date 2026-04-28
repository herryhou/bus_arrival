//! Arrival detection logic
//!
//! Provides stop corridor filtering and arrival probability computation.
//!
//! # Spatial Contract (F1 vs F3)
//!
//! This module intentionally uses **two spatial coordinate systems** for different features:
//!
//! - **F1 (distance likelihood)**: Uses `z_gps_cm` (raw GPS projection onto route)
//!   - Rationale: Measures "how close is the raw GPS fix to the stop?"
//!   - Captures GPS uncertainty directly
//!
//! - **F3 (progress likelihood)**: Uses `s_cm` (Kalman-filtered route position)
//!   - Rationale: Measures "how far along the route are we?"
//!   - Smooths out GPS noise for consistency
//!
//! This mixing is **intentional and bounded**: F1 and F3 capture complementary information
//! about arrival probability. The fallback logic (divergence > 2000cm) switches F1 from
//! `z_gps_cm` to `s_cm` to handle poor map matching — this is a controlled defensive
//! strategy, not ambiguous behavior.
#![allow(dead_code)]

use crate::lut::{GAUSSIAN_LUT, LOGISTIC_LUT};
use shared::{
    binfile::RouteData, probability_constants::*, PositionSignals, Prob8, SpeedCms, Stop,
};

// ===== Stop Corridor Filter =====

/// Find stops whose corridor contains the current route progress
/// no_std version - returns indices of active stops
pub fn find_active_stops(
    signals: PositionSignals,
    route_data: &RouteData,
) -> heapless::Vec<usize, 16> {
    let s_cm = signals.s_cm;
    let mut active = heapless::Vec::new();
    for i in 0..route_data.stop_count {
        if let Some(stop) = route_data.get_stop(i) {
            if s_cm >= stop.corridor_start_cm && s_cm <= stop.corridor_end_cm {
                if active.push(i).is_err() {
                    #[cfg(feature = "firmware")]
                    defmt::warn!("Too many active stops, ignoring overflow");
                    break;
                }
            }
        }
    }
    active
}

// ===== Arrival Probability Computation =====

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

/// Shared feature computation for arrival probability
///
/// # Spatial Contract
/// - F1 (p1): Uses `z_gps_cm` (raw GPS space) with fallback to `s_cm`
/// - F3 (p3): Uses `s_cm` (filtered route space)
///
/// This dual-space approach is intentional — see module-level docs.
fn compute_features(
    signals: PositionSignals,
    v_cms: SpeedCms,
    stop: &Stop,
    dwell_time_s: u16,
    gps_status: GpsStatus,
) -> (u32, u32, u32, u32) {
    // Feature 1: Distance likelihood (sigma_d = 2750 cm) - uses raw GPS
    // Defensive: blend z_gps_cm and s_cm based on divergence to handle
    // cases where map matcher produces poor projections during normal operation
    let divergence = signals.divergence_cm();
    let (d1_cm, use_fallback) = if gps_status == GpsStatus::Valid && divergence > 2000 {
        // When z_gps_cm and s_cm diverge significantly, use s_cm for p1
        // This prevents poor map matching from dragging down probability
        ((signals.s_cm - stop.progress_cm).abs(), true)
    } else {
        // Normal case: use z_gps_cm as per spec
        ((signals.z_gps_cm - stop.progress_cm).abs(), false)
    };
    let idx1 = ((d1_cm as i64 * 64) / SIGMA_D_CM as i64).min(255) as usize;
    let p1 = GAUSSIAN_LUT[idx1] as u32;

    // Feature 2: Speed likelihood (near 0 -> higher, v_stop = 200 cm/s)
    let idx2 = (v_cms / 10).max(0).min(SPEED_LUT_MAX_IDX as SpeedCms) as usize;
    let p2 = LOGISTIC_LUT[idx2] as u32;

    // Feature 3: Progress difference likelihood (sigma_p = 2000 cm) - uses Kalman output
    // Neutralize to 128 during dr_outage or off_route when s_cm may be phantom
    let p3 = if gps_status != GpsStatus::Valid && divergence > PHANTOM_DIVERGENCE_CM {
        128 // neutral: neither confirms nor denies arrival
    } else {
        let d_cm = (signals.s_cm - stop.progress_cm).abs();
        let idx3 = ((d_cm as i64 * 64) / SIGMA_P_CM as i64).min(255) as usize;
        GAUSSIAN_LUT[idx3] as u32
    };

    // Feature 4: Dwell time likelihood (T_ref = 10s)
    let p4 = ((dwell_time_s as u32) * 255 / 10).min(255) as u32;

    (p1, p2, p3, p4)
}

/// Compute arrival probability using LUTs (no_std compatible)
pub fn compute_arrival_probability(
    signals: PositionSignals,
    v_cms: SpeedCms,
    stop: &Stop,
    dwell_time_s: u16,
    gps_status: GpsStatus,
) -> Prob8 {
    let (p1, p2, p3, p4) = compute_features(signals, v_cms, stop, dwell_time_s, gps_status);
    ((13 * p1 + 6 * p2 + 10 * p3 + 3 * p4) / 32) as u8
}

/// Compute arrival probability with adaptive weights for close stops.
///
/// When next sequential stop is < 120m away, removes dwell time (p4)
/// weight and redistributes: (14, 7, 11, 0) instead of (13, 6, 10, 3).
pub fn compute_arrival_probability_adaptive(
    signals: PositionSignals,
    v_cms: SpeedCms,
    stop: &Stop,
    dwell_time_s: u16,
    gps_status: GpsStatus,
    next_stop: Option<&Stop>,
) -> Prob8 {
    // Full suppression during off-route - probability is meaningless when not on route
    if gps_status == GpsStatus::OffRoute {
        return 0;
    }

    let (p1, p2, p3, p4) = compute_features(signals, v_cms, stop, dwell_time_s, gps_status);

    // Adaptive weights based on next stop distance
    let (w1, w2, w3, w4) = if let Some(next) = next_stop {
        let dist_to_next = (next.progress_cm - stop.progress_cm).abs();
        if dist_to_next < 12_000 {
            // Close stop: remove p4, scale remaining to sum=32
            (14, 7, 11, 0)
        } else {
            // Normal stop: standard weights
            (13, 6, 10, 3)
        }
    } else {
        // Last stop: standard weights
        (13, 6, 10, 3)
    };

    ((w1 * p1 + w2 * p2 + w3 * p3 + w4 * p4) / 32) as u8
}

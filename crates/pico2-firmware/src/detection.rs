//! Arrival detection logic
//!
//! Provides stop corridor filtering and arrival probability computation.

use crate::lut::{GAUSSIAN_LUT, LOGISTIC_LUT};
use shared::{binfile::RouteData, DistCm, Prob8, SpeedCms, Stop};

// ===== Stop Corridor Filter =====

/// Find stops whose corridor contains the current route progress
/// no_std version - returns indices of active stops
pub fn find_active_stops(s_cm: DistCm, route_data: &RouteData) -> heapless::Vec<usize, 16> {
    let mut active = heapless::Vec::new();
    for i in 0..route_data.stop_count {
        if let Some(stop) = route_data.get_stop(i) {
            if s_cm >= stop.corridor_start_cm && s_cm <= stop.corridor_end_cm {
                if active.push(i).is_err() {
                    defmt::warn!("Too many active stops, ignoring overflow");
                    break;
                }
            }
        }
    }
    active
}

// ===== Arrival Probability Computation =====

/// Compute arrival probability using LUTs (no_std compatible)
pub fn compute_arrival_probability(
    s_cm: DistCm,
    v_cms: SpeedCms,
    stop: &Stop,
    dwell_time_s: u16,
) -> Prob8 {
    // Feature 1: Distance likelihood (sigma_d = 2750 cm)
    let d_cm = (s_cm - stop.progress_cm).abs();
    let idx1 = ((d_cm as i64 * 64) / 2750).min(255) as usize;
    let p1 = GAUSSIAN_LUT[idx1] as u32;

    // Feature 2: Speed likelihood (near 0 → higher, v_stop = 200 cm/s)
    let idx2 = (v_cms / 10).max(0).min(127) as usize;
    let p2 = LOGISTIC_LUT[idx2] as u32;

    // Feature 3: Progress difference likelihood (sigma_p = 2000 cm)
    let idx3 = ((d_cm as i64 * 64) / 2000).min(255) as usize;
    let p3 = GAUSSIAN_LUT[idx3] as u32;

    // Feature 4: Dwell time likelihood (T_ref = 10s)
    let p4 = ((dwell_time_s as u32) * 255 / 10).min(255) as u32;

    // Weighted sum: (13p₁ + 6p₂ + 10p₃ + 3p₄) / 32
    ((13 * p1 + 6 * p2 + 10 * p3 + 3 * p4) / 32) as u8
}

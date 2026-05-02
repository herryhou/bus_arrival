//! Kalman filter and GPS processing pipeline
//!
//! This module provides the main GPS processing pipeline including:
//! - Off-route detection with hysteresis (see `hysteresis` module)
//! - Speed and monotonicity filters (see `filters` module)
//! - 1D Kalman filter for route progress estimation
//! - Dead-reckoning for GPS outage compensation

mod hysteresis;
mod filters;

use core::cmp::Ord;

use crate::route_data::RouteData;
use shared::{DistCm, DrState, GpsPoint, KalmanState, PositionSignals, SpeedCms};

// Re-export constants and types from submodules
pub use hysteresis::{
    OFF_ROUTE_D2_THRESHOLD, OFF_ROUTE_CLEAR_TICKS, OFF_ROUTE_CONFIRM_TICKS,
    OffRouteStatus, update_off_route_hysteresis, reset_off_route_state,
};

pub use filters::{
    V_MAX_CMS, SIGMA_GPS_CM,
    check_speed_constraint, check_monotonic, update_dr_ema,
};

/// DR decay factors: (9/10)^dt * 10000 for integer arithmetic
const DR_DECAY_NUMERATOR: [u32; 11] = [
    10000, // dt=0: 1.0
    9000,  // dt=1: 0.9
    8100,  // dt=2: 0.81
    7290,  // dt=3: 0.729
    6561,  // dt=4: 0.6561
    5905,  // dt=5: 0.5905
    5314,  // dt=6: 0.5314
    4783,  // dt=7: 0.4783
    4305,  // dt=8: 0.4305
    3874,  // dt=9: 0.3874
    3487,  // dt=10: 0.3487
];

/// ProcessResult from GPS update
pub enum ProcessResult {
    Valid {
        signals: PositionSignals,
        v_cms: SpeedCms,
        seg_idx: usize,
        snapped: bool,
    },
    Rejected(&'static str),
    Outage,
    DrOutage {
        s_cm: DistCm,
        v_cms: SpeedCms,
    },
    /// GPS is off-route — position frozen, awaiting re-acquisition
    OffRoute {
        last_valid_s: DistCm,
        last_valid_v: SpeedCms,
        freeze_time: u64,
    },
    /// GPS is suspect off-route — position frozen, awaiting confirmation
    SuspectOffRoute {
        s_cm: DistCm,
        v_cms: SpeedCms,
    },
}

/// Main processing pipeline for each GPS update
pub fn process_gps_update(
    state: &mut KalmanState,
    dr: &mut DrState,
    gps: &GpsPoint,
    route_data: &RouteData,
    _current_time: u64,
    is_first_fix: bool,
    current_stop_idx: u8,
) -> ProcessResult {
    // 1. Check for GPS outage
    if !gps.has_fix {
        return handle_outage(state, dr, gps.timestamp);
    }

    // Calculate time delta since last GPS update
    let dt = match dr.last_gps_time {
        Some(t) => (gps.timestamp.saturating_sub(t)) as i32,
        None => 1, // First fix
    };

    // 2. Convert GPS to absolute coordinates (relative to fixed origin 120E, 20N)
    // Route nodes are stored as absolute coordinates, so GPS must also be absolute
    let (gps_x, gps_y) = crate::map_match::latlon_to_cm_absolute_with_lat_avg(
        gps.lat,
        gps.lon,
        route_data.lat_avg_deg,
    );

    // 3. Map matching
    // Use relaxed heading threshold during recovery (first fix OR post-outage recovery)
    // This improves segment matching when GPS heading is unreliable after signal loss
    let use_relaxed_heading = is_first_fix || dr.in_recovery;

    // CRITICAL: Track frozen position BEFORE hysteresis update
    // This is used to detect the transition from OffRoute/Suspect to Normal for re-entry snap
    let had_frozen_position = state.frozen_s_cm.is_some();

    let (seg_idx, match_d2) = crate::map_match::find_best_segment_restricted(
        gps_x,
        gps_y,
        gps.heading_cdeg,
        gps.speed_cms,
        route_data,
        state.last_seg_idx,
        use_relaxed_heading,
    );

    // Off-route detection (only when not in warmup)
    // CRITICAL: Check BEFORE projection to prevent s_cm from advancing during detour
    if !is_first_fix {
        let off_route_status = update_off_route_hysteresis(
            state,
            match_d2,
            gps.timestamp,
            current_stop_idx,
        );

        match off_route_status {
            OffRouteStatus::OffRoute => {
                // Confirmed off-route: return with frozen position
                let frozen_s = state.frozen_s_cm.unwrap_or(state.s_cm);
                let freeze_time = state.off_route_freeze_time.unwrap_or(gps.timestamp);
                return ProcessResult::OffRoute {
                    last_valid_s: frozen_s,
                    last_valid_v: state.v_cms,
                    freeze_time,
                };
            }
            OffRouteStatus::Suspect => {
                // During off-route suspicion, skip projection and filters to prevent s_cm advance
                dr.last_gps_time = Some(gps.timestamp);
                return ProcessResult::SuspectOffRoute {
                    s_cm: state.frozen_s_cm.unwrap_or(state.s_cm),
                    v_cms: state.v_cms,
                };
            }
            OffRouteStatus::Normal => {
                // CRITICAL: Check if we just transitioned from OffRoute/Suspect to Normal
                // If so, snap to re-entry position immediately to avoid detecting stops during catch-up
                if had_frozen_position {
                    // Set recovery flag to enable relaxed heading filter during off-route re-entry
                    dr.in_recovery = true;

                    // Just transitioned from OffRoute/Suspect to Normal
                    let frozen_s = state.frozen_s_cm.unwrap_or(state.s_cm);

                    // Use relaxed heading grid search for off-route recovery
                    let (new_seg_idx, _new_match_d2) = crate::map_match::find_best_segment_grid_only(
                        gps_x,
                        gps_y,
                        gps.heading_cdeg,
                        gps.speed_cms,
                        route_data,
                        use_relaxed_heading,
                    );

                    // Project to route to get re-entry position
                    let z_reentry = crate::map_match::project_to_route(gps_x, gps_y, new_seg_idx, route_data);

                    // CRITICAL: Only snap if the re-entry position is reasonably close to the frozen position
                    // This prevents backward snaps which would cause false arrivals at intermediate stops
                    if z_reentry >= frozen_s {
                        // Safe to snap - position is forward
                        state.s_cm = z_reentry;
                        state.frozen_s_cm = None;
                        state.off_route_suspect_ticks = 0;
                        // Blend v_cms using EMA instead of hard assignment (M3 fix)
                        let v_gps = gps.speed_cms.max(0).min(V_MAX_CMS);
                        state.v_cms = state.v_cms + 3 * (v_gps - state.v_cms) / 10;
                        state.last_seg_idx = new_seg_idx;
                        dr.last_gps_time = Some(gps.timestamp);
                        dr.last_valid_s = state.s_cm;
                        dr.filtered_v = state.v_cms;
                        dr.in_recovery = false;

                        let signals = PositionSignals {
                            z_gps_cm: z_reentry,
                            s_cm: state.s_cm,
                        };
                        return ProcessResult::Valid {
                            signals,
                            v_cms: state.v_cms,
                            seg_idx: new_seg_idx,
                            snapped: true,
                        };
                    }
                    // If z_reentry is backward, fall through to normal processing
                }
                // Continue normal processing
            }
        }
    }

    // 4. Projection
    let z_raw = crate::map_match::project_to_route(gps_x, gps_y, seg_idx, route_data);

    // NOTE: §4.5 (GPS Jump Recovery) removed per Bug 2 fix
    // M12 recovery in state.rs handles all post-off-route recovery scenarios
    // The hysteresis logic handles the off-route → normal transition
    // M12 uses 4-feature scoring and receives clean input (z_raw, not pre-snapped)

    if is_first_fix {
        state.s_cm = z_raw;

        // Blend v_cms using EMA instead of hard assignment (M3 fix)
        let v_gps = gps.speed_cms.max(0).min(V_MAX_CMS);
        state.v_cms = state.v_cms + 3 * (v_gps - state.v_cms) / 10;
        state.last_seg_idx = seg_idx;
        dr.last_gps_time = Some(gps.timestamp);
        dr.last_valid_s = state.s_cm;
        dr.filtered_v = state.v_cms;

        // Construct position signals for first fix
        let signals = PositionSignals {
            z_gps_cm: z_raw,
            s_cm: state.s_cm,
        };

        return ProcessResult::Valid {
            signals,
            v_cms: state.v_cms,
            seg_idx,
            snapped: false,
        };
    }

    // 5. Speed constraint filter
    // CRITICAL: Skip this check when position is frozen to allow off-route recovery
    // The snap logic handles validation of re-entry position
    // When frozen, we expect large position jumps (detour scenarios) and should
    // allow the snap logic to validate, not reject here
    if state.frozen_s_cm.is_none() {
        if !check_speed_constraint(z_raw, state.s_cm, dt) {
            // Per spec Section 9.2: "拒絕後的行為：跳過 Kalman 更新步驟，僅執行 predict step（ŝ += v̂），等效於短暫 Dead-Reckoning"
            // Do prediction step (DR mode) instead of returning Rejected with zero position
            state.s_cm += state.v_cms * (dt as DistCm);
            dr.last_gps_time = Some(gps.timestamp);
            return ProcessResult::DrOutage {
                s_cm: state.s_cm,
                v_cms: state.v_cms,
            };
        }
    }

    // 6. Monotonicity filter
    // CRITICAL: Skip this check when position is frozen to allow off-route recovery
    // The snap logic handles validation of re-entry position
    if state.frozen_s_cm.is_none() {
        if !check_monotonic(z_raw, state.s_cm) {
            // Per spec Section 9.2: same behavior as speed constraint rejection
            state.s_cm += state.v_cms * (dt as DistCm);
            dr.last_gps_time = Some(gps.timestamp);
            return ProcessResult::DrOutage {
                s_cm: state.s_cm,
                v_cms: state.v_cms,
            };
        }
    }

    // 7. Kalman update (HDOP-adaptive) with soft-resync for GPS recovery
    // H3: Apply soft-resync if in recovery mode (2/10 gain instead of full Kalman)
    // Per spec Section 11.3: conservative gain for BOTH position and velocity
    // to handle potentially noisy first post-outage GPS data
    if dr.in_recovery {
        // Soft resync for position: ŝ_resync = ŝ_DR + (2/10)*(z_gps - ŝ_DR)
        state.s_cm = state.s_cm + 2 * (z_raw - state.s_cm) / 10;

        // Soft resync for velocity: v_resync = v_DR + (2/10)*(v_gps - v_DR)
        // Using same conservative 2/10 gain for velocity during recovery
        state.v_cms = state.v_cms + 2 * (gps.speed_cms - state.v_cms) / 10;
        state.v_cms = state.v_cms.max(0);

        // Clear recovery flag after applying soft-resync
        dr.in_recovery = false;
    } else {
        // Normal Kalman update
        state.update_adaptive(z_raw, gps.speed_cms, gps.hdop_x10);
    }
    state.last_seg_idx = seg_idx;

    // Construct position signals with raw GPS and Kalman output
    let signals = PositionSignals {
        z_gps_cm: z_raw,  // Raw projection before Kalman
        s_cm: state.s_cm, // Kalman-filtered output
    };

    // 8. Update DR state
    // H4: Use EMA velocity filter instead of direct Kalman copy
    // Per spec Section 11.1: v_filtered(t) = v_filtered(t-1) + 3*(v_gps - v_filtered(t-1))/10
    dr.last_gps_time = Some(gps.timestamp);
    dr.last_valid_s = state.s_cm;
    dr.filtered_v = update_dr_ema(dr.filtered_v, gps.speed_cms);

    ProcessResult::Valid {
        signals,
        v_cms: state.v_cms,
        seg_idx,
        snapped: false,
    }
}

/// Handle GPS outage (max 10 seconds per spec Section 11.2)
fn handle_outage(state: &mut KalmanState, dr: &mut DrState, timestamp: u64) -> ProcessResult {
    let dt = match dr.last_gps_time {
        Some(t) => timestamp.saturating_sub(t),
        None => return ProcessResult::Rejected("no previous fix"),
    };

    if dt > 10 {
        // Set recovery flag even for long outages to allow relaxed heading filter
        // on first GPS fix after recovery. This improves map matching when GPS
        // heading is unreliable after extended signal loss.
        dr.in_recovery = true;
        return ProcessResult::Outage;
    }

    // H3: Set recovery flag - next valid GPS will need soft-resync
    dr.in_recovery = true;

    // Dead-reckoning: s(t) = s(t-1) + v_filtered * dt
    state.s_cm = dr.last_valid_s + dr.filtered_v * (dt as DistCm);

    // Speed decay normalized by dt: (9/10)^dt
    let dt_idx = dt.min(10) as usize;
    let decay_factor = DR_DECAY_NUMERATOR[dt_idx];
    dr.filtered_v = (dr.filtered_v as u32 * decay_factor / 10000) as SpeedCms;

    // Reset off-route counters on outage
    reset_off_route_state(state);

    ProcessResult::DrOutage {
        s_cm: state.s_cm,
        v_cms: dr.filtered_v,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== Test Constants =====

    /// Initial velocity for DR decay tests (10 m/s)
    const V_INITIAL: u32 = 1000;

    // ===== DR Decay Tests =====

    #[test]
    fn test_dr_decay_normalization() {
        // DR decay factors: (9/10)^dt * 10000 for integer arithmetic
        const EXPECTED_FACTORS: [u32; 11] = [
            10000, // dt=0: 1.0
            9000,  // dt=1: 0.9
            8100,  // dt=2: 0.81
            7290,  // dt=3: 0.729
            6561,  // dt=4: 0.6561
            5905,  // dt=5: 0.5905
            5314,  // dt=6: 0.5314
            4783,  // dt=7: 0.4783
            4305,  // dt=8: 0.4305
            3874,  // dt=9: 0.3874
            3487,  // dt=10: 0.3487
        ];

        // Verify LUT values match expected decay factors
        for (i, &expected) in EXPECTED_FACTORS.iter().enumerate() {
            assert_eq!(
                DR_DECAY_NUMERATOR[i], expected,
                "DR decay factor for dt={} should be {}",
                i, expected
            );
        }

        // Verify decay is normalized by dt (not constant)
        // dt=1: v = 1000 * 0.9 = 900
        let v_dt1 = (V_INITIAL * DR_DECAY_NUMERATOR[1] / 10000) as SpeedCms;
        assert_eq!(v_dt1, 900);

        // dt=2: v = 1000 * 0.81 = 810
        let v_dt2 = (V_INITIAL * DR_DECAY_NUMERATOR[2] / 10000) as SpeedCms;
        assert_eq!(v_dt2, 810);

        // dt=5: v = 1000 * 0.5905 = 590 (rounded)
        let v_dt5 = (V_INITIAL * DR_DECAY_NUMERATOR[5] / 10000) as SpeedCms;
        assert_eq!(v_dt5, 590);

        // Decay should be monotonic decreasing with dt
        assert!(
            v_dt1 > v_dt2 && v_dt2 > v_dt5,
            "DR decay should decrease monotonically with dt"
        );
    }
}

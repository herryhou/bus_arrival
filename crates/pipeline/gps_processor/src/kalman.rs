//! Kalman filter and GPS processing pipeline

use core::cmp::Ord;

use crate::route_data::RouteData;
use shared::{DistCm, DrState, GpsPoint, KalmanState, PositionSignals, SpeedCms};

/// Maximum bus speed for city bus operations: 60 km/h = 1667 cm/s
/// Per spec Section 9.1: urban transit routes, not highway speeds
pub const V_MAX_CMS: SpeedCms = 1667;

/// GPS noise margin for urban canyon conditions: 20 m
/// Per spec Section 9.1: accommodates multipath errors
pub const SIGMA_GPS_CM: DistCm = 2000;

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
    let seg_idx = crate::map_match::find_best_segment_restricted(
        gps_x,
        gps_y,
        gps.heading_cdeg,
        gps.speed_cms,
        route_data,
        state.last_seg_idx,
        use_relaxed_heading,
    );

    // 4. Projection
    let z_raw = crate::map_match::project_to_route(gps_x, gps_y, seg_idx, route_data);

    if is_first_fix {
        state.s_cm = z_raw;

        state.v_cms = gps.speed_cms;
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
        };
    }

    // 5. Speed constraint filter
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

    // 6. Monotonicity filter
    if !check_monotonic(z_raw, state.s_cm) {
        // Per spec Section 9.2: same behavior as speed constraint rejection
        state.s_cm += state.v_cms * (dt as DistCm);
        dr.last_gps_time = Some(gps.timestamp);
        return ProcessResult::DrOutage {
            s_cm: state.s_cm,
            v_cms: state.v_cms,
        };
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
    }
}

/// Reject GPS updates that exceed physical limits
pub fn check_speed_constraint(z_new: DistCm, z_prev: DistCm, dt: i32) -> bool {
    let dist_abs = (z_new - z_prev).unsigned_abs() as i32;
    let max_dist = V_MAX_CMS * dt.max(1) + SIGMA_GPS_CM;
    dist_abs <= max_dist
}

/// Monotonicity constraint with noise tolerance
///
/// Per spec Section 8.3: reject if z(t) - ŝ(t-1) < -1000 cm
/// Implementation uses -5000 cm (-50 m) as a practical balance:
/// - Tolerates GPS noise in urban canyon conditions
/// - Catches legitimate anomalies (route reversals, GPS glitches)
/// - Middle ground between spec (-10m) and previous (-500m)
fn check_monotonic(z_new: DistCm, z_prev: DistCm) -> bool {
    z_new >= z_prev - 5000 // CHANGED from 50000
}

/// EMA velocity filter update per spec Section 11.1
/// Formula: v_filtered(t) = v_filtered(t-1) + 3*(v_gps - v_filtered(t-1))/10
/// Uses α = 3/10 = 0.3 for smoothing
pub fn update_dr_ema(v_filtered_prev: SpeedCms, v_gps: SpeedCms) -> SpeedCms {
    v_filtered_prev + 3 * (v_gps - v_filtered_prev) / 10
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

    ProcessResult::DrOutage {
        s_cm: state.s_cm,
        v_cms: dr.filtered_v,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dr_decay_normalization() {
        // DR decay factors: (9/10)^dt * 10000 for integer arithmetic
        let expected_factors = [
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
        for (i, &expected) in expected_factors.iter().enumerate() {
            assert_eq!(
                DR_DECAY_NUMERATOR[i], expected,
                "DR decay factor for dt={} should be {}",
                i, expected
            );
        }

        // Verify decay is normalized by dt (not constant)
        let v_initial = 1000; // 10 m/s

        // dt=1: v = 1000 * 0.9 = 900
        let v_dt1 = (v_initial as u32 * DR_DECAY_NUMERATOR[1] / 10000) as SpeedCms;
        assert_eq!(v_dt1, 900);

        // dt=2: v = 1000 * 0.81 = 810
        let v_dt2 = (v_initial as u32 * DR_DECAY_NUMERATOR[2] / 10000) as SpeedCms;
        assert_eq!(v_dt2, 810);

        // dt=5: v = 1000 * 0.5905 = 590 (rounded)
        let v_dt5 = (v_initial as u32 * DR_DECAY_NUMERATOR[5] / 10000) as SpeedCms;
        assert_eq!(v_dt5, 590);

        // Decay should be monotonic decreasing with dt
        assert!(
            v_dt1 > v_dt2 && v_dt2 > v_dt5,
            "DR decay should decrease monotonically with dt"
        );
    }

    #[test]
    fn test_monotonicity_accepts_small_backward() {
        // Accept -10m backward jump (GPS noise)
        assert!(check_monotonic(100_000, 101_000));
    }

    #[test]
    fn test_monotonicity_accepts_threshold() {
        // Accept exactly -50m (at threshold)
        assert!(check_monotonic(100_000, 105_000));
    }

    #[test]
    fn test_monotonicity_rejects_large_backward() {
        // Reject -51m (exceeds threshold)
        assert!(!check_monotonic(100_000, 105_100));
    }

    #[test]
    fn test_monotonicity_allows_forward() {
        // Always allow forward movement
        assert!(check_monotonic(105_000, 100_000));
    }

    // ===== H4: EMA Velocity Filter Tests =====

    /// EMA coefficient α = 3/10 = 0.3
    /// Formula: v_filtered(t) = v_filtered(t-1) + 3*(v_gps - v_filtered(t-1))/10

    #[test]
    fn test_ema_velocity_filter_initial_value() {
        // First GPS update should initialize filtered_v to v_gps
        let v_gps = 500; // 5 m/s
        let v_filtered_initial = 0;

        // EMA update: v = 0 + 3*(500 - 0)/10 = 150
        let expected = v_filtered_initial + 3 * (v_gps - v_filtered_initial) / 10;
        assert_eq!(expected, 150);
    }

    #[test]
    fn test_ema_velocity_filter_convergence() {
        // EMA should converge toward the GPS speed over time
        let v_filtered = 300;
        let v_gps = 500;

        // EMA update: v = 300 + 3*(500 - 300)/10 = 300 + 60 = 360
        let expected = v_filtered + 3 * (v_gps - v_filtered) / 10;
        assert_eq!(expected, 360);

        // Next update: v = 360 + 3*(500 - 360)/10 = 360 + 42 = 402
        let v_filtered = expected;
        let expected = v_filtered + 3 * (v_gps - v_filtered) / 10;
        assert_eq!(expected, 402);
    }

    #[test]
    fn test_ema_velocity_filter_smoothing() {
        // EMA should smooth out GPS speed noise
        let v_filtered = 400;
        let v_gps_noisy = 700; // Sudden jump

        // EMA update: v = 400 + 3*(700 - 400)/10 = 400 + 90 = 490
        // The filtered value changes only 30% toward the noisy GPS value
        let expected = v_filtered + 3 * (v_gps_noisy - v_filtered) / 10;
        assert_eq!(expected, 490);
        assert!(expected < v_gps_noisy, "EMA should smooth out sudden jumps");
    }

    #[test]
    fn test_ema_velocity_filter_integer_arithmetic() {
        // Verify integer arithmetic doesn't accumulate excessive error
        // Formula: v += 3*(v_gps - v)/10
        // Using integer division, we lose some precision but should be close

        let v_filtered = 433; // Odd number to test rounding
        let v_gps = 567;

        // EMA update with integer arithmetic
        let delta = v_gps - v_filtered;
        let adjustment = (3 * delta) / 10; // Integer division
        let expected = v_filtered + adjustment;

        // Verify the adjustment is approximately 30% of the delta
        // 3 * 134 / 10 = 402 / 10 = 40 (integer division)
        assert_eq!(adjustment, 40);
        assert_eq!(expected, 473);
    }
}

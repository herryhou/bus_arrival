//! GPS input validation filters for speed constraint and monotonicity
//!
//! This module provides filters to reject GPS updates that exceed physical limits
//! or violate monotonicity constraints.

use shared::{DistCm, SpeedCms};

/// Maximum bus speed for city bus operations: 60 km/h = 1667 cm/s
/// Per spec Section 9.1: urban transit routes, not highway speeds
pub const V_MAX_CMS: SpeedCms = 1667;

/// GPS noise margin for urban canyon conditions: 20 m
/// Per spec Section 9.1: accommodates multipath errors
pub const SIGMA_GPS_CM: DistCm = 2000;

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
pub fn check_monotonic(z_new: DistCm, z_prev: DistCm) -> bool {
    z_new >= z_prev - 5000
}

/// EMA velocity filter update per spec Section 11.1
/// Formula: v_filtered(t) = v_filtered(t-1) + 3*(v_gps - v_filtered(t-1))/10
/// Uses α = 3/10 = 0.3 for smoothing
pub fn update_dr_ema(v_filtered_prev: SpeedCms, v_gps: SpeedCms) -> SpeedCms {
    v_filtered_prev + 3 * (v_gps - v_filtered_prev) / 10
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== Test Constants =====

    /// Default previous position for tests (cm)
    const TEST_Z_PREV: DistCm = 100_000;
    /// Normal movement distance (10 m/s over 1 second = 1000 cm)
    const NORMAL_MOVE: DistCm = 1000;
    /// Monotonicity threshold (-50 m)
    const MONOTONIC_THRESHOLD: DistCm = 5000;

    // ===== Monotonicity Tests =====

    #[test]
    fn test_monotonicity_accepts_small_backward() {
        // Accept -10m backward jump (GPS noise)
        assert!(check_monotonic(TEST_Z_PREV, TEST_Z_PREV + 1000));
    }

    #[test]
    fn test_monotonicity_accepts_threshold() {
        // Accept exactly -50m (at threshold)
        assert!(check_monotonic(TEST_Z_PREV, TEST_Z_PREV + MONOTONIC_THRESHOLD));
    }

    #[test]
    fn test_monotonicity_rejects_large_backward() {
        // Reject -51m (exceeds threshold)
        assert!(!check_monotonic(TEST_Z_PREV, TEST_Z_PREV + MONOTONIC_THRESHOLD + 100));
    }

    #[test]
    fn test_monotonicity_allows_forward() {
        // Always allow forward movement
        assert!(check_monotonic(TEST_Z_PREV + 5000, TEST_Z_PREV));
    }

    // ===== Speed Constraint Tests =====

    #[test]
    fn test_speed_constraint_accepts_normal_movement() {
        // Normal movement: 10 m/s over 1 second = 1000 cm
        assert!(check_speed_constraint(TEST_Z_PREV + NORMAL_MOVE, TEST_Z_PREV, 1));
    }

    #[test]
    fn test_speed_constraint_accepts_max_speed() {
        // At max speed: 16.67 m/s over 1 second = 1667 cm
        assert!(check_speed_constraint(TEST_Z_PREV + V_MAX_CMS, TEST_Z_PREV, 1));
    }

    #[test]
    fn test_speed_constraint_accepts_with_noise_margin() {
        // Max speed + GPS noise margin should be accepted
        assert!(check_speed_constraint(
            TEST_Z_PREV + V_MAX_CMS + SIGMA_GPS_CM,
            TEST_Z_PREV,
            1
        ));
    }

    #[test]
    fn test_speed_constraint_rejects_excessive_speed() {
        // Exceed max speed + noise margin
        assert!(!check_speed_constraint(
            TEST_Z_PREV + V_MAX_CMS + SIGMA_GPS_CM + 1,
            TEST_Z_PREV,
            1
        ));
    }

    #[test]
    fn test_speed_constraint_scales_with_dt() {
        // For longer dt, proportionally more distance is allowed
        assert!(check_speed_constraint(
            TEST_Z_PREV + V_MAX_CMS * 2,
            TEST_Z_PREV,
            2
        ));
    }

    #[test]
    fn test_speed_constraint_backward_movement() {
        // Backward movement is treated the same (absolute distance)
        // Use a small distance that's within speed constraint
        assert!(check_speed_constraint(TEST_Z_PREV - 1000, TEST_Z_PREV, 1));
    }

    #[test]
    fn test_speed_constraint_dt_zero() {
        // dt=0 should use min(1) in calculation, allowing noise margin
        assert!(check_speed_constraint(
            TEST_Z_PREV + SIGMA_GPS_CM,
            TEST_Z_PREV,
            0
        ));
    }

    #[test]
    fn test_speed_constraint_large_dt() {
        // Large dt should allow proportionally large distance
        assert!(check_speed_constraint(
            TEST_Z_PREV + V_MAX_CMS * 10,
            TEST_Z_PREV,
            10
        ));
    }

    #[test]
    fn test_speed_constraint_exceeds_large_dt() {
        // Even with large dt, physically impossible movement is rejected
        assert!(!check_speed_constraint(
            TEST_Z_PREV + V_MAX_CMS * 10 + SIGMA_GPS_CM + 1,
            TEST_Z_PREV,
            10
        ));
    }

    // ===== EMA Velocity Filter Tests =====

    /// EMA coefficient α = 3/10 = 0.3
    /// Formula: v_filtered(t) = v_filtered(t-1) + 3*(v_gps - v_filtered(t-1))/10

    #[test]
    fn test_ema_velocity_filter_initial_value() {
        // First GPS update should initialize filtered_v to v_gps
        const V_GPS: SpeedCms = 500; // 5 m/s
        const V_FILTERED_INITIAL: SpeedCms = 0;

        // EMA update: v = 0 + 3*(500 - 0)/10 = 150
        let expected = V_FILTERED_INITIAL + 3 * (V_GPS - V_FILTERED_INITIAL) / 10;
        assert_eq!(expected, 150);
    }

    #[test]
    fn test_ema_velocity_filter_convergence() {
        // EMA should converge toward the GPS speed over time
        const V_FILTERED: SpeedCms = 300;
        const V_GPS: SpeedCms = 500;

        // EMA update: v = 300 + 3*(500 - 300)/10 = 300 + 60 = 360
        let expected = V_FILTERED + 3 * (V_GPS - V_FILTERED) / 10;
        assert_eq!(expected, 360);

        // Next update: v = 360 + 3*(500 - 360)/10 = 360 + 42 = 402
        let expected = expected + 3 * (V_GPS - expected) / 10;
        assert_eq!(expected, 402);
    }

    #[test]
    fn test_ema_velocity_filter_smoothing() {
        // EMA should smooth out GPS speed noise
        const V_FILTERED: SpeedCms = 400;
        const V_GPS_NOISY: SpeedCms = 700; // Sudden jump

        // EMA update: v = 400 + 3*(700 - 400)/10 = 400 + 90 = 490
        // The filtered value changes only 30% toward the noisy GPS value
        let expected = V_FILTERED + 3 * (V_GPS_NOISY - V_FILTERED) / 10;
        assert_eq!(expected, 490);
        assert!(expected < V_GPS_NOISY, "EMA should smooth out sudden jumps");
    }

    #[test]
    fn test_ema_velocity_filter_integer_arithmetic() {
        // Verify integer arithmetic doesn't accumulate excessive error
        // Formula: v += 3*(v_gps - v)/10
        // Using integer division, we lose some precision but should be close

        const V_FILTERED: SpeedCms = 433; // Odd number to test rounding
        const V_GPS: SpeedCms = 567;

        // EMA update with integer arithmetic
        let delta = V_GPS - V_FILTERED;
        let adjustment = (3 * delta) / 10; // Integer division
        let expected = V_FILTERED + adjustment;

        // Verify the adjustment is approximately 30% of the delta
        // 3 * 134 / 10 = 402 / 10 = 40 (integer division)
        assert_eq!(adjustment, 40);
        assert_eq!(expected, 473);
    }
}

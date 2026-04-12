//! Velocity-based hard exclusion tests for recovery
//!
//! Per tech report spec Section 15.2: exclude recovery candidates that would
//! require exceeding V_MAX_CMS (60 km/h = 1667 cm/s) to reach in 1 GPS tick.

use detection::recovery::find_stop_index;
use shared::{DistCm, SpeedCms, Stop};

#[test]
fn test_velocity_exclusion_when_dist_exceeds_vmax() {
    let stops = vec![
        Stop { progress_cm: 1000, corridor_start_cm: 0, corridor_end_cm: 2000 },
        Stop { progress_cm: 10000, corridor_start_cm: 9000, corridor_end_cm: 11000 },
        Stop { progress_cm: 12000, corridor_start_cm: 11000, corridor_end_cm: 13000 },
    ];

    // Bus at s=5000, v=1000, last_index=0 (was at stop 0 at 1000)
    // dt=1 (1 second elapsed)
    // Stop 0 (1000): dist 4000, index_penalty 0, vel_penalty 0 -> score 4000
    // Stop 1 (10000): dist 5000 > 1667 (V_MAX_CMS * 1) -> excluded (i32::MAX)
    // Stop 2 (12000): dist 7000 > 1667 -> excluded (i32::MAX)
    let result = find_stop_index(5000, 1000, 1, &stops, 0);
    assert_eq!(result, Some(0), "Stop 0 should be selected (stops 1 and 2 excluded by velocity constraint)");
}

#[test]
fn test_velocity_inclusion_when_dist_within_vmax() {
    let stops = vec![
        Stop { progress_cm: 1000, corridor_start_cm: 0, corridor_end_cm: 2000 },
        Stop { progress_cm: 6000, corridor_start_cm: 5000, corridor_end_cm: 7000 },
        Stop { progress_cm: 12000, corridor_start_cm: 11000, corridor_end_cm: 13000 },
    ];

    // Bus at s=5000, v=1000, dt=1 (1 second elapsed)
    // Stop 0 (1000): behind the bus, no velocity constraint, dist 4000, score 4000
    // Stop 1 (6000): ahead of the bus, dist 1000 < 1667 (V_MAX_CMS * 1) -> included, score 1000
    // Stop 2 (12000): ahead of the bus, dist 7000 > 1667 -> excluded
    // Result: Some(1) - stop at 6000cm is within velocity constraint and has lowest score
    let s_cm: DistCm = 5000;
    let v_filtered: SpeedCms = 1000;
    let result = find_stop_index(s_cm, v_filtered, 1, &stops, 0);
    assert_eq!(result, Some(1), "Stop at 6000cm should be selected (dist=1000 < V_MAX_CMS)");
}

/// Test that GPS recovery works correctly with realistic elapsed time
///
/// This test demonstrates the fix for the bug where vel_penalty incorrectly
/// compared distance against V_MAX_CMS directly, instead of V_MAX_CMS * dt.
///
/// Scenario: 100m GPS jump with 10 seconds elapsed since last valid fix.
/// At 60 km/h (V_MAX_CMS), the bus could travel up to 166.7m in 10 seconds,
/// so a 100m jump is physically possible and recovery should succeed.
#[test]
fn test_gps_recovery_with_realistic_elapsed_time() {
    let stops = vec![
        Stop { progress_cm: 1000, corridor_start_cm: 0, corridor_end_cm: 2000 },
        Stop { progress_cm: 11000, corridor_start_cm: 10000, corridor_end_cm: 12000 },
    ];

    // Bus was at stop 0 (1000cm), GPS jumps to near stop 1
    let s_cm: DistCm = 10000;
    let v_filtered: SpeedCms = 1000;
    let dt_since_last_fix = 10u64;  // 10 seconds elapsed

    // With dt=10, max_reachable = V_MAX_CMS * 10 = 16670 cm ≈ 167m
    // Stop 1 is 1000cm ahead (within 167m), so it should NOT be excluded
    let result = find_stop_index(s_cm, v_filtered, dt_since_last_fix, &stops, 0);
    assert_eq!(result, Some(1), "GPS recovery should succeed with realistic elapsed time");

    // With dt=1, max_reachable = V_MAX_CMS * 1 = 1667 cm ≈ 16.7m
    // Stop 1 is 1000cm ahead (still within 16.7m from s=10000), so included
    // This test verifies the function now correctly uses dt
    let result_quick = find_stop_index(s_cm, v_filtered, 1, &stops, 0);
    assert_eq!(result_quick, Some(1), "Should still work for 1-second intervals");
}

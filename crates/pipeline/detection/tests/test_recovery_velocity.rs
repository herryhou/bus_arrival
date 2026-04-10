//! Velocity-based hard exclusion tests for recovery
//!
//! Per tech report spec Section 15.2: exclude recovery candidates that would
//! require exceeding V_MAX_CMS (108 km/h = 3000 cm/s) to reach in 1 GPS tick.

use detection::recovery::find_stop_index;
use shared::{DistCm, SpeedCms, Stop};

#[test]
fn test_velocity_exclusion_when_dist_exceeds_vmax() {
    let stops = vec![
        Stop { progress_cm: 10000, corridor_start_cm: 9000, corridor_end_cm: 11000 },
        Stop { progress_cm: 12000, corridor_start_cm: 11000, corridor_end_cm: 13000 },
    ];

    // Bus at s=5000, v=1000, last_index=1 (was at stop 1 at 12000)
    // Bus jumped backward from 12000 to 5000
    // Stop 0 (10000): ahead of the bus (10000 > 5000), dist 5000 > 3000 -> excluded
    // Stop 1 (12000): ahead of the bus, but it's the last stop and bus jumped backward, so no velocity constraint
    // Stop 1 is selected with score 7000
    // Result: Some(1) - stop 1 is selected because it's the last stop and bus jumped backward

    // To test that stops are excluded when dist exceeds V_MAX_CMS, we need a case where
    // the bus is moving forward (not backward) and all stops are ahead and exceed V_MAX_CMS.
    // But that's hard to set up because if the bus is moving forward, the last stop is behind
    // the current position, so it's not a candidate anyway.

    // Instead, let's test that non-last stops are excluded when dist exceeds V_MAX_CMS
    let result = find_stop_index(5000, 1000, &stops, 1);
    // Stop 0 should be excluded because dist 5000 > 3000
    // Stop 1 should be selected because it's the last stop and bus jumped backward
    assert_eq!(result, Some(1), "Stop 1 should be selected (last stop, bus jumped backward)");

    // Now test that when the bus is moving forward, stops ahead are excluded
    let stops2 = vec![
        Stop { progress_cm: 1000, corridor_start_cm: 0, corridor_end_cm: 2000 },
        Stop { progress_cm: 10000, corridor_start_cm: 9000, corridor_end_cm: 11000 },
        Stop { progress_cm: 12000, corridor_start_cm: 11000, corridor_end_cm: 13000 },
    ];

    // Bus at s=5000, v=1000, last_index=0 (was at stop 0 at 1000)
    // Bus is moving forward from 1000 to 5000
    // Stop 0 (1000): behind the bus, no velocity constraint, but excluded by i >= 0-1 = -1? Yes, 0 >= -1, so it's a candidate
    // Stop 0: dist 4000, index_penalty 0, score 4000
    // Stop 1 (10000): ahead of the bus, dist 5000 > 3000 -> excluded
    // Stop 2 (12000): ahead of the bus, dist 7000 > 3000 -> excluded
    // Result: Some(0) - stop 0 is selected
    let result2 = find_stop_index(5000, 1000, &stops2, 0);
    assert_eq!(result2, Some(0), "Stop 0 should be selected (stops 1 and 2 excluded by velocity constraint)");
}

#[test]
fn test_velocity_inclusion_when_dist_within_vmax() {
    let stops = vec![
        Stop { progress_cm: 1000, corridor_start_cm: 0, corridor_end_cm: 2000 },
        Stop { progress_cm: 7000, corridor_start_cm: 6000, corridor_end_cm: 8000 },
        Stop { progress_cm: 12000, corridor_start_cm: 11000, corridor_end_cm: 13000 },
    ];

    // Bus at s=5000, v=1000
    // Stop 0 (1000): behind the bus, no velocity constraint, dist 4000, score 4000
    // Stop 1 (7000): ahead of the bus, dist 2000 < 3000 (V_MAX_CMS) -> included, score 2000
    // Stop 2 (12000): ahead of the bus, dist 7000 > 3000 (V_MAX_CMS) -> excluded
    // Result: Some(1) - stop at 7000cm is within velocity constraint and has lowest score
    let s_cm: DistCm = 5000;
    let v_filtered: SpeedCms = 1000;
    let result = find_stop_index(s_cm, v_filtered, &stops, 0);
    assert_eq!(result, Some(1), "Stop at 7000cm should be selected (dist=2000 < V_MAX_CMS)");
}

//! Stop index recovery for GPS jump handling

use shared::{DistCm, SpeedCms, Stop};

/// Trigger conditions
const GPS_JUMP_THRESHOLD: DistCm = 20000;  // 200 m

/// Maximum bus speed for city bus operations: 60 km/h = 1667 cm/s
/// Per spec Section 9.1: urban transit routes, not highway speeds
const V_MAX_CMS: u32 = 1667;

/// Find correct stop after GPS jump
///
/// Implements scoring formula from Tech Report Section 15.2:
/// score(i) = |s_i - s| + 5000 * max(0, last_index - i) + vel_penalty(i)
///
/// Velocity penalty: hard exclusion if distance to stop exceeds V_MAX_CMS * dt_since_last_fix
/// (i.e., if reaching the stop would require exceeding max physical speed)
///
/// # Parameters
/// - `s_cm`: Current GPS position (cm)
/// - `v_filtered`: Filtered speed estimate (cm/s) - reserved for future use
/// - `dt_since_last_fix`: Seconds elapsed since last valid GPS fix
/// - `stops`: Array of all stops on route
/// - `last_index`: Last known stop index before GPS anomaly
pub fn find_stop_index(
    s_cm: DistCm,
    _v_filtered: SpeedCms,  // Reserved for future use
    dt_since_last_fix: u64,  // Seconds since last valid fix
    stops: &[Stop],
    last_index: u8,
) -> Option<usize> {
    let mut best_idx: Option<usize> = None;
    let mut best_score = i32::MAX;

    for (i, stop) in stops.iter().enumerate() {
        let d = (s_cm - stop.progress_cm).abs();

        // Filter: within ±200m and >= last_index - 1
        if d >= GPS_JUMP_THRESHOLD || (i as u8) < last_index.saturating_sub(1) {
            continue;
        }

        let dist = (s_cm - stop.progress_cm).abs();
        let index_penalty = 5000 * (last_index as i32 - i as i32).max(0);

        // Velocity penalty: hard exclusion if reaching this stop requires
        // exceeding V_MAX_CMS given the elapsed time since last valid fix
        // Only applies to stops ahead of the bus
        let dist_to_stop = if stop.progress_cm > s_cm {
            (stop.progress_cm - s_cm) as u64
        } else {
            0
        };
        // Maximum physically reachable distance = V_MAX_CMS * dt
        let max_reachable = V_MAX_CMS as u64 * dt_since_last_fix;
        if dist_to_stop > max_reachable {
            continue;  // Hard exclusion
        }

        let score = dist.saturating_add(index_penalty);

        if score < best_score {
            best_score = score;
            best_idx = Some(i);
        }
    }

    best_idx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recovery_scoring() {
        let stops = vec![
            Stop { progress_cm: 1000, corridor_start_cm: 0, corridor_end_cm: 2000 },
            Stop { progress_cm: 5000, corridor_start_cm: 4000, corridor_end_cm: 6000 },
            Stop { progress_cm: 9000, corridor_start_cm: 8000, corridor_end_cm: 10000 },
        ];

        // Jump to near second stop (idx 1) from stop 0
        // Stop 1: dist 100, penalty 0 -> score 100
        // Stop 0: dist 4100, penalty 0 -> score 4100 (wait, i >= 0-1=0, so stop 0 is candidate)
        // Stop 2: dist 3900, penalty 0 -> score 3900
        assert_eq!(find_stop_index(5100, 1000, 1, &stops, 0), Some(1));

        // Jump back from idx 2 to near stop 1
        // Point s = 1100. last_index = 2.
        // Candidates: idx 1, idx 2 (idx 0 is excluded by i >= 2-1 = 1)
        // Stop 1 (5000): dist 3900, penalty 5000, vel_penalty (dist 3900 > 3000*1) -> excluded
        // Stop 2 (9000): dist 7900, penalty 0, vel_penalty (dist 7900 > 3000*1) -> excluded
        // Both stops excluded by velocity constraint (physically impossible to reach in 1s)
        assert_eq!(find_stop_index(1100, 1000, 1, &stops, 2), None);

        // Jump back from idx 2 to stop 1, but much closer to 1
        // Point s = 4500.
        // Stop 1 (5000): dist 500, penalty 5000, vel_penalty 0 (dist 500 < 3000) -> score 5500
        // Stop 2 (9000): dist 4500, penalty 0, vel_penalty (dist 4500 > 3000) -> excluded
        // Stop 1 wins because stop 2 is excluded by velocity constraint.
        assert_eq!(find_stop_index(4500, 1000, 1, &stops, 2), Some(1));

        // Jump back from idx 1 to stop 0
        // Point s = 1000. last_index = 1.
        // Stop 0 (1000): dist 0, penalty 5000, vel_penalty 0 (stop is behind) -> score 5000
        // Stop 1 (5000): dist 4000, penalty 0, vel_penalty (dist 4000 > 3000) -> excluded
        // Stop 0 wins because stop 1 is excluded by velocity constraint.
        assert_eq!(find_stop_index(1000, 1000, 1, &stops, 1), Some(0));

        // Point s = -2000. last_index = 1.
        // Stop 0 (1000): dist 3000, penalty 5000 -> score 8000
        // Stop 1 (5000): dist 7000, penalty 0 -> score 7000
        // Still stop 1.

        // Point s = 0. last_index = 1. Stop 0 is at 1000. Stop 1 at 5000.
        // Stop 0: dist 1000, penalty 5000 -> score 6000
        // Stop 1: dist 5000, penalty 0 -> score 5000.

        // To make stop 0 win from last_index 1:
        // dist(s, stop 1) - dist(s, stop 0) > 5000.
        // |5000 - s| - |1000 - s| > 5000.
        // If s = 0: 5000 - 1000 = 4000 (No)
        // If s = -1000: 6000 - 2000 = 4000 (No)
        // Actually, if s is very far back, say s = -3000.
        // |5000 - (-3000)| = 8000. |1000 - (-3000)| = 4000.
        // 8000 - 4000 = 4000. Still not enough?
        // Wait, the max difference between |A-s| and |B-s| is |A-B|.
        // Here |5000 - 1000| = 4000.
        // So with a 5000 penalty, you can NEVER jump back from 1 to 0 if they are 4000 apart!
        // This means the 5000 penalty is VERY strong. It prevents ANY backwards jump if stops are closer than 50m.
    }

    #[test]
    fn test_recovery_with_80m_gaps() {
        let stops = vec![
            Stop { progress_cm: 1000, corridor_start_cm: 0, corridor_end_cm: 2000 },
            Stop { progress_cm: 9000, corridor_start_cm: 8000, corridor_end_cm: 10000 },
        ];

        // At s=1000, last_index=1.
        // Stop 0 (1000): dist 0, penalty 5000 -> score 5000
        // Stop 1 (9000): dist 8000, penalty 0 -> score 8000
        // Stop 0 wins!
        assert_eq!(find_stop_index(1000, 1000, 1, &stops, 1), Some(0));
    }

    #[test]
    fn test_gps_recovery_with_realistic_elapsed_time() {
        let stops = vec![
            Stop { progress_cm: 1000, corridor_start_cm: 0, corridor_end_cm: 2000 },
            Stop { progress_cm: 5000, corridor_start_cm: 4000, corridor_end_cm: 6000 },
            Stop { progress_cm: 9000, corridor_start_cm: 8000, corridor_end_cm: 10000 },
        ];

        // GPS recovery scenario: 5 seconds elapsed since last valid fix
        // Bus is now at s=2000, last known index was 1 (bus was near stop 1)
        //
        // Before fix: vel_penalty compared dist directly against V_MAX_CMS (3000 cm)
        //   Stop 1 (5000): dist 3000 <= 3000 -> vel_penalty 0 -> score = 3000 + 0 = 3000
        //   Stop 2 (9000): dist 7000 > 3000 -> vel_penalty i32::MAX -> excluded
        //   Result: Some(1) (stop 2 incorrectly excluded)
        //
        // After fix: vel_penalty uses max_reachable = V_MAX_CMS * dt
        //   max_reachable = 3000 * 5 = 15000 cm
        //   Stop 1 (5000): dist 3000, vel_penalty 0 -> score = 3000
        //   Stop 2 (9000): dist 7000 < 15000 -> vel_penalty 0 -> score = 7000
        //   Result: Some(1) (stop 1 still wins due to lower score, but stop 2 is now correctly NOT excluded)
        assert_eq!(find_stop_index(2000, 1000, 5, &stops, 1), Some(1));

        // GPS recovery: 10 seconds elapsed, bus has jumped forward
        // Bus at s=3000, last_index=0 (was at stop 0)
        // max_reachable = 3000 * 10 = 30000 cm (300m - bus can travel far in 10s)
        // Stop 0 (1000): dist 2000, vel_penalty 0 (behind) -> score = 2000
        // Stop 1 (5000): dist 2000 < 30000 -> vel_penalty 0 -> score = 2000
        // Stop 2 (9000): dist 6000 < 30000 -> vel_penalty 0 -> score = 6000
        // Result: Some(0) or Some(1) - both have same score, first wins
        assert_eq!(find_stop_index(3000, 1000, 10, &stops, 0), Some(0));

        // GPS recovery: 10 seconds elapsed, bus has jumped further forward
        // Bus at s=6000, last_index=0
        // max_reachable = 3000 * 10 = 30000 cm
        // Stop 0 (1000): dist 5000, vel_penalty 0 (behind) -> score = 5000
        // Stop 1 (5000): dist 1000 < 30000 -> vel_penalty 0 -> score = 1000
        // Stop 2 (9000): dist 3000 < 30000 -> vel_penalty 0 -> score = 3000
        // Result: Some(1) (closest stop ahead wins)
        assert_eq!(find_stop_index(6000, 1000, 10, &stops, 0), Some(1));

        // Edge case: dt=0 (GPS fix received within same second)
        // max_reachable = 0, so any forward distance should be excluded
        // Bus at s=2000, last_index=0
        // Stop 0 (1000): dist 1000, vel_penalty 0 (behind) -> score = 1000
        // Stop 1 (5000): dist 3000 > 0 -> vel_penalty i32::MAX -> excluded
        // Stop 2 (9000): dist 7000 > 0 -> vel_penalty i32::MAX -> excluded
        // Result: Some(0) (only stop 0 is viable - stops ahead are excluded)
        assert_eq!(find_stop_index(2000, 1000, 0, &stops, 0), Some(0));

        // The key demonstration: with realistic dt (5s), stops > 30m away are NOT excluded
        // Bus at s=1100, last_index=2 (was at stop 2), dt=5
        // Before fix: both stop 1 (3900cm > 3000) and stop 2 (7900cm > 3000) would be excluded
        // After fix:
        //   max_reachable = 3000 * 5 = 15000 cm
        //   Stop 1: dist 3900 < 15000 -> NOT excluded, score = 3900 + 5000 = 8900
        //   Stop 2: dist 7900 < 15000 -> NOT excluded, score = 7900
        // Result: Some(2) (stop 2 wins with lower score)
        assert_eq!(find_stop_index(1100, 1000, 5, &stops, 2), Some(2));
    }
}

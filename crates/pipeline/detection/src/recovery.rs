//! Stop index recovery for GPS jump handling

use shared::{DistCm, SpeedCms, Stop, FreezeContext};

/// Trigger conditions
const GPS_JUMP_THRESHOLD: DistCm = 30000;  // 300 m (increased for velocity constraint compatibility)

/// Maximum backward recovery distance (100 m)
/// Prevents pathological backward jumps while allowing legitimate re-selection
const MAX_BACKWARD_RECOVERY_CM: i64 = 100_00;

/// Maximum bus speed for city bus operations: 60 km/h = 1667 cm/s
/// Per spec Section 9.1: urban transit routes, not highway speeds
const V_MAX_CMS: u32 = 1667;

/// Minimum recovery rate (2 m/s = 200 cm/s)
/// Added as uncertainty buffer to velocity-derived reachable distance
const MIN_RECOVERY_RATE_CMS: i64 = 200;

/// Maximum base uncertainty term (200 m)
/// Caps the uncertainty buffer for very long outages to prevent it dominating velocity
const MAX_BASE_DISTANCE_CM: i64 = 200_00;

/// Maximum recovery distance cap (500 m)
/// Tuned for urban routes with ~100-200m stop spacing. Prevents search explosion.
const MAX_RECOVERY_DISTANCE_CM: i64 = 500_00;

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
/// - `v_filtered`: EMA-smoothed speed estimate (cm/s) from dr.filtered_v
///   This is the "clean" velocity signal without snap/re-entry artifacts
/// - `dt_since_last_fix`: Seconds elapsed since last valid GPS fix
/// - `stops`: Array of all stops on route
/// - `last_index`: Last known stop index before GPS anomaly
/// - `freeze_ctx`: Optional context from off-route freeze (C3 fix)
pub fn find_stop_index(
    s_cm: DistCm,
    v_filtered: SpeedCms,
    dt_since_last_fix: u64,  // Seconds since last valid fix
    stops: &[Stop],
    last_index: u8,
    freeze_ctx: &Option<FreezeContext>,  // C3: NEW PARAMETER
) -> Option<usize> {
    let mut best_idx: Option<usize> = None;
    let mut best_score = i32::MAX;

    // C3: Spatial anchor penalty - prefer stops at or after frozen position
    // Uses smooth piecewise linear penalty to avoid discontinuity
    let spatial_anchor_penalty = if let Some(ctx) = freeze_ctx {
        // Backward distance in cm
        let backward_cm = (ctx.frozen_s_cm - s_cm).max(0);
        // Absorb small GPS jitter (~2m)
        let backward_cm = backward_cm.saturating_sub(200);
        let backward_m = backward_cm / 100;

        if backward_m < 50 {
            // Smooth, low-gradient region
            5 * backward_m
        } else {
            // Stronger penalty beyond 50m
            250 + 20 * (backward_m - 50)
        }
    } else {
        0
    };

    for (i, stop) in stops.iter().enumerate() {
        let d = (s_cm - stop.progress_cm).abs();

        // Filter: within ±300m and >= last_index - 1
        if d >= GPS_JUMP_THRESHOLD || (i as u8) < last_index.saturating_sub(1) {
            continue;
        }

        let dist = (s_cm - stop.progress_cm).abs();
        let index_penalty = 5000 * (last_index as i32 - i as i32).max(0);

        // Backward constraint: prevent pathological far-backward jumps
        let backward_dist = if stop.progress_cm < s_cm {
            (s_cm - stop.progress_cm) as i64
        } else {
            0
        };
        if backward_dist > MAX_BACKWARD_RECOVERY_CM {
            continue;
        }

        // Velocity constraint: forward stops must be reachable
        let dist_to_stop = if stop.progress_cm > s_cm {
            (stop.progress_cm - s_cm) as i64
        } else {
            0
        };

        // Guard against dt=0 (GPS fixes within same second)
        let dt = dt_since_last_fix.max(1) as i64;

        // Cap velocity at V_MAX to prevent over-permissive search during GPS spikes
        let v_capped = (v_filtered as i64).min(V_MAX_CMS as i64);

        // Compute reachable distance as: velocity-derived + uncertainty buffer
        // - Dynamic component: actual motion (v_capped * dt)
        // - Base component: minimum uncertainty (2 m/s equivalent * dt), capped separately
        // This preserves velocity discrimination while ensuring a floor for low speeds
        let base = (MIN_RECOVERY_RATE_CMS * dt).min(MAX_BASE_DISTANCE_CM);
        let dynamic = v_capped * dt;

        // Global cap prevents search explosion after very long outages
        let max_reachable = (dynamic + base).min(MAX_RECOVERY_DISTANCE_CM);

        if dist_to_stop > max_reachable {
            continue;  // Hard exclusion
        }

        // Score combines: distance + index penalty + spatial anchor penalty
        // NOTE: All terms scaled to ~[0, 10k] range for balance
        let score = dist.saturating_add(index_penalty).saturating_add(spatial_anchor_penalty);

        if score < best_score {
            best_score = score;
            best_idx = Some(i);
        }
    }

    best_idx
}

#[cfg(all(test, feature = "std"))]
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
        assert_eq!(find_stop_index(5100, 1000, 1, &stops, 0, &None), Some(1));

        // Jump back from idx 2 to near stop 1
        // Point s = 1100. last_index = 2.
        // Candidates: idx 1, idx 2 (idx 0 is excluded by i >= 2-1 = 1)
        // Stop 1 (5000): dist 3900, penalty 5000, vel_penalty (dist 3900 > 1667*1) -> excluded
        // Stop 2 (9000): dist 7900, penalty 0, vel_penalty (dist 7900 > 1667*1) -> excluded
        // Both stops excluded by velocity constraint (physically impossible to reach in 1s)
        assert_eq!(find_stop_index(1100, 1000, 1, &stops, 2, &None), None);

        // Jump back from idx 2 to stop 1, but much closer to 1
        // Point s = 4500.
        // Stop 1 (5000): dist 500, penalty 5000, vel_penalty 0 (dist 500 < 1667) -> score 5500
        // Stop 2 (9000): dist 4500, penalty 0, vel_penalty (dist 4500 > 1667) -> excluded
        // Stop 1 wins because stop 2 is excluded by velocity constraint.
        assert_eq!(find_stop_index(4500, 1000, 1, &stops, 2, &None), Some(1));

        // Jump back from idx 1 to stop 0
        // Point s = 1000. last_index = 1.
        // Stop 0 (1000): dist 0, penalty 5000, vel_penalty 0 (stop is behind) -> score 5000
        // Stop 1 (5000): dist 4000, penalty 0, vel_penalty (dist 4000 > 1667) -> excluded
        // Stop 0 wins because stop 1 is excluded by velocity constraint.
        assert_eq!(find_stop_index(1000, 1000, 1, &stops, 1, &None), Some(0));

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
        assert_eq!(find_stop_index(1000, 1000, 1, &stops, 1, &None), Some(0));
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
        // M4 fix: uses actual filtered speed (1000) instead of V_MAX_CMS (3000)
        //   max_reachable = 1000 * 5 = 5000 cm
        //   Stop 1 (5000): dist 3000, vel_penalty 0 -> score = 3000
        //   Stop 2 (9000): dist 7000 > 5000 -> EXCLUDED by velocity penalty
        //   Result: Some(1) (stop 1 wins, stop 2 correctly excluded as beyond physical reach)
        assert_eq!(find_stop_index(2000, 1000, 5, &stops, 1, &None), Some(1));

        // GPS recovery: 10 seconds elapsed, bus has jumped forward
        // Bus at s=3000, last_index=0 (was at stop 0)
        // M4: max_reachable = 1000 * 10 = 10000 cm (100m at 10m/s for 10s)
        // Stop 0 (1000): dist 2000, vel_penalty 0 (behind) -> score = 2000
        // Stop 1 (5000): dist 2000 < 10000 -> vel_penalty 0 -> score = 2000
        // Stop 2 (9000): dist 6000 < 10000 -> vel_penalty 0 -> score = 6000
        // Result: Some(0) or Some(1) - both have same score, first wins
        assert_eq!(find_stop_index(3000, 1000, 10, &stops, 0, &None), Some(0));

        // GPS recovery: 10 seconds elapsed, bus has jumped further forward
        // Bus at s=6000, last_index=0
        // M4: max_reachable = 1000 * 10 = 10000 cm
        // Stop 0 (1000): dist 5000, vel_penalty 0 (behind) -> score = 5000
        // Stop 1 (5000): dist 1000 < 10000 -> vel_penalty 0 -> score = 1000
        // Stop 2 (9000): dist 3000 < 10000 -> vel_penalty 0 -> score = 3000
        // Result: Some(1) (closest behind wins)
        assert_eq!(find_stop_index(6000, 1000, 10, &stops, 0, &None), Some(1));

        // Edge case: dt=0 (GPS fix received within same second)
        // max_reachable = 0, so any forward distance should be excluded
        // Bus at s=2000, last_index=0
        // Stop 0 (1000): dist 1000, vel_penalty 0 (behind) -> score = 1000
        // Stop 1 (5000): dist 3000 > 0 -> vel_penalty i32::MAX -> excluded
        // Stop 2 (9000): dist 7000 > 0 -> vel_penalty i32::MAX -> excluded
        // Result: Some(0) (only stop 0 is viable - stops ahead are excluded)
        assert_eq!(find_stop_index(2000, 1000, 0, &stops, 0, &None), Some(0));

        // The key demonstration: with realistic dt (5s), stops beyond physical reach ARE excluded
        // Bus at s=1100, last_index=2 (was at stop 2), dt=5, v_filtered=1000
        // M4 fix: uses actual filtered speed (1000) instead of V_MAX_CMS (3000)
        //   max_reachable = 1000 * 5 = 5000 cm
        //   Stop 1: dist 3900 < 5000 -> NOT excluded, score = 3900 + 5000 = 8900
        //   Stop 2: dist 7900 > 5000 -> EXCLUDED (beyond physical reach in 5s at 10m/s)
        // Result: Some(1) (stop 1 wins - stop 2 correctly excluded by velocity constraint)
        assert_eq!(find_stop_index(1100, 1000, 5, &stops, 2, &None), Some(1));
    }
}

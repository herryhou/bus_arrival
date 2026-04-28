//! Recovery function with search window limitation

use crate::recovery::RecoveryInput;
use shared::{DistCm, Stop};

/// Maximum backward recovery distance (100 m)
const MAX_BACKWARD_RECOVERY_CM: i64 = 100_00;

/// Maximum bus speed for city bus operations: 60 km/h = 1667 cm/s
const V_MAX_CMS: u32 = 1667;

/// Minimum recovery rate (2 m/s = 200 cm/s)
const MIN_RECOVERY_RATE_CMS: i64 = 200;

/// Maximum base uncertainty term (200 m)
const MAX_BASE_DISTANCE_CM: i64 = 200_00;

/// Maximum recovery distance cap (500 m)
const MAX_RECOVERY_DISTANCE_CM: i64 = 500_00;

/// Find correct stop after GPS anomaly
///
/// Pure function with all inputs explicit. Uses scoring formula:
/// score(i) = |s_i - s| + 5000 × max(0, hint_idx - i) + spatial_anchor_penalty
///
/// # Parameters
/// - `input`: RecoveryInput with all required parameters
///
/// # Returns
/// - `Some(idx)`: Recovered stop index
/// - `None`: No valid stop found (recovery failed)
pub fn recover(input: RecoveryInput) -> Option<u8> {
    let mut best_idx: Option<usize> = None;
    let mut best_score = i32::MAX;

    // Spatial anchor penalty — prefer stops at or after frozen position
    let spatial_anchor_penalty = input.frozen_s_cm
        .map(|frozen| compute_spatial_anchor_penalty(input.s_cm, frozen))
        .unwrap_or(0);

    // Search window: hint_idx ± search_window
    let min_idx = input.hint_idx.saturating_sub(input.search_window);
    let max_idx = (input.hint_idx + input.search_window)
        .min(input.stops.len() as u8);

    for (i, stop) in input.stops.iter().enumerate() {
        // Skip if outside search window
        if (i as u8) < min_idx || (i as u8) > max_idx {
            continue;
        }

        let d = (input.s_cm - stop.progress_cm).abs();

        // Filter: within ±300m and ≥ hint_idx - 1
        if d >= 30000 || (i as u8) < input.hint_idx.saturating_sub(1) {
            continue;
        }

        // Backward constraint: prevent pathological far-backward jumps
        let backward_dist = if stop.progress_cm < input.s_cm {
            (input.s_cm - stop.progress_cm) as i64
        } else {
            0
        };
        if backward_dist > MAX_BACKWARD_RECOVERY_CM {
            continue;
        }

        // Velocity constraint: forward stops must be reachable
        let dist_to_stop = if stop.progress_cm > input.s_cm {
            (stop.progress_cm - input.s_cm) as i64
        } else {
            0
        };

        let dt = input.dt_seconds.max(1) as i64;
        let v_capped = (input.v_cms as i64).min(V_MAX_CMS as i64);

        // Compute reachable distance
        let base = (MIN_RECOVERY_RATE_CMS * dt).min(MAX_BASE_DISTANCE_CM);
        let dynamic = v_capped * dt;
        let max_reachable = (dynamic + base).min(MAX_RECOVERY_DISTANCE_CM);

        if dist_to_stop > max_reachable {
            continue;  // Hard exclusion
        }

        // Score: distance + index penalty + spatial anchor penalty
        let index_penalty = 5000 * (input.hint_idx as i32 - i as i32).max(0);
        let score = d.saturating_add(index_penalty)
                       .saturating_add(spatial_anchor_penalty as i32);

        if score < best_score {
            best_score = score;
            best_idx = Some(i);
        }
    }

    best_idx.map(|i| i as u8)
}

/// Compute spatial anchor penalty (smooth piecewise linear)
fn compute_spatial_anchor_penalty(s_cm: DistCm, frozen_s_cm: DistCm) -> i32 {
    let backward_cm = (frozen_s_cm - s_cm).max(0);
    let backward_cm = backward_cm.saturating_sub(200);  // Absorb 2m jitter
    let backward_m = backward_cm / 100;

    if backward_m < 50 {
        5 * backward_m
    } else {
        250 + 20 * (backward_m - 50)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use heapless::Vec;

    #[test]
    fn test_recovery_with_hint() {
        let stops = Vec::from_slice(&[
            Stop { progress_cm: 1000, corridor_start_cm: 0, corridor_end_cm: 2000 },
            Stop { progress_cm: 5000, corridor_start_cm: 4000, corridor_end_cm: 6000 },
            Stop { progress_cm: 9000, corridor_start_cm: 8000, corridor_end_cm: 10000 },
        ]).unwrap();

        let input = RecoveryInput {
            s_cm: 5100,
            v_cms: 1000,
            dt_seconds: 1,
            stops,
            hint_idx: 1,
            frozen_s_cm: None,
            search_window: 10,
        };

        assert_eq!(recover(input), Some(1));
    }

    #[test]
    fn test_recovery_with_spatial_anchor() {
        let stops = Vec::from_slice(&[
            Stop { progress_cm: 1000, corridor_start_cm: 0, corridor_end_cm: 2000 },
            Stop { progress_cm: 5000, corridor_start_cm: 4000, corridor_end_cm: 6000 },
        ]).unwrap();

        // Frozen at 5000, current at 4800 — should prefer stop 1
        // With dt_seconds=5 and v_cms=1000, max_reachable = 1000*5 + 200 = 5200 cm
        // Stop 1 is at 5000, which is 200 cm away (within reach)
        let input = RecoveryInput {
            s_cm: 4800,
            v_cms: 1000,
            dt_seconds: 5,
            stops,
            hint_idx: 1,
            frozen_s_cm: Some(5000),
            search_window: 10,
        };

        assert_eq!(recover(input), Some(1));
    }

    #[test]
    fn test_search_window_limitation() {
        let stops = Vec::from_slice(&[
            Stop { progress_cm: 1000, corridor_start_cm: 0, corridor_end_cm: 2000 },
            Stop { progress_cm: 2000, corridor_start_cm: 1000, corridor_end_cm: 3000 },
            Stop { progress_cm: 3000, corridor_start_cm: 2000, corridor_end_cm: 4000 },
            Stop { progress_cm: 4000, corridor_start_cm: 3000, corridor_end_cm: 5000 },
            Stop { progress_cm: 5000, corridor_start_cm: 4000, corridor_end_cm: 6000 },
        ]).unwrap();

        // hint_idx=2, search_window=1 → only search stops 1-3
        let input = RecoveryInput {
            s_cm: 4000,
            v_cms: 1000,
            dt_seconds: 1,
            stops,
            hint_idx: 2,
            frozen_s_cm: None,
            search_window: 1,
        };

        // Should find stop 3 (within window) not stop 4 (outside window)
        assert_eq!(recover(input), Some(3));
    }
}

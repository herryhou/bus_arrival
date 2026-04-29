//! Stop corridor filter
//!
//! # Architectural Assumptions
//!
//! Corridor-based detection assumes that `s_cm` (longitudinal route position)
//! has already been validated by map matching. This is a deliberate design choice:
//!
//! - **Lateral distance** is validated by `gps_processor::kalman::process_gps_update()`
//!   via spatial grid search with `d2` threshold
//! - **Heading constraints** are validated during map matching
//! - Corridor checking occurs AFTER map matching, so only longitudinal position is checked
//!
//! # Preconditions
//!
//! Functions in this module require `s_cm` to be a **map-matched position** from
//! `process_gps_update()`. Do NOT call with raw GPS projections or positions that
//! have not undergone lateral distance validation.
//!
//! # Off-Route Reentry Snapping
//!
//! When the bus returns from an off-route detour, `s_cm` may undergo **geometric snapping**:
//!
//! - Position transitions from frozen (`frozen_s_cm`) to snapped (`z_reentry`) in one tick
//! - This is a **discontinuous jump** in `s_cm` (bypasses normal Kalman smoothing)
//! - The snapped position is still map-matched (goes through `find_best_segment_grid_only()`)
//!
//! **Corridor impact**: A snap can jump OVER a corridor boundary, potentially:
//! - Skipping corridor entry (missed announcement)
//! - Jumping directly into the middle of a corridor
//!
//! The corridor check remains safe because snapped positions are still validated via
//! spatial grid search, but callers should be aware that `s_cm` can change discontinuously
//! during off-route recovery.

#[cfg(feature = "std")]
use shared::{DistCm, Stop};

/// Find stops whose corridor contains the current route progress.
///
/// # Preconditions
///
/// - `s_cm` MUST be a map-matched position from `process_gps_update()`
/// - Lateral distance must already be validated by spatial grid search
/// - Do NOT call with raw GPS projections (`z_gps_cm`)
///
/// # Why Only Longitudinal Check?
///
/// This function only checks `s_cm ∈ [corridor_start_cm, corridor_end_cm]`
/// because map matching already validated lateral distance. Adding lateral
/// distance checks here would be redundant and break during dead-reckoning
/// outages where `s_cm` is extrapolated from previous valid positions.
///
/// # Arguments
///
/// * `s_cm` - Map-matched route position in centimeters (from `process_gps_update()`)
/// * `stops` - Slice of stop definitions with corridor bounds
///
/// # Returns
///
/// Indices of stops whose corridor contains `s_cm`
#[cfg(feature = "std")]
pub fn find_active_stops(s_cm: DistCm, stops: &[Stop]) -> Vec<usize> {
    stops.iter()
        .enumerate()
        .filter(|(_, stop)| {
            s_cm >= stop.corridor_start_cm && s_cm <= stop.corridor_end_cm
        })
        .map(|(i, _)| i)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_active_stops() {
        let stops = vec![
            Stop { progress_cm: 10000, corridor_start_cm: 2000, corridor_end_cm: 14000 },
        ];
        assert!(find_active_stops(0, &stops).is_empty());
    }

    #[test]
    fn test_one_active_stop() {
        let stops = vec![
            Stop { progress_cm: 10000, corridor_start_cm: 2000, corridor_end_cm: 14000 },
        ];
        let result = find_active_stops(10000, &stops);
        assert_eq!(result, vec![0]);
    }

    // === Off-Route Reentry Snap Tests ===
    // These tests verify corridor behavior when s_cm undergoes geometric snapping
    // during off-route recovery (see kalman.rs:242-278 for snap implementation)

    #[test]
    fn test_snap_from_before_to_inside_corridor() {
        // Scenario: Bus was off-route before corridor, snaps directly into corridor
        // This simulates: frozen_s_cm=1000 → snapped s_cm=5000 (jumps over corridor_start_cm=2000)

        let stops = vec![
            Stop { progress_cm: 10000, corridor_start_cm: 2000, corridor_end_cm: 14000 },
        ];

        // Before snap: outside corridor
        let active_before = find_active_stops(1000, &stops);
        assert_eq!(active_before.len(), 0, "Should not be active before snap");

        // After snap: inside corridor (jumped from 1000 to 5000)
        let active_after = find_active_stops(5000, &stops);
        assert_eq!(active_after, vec![0], "Should be active after snap");
    }

    #[test]
    fn test_snap_over_corridor_entirely() {
        // Scenario: Bus snaps so quickly it jumps OVER the entire corridor
        // This simulates: frozen_s_cm=1000 → snapped s_cm=15000 (jumps over corridor [2000, 14000])

        let stops = vec![
            Stop { progress_cm: 10000, corridor_start_cm: 2000, corridor_end_cm: 14000 },
        ];

        // Before snap: outside corridor
        let active_before = find_active_stops(1000, &stops);
        assert_eq!(active_before.len(), 0, "Should not be active before snap");

        // After snap: jumped OVER corridor entirely
        let active_after = find_active_stops(15000, &stops);
        assert_eq!(active_after.len(), 0, "Should miss corridor when snap jumps over it");
    }

    #[test]
    fn test_snap_between_corridors() {
        // Scenario: Multiple stops, snap jumps from one corridor to another
        // Stop A corridor: [0, 4000], Stop B corridor: [6000, 10000]
        // Snap: 2000 (in A) → 7000 (in B)

        let stops = vec![
            Stop { progress_cm: 3000, corridor_start_cm: 0, corridor_end_cm: 4000 },
            Stop { progress_cm: 8000, corridor_start_cm: 6000, corridor_end_cm: 10000 },
        ];

        // Before snap: in Stop A's corridor
        let active_before = find_active_stops(2000, &stops);
        assert_eq!(active_before, vec![0], "Should be in Stop A corridor before snap");

        // After snap: in Stop B's corridor (discontinuous jump)
        let active_after = find_active_stops(7000, &stops);
        assert_eq!(active_after, vec![1], "Should be in Stop B corridor after snap");
    }

    #[test]
    fn test_snap_to_corridor_boundary() {
        // Scenario: Snap lands exactly on corridor boundary
        // Tests edge case: frozen_s_cm=0 → snapped s_cm=2000 (exactly at corridor_start_cm)

        let stops = vec![
            Stop { progress_cm: 10000, corridor_start_cm: 2000, corridor_end_cm: 14000 },
        ];

        // Snap to exact start boundary
        let active_at_start = find_active_stops(2000, &stops);
        assert_eq!(active_at_start, vec![0], "Should be active at exact start boundary");

        // Snap to exact end boundary
        let active_at_end = find_active_stops(14000, &stops);
        assert_eq!(active_at_end, vec![0], "Should be active at exact end boundary");
    }

    #[test]
    fn test_snap_during_ongoing_trip() {
        // Scenario: Realistic multi-stop route with snap during trip
        // Route: Stop0 (0m), Stop1 (500m), Stop2 (1000m)
        // Snap occurs at 300m → jumps to 700m

        let stops = vec![
            Stop { progress_cm: 0, corridor_start_cm: 0, corridor_end_cm: 20000 },
            Stop { progress_cm: 50000, corridor_start_cm: 30000, corridor_end_cm: 70000 },
            Stop { progress_cm: 100000, corridor_start_cm: 80000, corridor_end_cm: 120000 },
        ];

        // Before snap: approaching Stop1 (in corridor)
        let active_before = find_active_stops(30000, &stops);
        assert_eq!(active_before, vec![1], "Should be in Stop1 corridor before snap");

        // After snap: past Stop1, approaching Stop2 (in Stop2 corridor)
        let active_after = find_active_stops(90000, &stops);
        assert_eq!(active_after, vec![2], "Should be in Stop2 corridor after snap");
    }

    #[test]
    fn test_snap_does_not_create_duplicate_active_stops() {
        // Scenario: Ensure snap doesn't somehow create duplicate active stop indices
        // Corridors are non-overlapping in valid route data

        let stops = vec![
            Stop { progress_cm: 0, corridor_start_cm: 0, corridor_end_cm: 10000 },
            Stop { progress_cm: 20000, corridor_start_cm: 15000, corridor_end_cm: 25000 },
            Stop { progress_cm: 40000, corridor_start_cm: 35000, corridor_end_cm: 45000 },
        ];

        // Snap to various positions
        for snap_position in [0, 5000, 15000, 20000, 35000, 40000, 50000] {
            let active = find_active_stops(snap_position, &stops);
            assert!(
                active.len() <= 1,
                "Snap position {} should not create duplicate active stops, got {:?}",
                snap_position, active
            );
        }
    }
}

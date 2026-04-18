//! Off-route GPS jump recovery test
//!
//! Tests the "reboot" behavior when GPS returns from off-route detour.
//! When GPS appears at a stop location after being off-route, the system
//! should jump directly to that stop, not gradually progress through intermediate positions.
//!
//! Test scenario:
//! - Route with stops at s_cm: 10000, 50000, 100000, 150000, 180000
//! - Bus travels to stop 1 (s_cm=10000)
//! - Off-route detour begins, position freezes at s_cm=15000
//! - 60 seconds later, GPS appears at stop 5 (s_cm=180000)
//! - Expected: s_cm jumps from 15000 → 180000 (stops 2-4 skipped)

#![cfg(feature = "std")]

use gps_processor::kalman::{process_gps_update, ProcessResult};
use shared::{DrState, GpsPoint, KalmanState, RouteNode, SpatialGrid, Stop};

#[test]
fn test_off_route_jump_recovery_to_stop() {
    // Create a route with 5 stops along X-axis
    // Stop 0: s=10000, Stop 1: s=50000, Stop 2: s=100000
    // Stop 3: s=150000, Stop 4: s=180000
    let nodes = vec![
        RouteNode {
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            seg_len_mm: 50000, // 50m
            dx_cm: 5000,
            dy_cm: 0,
            heading_cdeg: 9000,
            _pad: 0,
        },
        RouteNode {
            x_cm: 5000,
            y_cm: 0,
            cum_dist_cm: 5000,
            seg_len_mm: 45000,
            dx_cm: 4500,
            dy_cm: 0,
            heading_cdeg: 9000,
            _pad: 0,
        },
        RouteNode {
            x_cm: 9500,
            y_cm: 0,
            cum_dist_cm: 9500,
            seg_len_mm: 40500,
            dx_cm: 4050,
            dy_cm: 0,
            heading_cdeg: 9000,
            _pad: 0,
        },
        RouteNode {
            x_cm: 13550,
            y_cm: 0,
            cum_dist_cm: 13550,
            seg_len_mm: 36450,
            dx_cm: 3645,
            dy_cm: 0,
            heading_cdeg: 9000,
            _pad: 0,
        },
        RouteNode {
            x_cm: 17195,
            y_cm: 0,
            cum_dist_cm: 17195,
            seg_len_mm: 32805,
            dx_cm: 3280,
            dy_cm: 0,
            heading_cdeg: 9000,
            _pad: 0,
        },
        RouteNode {
            x_cm: 20475,
            y_cm: 0,
            cum_dist_cm: 20475,
            seg_len_mm: 0,
            dx_cm: 0,
            dy_cm: 0,
            heading_cdeg: 9000,
            _pad: 0,
        },
    ];

    let stops = vec![
        Stop {
            progress_cm: 10000,
            corridor_start_cm: 5000,
            corridor_end_cm: 15000,
        },
        Stop {
            progress_cm: 50000,
            corridor_start_cm: 45000,
            corridor_end_cm: 55000,
        },
        Stop {
            progress_cm: 100000,
            corridor_start_cm: 95000,
            corridor_end_cm: 105000,
        },
        Stop {
            progress_cm: 150000,
            corridor_start_cm: 145000,
            corridor_end_cm: 155000,
        },
        Stop {
            progress_cm: 180000,
            corridor_start_cm: 175000,
            corridor_end_cm: 185000,
        },
    ];

    let grid = SpatialGrid {
        cells: vec![vec![0, 1, 2, 3], vec![0, 1, 2, 3]],
        grid_size_cm: 10000,
        cols: 4,
        rows: 2,
        x0_cm: 0,
        y0_cm: 0,
    };

    // Pack route data with stops
    let mut buffer = Vec::new();
    shared::binfile::pack_route_data(&nodes, &stops, &grid, 0.0, &mut buffer)
        .expect("Failed to pack test route data");

    let route_data = shared::binfile::RouteData::load(&buffer).expect("Failed to load route data");

    // Initialize state
    let mut state = KalmanState::new();
    let mut dr = DrState::new();

    // Phase 1: Travel to stop 1 (s_cm=10000)
    let init_gps = GpsPoint {
        timestamp: 1000,
        lat: 20.0,
        lon: 120.0,
        heading_cdeg: 9000,
        speed_cms: 500,
        hdop_x10: 10,
        has_fix: true,
    };

    let result = process_gps_update(&mut state, &mut dr, &init_gps, &route_data, 1000, true);
    assert!(matches!(result, ProcessResult::Valid { .. }));

    // Travel to stop 1
    for i in 1..=20 {
        let gps = GpsPoint {
            timestamp: 1000 + i,
            lat: 20.0 + (i as f64 * 0.00005), // Moving east
            lon: 120.0,
            heading_cdeg: 9000,
            speed_cms: 500,
            hdop_x10: 10,
            has_fix: true,
        };
        let _ = process_gps_update(&mut state, &mut dr, &gps, &route_data, 1000 + i, false);
    }

    let s_before_detour = state.s_cm;
    println!("Before detour: s_cm={}", s_before_detour);

    // Phase 2: Trigger off-route (5 ticks with poor GPS match)
    let off_route_s_cm = state.s_cm;
    for i in 1..=5 {
        let gps = GpsPoint {
            timestamp: 1020 + i,
            lat: 20.001, // 60m north (off-route)
            lon: 120.0,
            heading_cdeg: 9000,
            speed_cms: 500,
            hdop_x10: 10,
            has_fix: true,
        };

        let result = process_gps_update(&mut state, &mut dr, &gps, &route_data, 1020 + i, false);

        if i == 5 {
            // Should trigger OffRoute
            assert!(
                matches!(result, ProcessResult::OffRoute { .. }),
                "Tick {} should trigger OffRoute",
                i
            );
            if let ProcessResult::OffRoute { last_valid_s, .. } = result {
                // Position should be frozen
                assert_eq!(last_valid_s, off_route_s_cm, "Position should freeze at s_cm={}", off_route_s_cm);
            }
        }
    }

    // Simulate 60 seconds of off-route GPS (frozen position)
    let ts_after_detour = 1020 + 5 + 60;
    let frozen_s = state.s_cm;

    // Phase 3: GPS appears at stop 5 (s_cm=180000)
    // This should trigger GPS jump recovery
    let gps_at_stop5 = GpsPoint {
        timestamp: ts_after_detour,
        // GPS at stop 5's location (approximate lat/lon for s_cm=180000)
        lat: 20.0 + (180000.0 / 111000.0), // ~1.62° north
        lon: 120.0,
        heading_cdeg: 9000,
        speed_cms: 500,
        hdop_x10: 10,
        has_fix: true,
    };

    let result = process_gps_update(&mut state, &mut dr, &gps_at_stop5, &route_data, ts_after_detour, false);

    // Expected: GPS jump recovery should trigger
    // s_cm should jump from frozen_s to stop 5 (180000)
    // NOT gradually progress through intermediate positions

    // This test currently FAILS - GPS jump recovery is not implemented
    if let ProcessResult::Valid { signals, .. } = result {
        // Current behavior: s_cm doesn't jump to stop 5
        println!("Frozen s_cm: {}", frozen_s);
        println!("After detour recovery: s_cm={} (expected: 180000)", signals.s_cm);

        // This assertion will FAIL until GPS jump recovery is implemented
        assert_eq!(signals.s_cm, 180000, "s_cm should jump to stop 5 (180000) after GPS jump recovery");
        assert!(signals.s_cm > 150000, "s_cm should be past stops 2-4 (150000)");
    }
}

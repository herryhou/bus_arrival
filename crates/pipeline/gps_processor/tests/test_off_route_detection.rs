//! Off-route detection tests with hysteresis and warmup guard
//!
//! Per spec Section 12.2: Off-route detection requires 5 consecutive poor matches
//! to confirm off-route status, with 2 consecutive good matches to clear.
//! Detection is disabled during warmup (is_first_fix=true).

#![cfg(feature = "std")]

use gps_processor::kalman::{process_gps_update, ProcessResult};
use shared::{DrState, GpsPoint, KalmanState, RouteNode, SpatialGrid};

#[test]
fn test_off_route_confirms_after_5_ticks() {
    // Create a simple straight route along X-axis
    // Segment 0: (0, 0) to (10000, 0) - 100m east
    // Segment 1: (10000, 0) to (20000, 0) - 100m east
    let nodes = vec![
        RouteNode {
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            seg_len_mm: 100000, // 100m in mm
            dx_cm: 10000,       // 100m
            dy_cm: 0,
            heading_cdeg: 9000, // 90 degrees
            _pad: 0,
        },
        RouteNode {
            x_cm: 10000,
            y_cm: 0,
            cum_dist_cm: 10000,
            seg_len_mm: 100000, // 100m in mm
            dx_cm: 10000,       // 100m
            dy_cm: 0,
            heading_cdeg: 9000, // 90 degrees
            _pad: 0,
        },
        RouteNode {
            x_cm: 20000,
            y_cm: 0,
            cum_dist_cm: 20000,
            seg_len_mm: 0, // Last node
            dx_cm: 0,
            dy_cm: 0,
            heading_cdeg: 9000,
            _pad: 0,
        },
    ];

    let grid = SpatialGrid {
        cells: vec![vec![0, 1], vec![0, 1]], // 2x2 grid covering the route
        grid_size_cm: 10000,
        cols: 2,
        rows: 2,
        x0_cm: 0,
        y0_cm: 0,
    };

    // Pack route data
    let mut buffer = Vec::new();
    shared::binfile::pack_route_data(&nodes, &[], &grid, 0.0, &mut buffer)
        .expect("Failed to pack test route data");

    // Load route data
    let route_data = shared::binfile::RouteData::load(&buffer).expect("Failed to load route data");

    // Initialize state
    let mut state = KalmanState::new();
    let mut dr = DrState::new();

    // Initialize with a valid GPS fix on-route
    // GPS at (120°E, 20°N) should be close to our route at (0, 0)-(20000, 0)
    let init_gps = GpsPoint {
        timestamp: 1000,
        lat: 20.0,  // 20°N (matches origin)
        lon: 120.0, // 120°E (matches origin)
        heading_cdeg: 9000,
        speed_cms: 500,
        hdop_x10: 10,
        has_fix: true,
    };

    // First fix initializes state (warmup)
    let result = process_gps_update(&mut state, &mut dr, &init_gps, &route_data, 1000, true);
    assert!(matches!(result, ProcessResult::Valid { .. }));

    // Simulate 5 GPS fixes with poor match quality (> 50m from route)
    // GPS offset from route by ~60m to trigger off-route detection
    for i in 1..=5 {
        let gps = GpsPoint {
            timestamp: 1000 + i,
            lat: 20.0005, // ~60m north of route (1° ≈ 111km, so 0.0005° ≈ 55.5m)
            lon: 120.0,
            heading_cdeg: 9000,
            speed_cms: 500,
            hdop_x10: 10,
            has_fix: true,
        };

        let result = process_gps_update(&mut state, &mut dr, &gps, &route_data, 1000 + i, false);

        match i {
            1..=4 => {
                // First 4 ticks should NOT trigger OffRoute yet
                assert!(
                    !matches!(result, ProcessResult::OffRoute { .. }),
                    "Tick {} should NOT trigger OffRoute yet",
                    i
                );
                // During suspect state, should return DrOutage with frozen position
                assert!(
                    matches!(result, ProcessResult::DrOutage { .. }),
                    "Tick {} should return DrOutage (suspect state)",
                    i
                );
                // Verify suspect counter is incrementing
                assert_eq!(
                    state.off_route_suspect_ticks, i as u8,
                    "Suspect ticks should be {} after tick {}",
                    i, i
                );
            }
            5 => {
                // 5th tick SHOULD trigger OffRoute
                assert!(
                    matches!(result, ProcessResult::OffRoute { .. }),
                    "Tick 5 SHOULD trigger OffRoute"
                );
                if let ProcessResult::OffRoute { last_valid_s, last_valid_v, freeze_time: _ } = result {
                    // Should return last valid position (frozen)
                    assert_eq!(last_valid_s, state.s_cm);
                    assert_eq!(last_valid_v, state.v_cms);
                }
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn test_off_route_disabled_during_warmup() {
    // Create test route (same as above)
    let nodes = vec![
        RouteNode {
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            seg_len_mm: 100000,
            dx_cm: 10000,
            dy_cm: 0,
            heading_cdeg: 9000,
            _pad: 0,
        },
        RouteNode {
            x_cm: 10000,
            y_cm: 0,
            cum_dist_cm: 10000,
            seg_len_mm: 100000,
            dx_cm: 10000,
            dy_cm: 0,
            heading_cdeg: 9000,
            _pad: 0,
        },
        RouteNode {
            x_cm: 20000,
            y_cm: 0,
            cum_dist_cm: 20000,
            seg_len_mm: 0,
            dx_cm: 0,
            dy_cm: 0,
            heading_cdeg: 9000,
            _pad: 0,
        },
    ];

    let grid = SpatialGrid {
        cells: vec![vec![0, 1], vec![0, 1]],
        grid_size_cm: 10000,
        cols: 2,
        rows: 2,
        x0_cm: 0,
        y0_cm: 0,
    };

    let mut buffer = Vec::new();
    shared::binfile::pack_route_data(&nodes, &[], &grid, 0.0, &mut buffer)
        .expect("Failed to pack test route data");

    let route_data = shared::binfile::RouteData::load(&buffer).expect("Failed to load route data");

    let mut state = KalmanState::new();
    let mut dr = DrState::new();

    // Simulate 5 GPS fixes with poor match quality during warmup
    // is_first_fix=true means warmup is active
    for i in 1..=5 {
        let gps = GpsPoint {
            timestamp: 1000 + i,
            lat: 20.0005, // ~60m north of route
            lon: 120.0,
            heading_cdeg: 9000,
            speed_cms: 500,
            hdop_x10: 10,
            has_fix: true,
        };

        let result = process_gps_update(&mut state, &mut dr, &gps, &route_data, 1000 + i, true);

        // During warmup, OffRoute should NEVER trigger
        assert!(
            !matches!(result, ProcessResult::OffRoute { .. }),
            "Tick {} with is_first_fix=true should NOT trigger OffRoute",
            i
        );

        // Should always return Valid during warmup
        assert!(
            matches!(result, ProcessResult::Valid { .. }),
            "Tick {} should return Valid during warmup",
            i
        );

        // Verify suspect counter never increments during warmup
        assert_eq!(
            state.off_route_suspect_ticks, 0,
            "Suspect ticks should remain 0 during warmup (tick {})",
            i
        );
    }
}

#[test]
fn test_off_route_clears_after_2_good_ticks() {
    // Create test route
    let nodes = vec![
        RouteNode {
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            seg_len_mm: 100000,
            dx_cm: 10000,
            dy_cm: 0,
            heading_cdeg: 9000,
            _pad: 0,
        },
        RouteNode {
            x_cm: 10000,
            y_cm: 0,
            cum_dist_cm: 10000,
            seg_len_mm: 100000,
            dx_cm: 10000,
            dy_cm: 0,
            heading_cdeg: 9000,
            _pad: 0,
        },
        RouteNode {
            x_cm: 20000,
            y_cm: 0,
            cum_dist_cm: 20000,
            seg_len_mm: 0,
            dx_cm: 0,
            dy_cm: 0,
            heading_cdeg: 9000,
            _pad: 0,
        },
    ];

    let grid = SpatialGrid {
        cells: vec![vec![0, 1], vec![0, 1]],
        grid_size_cm: 10000,
        cols: 2,
        rows: 2,
        x0_cm: 0,
        y0_cm: 0,
    };

    let mut buffer = Vec::new();
    shared::binfile::pack_route_data(&nodes, &[], &grid, 0.0, &mut buffer)
        .expect("Failed to pack test route data");

    let route_data = shared::binfile::RouteData::load(&buffer).expect("Failed to load route data");

    let mut state = KalmanState::new();
    let mut dr = DrState::new();

    // Initialize with valid GPS
    let init_gps = GpsPoint {
        timestamp: 1000,
        lat: 20.0,  // 20°N (on route)
        lon: 120.0, // 120°E (on route)
        heading_cdeg: 9000,
        speed_cms: 500,
        hdop_x10: 10,
        has_fix: true,
    };
    let _ = process_gps_update(&mut state, &mut dr, &init_gps, &route_data, 1000, true);

    // Build up suspect ticks (3 ticks)
    for i in 1..=3 {
        let gps = GpsPoint {
            timestamp: 1000 + i,
            lat: 20.0005, // ~60m north of route
            lon: 120.0,
            heading_cdeg: 9000,
            speed_cms: 500,
            hdop_x10: 10,
            has_fix: true,
        };
        let _ = process_gps_update(&mut state, &mut dr, &gps, &route_data, 1000 + i, false);
    }
    assert_eq!(state.off_route_suspect_ticks, 3, "Should have 3 suspect ticks");

    // Now provide 2 good GPS fixes (close to route)
    for i in 4..=5 {
        let gps = GpsPoint {
            timestamp: 1000 + i,
            lat: 20.0,  // Back on route
            lon: 120.0,
            heading_cdeg: 9000,
            speed_cms: 500,
            hdop_x10: 10,
            has_fix: true,
        };
        let result = process_gps_update(&mut state, &mut dr, &gps, &route_data, 1000 + i, false);

        match i {
            4 => {
                // First good tick: still in suspect state (clear_ticks = 1 < 2)
                assert!(
                    matches!(result, ProcessResult::DrOutage { .. }),
                    "Tick 4 should return DrOutage (still suspect, clear_ticks=1)"
                );
            }
            5 => {
                // Second good tick: cleared (clear_ticks = 2)
                assert!(
                    matches!(result, ProcessResult::Valid { .. }),
                    "Tick 5 with good GPS should return Valid (cleared)"
                );
            }
            _ => unreachable!(),
        }
    }

    // After 2 good ticks, suspect counter should be cleared
    assert_eq!(state.off_route_suspect_ticks, 0, "Suspect ticks should be cleared after 2 good ticks");
    assert_eq!(state.off_route_clear_ticks, 2, "Clear ticks should be 2");
}

#[test]
fn test_off_route_hysteresis_partial_clear() {
    // Create test route
    let nodes = vec![
        RouteNode {
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            seg_len_mm: 100000,
            dx_cm: 10000,
            dy_cm: 0,
            heading_cdeg: 9000,
            _pad: 0,
        },
        RouteNode {
            x_cm: 10000,
            y_cm: 0,
            cum_dist_cm: 10000,
            seg_len_mm: 100000,
            dx_cm: 10000,
            dy_cm: 0,
            heading_cdeg: 9000,
            _pad: 0,
        },
        RouteNode {
            x_cm: 20000,
            y_cm: 0,
            cum_dist_cm: 20000,
            seg_len_mm: 0,
            dx_cm: 0,
            dy_cm: 0,
            heading_cdeg: 9000,
            _pad: 0,
        },
    ];

    let grid = SpatialGrid {
        cells: vec![vec![0, 1], vec![0, 1]],
        grid_size_cm: 10000,
        cols: 2,
        rows: 2,
        x0_cm: 0,
        y0_cm: 0,
    };

    let mut buffer = Vec::new();
    shared::binfile::pack_route_data(&nodes, &[], &grid, 0.0, &mut buffer)
        .expect("Failed to pack test route data");

    let route_data = shared::binfile::RouteData::load(&buffer).expect("Failed to load route data");

    let mut state = KalmanState::new();
    let mut dr = DrState::new();

    // Initialize with valid GPS
    let init_gps = GpsPoint {
        timestamp: 1000,
        lat: 20.0,  // 20°N (on route)
        lon: 120.0, // 120°E (on route)
        heading_cdeg: 9000,
        speed_cms: 500,
        hdop_x10: 10,
        has_fix: true,
    };
    let _ = process_gps_update(&mut state, &mut dr, &init_gps, &route_data, 1000, true);

    // Build up suspect ticks (4 ticks)
    for i in 1..=4 {
        let gps = GpsPoint {
            timestamp: 1000 + i,
            lat: 20.0005, // ~60m north of route
            lon: 120.0,
            heading_cdeg: 9000,
            speed_cms: 500,
            hdop_x10: 10,
            has_fix: true,
        };
        let _ = process_gps_update(&mut state, &mut dr, &gps, &route_data, 1000 + i, false);
    }
    assert_eq!(state.off_route_suspect_ticks, 4, "Should have 4 suspect ticks");

    // Only 1 good tick (not enough to clear)
    let gps = GpsPoint {
        timestamp: 1005,
        lat: 20.0,  // Back on route
        lon: 120.0,
        heading_cdeg: 9000,
        speed_cms: 500,
        hdop_x10: 10,
        has_fix: true,
    };
    let _ = process_gps_update(&mut state, &mut dr, &gps, &route_data, 1005, false);

    // Suspect counter should NOT be cleared yet (need 2 good ticks)
    assert_eq!(state.off_route_suspect_ticks, 4, "Suspect ticks should remain at 4 after only 1 good tick");
    assert_eq!(state.off_route_clear_ticks, 1, "Clear ticks should be 1");

    // Another bad tick should increment suspect to 5 and trigger OffRoute
    let gps = GpsPoint {
        timestamp: 1006,
        lat: 20.0005, // ~60m north of route again
        lon: 120.0,
        heading_cdeg: 9000,
        speed_cms: 500,
        hdop_x10: 10,
        has_fix: true,
    };
    let result = process_gps_update(&mut state, &mut dr, &gps, &route_data, 1006, false);
    assert!(matches!(result, ProcessResult::OffRoute { .. }), "Should trigger OffRoute after 5th bad tick");
}

#[test]
fn test_off_route_counter_resets_on_outage() {
    // Create test route
    let nodes = vec![
        RouteNode {
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            seg_len_mm: 100000,
            dx_cm: 10000,
            dy_cm: 0,
            heading_cdeg: 9000,
            _pad: 0,
        },
        RouteNode {
            x_cm: 10000,
            y_cm: 0,
            cum_dist_cm: 10000,
            seg_len_mm: 100000,
            dx_cm: 10000,
            dy_cm: 0,
            heading_cdeg: 9000,
            _pad: 0,
        },
        RouteNode {
            x_cm: 20000,
            y_cm: 0,
            cum_dist_cm: 20000,
            seg_len_mm: 0,
            dx_cm: 0,
            dy_cm: 0,
            heading_cdeg: 9000,
            _pad: 0,
        },
    ];

    let grid = SpatialGrid {
        cells: vec![vec![0, 1], vec![0, 1]],
        grid_size_cm: 10000,
        cols: 2,
        rows: 2,
        x0_cm: 0,
        y0_cm: 0,
    };

    let mut buffer = Vec::new();
    shared::binfile::pack_route_data(&nodes, &[], &grid, 0.0, &mut buffer)
        .expect("Failed to pack test route data");

    let route_data = shared::binfile::RouteData::load(&buffer).expect("Failed to load route data");

    let mut state = KalmanState::new();
    let mut dr = DrState::new();

    // Initialize with valid GPS
    let init_gps = GpsPoint {
        timestamp: 1000,
        lat: 20.0,  // 20°N (on route)
        lon: 120.0, // 120°E (on route)
        heading_cdeg: 9000,
        speed_cms: 500,
        hdop_x10: 10,
        has_fix: true,
    };
    let _ = process_gps_update(&mut state, &mut dr, &init_gps, &route_data, 1000, true);

    // Build up suspect count
    for i in 1..=3 {
        let gps = GpsPoint {
            timestamp: 1000 + i,
            lat: 20.0005, // ~60m north of route
            lon: 120.0,
            heading_cdeg: 9000,
            speed_cms: 500,
            hdop_x10: 10,
            has_fix: true,
        };
        let _ = process_gps_update(&mut state, &mut dr, &gps, &route_data, 1000 + i, false);
    }

    assert_eq!(state.off_route_suspect_ticks, 3, "Should have 3 suspect ticks");

    // Simulate GPS outage
    let outage_gps = GpsPoint {
        timestamp: 1004,
        lat: 20.0005,
        lon: 120.0,
        heading_cdeg: 9000,
        speed_cms: 500,
        hdop_x10: 10,
        has_fix: false,  // NO FIX
    };
    let _ = process_gps_update(&mut state, &mut dr, &outage_gps, &route_data, 1004, false);

    // Counters should be reset
    assert_eq!(state.off_route_suspect_ticks, 0, "Suspect ticks should be reset after outage");
    assert_eq!(state.off_route_clear_ticks, 0, "Clear ticks should be reset after outage");
}

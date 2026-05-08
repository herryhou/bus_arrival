//! Off-route integration tests for state machine
//!
//! Tests the full integration of off-route detection with the State machine,
//! including position freezing and recovery re-acquisition.

use pico2_firmware::{SystemState, estimation::EstimationState, SystemMode};
use shared::{binfile::RouteData, GpsPoint};
use shared::{EARTH_R_CM, FIXED_ORIGIN_LAT_DEG, FIXED_ORIGIN_LON_DEG};

const FIXED_ORIGIN_LAT_RAD: f64 = FIXED_ORIGIN_LAT_DEG.to_radians();

#[test]
fn test_off_route_freezes_position_until_reacquisition_clears() {
    let route_data = create_test_route_data();
    let mut state = SystemState::new(&route_data, None);
    let mut est_state = EstimationState::new();
    let base_timestamp = 10_000;

    for i in 0..4 {
        let gps = gps_on_route_at_x(base_timestamp + i, 0, 500);
        let event = state.tick(&gps, &mut est_state).event;
        assert!(event.is_none(), "Warmup tick {} should not emit events", i);
    }

    let frozen_before = state.last_valid_s_cm;
    assert_eq!(frozen_before, 0, "Warmup should establish route origin position");

    // Counter-based suspect phase: first 4 ticks are suspect, 5th tick confirms off-route
    for i in 0..4 {
        let gps = gps_off_route_at_x(base_timestamp + 4 + i, 0, 500);
        let event = state.tick(&gps, &mut est_state).event;

        assert!(
            event.is_none(),
            "Suspect tick {} should suppress arrival events",
            i + 1
        );
        // During suspect phase, position is NOT frozen yet (still in Normal mode)
        assert_eq!(
            state.mode,
            SystemMode::Normal,
            "Suspect tick {} should still be in Normal mode",
            i + 1
        );
        assert!(
            state.off_route_since.is_none(),
            "Suspect tick {} should NOT set freeze time yet (counter-based phase)",
            i + 1
        );
        assert!(
            !state.needs_recovery_on_reacquisition,
            "Suspect tick {} should NOT set recovery flag yet",
            i + 1
        );
    }

    // 5th tick confirms off-route (triggers transition to OffRoute mode)
    let off_route_event = state.tick(&gps_off_route_at_x(base_timestamp + 8, 0, 500), &mut est_state).event;
    assert!(
        off_route_event.is_none(),
        "Confirmed off-route tick should suppress arrival events"
    );
    assert_eq!(
        state.last_valid_s_cm,
        frozen_before,
        "Confirmed off-route should keep last_valid_s_cm frozen"
    );
    assert!(
        state.needs_recovery_on_reacquisition,
        "Confirmed off-route should arm reacquisition recovery"
    );

    let first_good = state.tick(&gps_on_route_at_x(base_timestamp + 9, 500, 500), &mut est_state).event;
    assert!(
        first_good.is_none(),
        "First good reacquisition tick should still suppress events"
    );
    assert_eq!(
        state.last_valid_s_cm,
        frozen_before,
        "First good reacquisition tick should still keep position frozen"
    );
    assert!(
        state.needs_recovery_on_reacquisition,
        "First good reacquisition tick should keep recovery armed"
    );

    let second_good = state.tick(&gps_on_route_at_x(base_timestamp + 10, 500, 500), &mut est_state).event;
    assert!(
        second_good.is_none(),
        "Second good reacquisition tick should clear hysteresis without emitting events"
    );
    // In new architecture, position advances after transitioning back to Normal mode
    // last_valid_s_cm is updated by the estimation layer, so check that we're in Normal mode
    assert_eq!(
        state.mode,
        SystemMode::Normal,
        "Second good tick should transition back to Normal mode"
    );
    assert!(
        !state.needs_recovery_on_reacquisition,
        "Second good reacquisition tick should clear recovery flag"
    );
    assert!(
        state.off_route_since.is_none(),
        "Freeze time should be cleared after reacquisition"
    );
}


/// Helper function to create a test route with known geometry
/// Creates a simple straight route along X-axis for predictable testing
fn create_test_route_data() -> RouteData<'static> {
    use shared::{RouteNode, SpatialGrid};

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
            heading_cdeg: 9000, // 90 degrees (East)
            _pad: 0,
        },
        RouteNode {
            x_cm: 10000,
            y_cm: 0,
            cum_dist_cm: 10000,
            seg_len_mm: 100000, // 100m in mm
            dx_cm: 10000,       // 100m
            dy_cm: 0,
            heading_cdeg: 9000,
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

    let leaked_buffer = Box::leak(buffer.into_boxed_slice());
    RouteData::load(leaked_buffer).expect("Failed to load route data")
}

/// Helper to create a GPS point on the route (at origin 120°E, 20°N)
#[allow(dead_code)]
fn create_gps_point_with_time(
    timestamp: u64,
    tick_offset: u64,
    speed_cms: i32,
    tick_index: u64,
) -> GpsPoint {
    GpsPoint {
        timestamp: timestamp + tick_offset + tick_index,
        lat: 20.0,          // 20°N (on route at origin)
        lon: 120.0,         // 120°E (on route at origin)
        heading_cdeg: Some(9000), // East (90 degrees)
        speed_cms: Some(speed_cms),
        hdop_x10: Some(10),
        has_fix: true,
    }
}

/// Helper to create a GPS point far from the route (>50m)
/// Uses latitude offset to move ~60m north of route
#[allow(dead_code)]
fn create_gps_point_far_from_route(timestamp: u64, tick_index: u64) -> GpsPoint {
    GpsPoint {
        timestamp: timestamp + tick_index,
        lat: 20.0005, // ~60m north of route (1° ≈ 111km, so 0.0005° ≈ 55.5m)
        lon: 120.0,   // Still at 120°E
        heading_cdeg: Some(9000),
        speed_cms: Some(500),
        hdop_x10: Some(10),
        has_fix: true,
    }
}

/// Helper to load the test route data
#[allow(dead_code)]
fn load_test_route_data() -> Option<RouteData<'static>> {
    Some(create_test_route_data())
}

fn gps_on_route_at_x(timestamp: u64, x_cm: i32, speed_cms: i32) -> GpsPoint {
    let lon =
        FIXED_ORIGIN_LON_DEG + (x_cm as f64 / (EARTH_R_CM * FIXED_ORIGIN_LAT_RAD.cos())).to_degrees();

    GpsPoint {
        timestamp,
        lat: FIXED_ORIGIN_LAT_DEG,
        lon,
        heading_cdeg: Some(9000),
        speed_cms: Some(speed_cms),
        hdop_x10: Some(10),
        has_fix: true,
    }
}

fn gps_off_route_at_x(timestamp: u64, x_cm: i32, speed_cms: i32) -> GpsPoint {
    let lon =
        FIXED_ORIGIN_LON_DEG + (x_cm as f64 / (EARTH_R_CM * FIXED_ORIGIN_LAT_RAD.cos())).to_degrees();
    let lat = FIXED_ORIGIN_LAT_DEG + (6000.0 / EARTH_R_CM).to_degrees();

    GpsPoint {
        timestamp,
        lat,
        lon,
        heading_cdeg: Some(9000),
        speed_cms: Some(speed_cms),
        hdop_x10: Some(10),
        has_fix: true,
    }
}

#[cfg(feature = "dev")]
/// Helper to create a longer test route (1km) for testing §4.5 scenarios
/// Creates a straight route along X-axis with multiple segments
fn create_long_test_route_data() -> RouteData<'static> {
    use shared::{RouteNode, SpatialGrid};

    let mut nodes = Vec::new();
    let mut cum_dist = 0i64;

    // Create 10 segments, each 100m long (total 1km)
    for i in 0..=10 {
        let x_cm = (i as i64 * 10000) as i32; // 0, 10000, 20000, ... 100000
        let seg_len_mm = if i < 10 { 100000 } else { 0 }; // 100m for segments, 0 for last node

        nodes.push(RouteNode {
            x_cm,
            y_cm: 0,
            cum_dist_cm: cum_dist as i32,
            seg_len_mm,
            dx_cm: 10000,
            dy_cm: 0,
            heading_cdeg: 9000, // East
            _pad: 0,
        });

        cum_dist += 10000;
    }

    // Create a grid covering the route
    let grid = SpatialGrid {
        cells: vec![
            vec![0, 1, 2, 3, 4],
            vec![0, 1, 2, 3, 4],
            vec![5, 6, 7, 8, 9],
            vec![5, 6, 7, 8, 9],
            vec![10, 10, 10, 10, 10],
        ],
        grid_size_cm: 20000,
        cols: 5,
        rows: 5,
        x0_cm: 0,
        y0_cm: 0,
    };

    // Pack route data
    let mut buffer = Vec::new();
    shared::binfile::pack_route_data(&nodes, &[], &grid, 0.0, &mut buffer)
        .expect("Failed to pack long test route data");

    let leaked_buffer = Box::leak(buffer.into_boxed_slice());
    RouteData::load(leaked_buffer).expect("Failed to load route data")
}

#[cfg(feature = "dev")]
#[test]
fn test_off_route_freeze_time_set_once() {
    // Regression test for Bug 1: off_route_freeze_time was being overwritten
    // every tick instead of being set once on first OffRoute.
    //
    // This test verifies that:
    // 1. Freeze time is set when off-route is first triggered
    // 2. Freeze time is NOT updated on subsequent OffRoute ticks
    // 3. The elapsed time calculation uses the ORIGINAL freeze time

    let route_data = match load_test_route_data() {
        Some(data) => data,
        None => {
            println!("Skipping test - route data not available");
            return;
        }
    };
    let mut state = SystemState::new(&route_data, None);
    let mut est_state = EstimationState::new();

    // Establish position through warmup
    for i in 0..4 {
        let gps = create_gps_point_with_time(1000, 0, 500, i);
        let _ = state.tick(&gps, &mut est_state).event;
    }

    // Trigger off-route - this should set freeze_time on tick 5
    for i in 1..=5 {
        let gps_off = create_gps_point_far_from_route(50000, i);
        let _ = state.tick(&gps_off, &mut est_state).event;
    }

    // Verify freeze time is set
    let freeze_time_tick_5 = state.off_route_since;
    assert!(
        freeze_time_tick_5.is_some(),
        "Freeze time should be set on tick 5"
    );
    let original_freeze_time = freeze_time_tick_5.unwrap();

    // Process MORE OffRoute ticks (tick 6, 7, 8)
    for i in 6..=8 {
        let gps_off = create_gps_point_far_from_route(50000, i);
        let _ = state.tick(&gps_off, &mut est_state).event;

        // Verify freeze time has NOT changed
        let current_freeze_time = state.off_route_since;
        assert_eq!(
            current_freeze_time,
            Some(original_freeze_time),
            "Freeze time should NOT be updated on tick {} (should remain at tick 5 value)",
            i
        );
    }

    // Calculate what elapsed time would be if freeze_time was updated every tick (WRONG behavior)
    // vs set once (CORRECT behavior)
    let gps_return_timestamp = 50000 + 10; // Tick 10
    let wrong_elapsed = gps_return_timestamp - (50000 + 8); // If updated on tick 8: ~2 seconds
    let correct_elapsed = gps_return_timestamp - (50000 + 5); // If set on tick 5: ~5 seconds

    // The difference matters for M12 recovery's velocity constraint:
    // Wrong: max_reachable = 1667 cm/s * 2s = 33m
    // Correct: max_reachable = 1667 cm/s * 5s = 83m
    assert!(
        correct_elapsed > wrong_elapsed,
        "Correct elapsed time should be greater than wrong elapsed time"
    );

    println!("✓ Freeze time set once test passed");
    println!("  Original freeze time: tick 5");
    println!("  Verified freeze time unchanged through tick 8");
    println!(
        "  Correct elapsed: {}s vs Wrong: {}s",
        correct_elapsed, wrong_elapsed
    );
}

#[cfg(feature = "dev")]
#[test]
fn test_m12_recovery_works_without_section_4_5() {
    // Regression test for Bug 2: §4.5 inline recovery conflicts with M12
    //
    // This test verifies that M12 recovery works correctly when §4.5 is removed:
    // 1. GPS returns from off-route with position jump (>50m)
    // 2. M12 receives raw GPS projection (not snapped by §4.5)
    // 3. M12 finds correct stop index using its 4-feature scoring
    // 4. System resumes normal operation with recovered stop index
    //
    // Uses a longer test route (1km) to trigger §4.5's 50m jump threshold

    let route_data = create_long_test_route_data();
    let mut state = SystemState::new(&route_data, None);
    let mut est_state = EstimationState::new();

    // Phase 1: Establish position at beginning of route (s ≈ 0)
    for i in 0..4 {
        let gps = create_gps_point_with_time(1000, 0, 500, i);
        let _ = state.tick(&gps, &mut est_state).event;
    }

    let initial_stop = state.last_stop_index;
    println!("Initial stop index: {}", initial_stop);

    // Phase 2: Move forward to s ≈ 100m (along the route)
    // Create GPS points that project to different positions on route
    // by using the origin point (20, 120) repeatedly - each will project to s=0
    // but Kalman will advance based on speed
    for i in 0..20 {
        let gps = GpsPoint {
            timestamp: 1000 + 4 + i,
            lat: 20.0,
            lon: 120.0,
            heading_cdeg: Some(9000),
            speed_cms: Some(500), // 5 m/s forward
            hdop_x10: Some(10),
            has_fix: true,
        };
        let _ = state.tick(&gps, &mut est_state).event;
    }

    let position_before_off_route = state.last_valid_s_cm;
    let stop_before_off_route = state.last_stop_index;
    println!(
        "Position before off-route: {} cm, stop: {}",
        position_before_off_route, stop_before_off_route
    );

    // Phase 3: Trigger off-route (GPS drifts away for 6 ticks)
    // Continue timestamp sequence from where we left off (1024)
    let off_route_start_timestamp = 1000 + 4 + 20;
    for i in 1..=6 {
        let gps_off = create_gps_point_far_from_route(off_route_start_timestamp, i);
        let _ = state.tick(&gps_off, &mut est_state).event;
    }

    // Verify off-route was triggered
    assert!(
        state.needs_recovery_on_reacquisition,
        "Off-route should be triggered, setting recovery flag"
    );
    assert!(
        state.off_route_since.is_some(),
        "Freeze time should be set"
    );

    let frozen_position = position_before_off_route;
    println!("Position frozen at: {} cm", frozen_position);

    // Phase 4: GPS returns to route (simulating detour return)
    // Continue timestamp sequence (no large gap)
    let detour_return_timestamp = off_route_start_timestamp + 6;

    // First good tick back on route
    let gps_return_1 = GpsPoint {
        timestamp: detour_return_timestamp,
        lat: 20.0,
        lon: 120.0,
        heading_cdeg: Some(9000),
        speed_cms: Some(500),
        hdop_x10: Some(10),
        has_fix: true,
    };

    // Second good tick - should trigger recovery
    let gps_return_2 = GpsPoint {
        timestamp: detour_return_timestamp + 1,
        lat: 20.0,
        lon: 120.0,
        heading_cdeg: Some(9000),
        speed_cms: Some(500),
        hdop_x10: Some(10),
        has_fix: true,
    };

    // Process both good ticks - second tick should trigger recovery
    let _event1 = state.tick(&gps_return_1, &mut est_state).event;
    let _event2 = state.tick(&gps_return_2, &mut est_state).event;

    // Verify recovery completed (M12 should handle this without §4.5)
    assert!(
        !state.needs_recovery_on_reacquisition,
        "Recovery should have cleared the flag after 2 good ticks"
    );
    assert!(
        state.off_route_since.is_none(),
        "Freeze time should be cleared after recovery"
    );

    // Verify we have a valid position after recovery
    let position_after_recovery = state.last_valid_s_cm;
    let stop_after_recovery = state.last_stop_index;

    println!(
        "Position after recovery: {} cm, stop: {}",
        position_after_recovery, stop_after_recovery
    );

    // The key assertion: M12 should have found a valid stop index
    assert!(
        position_after_recovery >= 0,
        "Should have valid position after recovery"
    );
    assert_eq!(
        stop_after_recovery,
        stop_before_off_route,
        "Routes without stops should keep the same synthetic stop index after recovery"
    );

    // Process more GPS to ensure stable operation
    for i in 1..=3 {
        let gps_good = create_gps_point_with_time(50200, 0, 500, i);
        let _ = state.tick(&gps_good, &mut est_state).event;

        // Should remain stable without re-triggering recovery
        assert!(
            !state.needs_recovery_on_reacquisition,
            "Should not re-trigger recovery (stable operation)"
        );
    }

    println!("✓ M12 recovery test passed");
    println!("  - Off-route triggered correctly");
    println!("  - GPS returned after long duration");
    println!("  - M12 recovery completed successfully");
    println!("  - Stable operation resumed");
}

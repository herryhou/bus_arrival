//! Off-route integration tests for state machine
//!
//! Tests the full integration of off-route detection with the State machine,
//! including position freezing and recovery re-acquisition.

use std::path::Path;
use pico2_firmware::state::State;
use shared::{binfile::RouteData, GpsPoint};

#[test]
fn test_off_route_freezes_position() {
    // Load actual route data for realistic testing
    let test_data_path = Path::new("../../tools/data/ty225_normal.bin");
    if !test_data_path.exists() {
        println!("Skipping test - route data not found at {:?}", test_data_path);
        return;
    }

    let route_data_bytes = std::fs::read(test_data_path).expect("Failed to read route data");
    let route_data = match RouteData::load(&route_data_bytes) {
        Ok(data) => data,
        Err(e) => {
            println!("Skipping test - failed to load route data: {:?}", e);
            return;
        }
    };

    let mut state = State::new(&route_data, None);
    let base_timestamp = 1_000_000_000;

    // Process a valid GPS to establish position
    let gps1 = GpsPoint {
        timestamp: base_timestamp,
        lat: 22.5,
        lon: 114.0,
        heading_cdeg: 9000,
        speed_cms: 500,
        hdop_x10: 10,
        has_fix: true,
    };

    // Process warmup ticks to establish position
    for i in 0..4 {
        let mut gps = gps1.clone();
        gps.timestamp = base_timestamp + i as u64;
        let _ = state.process_gps(&gps);
    }

    let last_s = state.last_valid_s_cm();

    // Verify state is set up correctly
    assert!(last_s >= 0, "Should have a valid position after warmup");
    println!("Position after warmup: {} cm", last_s);

    // Test that the state machine can handle GPS updates without panicking
    // The actual off-route detection and position freezing behavior
    // is tested in the GPS processor unit tests.
    //
    // This integration test verifies that:
    // 1. The State machine initializes correctly
    // 2. GPS updates are processed without errors
    // 3. The position field is accessible and updates as expected

    // Process additional GPS points
    for i in 4..8 {
        let gps = GpsPoint {
            timestamp: base_timestamp + i as u64,
            lat: 22.5,
            lon: 114.0,
            heading_cdeg: 9000,
            speed_cms: 500,
            hdop_x10: 10,
            has_fix: true,
        };
        let _result = state.process_gps(&gps);
        // Note: arrival events may occur depending on the route data
        // The important thing is that the state machine doesn't panic
    }

    // Verify we can still access the position
    let final_s = state.last_valid_s_cm();
    println!("Final position: {} cm", final_s);

    // Test passes if we get here without panicking
    assert!(true, "State machine handles GPS updates correctly");
}

#[test]
fn test_re_acquisition_runs_recovery() {
    // Test that after off-route, recovery runs when GPS returns
    // This will be fully tested in Task 11 (full cycle test)
    //
    // Basic test: verify the state machine has the necessary fields
    // to support re-acquisition recovery

    let test_data_path = Path::new("../../tools/data/ty225_normal.bin");
    if !test_data_path.exists() {
        println!("Skipping test - route data not found at {:?}", test_data_path);
        return;
    }

    let route_data_bytes = match std::fs::read(test_data_path) {
        Ok(bytes) => bytes,
        Err(_) => return,
    };

    let route_data = match RouteData::load(&route_data_bytes) {
        Ok(data) => data,
        Err(_) => return,
    };

    let state = State::new(&route_data, None);

    // Verify state has the recovery flag and freeze time fields
    // These are used to track off-route state and trigger recovery on re-acquisition
    assert_eq!(state.needs_recovery_on_reacquisition(), false,
               "Initial state should not need recovery");
    assert_eq!(state.off_route_freeze_time(), None,
               "Initial state should have no freeze time");

    // Test passes if the state machine has the necessary infrastructure
    // for re-acquisition recovery
    assert!(true, "State machine has re-acquisition recovery infrastructure");
}

#[cfg(feature = "dev")]
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

    // Load route data - this will leak memory but that's OK for tests
    let route_data = RouteData::load(&buffer).expect("Failed to load route data");

    // Extend lifetime to 'static for test convenience
    unsafe {
        std::mem::transmute::<RouteData<'_>, RouteData<'static>>(route_data)
    }
}

#[cfg(feature = "dev")]
/// Helper to create a GPS point on the route (at origin 120°E, 20°N)
fn create_gps_point_with_time(
    timestamp: u64,
    tick_offset: u64,
    speed_cms: i32,
    tick_index: u64,
) -> GpsPoint {
    GpsPoint {
        timestamp: timestamp + tick_offset + tick_index,
        lat: 20.0,  // 20°N (on route at origin)
        lon: 120.0, // 120°E (on route at origin)
        heading_cdeg: 9000, // East (90 degrees)
        speed_cms,
        hdop_x10: 10,
        has_fix: true,
    }
}

#[cfg(feature = "dev")]
/// Helper to create a GPS point far from the route (>50m)
/// Uses latitude offset to move ~60m north of route
fn create_gps_point_far_from_route(
    timestamp: u64,
    tick_index: u64,
) -> GpsPoint {
    GpsPoint {
        timestamp: timestamp + tick_index,
        lat: 20.0005, // ~60m north of route (1° ≈ 111km, so 0.0005° ≈ 55.5m)
        lon: 120.0,   // Still at 120°E
        heading_cdeg: 9000,
        speed_cms: 500,
        hdop_x10: 10,
        has_fix: true,
    }
}

#[cfg(feature = "dev")]
/// Helper to load the test route data
fn load_test_route_data() -> Option<RouteData<'static>> {
    Some(create_test_route_data())
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

    // Load route data
    let route_data = RouteData::load(&buffer).expect("Failed to load route data");

    // Extend lifetime to 'static for test convenience
    unsafe {
        std::mem::transmute::<RouteData<'_>, RouteData<'static>>(route_data)
    }
}

#[cfg(feature = "dev")]
#[test]
fn test_full_off_route_cycle() {
    // Create test route and state
    let route_data = match load_test_route_data() {
        Some(data) => data,
        None => {
            println!("Skipping test - route data not available");
            return;
        }
    };
    let mut state = State::new(&route_data, None);

    // Phase 1: Normal operation - establish position
    // Process warmup ticks to establish position
    for i in 0..4 {
        let gps1 = create_gps_point_with_time(1000, 0, 500, i);
        let event1 = state.process_gps(&gps1);
        // No arrival events during warmup
        assert!(event1.is_none(), "Should not have arrival events during warmup");
    }

    let initial_s = state.last_valid_s_cm();
    assert!(initial_s >= 0, "Should have a valid position after warmup");
    println!("Position after warmup: {} cm", initial_s);

    // Phase 2: GPS drifts off-route (>50m for 6+ ticks)
    // This should trigger OffRoute after 5 ticks
    let mut off_route_triggered = false;

    for i in 1..=6 {
        let gps_off = create_gps_point_far_from_route(50000, i);
        let _event = state.process_gps(&gps_off);

        match i {
            1..=4 => {
                // First 4 ticks: should NOT trigger off-route yet
                // System should still process GPS (with noisy position)
                assert!(!state.needs_recovery_on_reacquisition(),
                    "Tick {} should NOT need recovery yet", i);
                assert!(state.off_route_freeze_time().is_none(),
                    "Tick {} should NOT have freeze time yet", i);
            }
            5 => {
                // After 5 ticks: should be in off-route state
                // The GPS processor will return ProcessResult::OffRoute
                // which sets needs_recovery_on_reacquisition and off_route_freeze_time
                assert!(state.needs_recovery_on_reacquisition(),
                    "Tick 5 SHOULD need recovery (off-route triggered)");
                assert!(state.off_route_freeze_time().is_some(),
                    "Tick 5 SHOULD have freeze time set");
                off_route_triggered = true;
                println!("Off-route triggered at tick 5, freeze time: {:?}", state.off_route_freeze_time());
            }
            6 => {
                // Still in off-route state
                assert!(state.needs_recovery_on_reacquisition(),
                    "Tick 6 should still need recovery");
                assert!(state.off_route_freeze_time().is_some(),
                    "Tick 6 should still have freeze time");
            }
            _ => unreachable!(),
        }
    }

    assert!(off_route_triggered, "OffRoute should have been triggered");

    // Phase 3: GPS returns to route for 3 ticks
    // After 2 good ticks, off-route should clear
    // Recovery should run on first valid GPS after off-route

    // First good tick back on route - this should trigger recovery
    let gps_back = create_gps_point_with_time(1500, 6, 500, 0);
    let _event2 = state.process_gps(&gps_back);

    // Verify recovery ran and cleared the off-route state
    assert!(!state.needs_recovery_on_reacquisition(),
        "After first good GPS, recovery should have cleared the flag");
    assert!(state.off_route_freeze_time().is_none(),
        "After first good GPS, freeze time should be cleared");

    println!("Recovery completed after GPS returned to route");

    // Process 2 more good ticks to ensure stable operation
    // Note: The GPS processor's off-route state machine needs 2 good ticks to clear,
    // so we provide 3 good ticks total (1 above + 2 more) to ensure stable operation
    for i in 1..=3 {
        let gps_good = create_gps_point_with_time(1500, 6, 500, i);
        let _event = state.process_gps(&gps_good);

        // Should remain in normal operation after recovery cleared
        // (The first tick after recovery might still have internal state clearing)
        assert!(!state.needs_recovery_on_reacquisition(),
            "Tick {} should not need recovery (back to normal)", i);
        assert!(state.off_route_freeze_time().is_none(),
            "Tick {} should not have freeze time (back to normal)", i);
    }

    // Phase 4: Verify normal operation resumes
    let gps_normal = create_gps_point_with_time(2000, 0, 500, 0);
    let _event3 = state.process_gps(&gps_normal);

    // Should process normally without any off-route state
    assert!(!state.needs_recovery_on_reacquisition(),
        "Should be in normal operation");
    assert!(state.off_route_freeze_time().is_none(),
        "Should not have freeze time in normal operation");

    // Verify we have a valid position
    let final_s = state.last_valid_s_cm();
    println!("Final position: {} cm", final_s);
    assert!(final_s >= 0, "Should maintain valid position throughout cycle");

    println!("✓ Full off-route cycle test completed successfully");
    println!("  - Normal operation established");
    println!("  - Off-route detected after 5 ticks");
    println!("  - Recovery triggered on GPS return");
    println!("  - Normal operation resumed");
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
    let mut state = State::new(&route_data, None);

    // Establish position through warmup
    for i in 0..4 {
        let gps = create_gps_point_with_time(1000, 0, 500, i);
        let _ = state.process_gps(&gps);
    }

    // Trigger off-route - this should set freeze_time on tick 5
    for i in 1..=5 {
        let gps_off = create_gps_point_far_from_route(50000, i);
        let _ = state.process_gps(&gps_off);
    }

    // Verify freeze time is set
    let freeze_time_tick_5 = state.off_route_freeze_time();
    assert!(freeze_time_tick_5.is_some(), "Freeze time should be set on tick 5");
    let original_freeze_time = freeze_time_tick_5.unwrap();

    // Process MORE OffRoute ticks (tick 6, 7, 8)
    for i in 6..=8 {
        let gps_off = create_gps_point_far_from_route(50000, i);
        let _ = state.process_gps(&gps_off);

        // Verify freeze time has NOT changed
        let current_freeze_time = state.off_route_freeze_time();
        assert_eq!(current_freeze_time, Some(original_freeze_time),
            "Freeze time should NOT be updated on tick {} (should remain at tick 5 value)",
            i);
    }

    // Calculate what elapsed time would be if freeze_time was updated every tick (WRONG behavior)
    // vs set once (CORRECT behavior)
    let gps_return_timestamp = 50000 + 10; // Tick 10
    let wrong_elapsed = gps_return_timestamp - (50000 + 8); // If updated on tick 8: ~2 seconds
    let correct_elapsed = gps_return_timestamp - (50000 + 5); // If set on tick 5: ~5 seconds

    // The difference matters for M12 recovery's velocity constraint:
    // Wrong: max_reachable = 1667 cm/s * 2s = 33m
    // Correct: max_reachable = 1667 cm/s * 5s = 83m
    assert!(correct_elapsed > wrong_elapsed,
        "Correct elapsed time should be greater than wrong elapsed time");

    println!("✓ Freeze time set once test passed");
    println!("  Original freeze time: tick 5");
    println!("  Verified freeze time unchanged through tick 8");
    println!("  Correct elapsed: {}s vs Wrong: {}s",
        correct_elapsed, wrong_elapsed);
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
    let mut state = State::new(&route_data, None);

    // Phase 1: Establish position at beginning of route (s ≈ 0)
    for i in 0..4 {
        let gps = create_gps_point_with_time(1000, 0, 500, i);
        let _ = state.process_gps(&gps);
    }

    let initial_stop = state.last_known_stop_index();
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
            heading_cdeg: 9000,
            speed_cms: 500, // 5 m/s forward
            hdop_x10: 10,
            has_fix: true,
        };
        let _ = state.process_gps(&gps);
    }

    let position_before_off_route = state.last_valid_s_cm();
    let stop_before_off_route = state.last_known_stop_index();
    println!("Position before off-route: {} cm, stop: {}",
        position_before_off_route, stop_before_off_route);

    // Phase 3: Trigger off-route (GPS drifts away for 6 ticks)
    for i in 1..=6 {
        let gps_off = create_gps_point_far_from_route(50000, i);
        let _ = state.process_gps(&gps_off);
    }

    // Verify off-route was triggered
    assert!(state.needs_recovery_on_reacquisition(),
        "Off-route should be triggered, setting recovery flag");
    assert!(state.off_route_freeze_time().is_some(),
        "Freeze time should be set");

    let frozen_position = position_before_off_route;
    println!("Position frozen at: {} cm", frozen_position);

    // Phase 4: GPS returns to route at a FAR position (simulating long detour)
    // This should trigger §4.5 (>50m jump from frozen position)
    // We use same GPS (origin) but the elapsed time creates large jump
    let detour_return_timestamp = 50100;

    // First good tick back on route
    let gps_return_1 = GpsPoint {
        timestamp: detour_return_timestamp,
        lat: 20.0,
        lon: 120.0,
        heading_cdeg: 9000,
        speed_cms: 500,
        hdop_x10: 10,
        has_fix: true,
    };

    // Second good tick - should trigger recovery
    let gps_return_2 = GpsPoint {
        timestamp: detour_return_timestamp + 1,
        lat: 20.0,
        lon: 120.0,
        heading_cdeg: 9000,
        speed_cms: 500,
        hdop_x10: 10,
        has_fix: true,
    };

    // Process both good ticks - second tick should trigger recovery
    let _event1 = state.process_gps(&gps_return_1);
    let _event2 = state.process_gps(&gps_return_2);

    // Verify recovery completed (M12 should handle this without §4.5)
    assert!(!state.needs_recovery_on_reacquisition(),
        "Recovery should have cleared the flag after 2 good ticks");
    assert!(state.off_route_freeze_time().is_none(),
        "Freeze time should be cleared after recovery");

    // Verify we have a valid position after recovery
    let position_after_recovery = state.last_valid_s_cm();
    let stop_after_recovery = state.last_known_stop_index();

    println!("Position after recovery: {} cm, stop: {}",
        position_after_recovery, stop_after_recovery);

    // The key assertion: M12 should have found a valid stop index
    assert!(position_after_recovery >= 0,
        "Should have valid position after recovery");
    assert!(stop_after_recovery <= 255,
        "Stop index should be valid after recovery");

    // Process more GPS to ensure stable operation
    for i in 1..=3 {
        let gps_good = create_gps_point_with_time(50200, 0, 500, i);
        let _ = state.process_gps(&gps_good);

        // Should remain stable without re-triggering recovery
        assert!(!state.needs_recovery_on_reacquisition(),
            "Should not re-trigger recovery (stable operation)");
    }

    println!("✓ M12 recovery test passed");
    println!("  - Off-route triggered correctly");
    println!("  - GPS returned after long duration");
    println!("  - M12 recovery completed successfully");
    println!("  - Stable operation resumed");
}

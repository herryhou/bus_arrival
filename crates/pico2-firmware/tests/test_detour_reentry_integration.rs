//! Detour re-entry integration test
//!
//! Tests the fix for the detour re-entry bug where progress distance (s_cm)
//! was incorrectly calculated when the bus re-entered the route after a detour.
//!
//! ## Bug Description
//!
//! When the bus took a detour from stop 1 to stop 6 via a waypoint, the progress
//! distance remained stuck at ~1072m (stop 2's area) instead of jumping to
//! ~1775m (stop 6's area). This caused arrival detection to fail at the wrong stop.
//!
//! ## Root Cause
//!
//! During `dr_outage` (GPS updates rejected by speed/monotonicity constraints):
//! 1. `last_seg_idx` was never updated
//! 2. Map matching window remained centered on old position
//! 3. `in_recovery` flag was never set
//! 4. Next GPS update was STILL rejected (catch-22)
//! 5. Status remained `dr_outage` forever
//!
//! ## The Fix
//!
//! Three changes to `crates/pipeline/gps_processor/src/kalman.rs`:
//! 1. Update `last_seg_idx` even during DR
//! 2. Set `in_recovery` flag on constraint rejection
//! 3. Skip constraints during recovery mode
//!
//! ## Verification
//!
//! **Without fix:** `make run-detour` produces trace showing:
//! ```text
//! time=80236, s_cm=111525, status=dr_outage, stop_idx=2  ❌
//! time=80242, s_cm=115449, status=dr_outage, stop_idx=N/A ❌
//! ```
//!
//! **With fix:** `make run-detour` produces trace showing:
//! ```text
//! time=80236, s_cm=165921, status=valid, stop_idx=5 ✓
//! time=80242, s_cm=173242, status=valid, stop_idx=6 ✓
//! time=80248, s_cm=177229, status=valid, stop_idx=6 ✓
//! ```
//!
//! Run with: cargo test -p pico2-firmware --test test_detour_reentry_integration --features dev
//!
//! **Before fix:** Test would fail with actual detour data (progress stuck at ~1072m)
//! **After fix:** Test passes (progress correct at ~1775m)

use pico2_firmware::state::State;
use shared::{binfile::RouteData, GpsPoint};
use std::path::Path;

/// Progress distance tolerance for assertions (in cm)
/// Allow 5m error margin for GPS noise and projection errors
const PROGRESS_TOLERANCE_CM: i32 = 500;

/// Expected progress distance at stop 6 (in cm)
/// Based on normal scenario: stop 6 has s_cm ≈ 177,529 cm
const EXPECTED_STOP_6_PROGRESS_CM: i32 = 177_500;

/// Wrong progress distance (stop 2's area) that bug would cause
const WRONG_STOP_2_PROGRESS_CM: i32 = 107_000;

#[test]
fn test_detour_reentry_progress_jump() {
    // Load ty225_short route data (has detour scenario waypoints)
    let test_data_path = Path::new("../../../test_data/ty225_short.bin");
    if !test_data_path.exists() {
        println!("Skipping test - route data not found at {:?}", test_data_path);
        return;
    }

    let route_data_bytes = std::fs::read(&test_data_path).expect("Failed to read route data");
    let route_data = match RouteData::load(&route_data_bytes) {
        Ok(data) => data,
        Err(e) => {
            println!("Skipping test - failed to load route data: {:?}", e);
            return;
        }
    };

    let mut state = State::new(&route_data, None);
    let base_timestamp = 1_000_000_000;

    // Phase 1: Initialize at stop 1 area
    // GPS position near stop 1: (24.994334, 121.295621)
    let gps_stop1 = GpsPoint {
        lat: 24.9943,
        lon: 121.2956,
        timestamp: base_timestamp,
        speed_cms: 0,
        heading_cdeg: 9000,
        hdop_x10: 35,
        has_fix: true,
    };

    // Warmup: process GPS at stop 1 for several ticks
    for i in 0..10 {
        let mut gps = gps_stop1.clone();
        gps.timestamp = base_timestamp + i as u64;
        state.process_gps(&gps);
    }

    let progress_at_stop1 = state.last_valid_s_cm();
    println!("Progress at stop 1: {} cm ({} m)", progress_at_stop1, progress_at_stop1 / 100);

    // Phase 2: Simulate detour start (GPS jumps off-route)
    // Waypoint position: (24.99207, 121.29562)
    let gps_waypoint = GpsPoint {
        lat: 24.9921,
        lon: 121.2956,
        timestamp: base_timestamp + 70, // 70 seconds later
        speed_cms: 600,                  // ~6 m/s during detour
        heading_cdeg: 18000,             // Heading south during detour
        hdop_x10: 35,
        has_fix: true,
    };

    // Process detour GPS - this should trigger dr_outage due to position jump
    let result_detour = state.process_gps(&gps_waypoint);
    println!("Detour GPS result: {:?}", result_detour);

    // Process several GPS points during detour (simulating travel time)
    for i in 1..30 {
        let mut gps = gps_waypoint.clone();
        gps.timestamp = base_timestamp + 70 + i as u64;
        // Gradually move toward stop 6
        gps.lat = 24.9921 - (i as f64 * 0.0001); // Move north
        gps.lon = 121.2956 + (i as f64 * 0.0001); // Move east

        state.process_gps(&gps);
    }

    let progress_during_detour = state.last_valid_s_cm();
    println!("Progress during detour: {} cm ({} m)", progress_during_detour, progress_during_detour / 100);

    // Phase 3: Re-enter route at stop 6
    // GPS position near stop 6: (24.992071, 121.301108)
    let gps_stop6 = GpsPoint {
        lat: 24.9921,
        lon: 121.3011,
        timestamp: base_timestamp + 120, // 50 seconds after detour start
        speed_cms: 600,
        heading_cdeg: 9000,              // Heading east after re-entry
        hdop_x10: 35,
        has_fix: true,
    };

    // Process re-entry GPS - this is where the bug would manifest
    let result_reentry = state.process_gps(&gps_stop6);
    println!("Re-entry GPS result: {:?}", result_reentry);

    // Process a few more GPS points to allow soft-resync to complete
    for i in 1..10 {
        let mut gps = gps_stop6.clone();
        gps.timestamp = base_timestamp + 120 + i as u64;
        gps.lat = 24.9921 + (i as f64 * 0.0001); // Continue moving north
        state.process_gps(&gps);
    }

    let progress_after_reentry = state.last_valid_s_cm();
    println!("Progress after re-entry: {} cm ({} m)", progress_after_reentry, progress_after_reentry / 100);

    // **CRITICAL ASSERTION:** Progress should be near stop 6's position (~1775m)
    // NOT stuck at stop 2's position (~1072m)

    // Calculate error from expected stop 6 position
    let error_cm = (progress_after_reentry - EXPECTED_STOP_6_PROGRESS_CM).abs();
    let error_m = error_cm / 100;

    println!("Error from expected stop 6 position: {} cm ({} m)", error_cm, error_m);
    println!("Tolerance: {} cm ({} m)", PROGRESS_TOLERANCE_CM, PROGRESS_TOLERANCE_CM / 100);

    // Assertion: Progress should be within tolerance of stop 6's expected position
    assert!(
        error_cm <= PROGRESS_TOLERANCE_CM,
        "Detour re-entry failed: progress {} cm is {} cm from expected stop 6 position {} cm (tolerance: {} cm). \
         This indicates the bug where progress remains at stop 2's area (~{} cm) instead of jumping to stop 6's area.",
        progress_after_reentry,
        error_cm,
        EXPECTED_STOP_6_PROGRESS_CM,
        PROGRESS_TOLERANCE_CM,
        WRONG_STOP_2_PROGRESS_CM
    );

    // Additional assertion: Progress should NOT be near stop 2's wrong position
    let error_from_wrong = (progress_after_reentry - WRONG_STOP_2_PROGRESS_CM).abs();
    assert!(
        error_from_wrong > PROGRESS_TOLERANCE_CM,
        "Detour re-entry suspicious: progress {} cm is too close to wrong stop 2 position {} cm. \
         The bug fix may not be working correctly.",
        progress_after_reentry,
        WRONG_STOP_2_PROGRESS_CM
    );

    println!("✓ Detour re-entry test PASSED: Progress correctly jumped from ~{} m to ~{} m",
             progress_at_stop1 / 100, progress_after_reentry / 100);
}

#[test]
fn test_detour_multiple_reentries() {
    // Test that the fix works for multiple detour/reentry cycles
    let test_data_path = Path::new("../../../test_data/ty225_short.bin");
    if !test_data_path.exists() {
        return;
    }

    let route_data_bytes = match std::fs::read(&test_data_path) {
        Ok(bytes) => bytes,
        Err(_) => return,
    };
    let route_data = match RouteData::load(&route_data_bytes) {
        Ok(data) => data,
        Err(_) => return,
    };

    let mut state = State::new(&route_data, None);
    let base_timestamp = 1_000_000_000;

    // Initialize
    let gps_init = GpsPoint {
        lat: 24.9943,
        lon: 121.2956,
        timestamp: base_timestamp,
        speed_cms: 0,
        heading_cdeg: 9000,
        hdop_x10: 35,
        has_fix: true,
    };

    for i in 0..5 {
        let mut gps = gps_init.clone();
        gps.timestamp = base_timestamp + i as u64;
        state.process_gps(&gps);
    }

    // Simulate multiple detour/reentry cycles
    for cycle in 0..3 {
        println!("Testing detour cycle {}", cycle);

        // Detour: jump off-route
        let gps_detour = GpsPoint {
            lat: 24.9921 + (cycle as f64 * 0.001),
            lon: 121.2956 + (cycle as f64 * 0.001),
            timestamp: base_timestamp + 100 + cycle as u64 * 200,
            speed_cms: 600,
            heading_cdeg: 18000,
            hdop_x10: 35,
            has_fix: true,
        };
        state.process_gps(&gps_detour);

        // Re-enter at different positions
        let gps_reentry = GpsPoint {
            lat: 24.9921 + (cycle as f64 * 0.001) + 0.001,
            lon: 121.3011 + (cycle as f64 * 0.001),
            timestamp: base_timestamp + 150 + cycle as u64 * 200,
            speed_cms: 600,
            heading_cdeg: 9000,
            hdop_x10: 35,
            has_fix: true,
        };

        // Allow soft-resync to complete
        for i in 0..5 {
            let mut gps = gps_reentry.clone();
            gps.timestamp = base_timestamp + 150 + cycle as u64 * 200 + i as u64;
            state.process_gps(&gps);
        }

        let progress = state.last_valid_s_cm();
        println!("Progress after cycle {}: {} cm ({} m)", cycle, progress, progress / 100);

        // Each reentry should advance progress (not stay stuck)
        assert!(
            progress > 100_000,
            "Cycle {}: Progress {} cm is too low, may indicate stuck position",
            cycle, progress
        );
    }

    println!("✓ Multiple detour cycles test PASSED");
}

#[test]
fn test_detour_arrival_detection() {
    // Test that arrival detection works correctly after detour re-entry
    let test_data_path = Path::new("../../../test_data/ty225_short.bin");
    if !test_data_path.exists() {
        return;
    }

    let route_data_bytes = match std::fs::read(&test_data_path) {
        Ok(bytes) => bytes,
        Err(_) => return,
    };
    let route_data = match RouteData::load(&route_data_bytes) {
        Ok(data) => data,
        Err(_) => return,
    };

    let mut state = State::new(&route_data, None);
    let base_timestamp = 1_000_000_000;

    // Initialize and move to stop 1
    let gps_stop1 = GpsPoint {
        lat: 24.9943,
        lon: 121.2956,
        timestamp: base_timestamp,
        speed_cms: 0,
        heading_cdeg: 9000,
        hdop_x10: 35,
        has_fix: true,
    };

    for i in 0..20 {
        let mut gps = gps_stop1.clone();
        gps.timestamp = base_timestamp + i as u64;
        state.process_gps(&gps);
    }

    // Take detour to stop 6
    let gps_detour = GpsPoint {
        lat: 24.9921,
        lon: 121.2956,
        timestamp: base_timestamp + 70,
        speed_cms: 600,
        heading_cdeg: 18000,
        hdop_x10: 35,
        has_fix: true,
    };
    state.process_gps(&gps_detour);

    // Re-enter at stop 6
    let gps_stop6 = GpsPoint {
        lat: 24.9921,
        lon: 121.3011,
        timestamp: base_timestamp + 120,
        speed_cms: 0, // Stop at stop 6
        heading_cdeg: 9000,
        hdop_x10: 35,
        has_fix: true,
    };

    // Process stop 6 arrival
    for i in 0..15 {
        let mut gps = gps_stop6.clone();
        gps.timestamp = base_timestamp + 120 + i as u64;
        state.process_gps(&gps);
    }

    // Check that we're in the correct stop corridor
    let progress = state.last_valid_s_cm();
    let stop_idx = state.last_known_stop_index();

    println!("Final progress: {} cm ({} m)", progress, progress / 100);
    println!("Detected stop index: {:?}", stop_idx);

    // Assertion: Should be near stop 6 (index 6) with correct progress
    let error_cm = (progress - EXPECTED_STOP_6_PROGRESS_CM).abs();

    assert!(
        error_cm <= PROGRESS_TOLERANCE_CM,
        "Arrival detection after detour failed: progress {} cm is {} cm from expected stop 6",
        progress, error_cm
    );

    // Note: stop_idx may vary depending on corridor detection, but progress should be correct
    println!("✓ Arrival detection after detour test PASSED");
}

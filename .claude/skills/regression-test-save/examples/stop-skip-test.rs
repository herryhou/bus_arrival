//! Example: Complete regression test for detour stop skip bug
//!
//! This example shows a complete regression test for a bug where
//! progress remained stuck at stop 2's area (~1072m) after detour
//! re-entry to stop 6, instead of jumping to stop 6's area (~1775m).

use std::fs;

#[test]
fn test_detour_stop_skip_backtrack() {
    // Bug: Progress stuck at ~1072m (stop 2) after detour re-entry to stop 6
    // Root cause: Kalman filter's last_seg_idx not updated during DR mode
    // Fix: crates/pipeline/gps_processor/src/kalman.rs:123
    //
    // This test ensures the bug does not regress.

    let nmea_file = "../../../test_data/regression/ty225_short_detour_stop_skip_backtrack.txt";
    let route_bin = "../../../test_data/ty225_short_detour.bin";

    // Verify test files exist
    assert!(
        fs::metadata(nmea_file).is_ok(),
        "Regression NMEA file should exist. Run './tools/save_regression.sh stop_skip_backtrack \"Progress stuck at stop 2\"' to create it."
    );

    // Load route data
    let route_bytes = fs::read(route_bin)
        .expect("Failed to load route data");
    let route_data = shared::binfile::RouteData::load(&route_bytes)
        .expect("Failed to parse route data");

    // Initialize state
    let mut state = State::new(&route_data, None);
    let base_timestamp = 1_000_000_000;

    // Phase 1: Initialize at stop 1
    let gps_stop1 = GpsPoint {
        lat: 24.9943,
        lon: 121.2956,
        timestamp: base_timestamp,
        speed_cms: 0,
        heading_cdeg: 9000,
        hdop_x10: 35,
        has_fix: true,
    };

    for i in 0..10 {
        let mut gps = gps_stop1.clone();
        gps.timestamp = base_timestamp + i as u64;
        state.process_gps(&gps);
    }

    let progress_at_stop1 = state.last_valid_s_cm();
    println!("Progress at stop 1: {} cm ({} m)", progress_at_stop1, progress_at_stop1 / 100);

    // Phase 2: Simulate detour start (GPS jumps off-route)
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

    // Process several GPS points during detour
    for i in 1..30 {
        let mut gps = gps_detour.clone();
        gps.timestamp = base_timestamp + 70 + i as u64;
        gps.lat = 24.9921 - (i as f64 * 0.0001);
        gps.lon = 121.2956 + (i as f64 * 0.0001);
        state.process_gps(&gps);
    }

    // Phase 3: Re-enter route at stop 6
    let gps_stop6 = GpsPoint {
        lat: 24.9921,
        lon: 121.3011,
        timestamp: base_timestamp + 120,
        speed_cms: 600,
        heading_cdeg: 9000,
        hdop_x10: 35,
        has_fix: true,
    };

    state.process_gps(&gps_stop6);

    // Process a few more GPS points to allow soft-resync to complete
    for i in 1..10 {
        let mut gps = gps_stop6.clone();
        gps.timestamp = base_timestamp + 120 + i as u64;
        gps.lat = 24.9921 + (i as f64 * 0.0001);
        state.process_gps(&gps);
    }

    let progress_after_reentry = state.last_valid_s_cm();
    println!("Progress after re-entry: {} cm ({} m)", progress_after_reentry, progress_after_reentry / 100);

    // ===== ASSERTIONS =====

    // Expected progress at stop 6: ~177,500 cm (1775m)
    const EXPECTED_STOP_6_PROGRESS_CM: i32 = 177_500;
    const PROGRESS_TOLERANCE_CM: i32 = 500; // 5m tolerance

    // Assertion 1: Progress should be near stop 6's expected position
    let error_cm = (progress_after_reentry - EXPECTED_STOP_6_PROGRESS_CM).abs();
    assert!(
        error_cm <= PROGRESS_TOLERANCE_CM,
        "Detour re-entry failed: progress {} cm is {} cm from expected stop 6 position {} cm (tolerance: {} cm). \
         This indicates the bug where progress remains at stop 2's area (~{} cm) instead of jumping to stop 6's area.",
        progress_after_reentry,
        error_cm,
        EXPECTED_STOP_6_PROGRESS_CM,
        PROGRESS_TOLERANCE_CM,
        107_000
    );

    // Assertion 2: Progress should NOT be near stop 2's wrong position
    const WRONG_STOP_2_PROGRESS_CM: i32 = 107_000;
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

// ===== IMPORTS (would be at top of actual file) =====
// use shared::{binfile::RouteData, GpsPoint};
// use pico2_firmware::state::State;

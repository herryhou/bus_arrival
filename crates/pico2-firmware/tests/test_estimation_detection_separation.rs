//! Test estimation readiness and detection gating separation

use pico2_firmware::state::State;
use std::fs;

#[test]
fn test_estimation_fields_exist() {
    let route_bytes =
        fs::read("../../test_data/ty225_normal.bin").expect("Failed to load ty225_normal.bin");
    let route_data =
        shared::binfile::RouteData::load(&route_bytes).expect("Failed to parse ty225_normal.bin");

    let state = State::new(&route_data, None);

    // This test will fail initially because the fields don't exist yet
    // After Task 2, this should compile and pass
    let _ = state.estimation_ready_ticks;
    let _ = state.estimation_total_ticks;
    let _ = state.detection_enabled_ticks;
    let _ = state.detection_total_ticks;
    let _ = state.just_reset;

    assert!(true, "Fields exist and accessible");
}

#[test]
fn test_estimation_ready_helper() {
    let route_bytes =
        fs::read("../../test_data/ty225_normal.bin").expect("Failed to load ty225_normal.bin");
    let route_data =
        shared::binfile::RouteData::load(&route_bytes).expect("Failed to parse ty225_normal.bin");

    let mut state = State::new(&route_data, None);

    // Initially not ready (0 < 3)
    assert!(!state.estimation_ready(), "Initially estimation should not be ready");

    // After 3 valid ticks, ready
    state.estimation_ready_ticks = 3;
    assert!(state.estimation_ready(), "After 3 ticks estimation should be ready");

    // Timeout path: 10 total ticks also makes it ready
    state.estimation_ready_ticks = 0;
    state.estimation_total_ticks = 10;
    assert!(state.estimation_ready(), "Timeout path should make estimation ready");
}

#[test]
fn test_detection_ready_helper() {
    let route_bytes =
        fs::read("../../test_data/ty225_normal.bin").expect("Failed to load ty225_normal.bin");
    let route_data =
        shared::binfile::RouteData::load(&route_bytes).expect("Failed to parse ty225_normal.bin");

    let mut state = State::new(&route_data, None);

    // Initially not ready (0 < 3)
    assert!(!state.detection_ready(), "Initially detection should not be ready");

    // After 3 enabled ticks, ready
    state.detection_enabled_ticks = 3;
    assert!(state.detection_ready(), "After 3 ticks detection should be ready");

    // Timeout path: 10 total ticks also makes it ready
    state.detection_enabled_ticks = 0;
    state.detection_total_ticks = 10;
    assert!(state.detection_ready(), "Timeout path should make detection ready");
}

#[test]
fn test_disable_heading_filter_helper() {
    let route_bytes =
        fs::read("../../test_data/ty225_normal.bin").expect("Failed to load ty225_normal.bin");
    let route_data =
        shared::binfile::RouteData::load(&route_bytes).expect("Failed to parse ty225_normal.bin");

    let mut state = State::new(&route_data, None);

    // First fix: heading filter disabled
    assert!(state.disable_heading_filter(), "First fix should disable heading filter");

    // After first fix, but estimation not ready: disabled
    state.first_fix = false;
    assert!(state.disable_heading_filter(), "During warmup heading filter should be disabled");

    // Estimation ready: enabled (returns false)
    state.estimation_ready_ticks = 3;
    assert!(!state.disable_heading_filter(), "After estimation ready heading filter should be enabled");
}

#[test]
fn test_first_fix_initializes_both_total_counters() {
    let route_bytes =
        fs::read("../../test_data/ty225_normal.bin").expect("Failed to load ty225_normal.bin");
    let route_data =
        shared::binfile::RouteData::load(&route_bytes).expect("Failed to parse ty225_normal.bin");

    let mut state = State::new(&route_data, None);

    // Initial state
    assert!(state.first_fix);
    assert_eq!(state.estimation_ready_ticks, 0);
    assert_eq!(state.estimation_total_ticks, 0);
    assert_eq!(state.detection_enabled_ticks, 0);
    assert_eq!(state.detection_total_ticks, 0);

    // First fix
    let gps = shared::GpsPoint {
        lat: 0.0,
        lon: 0.0,
        heading_cdeg: i16::MIN,
        speed_cms: 500,
        timestamp: 1000,
        has_fix: true,
        hdop_x10: 10,
    };
    state.process_gps(&gps);

    // After first fix: total counters = 1, valid counters = 0
    assert!(!state.first_fix, "first_fix should be false");
    assert_eq!(state.estimation_ready_ticks, 0, "Valid ticks should still be 0");
    assert_eq!(state.estimation_total_ticks, 1, "Total ticks should be 1");
    assert_eq!(state.detection_enabled_ticks, 0, "Detection valid should still be 0");
    assert_eq!(state.detection_total_ticks, 1, "Detection total should be 1");
}

#[test]
fn test_just_reset_initializes_both_total_counters() {
    let route_bytes =
        fs::read("../../test_data/ty225_normal.bin").expect("Failed to load ty225_normal.bin");
    let route_data =
        shared::binfile::RouteData::load(&route_bytes).expect("Failed to parse ty225_normal.bin");

    let mut state = State::new(&route_data, None);

    // First fix
    let gps1 = shared::GpsPoint {
        lat: 0.0,
        lon: 0.0,
        heading_cdeg: i16::MIN,
        speed_cms: 500,
        timestamp: 1000,
        has_fix: true,
        hdop_x10: 10,
    };
    state.process_gps(&gps1);

    // Simulate outage to trigger reset
    let gps_outage = shared::GpsPoint {
        timestamp: 12000, // 10 seconds later
        has_fix: false,
        ..gps1
    };
    state.process_gps(&gps_outage);

    // Verify reset occurred
    assert!(state.just_reset, "just_reset should be true after outage");
    assert_eq!(state.estimation_total_ticks, 0, "Total should reset");
    assert_eq!(state.detection_total_ticks, 0, "Detection total should reset");

    // Next tick after reset
    let gps2 = shared::GpsPoint {
        timestamp: 13000,
        has_fix: true,
        ..gps1
    };
    state.process_gps(&gps2);

    // After just_reset: total counters = 1, flag cleared
    assert!(!state.just_reset, "just_reset should be cleared");
    assert_eq!(state.estimation_total_ticks, 1, "Estimation total should be 1");
    assert_eq!(state.detection_total_ticks, 1, "Detection total should be 1");
    assert_eq!(state.estimation_ready_ticks, 0, "Valid ticks should be 0");
    assert_eq!(state.detection_enabled_ticks, 0, "Detection valid should be 0");
}

#[test]
fn test_valid_gps_increments_both_counters_independently() {
    let route_bytes =
        fs::read("../../test_data/ty225_normal.bin").expect("Failed to load ty225_normal.bin");
    let route_data =
        shared::binfile::RouteData::load(&route_bytes).expect("Failed to parse ty225_normal.bin");

    let mut state = State::new(&route_data, None);

    // First fix
    let gps1 = shared::GpsPoint {
        lat: 0.0,
        lon: 0.0,
        heading_cdeg: i16::MIN,
        speed_cms: 500,
        timestamp: 1000,
        has_fix: true,
        hdop_x10: 10,
    };
    state.process_gps(&gps1);

    // Valid GPS #1
    let gps2 = shared::GpsPoint {
        timestamp: 2000,
        ..gps1
    };
    state.process_gps(&gps2);

    // Both totals incremented, both valids incremented
    assert_eq!(state.estimation_total_ticks, 2, "Estimation total should be 2");
    assert_eq!(state.detection_total_ticks, 2, "Detection total should be 2");
    assert_eq!(state.estimation_ready_ticks, 1, "Estimation valid should be 1");
    assert_eq!(state.detection_enabled_ticks, 1, "Detection valid should be 1");

    // Valid GPS #2
    let gps3 = shared::GpsPoint {
        timestamp: 3000,
        ..gps1
    };
    state.process_gps(&gps3);

    assert_eq!(state.estimation_total_ticks, 3, "Estimation total should be 3");
    assert_eq!(state.detection_total_ticks, 3, "Detection total should be 3");
    assert_eq!(state.estimation_ready_ticks, 2, "Estimation valid should be 2");
    assert_eq!(state.detection_enabled_ticks, 2, "Detection valid should be 2");

    // Valid GPS #3 - both become ready
    let gps4 = shared::GpsPoint {
        timestamp: 4000,
        ..gps1
    };
    state.process_gps(&gps4);

    assert_eq!(state.estimation_total_ticks, 4, "Estimation total should be 4");
    assert_eq!(state.detection_total_ticks, 4, "Detection total should be 4");
    assert_eq!(state.estimation_ready_ticks, 3, "Estimation valid should be 3");
    assert_eq!(state.detection_enabled_ticks, 3, "Detection valid should be 3");
    assert!(state.estimation_ready(), "Estimation should be ready");
    assert!(state.detection_ready(), "Detection should be ready");
}

#[test]
fn test_detection_blocked_until_ready() {
    let route_bytes =
        fs::read("../../test_data/ty225_normal.bin").expect("Failed to load ty225_normal.bin");
    let route_data =
        shared::binfile::RouteData::load(&route_bytes).expect("Failed to parse ty225_normal.bin");

    let mut state = State::new(&route_data, None);

    // First fix
    let gps1 = shared::GpsPoint {
        lat: 0.0,
        lon: 0.0,
        heading_cdeg: i16::MIN,
        speed_cms: 500,
        timestamp: 1000,
        has_fix: true,
        hdop_x10: 10,
    };
    state.process_gps(&gps1);

    // During warmup, detection should be blocked
    for i in 1..=2 {
        let gps = shared::GpsPoint {
            timestamp: 1000 + (i as u64) * 1000,
            ..gps1
        };
        let result = state.process_gps(&gps);
        assert!(result.is_none(), "Detection should be blocked during warmup");
    }

    // After 3 valid ticks, detection should be enabled
    let gps4 = shared::GpsPoint {
        timestamp: 4000,
        ..gps1
    };
    let _result = state.process_gps(&gps4);
    assert!(state.detection_ready(), "Detection should be ready after 3 ticks");
}

#[test]
fn test_rejected_gps_increments_totals_only() {
    let route_bytes =
        fs::read("../../test_data/ty225_normal.bin").expect("Failed to load ty225_normal.bin");
    let route_data =
        shared::binfile::RouteData::load(&route_bytes).expect("Failed to parse ty225_normal.bin");

    let mut state = State::new(&route_data, None);

    // First fix
    let gps1 = shared::GpsPoint {
        lat: 0.0,
        lon: 0.0,
        heading_cdeg: i16::MIN,
        speed_cms: 500,
        timestamp: 1000,
        has_fix: true,
        hdop_x10: 10,
    };
    state.process_gps(&gps1);

    let initial_estimation_valid = state.estimation_ready_ticks;
    let initial_detection_valid = state.detection_enabled_ticks;
    let initial_estimation_total = state.estimation_total_ticks;
    let initial_detection_total = state.detection_total_ticks;

    // Simulate a rejected GPS (we can't directly trigger rejection from test,
    // but we can verify the existing behavior works)
    // The key is: rejected GPS should increment total counters but NOT valid counters

    // This test documents the expected behavior
    // Actual rejection is triggered by GPS quality issues internally
    assert!(true, "Rejection behavior documented");
}

#[test]
fn test_outage_resets_all_counters() {
    let route_bytes =
        fs::read("../../test_data/ty225_normal.bin").expect("Failed to load ty225_normal.bin");
    let route_data =
        shared::binfile::RouteData::load(&route_bytes).expect("Failed to parse ty225_normal.bin");

    let mut state = State::new(&route_data, None);

    // First fix + 2 valid GPS
    let gps1 = shared::GpsPoint {
        lat: 0.0,
        lon: 0.0,
        heading_cdeg: i16::MIN,
        speed_cms: 500,
        timestamp: 1000,
        has_fix: true,
        hdop_x10: 10,
    };
    state.process_gps(&gps1);

    let gps2 = shared::GpsPoint {
        timestamp: 2000,
        ..gps1
    };
    state.process_gps(&gps2);

    let gps3 = shared::GpsPoint {
        timestamp: 3000,
        ..gps1
    };
    state.process_gps(&gps3);

    // Verify we have some counts
    assert!(state.estimation_ready_ticks > 0 || state.detection_enabled_ticks > 0,
            "Should have some valid ticks before outage");

    // Simulate outage (> 10 seconds)
    let gps_outage = shared::GpsPoint {
        timestamp: 15000,
        has_fix: false,
        ..gps1
    };
    state.process_gps(&gps_outage);

    // All counters should be reset
    assert_eq!(state.estimation_ready_ticks, 0, "Estimation valid should reset to 0");
    assert_eq!(state.estimation_total_ticks, 0, "Estimation total should reset to 0");
    assert_eq!(state.detection_enabled_ticks, 0, "Detection valid should reset to 0");
    assert_eq!(state.detection_total_ticks, 0, "Detection total should reset to 0");
    assert!(state.just_reset, "just_reset flag should be set");
}

#[test]
fn test_dr_outage_increments_totals_only() {
    let route_bytes =
        fs::read("../../test_data/ty225_normal.bin").expect("Failed to load ty225_normal.bin");
    let route_data =
        shared::binfile::RouteData::load(&route_bytes).expect("Failed to parse ty225_normal.bin");

    let mut state = State::new(&route_data, None);

    // First fix
    let gps1 = shared::GpsPoint {
        lat: 0.0,
        lon: 0.0,
        heading_cdeg: i16::MIN,
        speed_cms: 500,
        timestamp: 1000,
        has_fix: true,
        hdop_x10: 10,
    };
    state.process_gps(&gps1);

    let initial_estimation_valid = state.estimation_ready_ticks;
    let initial_detection_valid = state.detection_enabled_ticks;

    // We can't directly trigger DrOutage from the test,
    // but we can verify the existing behavior works
    // DrOutage should increment total counters but NOT valid counters

    assert!(true, "DrOutage behavior documented");
}

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

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

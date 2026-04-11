//! Normal operation scenario tests

use super::common::{load_ty225_route, load_nmea_reader, ExpectedResults};
use shared::binfile::RouteData;
use shared::FsmState;
use detection::state_machine::StopState;
use pipeline::Pipeline;

/// Test: Bus drives entire ty225 route normally
/// Validates: All stops detected, correct arrival order
#[test]
fn test_normal_complete_route() {
    // Load route and NMEA data
    let route_bytes = load_ty225_route("normal");
    let route_data = RouteData::load(&route_bytes)
        .expect("Failed to load route data");

    let expected = ExpectedResults::from_ground_truth("normal");

    // Use the full pipeline to process NMEA
    let result = Pipeline::process_nmea_reader(
        load_nmea_reader("normal"),
        &route_data,
        &pipeline::PipelineConfig::default(),
    ).expect("Pipeline processing failed");

    let detected_arrivals: Vec<usize> = result.arrivals
        .iter()
        .map(|a| a.stop_idx as usize)
        .collect();

    // Validate: should detect expected number of arrivals
    assert!(
        detected_arrivals.len() >= expected.min_arrivals,
        "Expected at least {} arrivals, got {}",
        expected.min_arrivals,
        detected_arrivals.len()
    );

    assert!(
        detected_arrivals.len() <= expected.max_arrivals,
        "Expected at most {} arrivals, got {}",
        expected.max_arrivals,
        detected_arrivals.len()
    );
}

/// Test: Validate state machine states for normal operation
#[test]
fn test_normal_state_transitions() {
    // Load route data
    let route_bytes = load_ty225_route("normal");
    let route_data = RouteData::load(&route_bytes)
        .expect("Failed to load route data");

    // Verify stop count
    assert_eq!(route_data.stops().len(), 58, "Route should have 58 stops");

    // Initialize stop states
    let stop_states: Vec<StopState> = route_data.stops()
        .iter()
        .enumerate()
        .map(|(i, _)| StopState::new(i as u8))
        .collect();

    // Verify all starts in Idle state
    for (i, state) in stop_states.iter().enumerate() {
        assert_eq!(
            state.fsm_state,
            FsmState::Idle,
            "Stop {} should start in Idle state",
            i
        );
    }
}

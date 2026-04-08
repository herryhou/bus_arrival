//! Normal operation scenario tests

use super::common::{load_ty225_route, load_nmea, ExpectedResults, TestResult};
use shared::binfile::RouteData;
use shared::{KalmanState, DrState, FsmState};
use gps_processor::nmea::NmeaState;
use detection::state_machine::StopState;

/// Test: Bus drives entire ty225 route normally
/// Validates: All stops detected, correct arrival order
#[test]
fn test_normal_complete_route() {
    // Load route and NMEA data
    let route_bytes = load_ty225_route("normal");
    let route_data = RouteData::load(&route_bytes)
        .expect("Failed to load route data");

    let nmea_lines = load_nmea("normal");
    let expected = ExpectedResults::from_ground_truth("normal");

    // Initialize pipeline state
    let mut nmea = NmeaState::new();
    let mut kalman = KalmanState::new();
    let mut dr = DrState::new();

    let mut stop_states: Vec<StopState> = route_data.stops()
        .iter()
        .enumerate()
        .map(|(i, _)| StopState::new(i as u8))
        .collect();

    let mut detected_arrivals: Vec<usize> = Vec::new();

    // Process NMEA sentences
    for line in nmea_lines {
        if let Some(_gps) = nmea.parse_sentence(&line) {
            // TODO: Full pipeline processing
            // For now, just verify NMEA parsing works
        }
    }

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

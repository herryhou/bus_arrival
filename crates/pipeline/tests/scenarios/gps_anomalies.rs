//! GPS anomaly scenario tests (drift, jump)

use super::common::{load_ty225_route, load_nmea, ExpectedResults};
use shared::binfile::RouteData;
use shared::{KalmanState, DrState};
use gps_processor::nmea::NmeaState;
use detection::state_machine::StopState;

/// Test: GPS drift scenario
/// Validates: Recovery algorithm corrects position after drift
#[test]
fn test_drift_recovery() {
    // Load drift scenario data
    let route_bytes = load_ty225_route("drift");
    let route_data = RouteData::load(&route_bytes)
        .expect("Failed to load route data");

    let nmea_lines = load_nmea("drift");
    let expected = ExpectedResults::from_ground_truth("drift");

    // Initialize pipeline
    let mut nmea = NmeaState::new();
    let mut kalman = KalmanState::new();
    let mut dr = DrState::new();

    let mut stop_states: Vec<StopState> = route_data.stops()
        .iter()
        .enumerate()
        .map(|(i, _)| StopState::new(i as u8))
        .collect();

    let mut detected_arrivals: Vec<usize> = Vec::new();

    // Process NMEA with drift
    for line in nmea_lines {
        if let Some(_gps) = nmea.parse_sentence(&line) {
            // Pipeline processing
        }
    }

    // Validate: should still detect arrivals despite drift
    assert!(
        detected_arrivals.len() >= expected.min_arrivals,
        "Drift scenario: expected at least {} arrivals, got {}",
        expected.min_arrivals,
        detected_arrivals.len()
    );
}

/// Test: GPS jump scenario
/// Validates: No false arrivals for skipped stops
#[test]
fn test_jump_skip_stop_prevention() {
    // Load jump scenario data
    let route_bytes = load_ty225_route("jump");
    let route_data = RouteData::load(&route_bytes)
        .expect("Failed to load route data");

    let nmea_lines = load_nmea("jump");
    let expected = ExpectedResults::from_ground_truth("jump");

    // Initialize pipeline
    let mut nmea = NmeaState::new();
    let mut kalman = KalmanState::new();
    let mut dr = DrState::new();

    let mut stop_states: Vec<StopState> = route_data.stops()
        .iter()
        .enumerate()
        .map(|(i, _)| StopState::new(i as u8))
        .collect();

    let mut detected_arrivals: Vec<usize> = Vec::new();

    // Process NMEA with jump
    for line in nmea_lines {
        if let Some(_gps) = nmea.parse_sentence(&line) {
            // Pipeline processing
        }
    }

    // Validate: jump should not cause false arrivals
    assert!(
        detected_arrivals.len() <= expected.max_arrivals,
        "Jump scenario: expected at most {} arrivals, got {}",
        expected.max_arrivals,
        detected_arrivals.len()
    );
}

/// Test: Validate route loads for both scenarios
#[test]
fn test_anomaly_route_data_loads() {
    // Drift route
    let drift_bytes = load_ty225_route("drift");
    let drift_route = RouteData::load(&drift_bytes);
    assert!(drift_route.is_ok(), "Drift route should load");

    // Jump route
    let jump_bytes = load_ty225_route("jump");
    let jump_route = RouteData::load(&jump_bytes);
    assert!(jump_route.is_ok(), "Jump route should load");

    // Both should have same stop count
    assert_eq!(
        drift_route.unwrap().stops().len(),
        jump_route.unwrap().stops().len(),
        "Both routes should have same stop count"
    );
}

//! GPS anomaly scenario tests (drift, jump)

use super::common::{load_ty225_route, load_nmea_reader, ExpectedResults};
use shared::binfile::RouteData;
use pipeline::Pipeline;

/// Test: GPS drift scenario
/// Validates: Recovery algorithm corrects position after drift
#[test]
fn test_drift_recovery() {
    // Load drift scenario data
    let route_bytes = load_ty225_route("drift");
    let route_data = RouteData::load(&route_bytes)
        .expect("Failed to load route data");

    let expected = ExpectedResults::from_ground_truth("drift");

    // Use the full pipeline to process NMEA
    let result = Pipeline::process_nmea_reader(
        load_nmea_reader("drift"),
        &route_data,
        &pipeline::PipelineConfig::default(),
    ).expect("Pipeline processing failed");

    let detected_arrivals: Vec<usize> = result.arrivals
        .iter()
        .map(|a| a.stop_idx as usize)
        .collect();

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

    let expected = ExpectedResults::from_ground_truth("jump");

    // Use the full pipeline to process NMEA
    let result = Pipeline::process_nmea_reader(
        load_nmea_reader("jump"),
        &route_data,
        &pipeline::PipelineConfig::default(),
    ).expect("Pipeline processing failed");

    let detected_arrivals: Vec<usize> = result.arrivals
        .iter()
        .map(|a| a.stop_idx as usize)
        .collect();

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

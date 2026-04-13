//! GPS anomaly scenario tests (drift, jump)

use super::common::{load_ty225_route, load_nmea_reader, ExpectedResults};
use super::common::{validate_arrivals_exact, load_expected_arrivals, validate_arrival_order};
use shared::binfile::RouteData;
use pipeline::Pipeline;

/// Test: GPS drift scenario
/// Validates: Recovery algorithm corrects position after drift
#[test]
fn test_drift_recovery() {
    // Disable debug output for this test run
    // eprintln!("Route has {} stops", route_data.stops().len());

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

/// Test: Exact stop matching for drift scenario
/// Validates: Recovery algorithm detects correct stops despite GPS drift
#[test]
fn test_drift_exact_stop_matching() {
    let route_bytes = load_ty225_route("drift");
    let route_data = RouteData::load(&route_bytes)
        .expect("Failed to load route data");

    let expected_arrivals = load_expected_arrivals("drift");

    let result = Pipeline::process_nmea_reader(
        load_nmea_reader("drift"),
        &route_data,
        &pipeline::PipelineConfig::default(),
    ).expect("Pipeline processing failed");

    let detected_arrivals: Vec<usize> = result.arrivals
        .iter()
        .map(|a| a.stop_idx as usize)
        .collect();

    // Exact match: drift should not cause missed or ghost stops
    let validation = validate_arrivals_exact(&detected_arrivals, &expected_arrivals);
    validation.print_report();

    // Allow slightly lower threshold for drift (95%)
    validation.assert_quality(0.95, 0.95)
        .unwrap();

    // Order must be maintained even with drift
    validate_arrival_order(&detected_arrivals)
        .unwrap();
}

/// Test: Exact stop matching for jump scenario
/// Validates: No false arrivals for skipped stops
#[test]
fn test_jump_exact_stop_matching() {
    let route_bytes = load_ty225_route("jump");
    let route_data = RouteData::load(&route_bytes)
        .expect("Failed to load route data");

    let expected_arrivals = load_expected_arrivals("jump");

    let result = Pipeline::process_nmea_reader(
        load_nmea_reader("jump"),
        &route_data,
        &pipeline::PipelineConfig::default(),
    ).expect("Pipeline processing failed");

    let detected_arrivals: Vec<usize> = result.arrivals
        .iter()
        .map(|a| a.stop_idx as usize)
        .collect();

    // Jump scenario: critical to have no false positives
    let validation = validate_arrivals_exact(&detected_arrivals, &expected_arrivals);
    validation.print_report();

    // High precision required (no ghost stops from jumps)
    validation.assert_quality(0.98, 0.95)
        .unwrap();

    // Order must be maintained
    validate_arrival_order(&detected_arrivals)
        .unwrap();
}

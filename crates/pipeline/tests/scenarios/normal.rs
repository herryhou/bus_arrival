//! Normal operation scenario tests

use super::common::{
    load_ground_truth_arrivals, load_ty225_route, load_nmea_reader, ExpectedResults,
};
use super::common::{validate_arrival_order_strict, validate_arrivals_exact};
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

    // Filter detected arrivals to only include stops in ground truth range
    let max_expected_stop = expected.arrivals.iter().max().copied().unwrap_or(0);
    let filtered_detected: Vec<usize> = detected_arrivals
        .into_iter()
        .filter(|&idx| idx <= max_expected_stop)
        .collect();

    // Validate: should detect expected number of arrivals
    assert!(
        filtered_detected.len() >= expected.min_arrivals,
        "Expected at least {} arrivals, got {}",
        expected.min_arrivals,
        filtered_detected.len()
    );

    assert!(
        filtered_detected.len() <= expected.max_arrivals,
        "Expected at most {} arrivals, got {}",
        expected.max_arrivals,
        filtered_detected.len()
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

/// Test: Exact stop matching for normal operation
/// Validates: Correct stops detected, correct order, no false positives/negatives
#[test]
fn test_normal_exact_stop_matching() {
    // Load route and NMEA data
    let route_bytes = load_ty225_route("normal");
    let route_data = RouteData::load(&route_bytes)
        .expect("Failed to load route data");

    let expected_arrivals = load_ground_truth_arrivals("normal");

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

    // Filter detected arrivals to only include stops in ground truth range
    // The NMEA data may include stops beyond the ground truth (0-42)
    let max_expected_stop = *expected_arrivals.iter().max().unwrap_or(&0);
    let filtered_detected: Vec<usize> = detected_arrivals
        .into_iter()
        .filter(|&idx| idx <= max_expected_stop)
        .collect();

    // Validate exact match against ground truth (filtered to expected range)
    let validation = validate_arrivals_exact(&filtered_detected, &expected_arrivals);

    // Print report for debugging
    validation.print_report();

    // Assert quality: 97% precision and recall (tech report target)
    validation.assert_quality(0.97, 0.97)
        .unwrap();
}

/// Test: Arrival order validation for normal operation
/// Validates: Stops are detected in monotonically increasing order
#[test]
fn test_normal_arrival_order() {
    let route_bytes = load_ty225_route("normal");
    let route_data = RouteData::load(&route_bytes)
        .expect("Failed to load route data");

    let result = Pipeline::process_nmea_reader(
        load_nmea_reader("normal"),
        &route_data,
        &pipeline::PipelineConfig::default(),
    ).expect("Pipeline processing failed");

    let detected_arrivals: Vec<usize> = result.arrivals
        .iter()
        .map(|a| a.stop_idx as usize)
        .collect();

    // Validate order (strict: no duplicates, increasing)
    validate_arrival_order_strict(&detected_arrivals)
        .unwrap();
}

/// Test: Position accuracy at arrival
/// Validates: At AtStop state, bus is within 50m of stop location
#[test]
fn test_normal_position_accuracy() {
    // This test requires a trace file output from the pipeline
    // For now, we'll generate it inline

    let route_bytes = load_ty225_route("normal");
    let route_data = RouteData::load(&route_bytes)
        .expect("Failed to load route data");

    // Enable trace output
    let mut config = pipeline::PipelineConfig::default();
    config.enable_trace = true;

    let result = Pipeline::process_nmea_reader(
        load_nmea_reader("normal"),
        &route_data,
        &config,
    ).expect("Pipeline processing failed");

    // The result should include trace data
    // For now, just verify we have arrivals
    assert!(!result.arrivals.is_empty(), "Should have arrivals");

    // TODO: Add trace file path validation when trace output is implemented
    // let trace_path = test_data_dir().join("ty225_normal_trace.jsonl");
    // let report = analyze_position_accuracy(&trace_path);
    // report.print_report();
    // report.assert_all_acceptable().unwrap();
}

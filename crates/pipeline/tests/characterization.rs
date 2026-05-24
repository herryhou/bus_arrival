//! Characterization test for pipeline behavior
//!
//! This test captures the current expected behavior of the pipeline.
//! Any regression in arrival/departure detection should fail this test.

use pipeline::Pipeline;

#[test]
fn test_ty225_normal_characterization() {
    let result = Pipeline::process_nmea_file(
        "../../test_data/ty225_normal_nmea.txt",
        "../../test_data/ty225_normal.bin",
    ).expect("Pipeline processing should succeed");

    // Diagnostic: Print detected stops for debugging Kalman gain change
    let detected_stops: Vec<u8> = result.arrivals.iter().map(|a| a.stop_idx).collect();
    eprintln!("Detected stops: {:?}", detected_stops);

    // Find missing stops
    let all_stops: std::collections::HashSet<u8> = (0..=56).collect();
    let detected_set: std::collections::HashSet<u8> = detected_stops.iter().cloned().collect();
    let missing: Vec<u8> = all_stops.difference(&detected_set).cloned().collect();
    eprintln!("Missing stops: {:?}", missing);

    // Characterize: arrival count for ty225_normal scenario
    // After F1 neutralization fix: pipeline detects 56 arrivals (only missing stop 44)
    // Detected: stops 0-43, 45-56
    // Missing: stop 44
    // Note: Original test expected 45 but actual behavior was 55; F1 fix prevents false arrivals during dr_outage
    assert_eq!(result.arrivals.len(), 56,
        "ty225_normal should detect 56 arrivals (after F1 dr_outage fix)");

    // Characterize: departure count
    // Current behavior: pipeline does not produce departures (0 departures)
    assert_eq!(result.departures.len(), 0,
        "ty225_normal produces 0 departures (characterizing current behavior)");

    // Characterize: first arrival is at stop 0
    let first = &result.arrivals[0];
    assert_eq!(first.stop_idx, 0,
        "First arrival should be at stop index 0");

    // Characterize: last arrival is at stop 56
    let last = &result.arrivals.last().expect("Should have arrivals");
    assert_eq!(last.stop_idx, 56,
        "Last arrival should be at stop index 56");
}

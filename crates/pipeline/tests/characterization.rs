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

    // Characterize: arrival count for ty225_normal scenario
    // Current behavior: pipeline detects arrivals at all 57 stops (stop_idx 0-56)
    assert_eq!(result.arrivals.len(), 57,
        "ty225_normal should detect 57 arrivals (characterizing current behavior)");

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

//! Regression tests for bugs found during detour testing
//!
//! Each test documents a specific bug that was found and fixed.
//! Tests use saved NMEA files from test_data/regression/.
//!
//! ## Adding New Regression Tests
//!
//! When you find a bug in detour testing:
//! 1. Run: `./tools/save_regression.sh <case-name> "<description>"`
//! 2. Implement the test assertions
//! 3. Fix the bug in code
//! 4. Verify test passes
//!
//! ## Running Tests
//!
//! ```bash
//! # Run all regression tests
//! cargo test -p pipeline --test regression_tests
//!
//! # Run specific test
//! cargo test -p pipeline --test regression_tests -- test_<case_name>
//! ```

use std::fs;
use std::path::PathBuf;

/// Test data directory path
fn test_data_dir() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("../../test_data");
    path
}

// ============================================================================
// Helper: Add your regression tests below
// ============================================================================
//
// Each test should follow the pattern:
//   1. Clear documentation of the bug (in comments)
//   2. Load the regression NMEA file
//   3. Load and process through pipeline
//   4. Assert the bug is fixed
//
// Example:
//
// #[test]
// fn test_detour_stop_skip_backtrack() {
//     // Bug: Progress stuck at stop 2 after detour re-entry to stop 6
//     // Root cause: Kalman filter not updating last_seg_idx during DR
//     // Fix: crates/pipeline/gps_processor/src/kalman.rs:123
//
//     let nmea_file = "../../../test_data/regression/ty225_short_detour_stop_skip_backtrack.txt";
//     let route_bin = "../../../test_data/ty225_short_detour.bin";
//
//     // Load and process...
//     // Assert progress jumped to stop 6 area (~1775m)
// }

#[test]
fn test_skip_stop5_on_offroute_reentry() {
    // Bug: Stop 5 should be SKIPPED on re-entry from off-route
    // Root cause: The snap progress on re-entry is near stop 5, but the snap point is located
    //            between stop 5 and stop 6, so stop 5 should be skipped
    // Fix: detection/src/arrival_detector.rs - arrival detection logic needs to skip stops
    //      when snap point is past them on re-entry
    //
    // This test ensures the bug does not regress.

    // Build file paths using test_data_dir for proper resolution
    let mut nmea_path = test_data_dir();
    nmea_path.push("regression/ty225_short_detour_stop6_missed_at_reentry.txt");

    let mut route_bin_path = test_data_dir();
    route_bin_path.push("ty225_short_detour.bin");

    // Load route data
    let route_bytes = fs::read(&route_bin_path).expect("Failed to load route data");
    let route_data =
        shared::binfile::RouteData::load(&route_bytes).expect("Failed to parse route data");

    // Load NMEA and process through pipeline
    let nmea_file = fs::File::open(&nmea_path).expect("Failed to open NMEA file");
    let reader = std::io::BufReader::new(nmea_file);

    let result = pipeline::Pipeline::process_nmea_reader(reader, &route_data)
        .expect("Pipeline processing failed");

    // Extract detected stops from trace_records based on any Approaching/Arriving/AtStop states
    let mut detected_stops = std::collections::HashSet::new();

    for record in &result.trace_records {
        for state in &record.stop_states {
            if matches!(state.fsm_state, shared::FsmState::Approaching | shared::FsmState::Arriving | shared::FsmState::AtStop) {
                detected_stops.insert(state.stop_idx as usize);
            }
        }
    }

    let mut mut_detected: Vec<_> = detected_stops.into_iter().collect();
    mut_detected.sort();

    // CRITICAL ASSERTION: Stop 5 should be SKIPPED
    // The snap point on re-entry is between stop 5 and 6, so stop 5 should not be detected
    assert!(
        !mut_detected.contains(&5),
        "Stop 5 should be SKIPPED on off-route re-entry. Detected: {:?}",
        mut_detected
    );

    // Expected stops: [0, 1, 6, 7, 8, 9] (stop 5 is skipped)
    let expected_stops = vec![0, 1, 6, 7, 8, 9];
    assert_eq!(
        mut_detected, expected_stops,
        "Expected stops [0, 1, 6, 7, 8, 9] with stop 5 skipped. Got: {:?}",
        mut_detected
    );

    println!("✓ Stop 5 correctly skipped on off-route re-entry");
    println!("  Detected stops: {:?} (stop 5 skipped)", mut_detected);
}

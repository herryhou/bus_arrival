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
// Regression Test Cases
// ============================================================================

/// Template for new regression tests
///
/// To add a new test:
/// 1. Use `./tools/save_regression.sh <name> "<desc>"` to save the NMEA
/// 2. Copy this template and implement the assertions
/// 3. Fix the bug in code
/// 4. Verify test passes
#[test]
fn test_template_regression_case() {
    // Bug: [Description of the bug]
    // Root cause: [Why it happened]
    // Fix: [Where in code it was fixed]
    //
    // This test ensures the bug does not regress.

    let nmea_file = "../../../test_data/regression/ty225_short_detour_template.txt";
    let route_bin = "../../../test_data/ty225_short_detour.bin";

    // Verify test files exist
    assert!(
        fs::metadata(nmea_file).is_ok(),
        "Regression NMEA file should exist. Run './tools/save_regression.sh template <desc>' to create it."
    );

    // Load route data (inline to avoid lifetime issues)
    let route_bytes = fs::read(route_bin)
        .expect("Failed to load route data");
    let _route_data = shared::binfile::RouteData::load(&route_bytes)
        .expect("Failed to parse route data");

    // Load NMEA file
    let nmea_file = fs::File::open(nmea_file)
        .expect("Failed to open NMEA file");
    let _reader = std::io::BufReader::new(nmea_file);

    // TODO: Run pipeline processing
    // For now, this is a placeholder that just verifies files load

    // TODO: Add assertions that validate the fix
    // Examples:
    // assert!(result.final_progress_cm.unwrap() > 170_000, "Progress should be at stop 6");
    // assert!(result.final_stop_idx.unwrap() == 6, "Should detect stop 6");

    // For template, just verify files load
    assert!(true, "Template test - implement assertions");
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
fn test_stop6_missed_at_reentry() {
    // Bug: Stop 6 should be detected with 8s dwell at detour re-entry but is currently missed
    // Root cause: Bus moves too fast through stop 6's corridor after detour re-entry snap
    // Fix: detection/src/arrival_detector.rs - arrival detection logic needs to handle re-entry dwell
    //
    // This test ensures the bug does not regress.

    // Build file paths using test_data_dir for proper resolution
    let mut nmea_path = test_data_dir();
    nmea_path.push("regression/ty225_short_detour_stop6_missed_at_reentry.txt");

    let mut route_bin_path = test_data_dir();
    route_bin_path.push("ty225_short_detour.bin");

    // Load route data
    let route_bytes = fs::read(&route_bin_path)
        .expect("Failed to load route data");
    let route_data = shared::binfile::RouteData::load(&route_bytes)
        .expect("Failed to parse route data");

    // Load NMEA and process through pipeline
    let nmea_file = fs::File::open(&nmea_path)
        .expect("Failed to open NMEA file");
    let reader = std::io::BufReader::new(nmea_file);

    let result = pipeline::Pipeline::process_nmea_reader(
        reader,
        &route_data,
    )
    .expect("Pipeline processing failed");

    // Extract detected arrivals
    let detected_stops: Vec<usize> = result
        .arrivals
        .iter()
        .map(|a| a.stop_idx as usize)
        .collect();

    // CRITICAL ASSERTION: Stop 6 should be detected as an arrival
    // This is the core bug - stop 6 is currently missed
    assert!(
        detected_stops.contains(&6),
        "Stop 6 should be DETECTED as an arrival at detour re-entry. Detected: {:?}",
        detected_stops
    );

    // Verify the arrival at stop 6 has appropriate dwell time
    let stop6_arrival = result.arrivals.iter()
        .find(|a| a.stop_idx == 6)
        .expect("Stop 6 arrival should exist");

    // Ground truth shows 8 seconds of dwell at stop 6
    // Allow some tolerance for timing variations
    assert!(
        stop6_arrival.time >= 80143 && stop6_arrival.time <= 80160,
        "Stop 6 arrival should occur around re-entry tick 80143. Got time: {}",
        stop6_arrival.time
    );

    println!("✓ Stop 6 correctly detected at detour re-entry");
    println!("  Arrival time: {}", stop6_arrival.time);
    println!("  All detected stops: {:?}", detected_stops);
}

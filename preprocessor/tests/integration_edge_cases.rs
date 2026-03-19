//! Cross-Module Integration Edge Cases
//!
//! Tests for edge cases that involve multiple modules working together:
//! - Real-world routes with all edge cases
//! - Minimal valid route
//! - Maximum load route
//! - Error propagation for invalid inputs

use assert_cmd::Command;
use serde_json::json;
use shared::binfile::RouteData;
use std::fs;
use tempfile::NamedTempFile;

// ============================================================================
// Full Pipeline Integration Tests
// ============================================================================

#[test]
fn test_minimal_valid_route() {
    // --- GIVEN ---
    // A route with just 2 points and 1 stop
    let route_json = json!({
        "route_points": [[25.0, 121.0], [25.01, 121.0]]
    }).to_string();

    let stops_json = json!({
        "stops": [{"lat": 25.0, "lon": 121.0}]
    }).to_string();

    let route_file = NamedTempFile::new().unwrap();
    let stops_file = NamedTempFile::new().unwrap();
    let output_file = NamedTempFile::new().unwrap();

    fs::write(route_file.path(), route_json).unwrap();
    fs::write(stops_file.path(), stops_json).unwrap();

    // --- WHEN ---
    Command::cargo_bin("preprocessor").unwrap()
        .arg(route_file.path())
        .arg(stops_file.path())
        .arg(output_file.path())
        .assert()
        .success();

    // --- THEN ---
    let bin_data = fs::read(output_file.path()).unwrap();
    let route_data = RouteData::load(&bin_data).expect("Should load");

    assert_eq!(route_data.stop_count, 1, "should have 1 stop");

    let stop = route_data.get_stop(0).unwrap();
    assert_eq!(stop.progress_cm, 0, "stop at route start");

    // Should have valid nodes (at least 2)
    assert!(route_data.node_count >= 2, "minimal route: at least 2 nodes");
}

#[test]
fn test_real_world_route_with_edge_cases() {
    // --- GIVEN ---
    // A route that includes sharp turns, close stops, and U-turns
    let route_json = json!({
        "route_points": [
            [25.0, 121.0],
            [25.01, 121.0],   // 1.1km east
            [25.01, 121.01],  // 1.1km north (sharp turn)
            [25.0, 121.01],   // 1.1km west (U-turn)
            [25.0, 121.02]    // 1.1km east (completes U)
        ]
    }).to_string();

    let stops_json = json!({
        "stops": [
            {"lat": 25.0, "lon": 121.0},      // Start
            {"lat": 25.01, "lon": 121.0},     // At first turn
            {"lat": 25.01, "lon": 121.01},    // At second turn
            {"lat": 25.0, "lon": 121.01},     // At third turn
            {"lat": 25.0, "lon": 121.02}      // End
        ]
    }).to_string();

    let route_file = NamedTempFile::new().unwrap();
    let stops_file = NamedTempFile::new().unwrap();
    let output_file = NamedTempFile::new().unwrap();

    fs::write(route_file.path(), route_json).unwrap();
    fs::write(stops_file.path(), stops_json).unwrap();

    // --- WHEN ---
    Command::cargo_bin("preprocessor").unwrap()
        .arg(route_file.path())
        .arg(stops_file.path())
        .arg(output_file.path())
        .assert()
        .success();

    // --- THEN ---
    let bin_data = fs::read(output_file.path()).unwrap();
    let route_data = RouteData::load(&bin_data).expect("Should load");

    assert_eq!(route_data.stop_count, 5);

    // Verify monotonicity
    for i in 0..route_data.stop_count - 1 {
        let stop1 = route_data.get_stop(i).unwrap();
        let stop2 = route_data.get_stop(i + 1).unwrap();
        assert!(
            stop1.progress_cm <= stop2.progress_cm,
            "real-world edge cases: monotonicity at {}: {} <= {}",
            i,
            stop1.progress_cm,
            stop2.progress_cm
        );
    }

    // Binary output should be valid
    assert!(route_data.node_count > 0);
}

#[test]
fn test_maximum_load_route() {
    // --- GIVEN ---
    // A route with 100 points and 20 stops (stress test)
    let mut route_points = Vec::new();
    for i in 0..100 {
        route_points.push(vec![25.0 + (i as f64 * 0.0001), 121.0]);
    }

    let mut stops = Vec::new();
    for i in 0..20 {
        stops.push(json!({
            "lat": 25.0 + (i as f64 * 0.0005),
            "lon": 121.0
        }));
    }

    let route_json = json!({ "route_points": route_points }).to_string();
    let stops_json = json!({ "stops": stops }).to_string();

    let route_file = NamedTempFile::new().unwrap();
    let stops_file = NamedTempFile::new().unwrap();
    let output_file = NamedTempFile::new().unwrap();

    fs::write(route_file.path(), route_json).unwrap();
    fs::write(stops_file.path(), stops_json).unwrap();

    // --- WHEN ---
    Command::cargo_bin("preprocessor").unwrap()
        .arg(route_file.path())
        .arg(stops_file.path())
        .arg(output_file.path())
        .assert()
        .success();

    // --- THEN ---
    let bin_data = fs::read(output_file.path()).unwrap();
    let route_data = RouteData::load(&bin_data).expect("Should load");

    assert_eq!(route_data.stop_count, 20);

    // Processing should complete in reasonable time
    // Binary size should be within limits
    assert!(bin_data.len() < 1_000_000, "binary size < 1MB for stress test");
}

#[test]
fn test_dense_stops_integration() {
    // --- GIVEN ---
    // More stops than route segments
    let route_json = json!({
        "route_points": [
            [25.0, 121.0],
            [25.01, 121.0],
            [25.02, 121.0]
        ]
    }).to_string();

    let stops_json = json!({
        "stops": [
            {"lat": 25.0, "lon": 121.0},
            {"lat": 25.005, "lon": 121.0},
            {"lat": 25.01, "lon": 121.0},
            {"lat": 25.015, "lon": 121.0},
            {"lat": 25.02, "lon": 121.0}
        ]
    }).to_string();

    let route_file = NamedTempFile::new().unwrap();
    let stops_file = NamedTempFile::new().unwrap();
    let output_file = NamedTempFile::new().unwrap();

    fs::write(route_file.path(), route_json).unwrap();
    fs::write(stops_file.path(), stops_json).unwrap();

    // --- WHEN ---
    Command::cargo_bin("preprocessor").unwrap()
        .arg(route_file.path())
        .arg(stops_file.path())
        .arg(output_file.path())
        .assert()
        .success();

    // --- THEN ---
    let bin_data = fs::read(output_file.path()).unwrap();
    let route_data = RouteData::load(&bin_data).expect("Should load");

    assert_eq!(route_data.stop_count, 5);

    // Verify monotonicity for dense stops
    for i in 0..route_data.stop_count - 1 {
        let stop1 = route_data.get_stop(i).unwrap();
        let stop2 = route_data.get_stop(i + 1).unwrap();
        assert!(
            stop1.progress_cm <= stop2.progress_cm,
            "dense stops integration: monotonicity"
        );
    }
}

// ============================================================================
// Error Propagation Tests
// ============================================================================

#[test]
fn test_missing_route_field() {
    // --- GIVEN ---
    // Route JSON with missing required field
    let route_json = json!({
        "points": [[25.0, 121.0], [25.01, 121.0]]  // Wrong field name
    }).to_string();

    let stops_json = json!({
        "stops": [{"lat": 25.0, "lon": 121.0}]
    }).to_string();

    let route_file = NamedTempFile::new().unwrap();
    let stops_file = NamedTempFile::new().unwrap();
    let output_file = NamedTempFile::new().unwrap();

    fs::write(route_file.path(), route_json).unwrap();
    fs::write(stops_file.path(), stops_json).unwrap();

    // --- WHEN ---
    let result = Command::cargo_bin("preprocessor").unwrap()
        .arg(route_file.path())
        .arg(stops_file.path())
        .arg(output_file.path())
        .assert();

    // --- THEN ---
    // Should fail with clear error message
    result.failure();
}

#[test]
fn test_missing_stops_field() {
    // --- GIVEN ---
    // Stops JSON with missing required field
    let route_json = json!({
        "route_points": [[25.0, 121.0], [25.01, 121.0]]
    }).to_string();

    let stops_json = json!({
        "stop": [{"lat": 25.0, "lon": 121.0}]  // Wrong field name
    }).to_string();

    let route_file = NamedTempFile::new().unwrap();
    let stops_file = NamedTempFile::new().unwrap();
    let output_file = NamedTempFile::new().unwrap();

    fs::write(route_file.path(), route_json).unwrap();
    fs::write(stops_file.path(), stops_json).unwrap();

    // --- WHEN ---
    let result = Command::cargo_bin("preprocessor").unwrap()
        .arg(route_file.path())
        .arg(stops_file.path())
        .arg(output_file.path())
        .assert();

    // --- THEN ---
    result.failure();
}

#[test]
fn test_invalid_gps_coordinates() {
    // --- GIVEN ---
    // Stop with invalid latitude > 90
    let route_json = json!({
        "route_points": [[25.0, 121.0], [25.01, 121.0]]
    }).to_string();

    let stops_json = json!({
        "stops": [{"lat": 95.0, "lon": 121.0}]  // Invalid lat
    }).to_string();

    let route_file = NamedTempFile::new().unwrap();
    let stops_file = NamedTempFile::new().unwrap();
    let output_file = NamedTempFile::new().unwrap();

    fs::write(route_file.path(), route_json).unwrap();
    fs::write(stops_file.path(), stops_json).unwrap();

    // --- WHEN ---
    let result = Command::cargo_bin("preprocessor").unwrap()
        .arg(route_file.path())
        .arg(stops_file.path())
        .arg(output_file.path())
        .assert();

    // --- THEN ---
    // Should fail with validation error
    result.failure();
}

#[test]
fn test_invalid_longitude() {
    // --- GIVEN ---
    // Stop with invalid longitude > 180
    let route_json = json!({
        "route_points": [[25.0, 121.0], [25.01, 121.0]]
    }).to_string();

    let stops_json = json!({
        "stops": [{"lat": 25.0, "lon": 185.0}]  // Invalid lon
    }).to_string();

    let route_file = NamedTempFile::new().unwrap();
    let stops_file = NamedTempFile::new().unwrap();
    let output_file = NamedTempFile::new().unwrap();

    fs::write(route_file.path(), route_json).unwrap();
    fs::write(stops_file.path(), stops_json).unwrap();

    // --- WHEN ---
    let result = Command::cargo_bin("preprocessor").unwrap()
        .arg(route_file.path())
        .arg(stops_file.path())
        .arg(output_file.path())
        .assert();

    // --- THEN ---
    result.failure();
}

#[test]
fn test_empty_route_points() {
    // --- GIVEN ---
    // Empty route_points array
    let route_json = json!({
        "route_points": []
    }).to_string();

    let stops_json = json!({
        "stops": [{"lat": 25.0, "lon": 121.0}]
    }).to_string();

    let route_file = NamedTempFile::new().unwrap();
    let stops_file = NamedTempFile::new().unwrap();
    let output_file = NamedTempFile::new().unwrap();

    fs::write(route_file.path(), route_json).unwrap();
    fs::write(stops_file.path(), stops_json).unwrap();

    // --- WHEN ---
    let result = Command::cargo_bin("preprocessor").unwrap()
        .arg(route_file.path())
        .arg(stops_file.path())
        .arg(output_file.path())
        .assert();

    // --- THEN ---
    // Should handle empty route gracefully
    result.failure();
}

#[test]
fn test_single_route_point() {
    // --- GIVEN ---
    // Only one route point (degenerate case)
    let route_json = json!({
        "route_points": [[25.0, 121.0]]
    }).to_string();

    let stops_json = json!({
        "stops": [{"lat": 25.0, "lon": 121.0}]
    }).to_string();

    let route_file = NamedTempFile::new().unwrap();
    let stops_file = NamedTempFile::new().unwrap();
    let output_file = NamedTempFile::new().unwrap();

    fs::write(route_file.path(), route_json).unwrap();
    fs::write(stops_file.path(), stops_json).unwrap();

    // --- WHEN ---
    let result = Command::cargo_bin("preprocessor").unwrap()
        .arg(route_file.path())
        .arg(stops_file.path())
        .arg(output_file.path())
        .assert();

    // --- THEN ---
    // Single-point routes may fail validation due to degenerate geometry
    // This is expected behavior - routes need at least 2 points for meaningful stop progression
    // The test checks that the preprocessor handles this edge case gracefully
    result.failure();
}

#[test]
fn test_empty_stops() {
    // --- GIVEN ---
    let route_json = json!({
        "route_points": [[25.0, 121.0], [25.01, 121.0]]
    }).to_string();

    let stops_json = json!({
        "stops": []
    }).to_string();

    let route_file = NamedTempFile::new().unwrap();
    let stops_file = NamedTempFile::new().unwrap();
    let output_file = NamedTempFile::new().unwrap();

    fs::write(route_file.path(), route_json).unwrap();
    fs::write(stops_file.path(), stops_json).unwrap();

    // --- WHEN ---
    Command::cargo_bin("preprocessor").unwrap()
        .arg(route_file.path())
        .arg(stops_file.path())
        .arg(output_file.path())
        .assert()
        .success();

    // --- THEN ---
    let bin_data = fs::read(output_file.path()).unwrap();
    let route_data = RouteData::load(&bin_data).expect("Should load");

    assert_eq!(route_data.stop_count, 0);
}

// ============================================================================
// Binary Format Edge Cases
// ============================================================================

#[test]
fn test_binary_valid_output_format() {
    // --- GIVEN ---
    let route_json = json!({
        "route_points": [[25.0, 121.0], [25.01, 121.0], [25.02, 121.0]]
    }).to_string();

    let stops_json = json!({
        "stops": [
            {"lat": 25.0, "lon": 121.0},
            {"lat": 25.01, "lon": 121.0}
        ]
    }).to_string();

    let route_file = NamedTempFile::new().unwrap();
    let stops_file = NamedTempFile::new().unwrap();
    let output_file = NamedTempFile::new().unwrap();

    fs::write(route_file.path(), route_json).unwrap();
    fs::write(stops_file.path(), stops_json).unwrap();

    // --- WHEN ---
    Command::cargo_bin("preprocessor").unwrap()
        .arg(route_file.path())
        .arg(stops_file.path())
        .arg(output_file.path())
        .assert()
        .success();

    // --- THEN ---
    let bin_data = fs::read(output_file.path()).unwrap();

    // Binary should have valid header
    assert!(bin_data.len() > 8, "binary has header");

    // Should be deserializable
    let route_data = RouteData::load(&bin_data).expect("Should load");
    assert_eq!(route_data.stop_count, 2);
    assert!(route_data.node_count > 0);
}

#[test]
fn test_binary_corrupted_data() {
    // --- GIVEN ---
    let route_json = json!({
        "route_points": [[25.0, 121.0], [25.01, 121.0]]
    }).to_string();

    let stops_json = json!({
        "stops": [{"lat": 25.0, "lon": 121.0}]
    }).to_string();

    let route_file = NamedTempFile::new().unwrap();
    let stops_file = NamedTempFile::new().unwrap();
    let output_file = NamedTempFile::new().unwrap();

    fs::write(route_file.path(), route_json).unwrap();
    fs::write(stops_file.path(), stops_json).unwrap();

    Command::cargo_bin("preprocessor").unwrap()
        .arg(route_file.path())
        .arg(stops_file.path())
        .arg(output_file.path())
        .assert()
        .success();

    // Corrupt the binary data
    let mut bin_data = fs::read(output_file.path()).unwrap();
    if bin_data.len() > 10 {
        bin_data[5] = 0xFF; // Corrupt a byte
    }

    // --- WHEN ---
    let result = RouteData::load(&bin_data);

    // --- THEN ---
    // Should fail gracefully
    assert!(result.is_err(), "corrupted binary should fail to load");
}

// ============================================================================
// Flash Size Constraints
// ============================================================================

#[test]
fn test_flash_size_within_limits() {
    // --- GIVEN ---
    // A typical route (100 points, 30 stops)
    let mut route_points = Vec::new();
    for i in 0..100 {
        route_points.push(vec![25.0 + (i as f64 * 0.0001), 121.0]);
    }

    let mut stops = Vec::new();
    for i in 0..30 {
        stops.push(json!({
            "lat": 25.0 + (i as f64 * 0.0003),
            "lon": 121.0
        }));
    }

    let route_json = json!({ "route_points": route_points }).to_string();
    let stops_json = json!({ "stops": stops }).to_string();

    let route_file = NamedTempFile::new().unwrap();
    let stops_file = NamedTempFile::new().unwrap();
    let output_file = NamedTempFile::new().unwrap();

    fs::write(route_file.path(), route_json).unwrap();
    fs::write(stops_file.path(), stops_json).unwrap();

    // --- WHEN ---
    Command::cargo_bin("preprocessor").unwrap()
        .arg(route_file.path())
        .arg(stops_file.path())
        .arg(output_file.path())
        .assert()
        .success();

    // --- THEN ---
    let bin_data = fs::read(output_file.path()).unwrap();

    // Flash size should be reasonable (< 100KB for typical route)
    assert!(bin_data.len() < 100_000, "binary size < 100KB: got {} bytes", bin_data.len());
}

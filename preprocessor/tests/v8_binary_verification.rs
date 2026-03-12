use assert_cmd::Command;
use shared::{RouteNode, Stop};
use std::fs;
use tempfile::NamedTempFile;

/// Helper to run preprocessor and return parsed results
fn run_preprocessor(route_json: &str, stops_json: &str) -> (Vec<RouteNode>, Vec<Stop>) {
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

    let data = fs::read(output_file.path()).unwrap();
    let node_count = u16::from_le_bytes(data[6..8].try_into().unwrap());
    let stop_count = data[8];

    let mut nodes = Vec::new();
    let mut offset = 17;
    for _ in 0..node_count {
        let node: RouteNode = unsafe { std::ptr::read_unaligned(data[offset..offset+52].as_ptr() as *const RouteNode) };
        nodes.push(node);
        offset += 52;
    }

    let mut stops = Vec::new();
    for _ in 0..stop_count {
        let stop: Stop = unsafe { std::ptr::read_unaligned(data[offset..offset+12].as_ptr() as *const Stop) };
        stops.push(stop);
        offset += 12;
    }

    (nodes, stops)
}

#[test]
fn test_sharp_turn_protection() {
    // --- GIVEN ---
    // A path with a turn that is subtle enough to be removed by 700cm epsilon,
    // but sharp enough (>20 degrees) to trigger the 250cm curve epsilon.
    // Path: (0,0) -> (1000, 300) -> (2000, 0)
    // Perpendicular distance of middle point to line (0,0)-(2000,0) is exactly 300cm.
    // Turn angle: atan2(300, 1000) is ~16.7 deg. Total turn is 16.7 * 2 = 33.4 deg (> 20).
    let route_json = r#"{
        "route_points": [
            [25.0, 121.0],
            [25.000027, 121.000099],
            [25.0, 121.000198]
        ]
    }"#; // Approximate coordinates for ~300cm deviation
    
    let stops_json = r#"{"stops": []}"#;

    // --- WHEN ---
    let (nodes, _) = run_preprocessor(route_json, stops_json);

    // --- THEN ---
    // If curve protection works, the middle point (300cm dev) must be KEPT 
    // because it's > 250cm, even though it's < 700cm.
    assert!(nodes.len() >= 3, "Curve protection should have preserved the corner node (300cm > 250cm epsilon_curve)");
}

#[test]
fn test_stop_proximity_protection() {
    // --- GIVEN ---
    // A route where a point is very close to the line (low deviation),
    // but it is the closest point to a bus stop.
    let route_json = r#"{
        "route_points": [
            [25.0, 121.0],
            [25.00001, 121.00005],
            [25.00002, 121.00010]
        ]
    }"#;
    
    // Stop is exactly at the middle point
    let stops_json = r#"{
        "stops": [{"lat": 25.00001, "lon": 121.00005}]
    }"#;

    // --- WHEN ---
    let (nodes, _) = run_preprocessor(route_json, stops_json);

    // --- THEN ---
    // The middle point should be kept even if it's perfectly on the line 
    // because it's protecting the stop location.
    assert!(nodes.len() >= 3, "Stop protection should have preserved the node nearest to the bus stop");
}

#[test]
fn test_corridor_truncation_and_separation() {
    // --- GIVEN ---
    // Two stops very close together (50m apart).
    // Stop A: progress 100m. Expected end: 140m.
    // Stop B: progress 150m. Expected start: 150-80 = 70m.
    // Conflict: B starts before A ends.
    let route_json = r#"{
        "route_points": [
            [25.0, 121.0],
            [25.005, 121.0]
        ]
    }"#;
    
    let stops_json = r#"{
        "stops": [
            {"lat": 25.0009, "lon": 121.0},
            {"lat": 25.00135, "lon": 121.0}
        ]
    }"#;

    // --- WHEN ---
    let (_, stops) = run_preprocessor(route_json, stops_json);

    // --- THEN ---
    assert_eq!(stops.len(), 2);
    let s0 = &stops[0];
    let s1 = &stops[1];

    // Verify 20m separation requirement
    let separation = s1.corridor_start_cm - s0.corridor_end_cm;
    assert!(separation >= 2000 || s1.corridor_start_cm == s1.progress_cm - 1, 
            "Stops must maintain 20m separation OR be truncated to 1cm before progress");
    
    // Verify progress is always inside corridor
    assert!(s1.corridor_start_cm < s1.progress_cm, "Stop 1 progress must be inside its corridor");
    assert!(s0.corridor_end_cm > s0.progress_cm, "Stop 0 progress must be inside its corridor");
}

#[test]
fn test_interpolation_completeness() {
    // --- GIVEN ---
    // A segment that is 1km long (100,000 cm)
    let route_json = r#"{
        "route_points": [
            [25.0, 121.0],
            [25.01, 121.0]
        ]
    }"#;
    let stops_json = r#"{"stops": []}"#;

    // --- WHEN ---
    let (nodes, _) = run_preprocessor(route_json, stops_json);

    // --- THEN ---
    // It should have inserted at least 100,000 / 3000 = 34 nodes
    assert!(nodes.len() > 30, "Should have interpolated many nodes for a 1km segment");
    
    for i in 0..nodes.len() - 1 {
        let n = &nodes[i];
        let len = ((n.dx_cm as f64).powi(2) + (n.dy_cm as f64).powi(2)).sqrt();
        assert!(len <= 3005.0, "Interpolated segment {} is too long: {}cm", i, len);
    }
}

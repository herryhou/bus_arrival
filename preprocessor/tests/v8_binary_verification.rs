use assert_cmd::Command;
use shared::binfile::RouteData;
use std::fs;
use tempfile::NamedTempFile;

/// Helper to run preprocessor and return parsed results using shared::RouteData
fn run_preprocessor_v8(route_json: &str, stops_json: &str) -> Vec<u8> {
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

    fs::read(output_file.path()).unwrap()
}

#[test]
fn test_v8_binary_loader_integration() {
    // --- GIVEN ---
    let route_json = r#"{
        "route_points": [[25.0, 121.0], [25.001, 121.0]]
    }"#;
    let stops_json = r#"{"stops": [{"lat": 25.0005, "lon": 121.0}]}"#;

    // --- WHEN ---
    let bin_data = run_preprocessor_v8(route_json, stops_json);
    
    // --- THEN ---
    let route_data = RouteData::load(&bin_data).expect("Shared loader should handle valid preprocessor output");

    assert!(route_data.node_count > 2, "Should have interpolated nodes");
    assert_eq!(route_data.stop_count, 1);
    
    // Verify a random node's geometry
    let n = route_data.get_node(0).unwrap();
    let lhs = n.line_a as i64 * n.x_cm as i64 + n.line_b as i64 * n.y_cm as i64 + n.line_c;
    assert_eq!(lhs, 0, "Line coefficient invariant must hold");

    // Verify LUTs are present
    assert_eq!(route_data.gaussian_lut.len(), 256);
    assert_eq!(route_data.logistic_lut.len(), 128);
}

#[test]
fn test_sharp_turn_protection() {
    let route_json = r#"{
        "route_points": [
            [25.0, 121.0],
            [25.000027, 121.000099],
            [25.0, 121.000198]
        ]
    }"#; 
    let stops_json = r#"{"stops": []}"#;

    let bin_data = run_preprocessor_v8(route_json, stops_json);
    let route_data = RouteData::load(&bin_data).unwrap();

    assert!(route_data.node_count >= 3);
}

#[test]
fn test_corridor_truncation_and_separation() {
    let route_json = r#"{
        "route_points": [[25.0, 121.0], [25.005, 121.0]]
    }"#;
    
    // Two stops very close together
    let stops_json = r#"{
        "stops": [
            {"lat": 25.0009, "lon": 121.0},
            {"lat": 25.00135, "lon": 121.0}
        ]
    }"#;

    let bin_data = run_preprocessor_v8(route_json, stops_json);
    let route_data = RouteData::load(&bin_data).unwrap();

    assert_eq!(route_data.stop_count, 2);
    let s0 = route_data.get_stop(0).unwrap();
    let s1 = route_data.get_stop(1).unwrap();

    // Verify 20m separation requirement
    let separation = s1.corridor_start_cm - s0.corridor_end_cm;
    assert!(separation >= 2000 || s1.corridor_start_cm == s1.progress_cm - 1);
    
    assert!(s1.corridor_start_cm < s1.progress_cm);
    assert!(s0.corridor_end_cm > s0.progress_cm);
}

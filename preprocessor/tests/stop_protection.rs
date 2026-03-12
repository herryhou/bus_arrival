use assert_cmd::Command;
use shared::binfile::RouteData;
use std::fs;
use tempfile::NamedTempFile;

#[test]
fn test_stop_protection_radius_30m() {
    // --- GIVEN ---
    // Create a straight route with points every 5m.
    // Douglas-Peucker with 7m epsilon would normally remove many of these.
    // We place a stop such that several points fall within its 30m radius.
    let mut route_points = Vec::new();
    for i in 0..20 {
        // approx 5m steps in latitude (1 degree ~ 111km, so 0.000045 ~ 5m)
        route_points.push(vec![25.0 + (i as f64 * 0.000045), 121.0]);
    }
    
    let route_json = serde_json::json!({
        "route_points": route_points
    }).to_string();

    // Stop at index 10 (approx 50m from start)
    // Points from index 4 to 16 should be within ~30m
    let stops_json = r#"{"stops": [{"lat": 25.00045, "lon": 121.0}]}"#;

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

    let bin_data = fs::read(output_file.path()).unwrap();
    let route_data = RouteData::load(&bin_data).expect("Should load");

    // --- THEN ---
    // Without protection, a straight line simplified with epsilon=7m 
    // would result in only 2 nodes (start and end).
    // With 30m radius protection, we expect many more nodes to be preserved.
    // 30m radius at 5m intervals should protect approx 12-13 points.
    assert!(route_data.node_count > 10, "Should have preserved points within 30m radius. Got only {}", route_data.node_count);
    
    // Check that there are nodes clustered around the stop progress
    let stop = route_data.get_stop(0).unwrap();
    let mut nodes_near_stop = 0;
    for i in 0..route_data.node_count {
        let node = route_data.get_node(i).unwrap();
        let dist = (node.cum_dist_cm - stop.progress_cm).abs();
        if dist < 2500 { // within 25m of stop progress
            nodes_near_stop += 1;
        }
    }
    assert!(nodes_near_stop >= 5, "Should have multiple nodes preserved near the stop. Got {}", nodes_near_stop);
}

#[test]
fn test_terminal_stops() {
    // --- GIVEN ---
    // A 200m route
    let route_json = r#"{
        "route_points": [[25.0, 121.0], [25.0018, 121.0]]
    }"#;
    
    // Stop at 0m (start) and 200m (end)
    let stops_json = r#"{
        "stops": [
            {"lat": 25.0, "lon": 121.0},
            {"lat": 25.0018, "lon": 121.0}
        ]
    }"#;

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

    let bin_data = fs::read(output_file.path()).unwrap();
    let route_data = RouteData::load(&bin_data).expect("Should load");

    // --- THEN ---
    assert_eq!(route_data.stop_count, 2);
    
    let first_stop = route_data.get_stop(0).unwrap();
    assert_eq!(first_stop.progress_cm, 0);
    // 80m pre, 40m post
    assert_eq!(first_stop.corridor_start_cm, -8000);
    assert_eq!(first_stop.corridor_end_cm, 4000);
    
    let last_stop = route_data.get_stop(1).unwrap();
    // 200m ≈ 19900-20000 cm depending on lat_avg
    assert!(last_stop.progress_cm > 19900);
    assert_eq!(last_stop.corridor_start_cm, last_stop.progress_cm - 8000);
    assert_eq!(last_stop.corridor_end_cm, last_stop.progress_cm + 4000);
}

#[test]
fn test_isolated_stop_guaranteed_closest() {
    // --- GIVEN ---
    // A straight 1km route
    let mut route_points = Vec::new();
    for i in 0..101 {
        route_points.push(vec![25.0 + (i as f64 * 0.00009), 121.0]); // ~10m steps
    }
    
    let route_json = serde_json::json!({
        "route_points": route_points
    }).to_string();

    // Stop at 500m but 100m off-route (far from any point)
    let stops_json = r#"{"stops": [{"lat": 25.0045, "lon": 121.001}]}"#;

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

    let bin_data = fs::read(output_file.path()).unwrap();
    let route_data = RouteData::load(&bin_data).expect("Should load");

    // --- THEN ---
    // Without the "guaranteed closest" rule, this stop would have 0 protected points
    // and Douglas-Peucker (epsilon=7m) would leave only 2 points for the 1km line.
    // With the rule, there should be at least 3 points (start, end, and the anchor point).
    assert!(route_data.node_count >= 3);
}

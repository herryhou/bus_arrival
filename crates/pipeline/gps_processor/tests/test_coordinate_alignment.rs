//! Test to check coordinate alignment between NMEA and route data

mod common;
use common::load_test_asset_bytes;
use shared::binfile::{BusError, RouteData};

#[test]
fn test_coordinate_alignment() {
    let data = load_test_asset_bytes("ty225_with_stop_at_gps.bin");
    let route_data = match RouteData::load(&data) {
        Ok(data) => data,
        Err(BusError::InvalidVersion) => {
            eprintln!("Skipping test: ty225_with_stop_at_gps.bin is VERSION 2, needs to be regenerated to VERSION 3");
            return;
        }
        Err(e) => panic!("Failed to load route data: {:?}", e),
    };

    println!(
        "Route data: {} nodes, {} stops",
        route_data.node_count, route_data.stop_count
    );

    // Check first few nodes
    println!("\nFirst 5 nodes:");
    for i in 0..5.min(route_data.node_count) {
        if let Some(node) = route_data.get_node(i) {
            let cum_dist = node.cum_dist_cm;
            let x = node.x_cm;
            let y = node.y_cm;
            println!(
                "Node {}: x_cm={}, y_cm={}, cum_dist_cm={}",
                i, x, y, cum_dist
            );
        }
    }

    // Check first few stops
    println!("\nFirst 5 stops:");
    for i in 0..5.min(route_data.stop_count) {
        if let Some(stop) = route_data.get_stop(i) {
            println!(
                "Stop {}: progress_cm={}, corridor=[{}, {}]",
                i, stop.progress_cm, stop.corridor_start_cm, stop.corridor_end_cm
            );
        }
    }

    // First NMEA position from jsonl: lat=25.004345, lon=121.286612, s_cm=1717259
    let nmea_lat: f64 = 25.004345;
    let nmea_lon: f64 = 121.286612;
    println!(
        "\nFirst NMEA position: lat={}, lon={}, s_cm=1717259",
        nmea_lat, nmea_lon
    );

    // Convert NMEA position to cm coordinates using the same projection as preprocessor
    use shared::{EARTH_R_CM, FIXED_ORIGIN_LAT_DEG, FIXED_ORIGIN_LON_DEG};

    let lat_avg = route_data.lat_avg_deg;
    println!("Route lat_avg_deg: {}", lat_avg);

    // Convert lat/lon to cm (same as preprocessor does)
    let lat_rad = nmea_lat.to_radians();
    let lon_rad = nmea_lon.to_radians();
    let lat0_rad = FIXED_ORIGIN_LAT_DEG.to_radians();
    let lon0_rad = FIXED_ORIGIN_LON_DEG.to_radians();

    let x_cm =
        ((lon_rad - lon0_rad) * (EARTH_R_CM as f64) * lat_avg.to_radians().cos()).round() as i32;
    let y_cm = ((lat_rad - lat0_rad) * (EARTH_R_CM as f64)).round() as i32;

    println!("NMEA in cm coordinates: x_cm={}, y_cm={}", x_cm, y_cm);

    // Find closest node to this position
    let mut min_dist2 = i64::MAX;
    let mut closest_idx = 0;

    for i in 0..route_data.node_count {
        if let Some(node) = route_data.get_node(i) {
            let dx = x_cm as i64 - node.x_cm as i64;
            let dy = y_cm as i64 - node.y_cm as i64;
            let dist2 = dx * dx + dy * dy;
            if dist2 < min_dist2 {
                min_dist2 = dist2;
                closest_idx = i;
            }
        }
    }

    if let Some(node) = route_data.get_node(closest_idx) {
        let cum_dist = node.cum_dist_cm;
        let x = node.x_cm;
        let y = node.y_cm;
        let dist_m = (min_dist2 as f64).sqrt() / 100.0;
        println!(
            "Closest node {}: x_cm={}, y_cm={}, cum_dist_cm={}, distance={}m",
            closest_idx, x, y, cum_dist, dist_m
        );
    }
}

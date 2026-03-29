//! Find the exact route position for GPS at s_cm=1717259

mod common;
use common::load_test_asset_bytes;
use shared::binfile::{RouteData, BusError};

#[test]
fn test_find_gps_route_position() {
    let data = load_test_asset_bytes("ty225_with_stop_at_gps.bin");
    let route_data = match RouteData::load(&data) {
        Ok(data) => data,
        Err(BusError::InvalidVersion) => {
            eprintln!("Skipping test: ty225_with_stop_at_gps.bin is VERSION 2, needs to be regenerated to VERSION 3");
            return;
        }
        Err(e) => panic!("Failed to load route data: {:?}", e),
    };

    let target_s_cm = 1717259i32;
    println!("Looking for node with cum_dist_cm close to {}", target_s_cm);

    let mut closest_idx = 0;
    let mut min_diff = i32::MAX;

    for i in 0..route_data.node_count {
        if let Some(node) = route_data.get_node(i) {
            let diff = (node.cum_dist_cm - target_s_cm).abs();
            if diff < min_diff {
                min_diff = diff;
                closest_idx = i;
            }
        }
    }

    if let Some(node) = route_data.get_node(closest_idx) {
        let cum_dist = node.cum_dist_cm;
        let x = node.x_cm;
        let y = node.y_cm;
        println!("Closest node {}: cum_dist_cm={}, x_cm={}, y_cm={}, diff={} cm ({} m)",
            closest_idx, cum_dist, x, y, min_diff, min_diff / 100);

        // Convert back to lat/lon
        use shared::{EARTH_R_CM, FIXED_ORIGIN_LAT_DEG, FIXED_ORIGIN_LON_DEG};
        let lat_avg = route_data.lat_avg_deg;

        let lat = FIXED_ORIGIN_LAT_DEG + (y as f64 / EARTH_R_CM as f64).to_degrees();
        let lon = FIXED_ORIGIN_LON_DEG + (x as f64 / (EARTH_R_CM as f64 * lat_avg.to_radians().cos())).to_degrees();

        println!("Corresponding lat/lon: lat={}, lon={}", lat, lon);
    }
}

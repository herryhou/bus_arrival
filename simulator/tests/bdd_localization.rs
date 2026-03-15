use shared::{DrState, GpsPoint, KalmanState, RouteNode};
use simulator::kalman::{process_gps_update, ProcessResult};
use simulator::route_data::RouteData;

// A single straight segment of 100m north, with a start node at (0,0) and end node at (0,10000cm).
fn setup_test_route_data() -> (Vec<u8>, i32, i32) {
    let mut nodes = Vec::new();
    let start_x = 1000000;
    let start_y = 1000000;

    // Segment 0: 100m North
    let line_a = -10000;
    let line_b = 0;
    nodes.push(RouteNode {
        len2_cm2: 10000 * 10000,
        line_c: -((line_a as i64 * start_x as i64) + (line_b as i64 * start_y as i64)),
        heading_cdeg: 0,
        _pad: 0,
        x_cm: start_x,
        y_cm: start_y,
        cum_dist_cm: 0,
        dx_cm: 0,
        dy_cm: 10000,
        seg_len_cm: 10000,
        line_a,
        line_b,
    });

    // End node
    nodes.push(RouteNode {
        len2_cm2: 0,
        line_c: 0,
        heading_cdeg: 0,
        _pad: 0,
        x_cm: start_x,
        y_cm: start_y + 10000,
        cum_dist_cm: 10000,
        dx_cm: 0,
        dy_cm: 0,
        seg_len_cm: 0,
        line_a: 0,
        line_b: 0,
    });

    let grid = shared::SpatialGrid {
        cells: vec![vec![0]], // First segment in first cell
        grid_size_cm: 10000,
        cols: 1,
        rows: 1,
        x0_cm: start_x,
        y0_cm: start_y,
    };

    let mut buffer = Vec::new();
    shared::binfile::pack_route_data(&nodes, &[], &grid, 25.0, &[0u8; 256], &[0u8; 128], &mut buffer)
        .expect("Failed to pack test route data");

    (buffer, start_x, start_y)
}

#[test]
fn test_localization_behavioral_scenarios() {
    let (buffer, start_x, start_y) = setup_test_route_data();
    let route_data = RouteData::load(&buffer).expect("Failed to load test route data");

    scenario_normal_forward_movement(&route_data, start_x, start_y);
    scenario_handle_gps_jump(&route_data, start_x, start_y);
    scenario_handle_gps_outage_with_dr(&route_data, start_x, start_y);
}

fn lat_from_y(y_cm: i32) -> f64 {
    use shared::{EARTH_R_CM, FIXED_ORIGIN_LAT_DEG};
    FIXED_ORIGIN_LAT_DEG + (y_cm as f64 / EARTH_R_CM).to_degrees()
}

fn lon_from_x(x_cm: i32, lat_avg_deg: f64) -> f64 {
    use shared::{EARTH_R_CM, FIXED_ORIGIN_LON_DEG};
    FIXED_ORIGIN_LON_DEG
        + (x_cm as f64 / (EARTH_R_CM * lat_avg_deg.to_radians().cos())).to_degrees()
}

fn scenario_normal_forward_movement(route_data: &RouteData, start_x: i32, start_y: i32) {
    let mut state = KalmanState::new();
    let mut dr = DrState::new();

    // Given: Initial GPS fix at route start
    let mut gps = GpsPoint::new();
    gps.has_fix = true;
    gps.timestamp = 1000;
    gps.lat = lat_from_y(start_y);
    gps.lon = lon_from_x(start_x, route_data.lat_avg_deg);
    gps.heading_cdeg = 0;
    gps.speed_cms = 1000; // 10m/s

    // When: Processing first fix
    let result = process_gps_update(&mut state, &mut dr, &gps, &route_data, 0, true);

    // Then: Progress should be 0
    if let ProcessResult::Valid { s_cm, .. } = result {
        assert_eq!(s_cm, 0);
    } else {
        panic!("First fix failed");
    }

    // When: Moving forward. After 1s, bus is at 10m (1000cm).
    gps.timestamp += 1;
    gps.lat = lat_from_y(start_y + 1000);
    let result = process_gps_update(&mut state, &mut dr, &gps, &route_data, 1, false);

    // Then: Progress should be 1000
    if let ProcessResult::Valid { s_cm, .. } = result {
        assert_eq!(s_cm, 1000);
    } else {
        panic!("Update failed");
    }

    // When: Moving forward with GPS noise. GPS says 2500cm.
    gps.timestamp += 1;
    gps.lat = lat_from_y(start_y + 2500);
    let result = process_gps_update(&mut state, &mut dr, &gps, &route_data, 2, false);

    // Then: Progress should be smoothed (s_pred = 1000 + 1000 = 2000, z = 2500)
    // s_new = 2000 + (ks * (2500 - 2000)) / 256
    // with hdop=0, ks=77. 77*500/256 = 150. s_new = 2150.
    if let ProcessResult::Valid { s_cm, .. } = result {
        assert_eq!(s_cm, 2150);
    } else {
        panic!("Noisy update failed");
    }
}

fn scenario_handle_gps_jump(route_data: &RouteData, start_x: i32, start_y: i32) {
    let mut state = KalmanState::new();
    let mut dr = DrState::new();

    // Given: Initial fix
    let mut gps = GpsPoint::new();
    gps.has_fix = true;
    gps.timestamp = 1000;
    gps.lat = lat_from_y(start_y);
    gps.lon = lon_from_x(start_x, route_data.lat_avg_deg);
    process_gps_update(&mut state, &mut dr, &gps, &route_data, 0, true);

    // When: A huge GPS jump (500m North) occurs.
    // max_dist = 3000*1 + 5000 = 8000cm.
    gps.timestamp += 1;
    gps.lat = lat_from_y(start_y + 50000); // 50000cm jump
    let result = process_gps_update(&mut state, &mut dr, &gps, &route_data, 1, false);

    // Then: It should be rejected due to speed constraint
    match result {
        ProcessResult::Rejected(reason) => assert_eq!(reason, "speed constraint"),
        _ => panic!("Expected rejection, got valid result"),
    }
}

fn scenario_handle_gps_outage_with_dr(route_data: &RouteData, start_x: i32, start_y: i32) {
    let mut state = KalmanState::new();
    let mut dr = DrState::new();

    // Given: A valid state moving at 10m/s (1000 cm/s)
    let mut gps = GpsPoint::new();
    gps.has_fix = true;
    gps.timestamp = 1000;
    gps.lat = lat_from_y(start_y);
    gps.lon = lon_from_x(start_x, route_data.lat_avg_deg);
    gps.speed_cms = 1000;
    process_gps_update(&mut state, &mut dr, &gps, &route_data, 0, true);

    // Mock speed into state/dr
    state.v_cms = 1000;
    dr.filtered_v = 1000;
    dr.last_valid_s = 0;
    dr.last_gps_time = Some(1000);

    // When: GPS signal is lost for 2 seconds
    let mut gps_lost = GpsPoint::new();
    gps_lost.has_fix = false;
    gps_lost.timestamp = 1002;
    let result = process_gps_update(&mut state, &mut dr, &gps_lost, &route_data, 2, false);

    // Then: Dead reckoning should estimate progress
    if let ProcessResult::DrOutage { s_cm, .. } = result {
        // Expected: last_s(0) + v(1000) * dt(2) = 2000
        assert_eq!(s_cm, 2000);
    } else {
        panic!("Expected DR result, got other");
    }
}

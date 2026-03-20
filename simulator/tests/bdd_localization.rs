use shared::{DrState, GpsPoint, KalmanState, RouteNode};
use simulator::kalman::{process_gps_update, ProcessResult};
use simulator::route_data::RouteData;

// A single straight segment of 100m north.
// Uses ABSOLUTE coordinates from fixed origin (120°E, 20°N).
fn setup_test_route_data() -> (Vec<u8>, i32, i32) {
    let mut nodes = Vec::new();

    // Calculate absolute coordinates for a Taiwan location (25°N, 121°E)
    use shared::{EARTH_R_CM, FIXED_ORIGIN_LON_DEG, FIXED_ORIGIN_Y_CM};

    const BASE_LAT: f64 = 25.0;
    const BASE_LON: f64 = 121.0;
    let lat_avg_rad = BASE_LAT.to_radians();
    let cos_lat = lat_avg_rad.cos();

    // Convert base lat/lon to absolute cm coordinates
    let lon_rad = BASE_LON.to_radians();
    let lat_rad = BASE_LAT.to_radians();
    let x_abs = EARTH_R_CM as f64 * lon_rad * cos_lat;
    let y_abs = EARTH_R_CM as f64 * lat_rad;
    let x0_abs = (FIXED_ORIGIN_LON_DEG.to_radians() * EARTH_R_CM as f64) * cos_lat;
    let y0_abs = FIXED_ORIGIN_Y_CM as f64;

    let start_x = (x_abs - x0_abs).round() as i32;
    let start_y = (y_abs - y0_abs).round() as i32;

    // Segment 0: 1km North
    nodes.push(RouteNode {
        len2_cm2: 100000 * 100000,
        heading_cdeg: 0,
        _pad: 0,
        x_cm: start_x,
        y_cm: start_y,
        cum_dist_cm: 0,
        dx_cm: 0,
        dy_cm: 100000,
        seg_len_cm: 100000,
    });

    // End node
    nodes.push(RouteNode {
        len2_cm2: 0,
        heading_cdeg: 0,
        _pad: 0,
        x_cm: start_x,
        y_cm: start_y + 100000,
        cum_dist_cm: 100000,
        dx_cm: 0,
        dy_cm: 0,
        seg_len_cm: 0,
    });

    let grid = shared::SpatialGrid {
        cells: vec![vec![0]],
        grid_size_cm: 100000,
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

// Creates an L-shaped route: 50m East, then 50m North
// Returns (buffer, start_x, start_y)
fn setup_l_shaped_route() -> (Vec<u8>, i32, i32) {
    let mut nodes = Vec::new();

    use shared::{EARTH_R_CM, FIXED_ORIGIN_LON_DEG, FIXED_ORIGIN_Y_CM};

    const BASE_LAT: f64 = 25.0;
    const BASE_LON: f64 = 121.0;
    let lat_avg_rad = BASE_LAT.to_radians();
    let cos_lat = lat_avg_rad.cos();

    let lon_rad = BASE_LON.to_radians();
    let lat_rad = BASE_LAT.to_radians();
    let x_abs = EARTH_R_CM as f64 * lon_rad * cos_lat;
    let y_abs = EARTH_R_CM as f64 * lat_rad;
    let x0_abs = (FIXED_ORIGIN_LON_DEG.to_radians() * EARTH_R_CM as f64) * cos_lat;
    let y0_abs = FIXED_ORIGIN_Y_CM as f64;

    let start_x = (x_abs - x0_abs).round() as i32;
    let start_y = (y_abs - y0_abs).round() as i32;

    // Segment 0: 50m East (heading = 9000 cdeg = 90°)
    nodes.push(RouteNode {
        len2_cm2: 5000 * 5000,
        heading_cdeg: 9000,
        _pad: 0,
        x_cm: start_x,
        y_cm: start_y,
        cum_dist_cm: 0,
        dx_cm: 5000,
        dy_cm: 0,
        seg_len_cm: 5000,
    });

    // Corner node (end of seg 0, start of seg 1)
    nodes.push(RouteNode {
        len2_cm2: 5000 * 5000,
        heading_cdeg: 0,  // North
        _pad: 0,
        x_cm: start_x + 5000,
        y_cm: start_y,
        cum_dist_cm: 5000,
        dx_cm: 0,
        dy_cm: 5000,
        seg_len_cm: 5000,
    });

    // End node
    nodes.push(RouteNode {
        len2_cm2: 0,
        heading_cdeg: 0,
        _pad: 0,
        x_cm: start_x + 5000,
        y_cm: start_y + 5000,
        cum_dist_cm: 10000,
        dx_cm: 0,
        dy_cm: 0,
        seg_len_cm: 0,
    });

    // Grid covering both segments
    let grid = shared::SpatialGrid {
        cells: vec![vec![0, 1]],
        grid_size_cm: 5000,
        cols: 2,
        rows: 1,
        x0_cm: start_x,
        y0_cm: start_y,
    };

    let mut buffer = Vec::new();
    shared::binfile::pack_route_data(&nodes, &[], &grid, 25.0, &[0u8; 256], &[0u8; 128], &mut buffer)
        .expect("Failed to pack L-shaped route");

    (buffer, start_x, start_y)
}

#[test]
fn test_localization_behavioral_scenarios() {
    let (buffer, start_x, start_y) = setup_test_route_data();
    let route_data = RouteData::load(&buffer).expect("Failed to load test route data");

    scenario_normal_forward_movement(&route_data, start_x, start_y);
    scenario_handle_gps_jump(&route_data, start_x, start_y);
    scenario_handle_gps_outage_with_dr(&route_data, start_x, start_y);
    scenario_heading_penalty_overlapping_routes(&route_data, start_x, start_y);
    scenario_monotonicity_tolerance(&route_data, start_x, start_y);
    scenario_max_speed_rejection(&route_data, start_x, start_y);
    scenario_hdop_adaptive_smoothing(&route_data, start_x, start_y);
    scenario_extended_gps_outage(&route_data, start_x, start_y);
    scenario_route_end_clamping(&route_data, start_x, start_y);

    // L-shaped route tests
    let (l_buffer, l_start_x, l_start_y) = setup_l_shaped_route();
    let l_route_data = RouteData::load(&l_buffer).expect("Failed to load L-shaped route");
    scenario_l_shaped_turn(&l_route_data, l_start_x, l_start_y);
}

fn scenario_hdop_adaptive_smoothing(route_data: &RouteData, start_x: i32, start_y: i32) {
    let mut state = KalmanState::new();
    let mut dr = DrState::new();

    // Given: Initial fix at 0m
    let mut gps = GpsPoint::new();
    gps.has_fix = true;
    gps.timestamp = 1000;
    gps.lat = lat_from_y(start_y);
    gps.lon = lon_from_x(start_x, route_data.lat_avg_deg);
    gps.speed_cms = 1000;
    gps.hdop_x10 = 10; // Accurate
    process_gps_update(&mut state, &mut dr, &gps, &route_data, 0, true);

    // When: GPS update at 20m with high noise (HDOP=5.0)
    // Predicted position: 0 + 1000*1 = 1000cm
    // Raw position: 2000cm
    gps.timestamp += 1;
    gps.lat = lat_from_y(start_y + 2000);
    gps.hdop_x10 = 50; // Noisy (Ks = 13 instead of 77)
    let result = process_gps_update(&mut state, &mut dr, &gps, &route_data, 1, false);

    // Then: Progress should stay closer to predicted (1000) than raw (2000)
    // s_pred = 0 + 1000 = 1000
    // ks for hdop=50 is 26
    // s_new = 1000 + (26 * (2000 - 1000)) / 256 = 1000 + 26000 / 256 = 1000 + 101 = 1101
    if let ProcessResult::Valid { s_cm, .. } = result {
        assert_eq!(s_cm, 1101);
        assert!(s_cm < 1200); // Verify it stayed close to prediction
    } else {
        panic!("HDOP update failed");
    }
}

fn scenario_extended_gps_outage(route_data: &RouteData, start_x: i32, start_y: i32) {
    let mut state = KalmanState::new();
    let mut dr = DrState::new();

    // Given: A valid state
    let mut gps = GpsPoint::new();
    gps.has_fix = true;
    gps.timestamp = 1000;
    gps.lat = lat_from_y(start_y);
    gps.lon = lon_from_x(start_x, route_data.lat_avg_deg);
    gps.speed_cms = 1000;
    process_gps_update(&mut state, &mut dr, &gps, &route_data, 0, true);

    // When: GPS signal is lost for 11 seconds
    let mut gps_lost = GpsPoint::new();
    gps_lost.has_fix = false;
    gps_lost.timestamp = 1011;
    let result = process_gps_update(&mut state, &mut dr, &gps_lost, &route_data, 11, false);

    // Then: Should return Outage status
    match result {
        ProcessResult::Outage => (),
        _ => panic!("Expected Outage, got other"),
    }
}

fn scenario_route_end_clamping(route_data: &RouteData, start_x: i32, start_y: i32) {
    let mut state = KalmanState::new();
    let mut dr = DrState::new();

    // Given: Bus is near the end of 1km route
    let mut gps = GpsPoint::new();
    gps.has_fix = true;
    gps.timestamp = 1000;
    gps.lat = lat_from_y(start_y + 90000); // 900m
    gps.lon = lon_from_x(start_x, route_data.lat_avg_deg);
    process_gps_update(&mut state, &mut dr, &gps, &route_data, 0, true);

    // When: multiple GPS updates place bus at 1.1km (past the 1km end)
    for _ in 0..10 {
        gps.timestamp += 10;
        gps.lat = lat_from_y(start_y + 110000); 
        let result = process_gps_update(&mut state, &mut dr, &gps, &route_data, 10, false);
        
        if let ProcessResult::Valid { s_cm, .. } = result {
            assert!(s_cm <= 100000, "Progress should never exceed 100000, got {}", s_cm);
        } else {
            panic!("End clamping update failed");
        }
    }

    // Then: Progress should be very close to 1km (100000cm)
    assert!(state.s_cm > 99000);
    assert!(state.s_cm <= 100000);
}

fn scenario_heading_penalty_overlapping_routes(route_data: &RouteData, start_x: i32, start_y: i32) {
    let mut state = KalmanState::new();
    let mut dr = DrState::new();

    // Given: A GPS point close to the route but with OPPOSITE heading
    let mut gps = GpsPoint::new();
    gps.has_fix = true;
    gps.timestamp = 1000;
    gps.lat = lat_from_y(start_y + 1000); // 10m north
    gps.lon = lon_from_x(start_x + 500, route_data.lat_avg_deg); // 5m east
    gps.heading_cdeg = 18000; // Moving SOUTH
    gps.speed_cms = 1000;

    // When: Processing the update
    let result = process_gps_update(&mut state, &mut dr, &gps, &route_data, 0, true);

    // Then: It should still snap to the segment but the heading penalty should be high in the score calculation
    // (internal to find_best_segment_restricted).
    // In this simple 1-segment route, it will still pick segment 0 because it's the only one.
    // In a real multi-segment route with a southbound segment nearby, it would prefer the southbound one.
    if let ProcessResult::Valid { seg_idx, .. } = result {
        assert_eq!(seg_idx, 0);
    } else {
        panic!("Should still be valid for single-segment route");
    }
}

fn scenario_monotonicity_tolerance(route_data: &RouteData, start_x: i32, start_y: i32) {
    let mut state = KalmanState::new();
    let mut dr = DrState::new();

    // Given: Initial position at 800m (80000cm)
    let mut gps = GpsPoint::new();
    gps.has_fix = true;
    gps.timestamp = 1000;
    gps.lat = lat_from_y(start_y + 80000);
    gps.lon = lon_from_x(start_x, route_data.lat_avg_deg);
    process_gps_update(&mut state, &mut dr, &gps, &route_data, 0, true);

    // When: GPS jumps BACKWARDS by 5m (500cm) - within 500m tolerance
    gps.timestamp += 1;
    gps.lat = lat_from_y(start_y + 79500);
    let result = process_gps_update(&mut state, &mut dr, &gps, &route_data, 1, false);

    // Then: It should be accepted
    if let ProcessResult::Valid { s_cm, .. } = result {
        assert!(s_cm < 80000);
    } else {
        panic!("Backward noise should be accepted");
    }

    // When: GPS jumps BACKWARDS by 600m (60000cm) - outside 500m tolerance
    // to position 200m (20000cm).
    // Increase timestamp by 60s to pass speed constraint (max_dist = 3000*60 + 5000 = 185000)
    gps.timestamp += 60;
    gps.lat = lat_from_y(start_y + 20000); 
    let result = process_gps_update(&mut state, &mut dr, &gps, &route_data, 2, false);

    // Then: It should be rejected by monotonicity
    match result {
        ProcessResult::Rejected(reason) => assert_eq!(reason, "monotonicity"),
        _ => panic!("Expected monotonicity rejection"),
    }
}

fn scenario_max_speed_rejection(route_data: &RouteData, start_x: i32, start_y: i32) {
    let mut state = KalmanState::new();
    let mut dr = DrState::new();

    // Given: Initial position at 0m
    let mut gps = GpsPoint::new();
    gps.has_fix = true;
    gps.timestamp = 1000;
    gps.lat = lat_from_y(start_y);
    gps.lon = lon_from_x(start_x, route_data.lat_avg_deg);
    process_gps_update(&mut state, &mut dr, &gps, &route_data, 0, true);

    // When: GPS jumps 100m in 1s (10000 cm/s)
    // V_MAX is 3000 cm/s. Max dist = 3000*1 + 5000 = 8000 cm.
    gps.timestamp += 1;
    gps.lat = lat_from_y(start_y + 10000);
    let result = process_gps_update(&mut state, &mut dr, &gps, &route_data, 1, false);

    // Then: It should be rejected by speed constraint
    match result {
        ProcessResult::Rejected(reason) => assert_eq!(reason, "speed constraint"),
        _ => panic!("Expected speed constraint rejection"),
    }
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

fn scenario_l_shaped_turn(route_data: &RouteData, start_x: i32, start_y: i32) {
    let mut state = KalmanState::new();
    let mut dr = DrState::new();

    // Given: Bus starts at the beginning of L-shaped route (going East)
    let mut gps = GpsPoint::new();
    gps.has_fix = true;
    gps.timestamp = 1000;
    gps.lat = lat_from_y(start_y);
    gps.lon = lon_from_x(start_x, route_data.lat_avg_deg);
    gps.heading_cdeg = 9000; // East
    gps.speed_cms = 500; // 5 m/s

    let result = process_gps_update(&mut state, &mut dr, &gps, &route_data, 0, true);
    if let ProcessResult::Valid { seg_idx, s_cm, .. } = result {
        assert_eq!(seg_idx, 0, "Should start on segment 0 (East)");
        assert_eq!(s_cm, 0);
    } else {
        panic!("Initial fix failed");
    }

    // When: Bus moves 25m East (halfway through first segment)
    gps.timestamp += 5;
    gps.lat = lat_from_y(start_y);
    gps.lon = lon_from_x(start_x + 2500, route_data.lat_avg_deg);
    let result = process_gps_update(&mut state, &mut dr, &gps, &route_data, 1, false);

    if let ProcessResult::Valid { seg_idx, s_cm, .. } = result {
        assert_eq!(seg_idx, 0, "Should still be on segment 0");
        assert_eq!(s_cm, 2500, "Progress should be 25m");
    } else {
        panic!("Mid-segment update failed");
    }

    // When: Bus reaches the corner and turns North
    gps.timestamp += 5;
    gps.lat = lat_from_y(start_y + 2500); // 25m North from corner
    gps.lon = lon_from_x(start_x + 5000, route_data.lat_avg_deg); // At corner x
    gps.heading_cdeg = 0; // North now
    let result = process_gps_update(&mut state, &mut dr, &gps, &route_data, 2, false);

    // Then: Map matcher should identify segment 1 (North)
    if let ProcessResult::Valid { seg_idx, s_cm, .. } = result {
        assert_eq!(seg_idx, 1, "Should transition to segment 1 (North)");
        assert_eq!(s_cm, 7500, "Progress should be 50m + 25m = 75m");
    } else {
        panic!("Turn transition failed");
    }
}

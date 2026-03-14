//! Test for active_stops functionality in simulator output
//!
//! This test verifies that the simulator correctly identifies active stops
//! based on route progress and stop corridor boundaries.

use shared::{DrState, GpsPoint, KalmanState, RouteNode, Stop};
use simulator::kalman::{process_gps_update, ProcessResult};
use simulator::route_data::RouteData;

fn setup_test_route_with_stop() -> (Vec<u8>, i32, i32) {
    let mut nodes = Vec::new();
    let start_x = 1000000;
    let start_y = 1000000;

    // Create a 200m route with a stop at 100m
    // Segment 0: 100m North
    let line_a = -10000;
    let line_b = 0;
    nodes.push(RouteNode {
        len2_cm2: 10000 * 10000,
        line_c: -((line_a as i64 * 0) + (line_b as i64 * 0)),
        heading_cdeg: 0,
        _pad: 0,
        x_cm: 0,
        y_cm: 0,
        cum_dist_cm: 0,
        dx_cm: 0,
        dy_cm: 10000,
        seg_len_cm: 10000,
        line_a,
        line_b,
    });

    // Segment 1: 100m North
    nodes.push(RouteNode {
        len2_cm2: 10000 * 10000,
        line_c: 0,
        heading_cdeg: 0,
        _pad: 0,
        x_cm: 0,
        y_cm: 10000,
        cum_dist_cm: 10000,
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
        x_cm: 0,
        y_cm: 20000,
        cum_dist_cm: 20000,
        dx_cm: 0,
        dy_cm: 0,
        seg_len_cm: 0,
        line_a: 0,
        line_b: 0,
    });

    // Create a stop at 100m (progress_cm = 10000)
    // Corridor: 10000 - 8000 = 2000 to 10000 + 4000 = 14000
    let stops = vec![
        Stop {
            progress_cm: 10000,
            corridor_start_cm: 2000,
            corridor_end_cm: 14000,
        }
    ];

    let grid = shared::SpatialGrid {
        cells: vec![vec![0, 1]], // Both segments in first cell
        grid_size_cm: 10000,
        cols: 1,
        rows: 1,
        x0_cm: start_x,
        y0_cm: start_y,
    };

    let mut buffer = Vec::new();
    shared::binfile::pack_route_data(&nodes, &stops, &grid, 25.0, &[0u8; 256], &[0u8; 128], &mut buffer)
        .expect("Failed to pack test route data");

    (buffer, start_x, start_y)
}

fn lat_from_y(y_cm: i32) -> f64 {
    use shared::{EARTH_R_CM, FIXED_ORIGIN_LAT_DEG};
    FIXED_ORIGIN_LAT_DEG + (y_cm as f64 / EARTH_R_CM).to_degrees()
}

fn lon_from_x(x_cm: i32) -> f64 {
    use shared::{EARTH_R_CM, FIXED_ORIGIN_LON_DEG, PROJECTION_LAT_AVG};
    FIXED_ORIGIN_LON_DEG
        + (x_cm as f64 / (EARTH_R_CM * PROJECTION_LAT_AVG.to_radians().cos())).to_degrees()
}

#[test]
fn test_active_stops_when_in_corridor() {
    let (buffer, start_x, start_y) = setup_test_route_with_stop();
    let route_data = RouteData::load(&buffer).expect("Failed to load test route data");

    // Verify stop is loaded correctly
    assert_eq!(route_data.stop_count, 1);
    let stop = route_data.get_stop(0).expect("Failed to get stop");
    println!("Stop: progress_cm={}, corridor_start_cm={}, corridor_end_cm={}",
        stop.progress_cm, stop.corridor_start_cm, stop.corridor_end_cm);
    assert_eq!(stop.progress_cm, 10000);
    assert_eq!(stop.corridor_start_cm, 2000);
    assert_eq!(stop.corridor_end_cm, 14000);

    let mut state = KalmanState::new();
    let mut dr = DrState::new();

    // Test Case 1: Before corridor (s_cm = 1000)
    // Should NOT find any active stops
    let mut gps = GpsPoint::new();
    gps.has_fix = true;
    gps.timestamp = 1000;
    gps.lat = lat_from_y(start_y + 1000);
    gps.lon = lon_from_x(start_x);
    gps.heading_cdeg = 0;
    gps.speed_cms = 1000;

    let result = process_gps_update(&mut state, &mut dr, &gps, &route_data, 0, true);
    if let ProcessResult::Valid { s_cm, .. } = result {
        println!("Test 1: s_cm={} (before corridor)", s_cm);
        let stops = route_data.stops();
        let active_stops: Vec<usize> = stops.iter()
            .enumerate()
            .filter(|(_, stop)| s_cm >= stop.corridor_start_cm && s_cm <= stop.corridor_end_cm)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(active_stops.len(), 0, "Should have no active stops before corridor");
    }

    // Test Case 2: Inside corridor (s_cm = 5000)
    // Should find the active stop
    gps.timestamp += 1;
    gps.lat = lat_from_y(start_y + 5000);
    let result = process_gps_update(&mut state, &mut dr, &gps, &route_data, 1, false);
    if let ProcessResult::Valid { s_cm, .. } = result {
        println!("Test 2: s_cm={} (inside corridor)", s_cm);
        let stops = route_data.stops();
        let active_stops: Vec<usize> = stops.iter()
            .enumerate()
            .filter(|(_, stop)| s_cm >= stop.corridor_start_cm && s_cm <= stop.corridor_end_cm)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(active_stops.len(), 1, "Should have 1 active stop inside corridor");
        assert_eq!(active_stops[0], 0, "Active stop should be index 0");
    }

    // Test Case 3: At stop (s_cm = 10000)
    // Should find the active stop
    gps.timestamp += 1;
    gps.lat = lat_from_y(start_y + 10000);
    let result = process_gps_update(&mut state, &mut dr, &gps, &route_data, 2, false);
    if let ProcessResult::Valid { s_cm, .. } = result {
        println!("Test 3: s_cm={} (at stop)", s_cm);
        let stops = route_data.stops();
        let active_stops: Vec<usize> = stops.iter()
            .enumerate()
            .filter(|(_, stop)| s_cm >= stop.corridor_start_cm && s_cm <= stop.corridor_end_cm)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(active_stops.len(), 1, "Should have 1 active stop at stop location");
        assert_eq!(active_stops[0], 0, "Active stop should be index 0");
    }

    // Test Case 4: After corridor (s_cm = 15000)
    // Should NOT find any active stops
    gps.timestamp += 1;
    gps.lat = lat_from_y(start_y + 15000);
    let result = process_gps_update(&mut state, &mut dr, &gps, &route_data, 3, false);
    if let ProcessResult::Valid { s_cm, .. } = result {
        println!("Test 4: s_cm={} (after corridor)", s_cm);
        let stops = route_data.stops();
        let active_stops: Vec<usize> = stops.iter()
            .enumerate()
            .filter(|(_, stop)| s_cm >= stop.corridor_start_cm && s_cm <= stop.corridor_end_cm)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(active_stops.len(), 0, "Should have no active stops after corridor");
    }
}

#[test]
fn test_active_stops_with_multiple_stops() {
    let mut nodes = Vec::new();
    let start_x = 1000000;
    let start_y = 1000000;

    // Create a 300m route with stops at 100m, 200m
    for i in 0..3 {
        let line_a = -10000;
        let line_b = 0;
        nodes.push(RouteNode {
            len2_cm2: 10000 * 10000,
            line_c: -((line_a as i64 * 0) + (line_b as i64 * 0)),
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 0,
            y_cm: (i * 10000) as i32,
            cum_dist_cm: (i * 10000) as i32,
            dx_cm: 0,
            dy_cm: 10000,
            seg_len_cm: 10000,
            line_a,
            line_b,
        });
    }
    nodes.push(RouteNode {
        len2_cm2: 0,
        line_c: 0,
        heading_cdeg: 0,
        _pad: 0,
        x_cm: 0,
        y_cm: 30000,
        cum_dist_cm: 30000,
        dx_cm: 0,
        dy_cm: 0,
        seg_len_cm: 0,
        line_a: 0,
        line_b: 0,
    });

    // Create two stops with overlapping corridors
    // Stop 0 at 100m: corridor [2000, 14000]
    // Stop 1 at 200m: corridor [12000, 24000]
    // Overlap region: [12000, 14000]
    let stops = vec![
        Stop {
            progress_cm: 10000,
            corridor_start_cm: 2000,
            corridor_end_cm: 14000,
        },
        Stop {
            progress_cm: 20000,
            corridor_start_cm: 12000,
            corridor_end_cm: 24000,
        }
    ];

    let grid = shared::SpatialGrid {
        cells: vec![vec![0, 1, 2]],
        grid_size_cm: 10000,
        cols: 1,
        rows: 1,
        x0_cm: start_x,
        y0_cm: start_y,
    };

    let mut buffer = Vec::new();
    shared::binfile::pack_route_data(&nodes, &stops, &grid, 25.0, &[0u8; 256], &[0u8; 128], &mut buffer)
        .expect("Failed to pack test route data");

    let route_data = RouteData::load(&buffer).expect("Failed to load test route data");
    assert_eq!(route_data.stop_count, 2);

    // Test at s_cm = 13000 (in overlap region)
    // Should find BOTH active stops
    let stops = route_data.stops();
    let s_cm = 13000i32;
    let active_stops: Vec<usize> = stops.iter()
        .enumerate()
        .filter(|(_, stop)| s_cm >= stop.corridor_start_cm && s_cm <= stop.corridor_end_cm)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(active_stops.len(), 2, "Should have 2 active stops in overlap region");
}

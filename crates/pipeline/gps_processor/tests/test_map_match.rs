//! Tests for map_match module - specifically for find_best_segment_restricted returning distance squared
//!
//! This test file validates that find_best_segment_restricted returns both the segment index
//! and the distance squared, as required for off-route detection (Task 3).

#![cfg(feature = "std")]

use gps_processor::map_match::find_best_segment_restricted;
use shared::{RouteNode, SpatialGrid};

#[test]
fn test_find_best_segment_returns_distance_squared() {
    // Create a simple route with 3 segments forming a straight line
    // Segment 0: (0, 0) to (1000, 0) - heading 0°
    // Segment 1: (1000, 0) to (2000, 0) - heading 0°
    // Segment 2: (2000, 0) to (3000, 0) - heading 0°

    let mut nodes = Vec::new();

    nodes.push(RouteNode {
        x_cm: 0,
        y_cm: 0,
        cum_dist_cm: 0,
        heading_cdeg: 0,
        seg_len_mm: 10000, // 1000 cm
        dx_cm: 1000,
        dy_cm: 0,
        _pad: 0,
    });

    nodes.push(RouteNode {
        x_cm: 1000,
        y_cm: 0,
        cum_dist_cm: 1000,
        heading_cdeg: 0,
        seg_len_mm: 10000, // 1000 cm
        dx_cm: 1000,
        dy_cm: 0,
        _pad: 0,
    });

    nodes.push(RouteNode {
        x_cm: 2000,
        y_cm: 0,
        cum_dist_cm: 2000,
        heading_cdeg: 0,
        seg_len_mm: 10000, // 1000 cm
        dx_cm: 1000,
        dy_cm: 0,
        _pad: 0,
    });

    let grid = SpatialGrid {
        cells: vec![vec![0, 1, 2]], // One row with 3 cells
        grid_size_cm: 1000,
        cols: 3,
        rows: 1,
        x0_cm: 0,
        y0_cm: 0,
    };

    // Pack route data
    let mut buffer = Vec::new();
    shared::binfile::pack_route_data(&nodes, &[], &grid, 25.0, &mut buffer)
        .expect("Failed to pack test route data");

    // Load route data
    let route_data = shared::binfile::RouteData::load(&buffer).expect("Failed to load route data");

    // Test 1: GPS point exactly on segment 1
    // Should return (1, 0) - segment 1, distance squared = 0
    let (seg_idx, dist2) = find_best_segment_restricted(
        1500, // x_cm - middle of segment 1
        0,    // y_cm - on the line
        0,    // gps_heading - matches segment heading
        500,  // gps_speed - moving, heading filter active
        &route_data,
        1,    // last_idx
        false, // is_first_fix
    );

    assert_eq!(seg_idx, 1, "Should select segment 1 when GPS is on it");
    assert_eq!(dist2, 0, "Distance squared should be 0 when GPS is exactly on segment");

    // Test 2: GPS point near segment 1 (500 cm perpendicular offset)
    // Distance should be 500² = 250000 cm²
    let (seg_idx, dist2) = find_best_segment_restricted(
        1500, // x_cm - middle of segment 1
        500,  // y_cm - 500 cm offset
        0,    // gps_heading - matches segment heading
        500,  // gps_speed - moving, heading filter active
        &route_data,
        1,    // last_idx
        false, // is_first_fix
    );

    assert_eq!(seg_idx, 1, "Should select segment 1 as closest");
    assert_eq!(dist2, 250000, "Distance squared should be 500² = 250000");

    // Test 3: GPS point with wrong heading - should still return distance²
    // Even if heading doesn't match, we should get the best_any result with its distance
    let (seg_idx, dist2) = find_best_segment_restricted(
        1500, // x_cm - middle of segment 1
        0,    // y_cm - on the line
        9000, // gps_heading - 90° off from segment heading (0°)
        500,  // gps_speed - moving, heading filter active
        &route_data,
        1,    // last_idx
        false, // is_first_fix
    );

    // With 90° heading difference, segment should be ineligible
    // But function should still return the best_any result
    assert!(dist2 >= 0, "Distance squared should always be non-negative");
    assert!(seg_idx < 3, "Segment index should be valid");

    // Test 4: First fix mode - heading filter disabled
    // Should work normally and return distance²
    let (seg_idx, dist2) = find_best_segment_restricted(
        1500, // x_cm - middle of segment 1
        200,  // y_cm - 200 cm offset
        9000, // gps_heading - doesn't matter in first fix mode
        500,  // gps_speed
        &route_data,
        1,    // last_idx
        true, // is_first_fix - relaxed heading filter
    );

    assert_eq!(seg_idx, 1, "Should select segment 1");
    assert_eq!(dist2, 40000, "Distance squared should be 200² = 40000");
}

#[test]
fn test_find_best_segment_distance_squared_properties() {
    // Create minimal route
    let mut nodes = Vec::new();

    nodes.push(RouteNode {
        x_cm: 1000,
        y_cm: 1000,
        cum_dist_cm: 0,
        heading_cdeg: 0,
        seg_len_mm: 10000,
        dx_cm: 1000,
        dy_cm: 0,
        _pad: 0,
    });

    let grid = SpatialGrid {
        cells: vec![vec![0]],
        grid_size_cm: 1000,
        cols: 1,
        rows: 1,
        x0_cm: 0,
        y0_cm: 0,
    };

    // Pack route data
    let mut buffer = Vec::new();
    shared::binfile::pack_route_data(&nodes, &[], &grid, 25.0, &mut buffer)
        .expect("Failed to pack test route data");

    // Load route data
    let route_data = shared::binfile::RouteData::load(&buffer).expect("Failed to load route data");

    // Test that distance² is always non-negative
    let (seg_idx, dist2) = find_best_segment_restricted(
        1500,
        1200,
        0,
        100,
        &route_data,
        0,
        false,
    );

    assert!(dist2 >= 0, "Distance squared must be non-negative");
    assert!(seg_idx < route_data.node_count, "Segment index must be valid");
}

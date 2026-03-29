//! Test that route nodes are stored as ABSOLUTE coordinates
//!
//! This test ensures the preprocessor stores route nodes relative to the FIXED origin
//! (120°E, 20°N), NOT relative to the spatial grid origin (x0_cm, y0_cm).
//!
//! CRITICAL: The visualizer expects absolute coordinates. If nodes are stored as
//! relative coordinates, bus positions will appear far from the route path.

use preprocessor::{coord, input, linearize, simplify, grid, stops};
use shared::RouteNode;

#[test]
fn test_route_nodes_are_absolute_coordinates() {
    // Arrange: Create a simple route at known coordinates
    let route_json = r#"{
        "route_points": [
            [25.0000, 121.0000],
            [25.0010, 121.0010]
        ]
    }"#;

    let route: input::RouteInput = serde_json::from_str(route_json).unwrap();
    let lat_avg = 25.0;

    // Act: Convert to cm (same as preprocessor does)
    let route_pts_cm: Vec<(i64, i64)> = route.route_points.iter()
        .map(|p| {
            let (x, y) = coord::latlon_to_cm_relative(p.lat(), p.lon(), lat_avg);
            (x as i64, y as i64)
        })
        .collect();

    // Build grid to get grid origin
    let route_nodes = linearize::linearize_route(&route_pts_cm);
    let grid = grid::build_grid(&route_nodes, 10000);

    // Get grid origin (minimum x and y from route nodes)
    let grid_origin_x = grid.x0_cm as i64;
    let grid_origin_y = grid.y0_cm as i64;

    // Assert: Route nodes are stored as ABSOLUTE coordinates
    // They should NOT be transformed to grid origin
    let first_node = &route_nodes[0];

    // The first node's coordinates should be close to the absolute coordinates
    // calculated directly from lat/lon
    let expected_x = route_pts_cm[0].0;
    let expected_y = route_pts_cm[0].1;

    // CRITICAL ASSERTION: Nodes are stored as ABSOLUTE coordinates
    // If this fails, it means nodes were incorrectly transformed to grid origin
    assert_eq!(first_node.x_cm as i64, expected_x,
        "Route node x_cm should be absolute (from fixed origin), not relative to grid origin");
    assert_eq!(first_node.y_cm as i64, expected_y,
        "Route node y_cm should be absolute (from fixed origin), not relative to grid origin");

    // Additional verification: Grid origin is NOT at (0, 0) for absolute coordinates
    // Grid origin should be the minimum x/y from the route
    assert!(grid_origin_x > 0, "Grid origin x should be positive (absolute coordinate system)");
    assert!(grid_origin_y > 0, "Grid origin y should be positive (absolute coordinate system)");

    // Verify first node is NOT at (0, 0) which would indicate relative coordinates
    let x_cm = first_node.x_cm;
    let y_cm = first_node.y_cm;
    assert_ne!(x_cm, 0, "First node x_cm should not be 0 for a route not at fixed origin");
    assert_ne!(y_cm, 0, "First node y_cm should not be 0 for a route not at fixed origin");
}

#[test]
fn test_grid_origin_is_minimum_not_origin() {
    // This test verifies that x0_cm/y0_cm in the spatial grid are the MINIMUM
    // x/y values from the route, used only for spatial indexing, NOT for
    // coordinate transformation.

    let route_json = r#"{
        "route_points": [
            [25.0000, 121.0000],
            [25.0010, 121.0010],
            [25.0020, 121.0020]
        ]
    }"#;

    let route: input::RouteInput = serde_json::from_str(route_json).unwrap();
    let lat_avg = 25.0;

    let route_pts_cm: Vec<(i64, i64)> = route.route_points.iter()
        .map(|p| {
            let (x, y) = coord::latlon_to_cm_relative(p.lat(), p.lon(), lat_avg);
            (x as i64, y as i64)
        })
        .collect();

    let route_nodes = linearize::linearize_route(&route_pts_cm);
    let grid = grid::build_grid(&route_nodes, 10000);

    // Find minimum x and y from route nodes
    let min_x = route_nodes.iter().map(|n| n.x_cm as i64).min().unwrap();
    let min_y = route_nodes.iter().map(|n| n.y_cm as i64).min().unwrap();

    // Assert: Grid origin equals the minimum x/y from route nodes
    assert_eq!(grid.x0_cm as i64, min_x,
        "Grid x0_cm should be the minimum x from route nodes (for spatial indexing)");
    assert_eq!(grid.y0_cm as i64, min_y,
        "Grid y0_cm should be the minimum y from route nodes (for spatial indexing)");

    // Important: This does NOT mean nodes are stored relative to grid origin!
    // Nodes are still stored as absolute coordinates.
    // The grid origin is just metadata for the spatial grid.
}

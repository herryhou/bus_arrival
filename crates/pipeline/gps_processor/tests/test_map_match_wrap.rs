//! Test for u32 wrap bug when GPS outside bounding box
//!
//! Bug: If GPS point is outside route bounding box (x < x0_cm or y < y0_cm),
//! the subtraction (gps_x - x0_cm) goes negative, then wraps to ~4 billion when cast to u32.
//! This causes garbage grid coordinates and incorrect map matching results.
//!
//! Fix: Add guard to return last_idx conservatively when GPS is outside bounds.

#![cfg(feature = "std")]

use gps_processor::map_match::find_best_segment_restricted;
use shared::{RouteNode, SpatialGrid};

#[test]
fn test_gps_outside_bounds_returns_last_idx() {
    // Create minimal route data with x0_cm=100000, y0_cm=100000
    let mut nodes = Vec::new();

    // Single segment at (100000, 100000) with heading 0 (North)
    nodes.push(RouteNode {
        seg_len_mm: 100000, // 100m
        x_cm: 100000,
        y_cm: 100000,
        cum_dist_cm: 0,
        dx_cm: 0,
        dy_cm: 10000,
        heading_cdeg: 0,
        _pad: 0,
    });

    // End node
    nodes.push(RouteNode {
        seg_len_mm: 0,
        x_cm: 100000,
        y_cm: 110000,
        cum_dist_cm: 10000,
        dx_cm: 0,
        dy_cm: 0,
        heading_cdeg: 0,
        _pad: 0,
    });

    // Create grid with x0_cm=100000, y0_cm=100000
    let grid = SpatialGrid {
        cells: vec![vec![0]],
        grid_size_cm: 10000,
        cols: 1,
        rows: 1,
        x0_cm: 100000,
        y0_cm: 100000,
    };

    // Pack route data
    let mut buffer = Vec::new();
    shared::binfile::pack_route_data(&nodes, &[], &grid, 25.0, &mut buffer)
        .expect("Failed to pack test route data");

    // Load route data
    let route_data = shared::binfile::RouteData::load(&buffer).expect("Failed to load route data");

    // GPS point outside bounds (x < x0_cm)
    // This would cause: (50000 - 100000) = -50000 -> wraps to ~4 billion when cast to u32
    let gps_x = 50000; // Less than x0_cm=100000
    let gps_y = 105000; // Within y bounds
    let last_idx = 0;

    let result = find_best_segment_restricted(gps_x, gps_y, 0, 0, &route_data, last_idx, false);

    // Should return last_idx conservatively, not wrap to garbage grid coordinates
    assert_eq!(
        result, last_idx,
        "GPS outside bounds should return last_idx conservatively, but got {}",
        result
    );
}

#[test]
fn test_gps_outside_y_bounds_returns_last_idx() {
    // Create minimal route data
    let mut nodes = Vec::new();

    nodes.push(RouteNode {
        seg_len_mm: 100000,
        x_cm: 100000,
        y_cm: 100000,
        cum_dist_cm: 0,
        dx_cm: 0,
        dy_cm: 10000,
        heading_cdeg: 0,
        _pad: 0,
    });

    nodes.push(RouteNode {
        seg_len_mm: 0,
        x_cm: 100000,
        y_cm: 110000,
        cum_dist_cm: 10000,
        dx_cm: 0,
        dy_cm: 0,
        heading_cdeg: 0,
        _pad: 0,
    });

    let grid = SpatialGrid {
        cells: vec![vec![0]],
        grid_size_cm: 10000,
        cols: 1,
        rows: 1,
        x0_cm: 100000,
        y0_cm: 100000,
    };

    let mut buffer = Vec::new();
    shared::binfile::pack_route_data(&nodes, &[], &grid, 25.0, &mut buffer)
        .expect("Failed to pack test route data");

    let route_data = shared::binfile::RouteData::load(&buffer).expect("Failed to load route data");

    // GPS point outside bounds (y < y0_cm)
    let gps_x = 105000; // Within x bounds
    let gps_y = 50000; // Less than y0_cm=100000
    let last_idx = 0;

    let result = find_best_segment_restricted(gps_x, gps_y, 0, 0, &route_data, last_idx, false);

    // Should return last_idx conservatively
    assert_eq!(
        result, last_idx,
        "GPS outside y bounds should return last_idx conservatively, but got {}",
        result
    );
}

#[test]
fn test_gps_inside_bounds_works_normally() {
    // Create minimal route data
    let mut nodes = Vec::new();

    nodes.push(RouteNode {
        seg_len_mm: 100000,
        x_cm: 100000,
        y_cm: 100000,
        cum_dist_cm: 0,
        dx_cm: 0,
        dy_cm: 10000,
        heading_cdeg: 0,
        _pad: 0,
    });

    nodes.push(RouteNode {
        seg_len_mm: 0,
        x_cm: 100000,
        y_cm: 110000,
        cum_dist_cm: 10000,
        dx_cm: 0,
        dy_cm: 0,
        heading_cdeg: 0,
        _pad: 0,
    });

    let grid = SpatialGrid {
        cells: vec![vec![0]],
        grid_size_cm: 10000,
        cols: 1,
        rows: 1,
        x0_cm: 100000,
        y0_cm: 100000,
    };

    let mut buffer = Vec::new();
    shared::binfile::pack_route_data(&nodes, &[], &grid, 25.0, &mut buffer)
        .expect("Failed to pack test route data");

    let route_data = shared::binfile::RouteData::load(&buffer).expect("Failed to load route data");

    // GPS point inside bounds - should work normally
    let gps_x = 100000; // Exactly at x0_cm
    let gps_y = 105000; // Within y bounds
    let last_idx = 0;

    let result = find_best_segment_restricted(gps_x, gps_y, 0, 0, &route_data, last_idx, false);

    // Should find a valid segment (either 0 or 1)
    assert!(
        result <= 1,
        "GPS inside bounds should find valid segment, but got {}",
        result
    );
}

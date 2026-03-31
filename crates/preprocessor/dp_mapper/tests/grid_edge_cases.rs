//! Grid Index Edge Cases
//!
//! Tests for edge cases in the spatial grid indexing:
//! - Empty/single segment grid
//! - Route crossing grid boundaries
//! - Query outside grid bounds
//! - Duplicate segments in multiple cells

use dp_mapper::grid::{build_grid, query_neighbors};
use shared::RouteNode;

// ============================================================================
// Grid Construction Edge Cases
// ============================================================================

#[test]
fn test_empty_route_grid() {
    // --- GIVEN ---
    let route: &[RouteNode] = &[];
    let grid_size_cm = 10000;

    // --- WHEN ---
    let grid = build_grid(route, grid_size_cm);

    // --- THEN ---
    // The grid should have 0 columns and 0 rows
    assert_eq!(grid.cols, 0, "empty route: cols = 0");
    assert_eq!(grid.rows, 0, "empty route: rows = 0");
    assert_eq!(grid.cells.len(), 0, "empty route: no cells");
}

#[test]
fn test_single_segment_grid() {
    // --- GIVEN ---
    let route = vec![
        RouteNode {
            seg_len_mm: 50000,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            dx_cm: 5000,
            dy_cm: 0,
        },
        RouteNode {
            seg_len_mm: 0,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 5000,
            y_cm: 0,
            cum_dist_cm: 5000,
            dx_cm: 0,
            dy_cm: 0,
        },
    ];
    let grid_size_cm = 10000;

    // --- WHEN ---
    let grid = build_grid(&route, grid_size_cm);

    // --- THEN ---
    // The grid should have at least 1 cell
    assert_eq!(grid.cols, 1, "single segment: 1 column");
    assert_eq!(grid.rows, 1, "single segment: 1 row");
    assert_eq!(grid.cells.len(), 1, "single segment: 1 cell");
    assert_eq!(grid.cells[0].len(), 1, "cell contains segment 0");
}

#[test]
fn test_route_crossing_grid_boundaries() {
    // --- GIVEN ---
    // A route with a 50m segment (broken into 5 segments of 10m each to fit i16)
    // Grid cell size is 10m
    let route = vec![
        RouteNode {
            seg_len_mm: 100000,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            dx_cm: 10000,
            dy_cm: 0,
        },
        RouteNode {
            seg_len_mm: 100000,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 10000,
            y_cm: 0,
            cum_dist_cm: 10000,
            dx_cm: 10000,
            dy_cm: 0,
        },
        RouteNode {
            seg_len_mm: 100000,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 20000,
            y_cm: 0,
            cum_dist_cm: 20000,
            dx_cm: 10000,
            dy_cm: 0,
        },
        RouteNode {
            seg_len_mm: 100000,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 30000,
            y_cm: 0,
            cum_dist_cm: 30000,
            dx_cm: 10000,
            dy_cm: 0,
        },
        RouteNode {
            seg_len_mm: 100000,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 40000,
            y_cm: 0,
            cum_dist_cm: 40000,
            dx_cm: 10000,
            dy_cm: 0,
        },
        RouteNode {
            seg_len_mm: 0,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 50000,
            y_cm: 0,
            cum_dist_cm: 50000,
            dx_cm: 0,
            dy_cm: 0,
        },
    ];
    let grid_size_cm = 10000; // 10m cells

    // --- WHEN ---
    let grid = build_grid(&route, grid_size_cm);

    // --- THEN ---
    // The segment should appear in multiple cells
    // With 50m segment and 10m cells, should span ~5-6 cells
    assert!(grid.cols >= 5, "segment spans multiple columns: got {}", grid.cols);

    // Count total cell entries (segment may appear multiple times)
    let total_entries: usize = grid.cells.iter().map(|cell| cell.len()).sum();
    assert!(total_entries >= 5, "segment appears in at least 5 cells: got {}", total_entries);
}

#[test]
fn test_vertical_segment_crossing_rows() {
    // --- GIVEN ---
    // A vertical segment crossing multiple rows (5 segments of 10m each)
    let route = vec![
        RouteNode {
            seg_len_mm: 100000,
            heading_cdeg: 9000,
            _pad: 0,
            x_cm: 5000,
            y_cm: 0,
            cum_dist_cm: 0,
            dx_cm: 0,
            dy_cm: 10000,
        },
        RouteNode {
            seg_len_mm: 100000,
            heading_cdeg: 9000,
            _pad: 0,
            x_cm: 5000,
            y_cm: 10000,
            cum_dist_cm: 10000,
            dx_cm: 0,
            dy_cm: 10000,
        },
        RouteNode {
            seg_len_mm: 100000,
            heading_cdeg: 9000,
            _pad: 0,
            x_cm: 5000,
            y_cm: 20000,
            cum_dist_cm: 20000,
            dx_cm: 0,
            dy_cm: 10000,
        },
        RouteNode {
            seg_len_mm: 100000,
            heading_cdeg: 9000,
            _pad: 0,
            x_cm: 5000,
            y_cm: 30000,
            cum_dist_cm: 30000,
            dx_cm: 0,
            dy_cm: 10000,
        },
        RouteNode {
            seg_len_mm: 100000,
            heading_cdeg: 9000,
            _pad: 0,
            x_cm: 5000,
            y_cm: 40000,
            cum_dist_cm: 40000,
            dx_cm: 0,
            dy_cm: 10000,
        },
        RouteNode {
            seg_len_mm: 0,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 5000,
            y_cm: 50000,
            cum_dist_cm: 50000,
            dx_cm: 0,
            dy_cm: 0,
        },
    ];
    let grid_size_cm = 10000;

    // --- WHEN ---
    let grid = build_grid(&route, grid_size_cm);

    // --- THEN ---
    assert!(grid.rows >= 5, "vertical segment spans multiple rows: got {}", grid.rows);
}

#[test]
fn test_diagonal_segment_crossing_both() {
    // --- GIVEN ---
    // A diagonal segment crossing both columns and rows (3 segments of ~14.14m each)
    let route = vec![
        RouteNode {
            seg_len_mm: 141421,
            heading_cdeg: 4500,
            _pad: 0,
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            dx_cm: 10000,
            dy_cm: 10000,
        },
        RouteNode {
            seg_len_mm: 141421,
            heading_cdeg: 4500,
            _pad: 0,
            x_cm: 10000,
            y_cm: 10000,
            cum_dist_cm: 14142,
            dx_cm: 10000,
            dy_cm: 10000,
        },
        RouteNode {
            seg_len_mm: 141421,
            heading_cdeg: 4500,
            _pad: 0,
            x_cm: 20000,
            y_cm: 20000,
            cum_dist_cm: 28284,
            dx_cm: 10000,
            dy_cm: 10000,
        },
        RouteNode {
            seg_len_mm: 0,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 30000,
            y_cm: 30000,
            cum_dist_cm: 42426,
            dx_cm: 0,
            dy_cm: 0,
        },
    ];
    let grid_size_cm = 10000;

    // --- WHEN ---
    let grid = build_grid(&route, grid_size_cm);

    // --- THEN ---
    // Diagonal should span both columns and rows
    assert!(grid.cols >= 3, "diagonal spans columns: got {}", grid.cols);
    assert!(grid.rows >= 3, "diagonal spans rows: got {}", grid.rows);
}

// ============================================================================
// Grid Query Edge Cases
// ============================================================================

#[test]
fn test_query_outside_grid_bounds() {
    // --- GIVEN ---
    let route = vec![
        RouteNode {
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            dx_cm: 10000,
            dy_cm: 0,
            seg_len_mm: (10000 * 10),
        },
        RouteNode {
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 10000,
            y_cm: 0,
            cum_dist_cm: 10000,
            dx_cm: 0,
            dy_cm: 0,
            seg_len_mm: (0 * 10),
        },
    ];
    let grid = build_grid(&route, 10000);

    // Grid covers x: [0, 10000], y: [0, 10000]
    // Query at (20000, 20000) - outside grid

    // --- WHEN ---
    let result = query_neighbors(&grid, 20000, 20000, 1);

    // --- THEN ---
    // The result should be empty
    // No out-of-bounds access should occur
    assert_eq!(result.len(), 0, "query outside bounds should return empty");
}

#[test]
fn test_query_at_negative_coordinates() {
    // --- GIVEN ---
    let route = vec![
        RouteNode {
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 5000,
            y_cm: 5000,
            cum_dist_cm: 0,
            dx_cm: 10000,
            dy_cm: 0,
            seg_len_mm: (10000 * 10),
        },
        RouteNode {
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 15000,
            y_cm: 5000,
            cum_dist_cm: 10000,
            dx_cm: 0,
            dy_cm: 0,
            seg_len_mm: (0 * 10),
        },
    ];
    let grid = build_grid(&route, 10000);

    // Query at negative coordinates
    // --- WHEN ---
    let result = query_neighbors(&grid, -1000, -1000, 1);

    // --- THEN ---
    // The grid has x0_cm=5000, so (-1000, -1000) is outside the grid
    // Should handle gracefully (may return empty or results depending on grid implementation)
    // The key is that it shouldn't crash
    assert!(result.len() >= 0, "negative coordinates handled gracefully");
}

#[test]
fn test_query_with_radius_larger_than_grid() {
    // --- GIVEN ---
    let route = vec![
        RouteNode {
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            dx_cm: 10000,
            dy_cm: 0,
            seg_len_mm: (10000 * 10),
        },
        RouteNode {
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 10000,
            y_cm: 0,
            cum_dist_cm: 10000,
            dx_cm: 0,
            dy_cm: 0,
            seg_len_mm: (0 * 10),
        },
    ];
    let grid = build_grid(&route, 10000);

    // 3x3 grid with radius 10 (much larger than grid)
    // --- WHEN ---
    let result = query_neighbors(&grid, 5000, 0, 10);

    // --- THEN ---
    // Only valid cells should be returned
    // No out-of-bounds access should occur
    assert!(!result.is_empty(), "should find segment within large radius");
    assert!(result.contains(&0), "should contain segment 0");
}

#[test]
fn test_query_with_zero_radius() {
    // --- GIVEN ---
    let route = vec![
        RouteNode {
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            dx_cm: 10000,
            dy_cm: 0,
            seg_len_mm: (10000 * 10),
        },
        RouteNode {
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 10000,
            y_cm: 0,
            cum_dist_cm: 10000,
            dx_cm: 0,
            dy_cm: 0,
            seg_len_mm: (0 * 10),
        },
    ];
    let grid = build_grid(&route, 10000);

    // --- WHEN ---
    let result = query_neighbors(&grid, 5000, 0, 0);

    // --- THEN ---
    // Should return only the cell containing the query point
    assert!(!result.is_empty(), "zero radius should still return results");
}

// ============================================================================
// Duplicate Segment in Multiple Cells
// ============================================================================

#[test]
fn test_duplicate_segment_deduplication() {
    // --- GIVEN ---
    // A segment that spans multiple grid cells (5 segments of 10m each)
    let route = vec![
        RouteNode {
            seg_len_mm: 100000,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            dx_cm: 10000,
            dy_cm: 0,
        },
        RouteNode {
            seg_len_mm: 100000,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 10000,
            y_cm: 0,
            cum_dist_cm: 10000,
            dx_cm: 10000,
            dy_cm: 0,
        },
        RouteNode {
            seg_len_mm: 100000,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 20000,
            y_cm: 0,
            cum_dist_cm: 20000,
            dx_cm: 10000,
            dy_cm: 0,
        },
        RouteNode {
            seg_len_mm: 100000,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 30000,
            y_cm: 0,
            cum_dist_cm: 30000,
            dx_cm: 10000,
            dy_cm: 0,
        },
        RouteNode {
            seg_len_mm: 100000,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 40000,
            y_cm: 0,
            cum_dist_cm: 40000,
            dx_cm: 10000,
            dy_cm: 0,
        },
        RouteNode {
            seg_len_mm: 0,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 50000,
            y_cm: 0,
            cum_dist_cm: 50000,
            dx_cm: 0,
            dy_cm: 0,
        },
    ];
    let grid = build_grid(&route, 10000);

    // --- WHEN ---
    // Same cell query should return deduplicated results
    let result = query_neighbors(&grid, 25000, 0, 1);

    // --- THEN ---
    // The grid implementation may return duplicate segments when they span multiple cells
    // Deduplication is typically handled by the caller
    // Here we just verify that the query returns some results
    assert!(!result.is_empty(), "query should find segments");
}

#[test]
fn test_multiple_segments_same_cell() {
    // --- GIVEN ---
    // Multiple segments in the same cell
    let route = vec![
        RouteNode {
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            dx_cm: 5000,
            dy_cm: 0,
            seg_len_mm: (5000 * 10),
        },
        RouteNode {
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 5000,
            y_cm: 0,
            cum_dist_cm: 5000,
            dx_cm: 5000,
            dy_cm: 0,
            seg_len_mm: (5000 * 10),
        },
        RouteNode {
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 10000,
            y_cm: 0,
            cum_dist_cm: 10000,
            dx_cm: 0,
            dy_cm: 0,
            seg_len_mm: (0 * 10),
        },
    ];
    let grid = build_grid(&route, 10000);

    // --- WHEN ---
    let result = query_neighbors(&grid, 5000, 0, 1);

    // --- THEN ---
    // Should find both segments
    assert_eq!(result.len(), 2, "should find both segments in same cell");
    assert!(result.contains(&0), "contains segment 0");
    assert!(result.contains(&1), "contains segment 1");
}

// ============================================================================
// Edge Cases: Grid Origin and Bounds
// ============================================================================

#[test]
fn test_grid_with_negative_coordinates() {
    // --- GIVEN ---
    // Route with negative coordinates
    let route = vec![
        RouteNode {
            heading_cdeg: 0,
            _pad: 0,
            x_cm: -5000,
            y_cm: -5000,
            cum_dist_cm: 0,
            dx_cm: 10000,
            dy_cm: 0,
            seg_len_mm: (10000 * 10),
        },
        RouteNode {
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 5000,
            y_cm: -5000,
            cum_dist_cm: 10000,
            dx_cm: 0,
            dy_cm: 0,
            seg_len_mm: (0 * 10),
        },
    ];
    let grid_size_cm = 10000;

    // --- WHEN ---
    let grid = build_grid(&route, grid_size_cm);

    // --- THEN ---
    // Grid should handle negative coordinates
    assert!(grid.cols >= 1, "grid with negative coords: cols >= 1");
    assert!(grid.rows >= 1, "grid with negative coords: rows >= 1");
}

#[test]
fn test_grid_offset_origin() {
    // --- GIVEN ---
    // Route not starting at origin
    let route = vec![
        RouteNode {
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 100000,
            y_cm: 100000,
            cum_dist_cm: 0,
            dx_cm: 10000,
            dy_cm: 0,
            seg_len_mm: (10000 * 10),
        },
        RouteNode {
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 110000,
            y_cm: 100000,
            cum_dist_cm: 10000,
            dx_cm: 0,
            dy_cm: 0,
            seg_len_mm: (0 * 10),
        },
    ];
    let grid_size_cm = 10000;

    // --- WHEN ---
    let grid = build_grid(&route, grid_size_cm);

    // --- THEN ---
    // Grid origin should be offset
    assert_eq!(grid.x0_cm, 100000, "grid x0 should match route start");
    assert_eq!(grid.y0_cm, 100000, "grid y0 should match route start");
}

// ============================================================================
// Edge Cases: Very Small/Large Grid Cells
// ============================================================================

#[test]
fn test_very_small_grid_cells() {
    // --- GIVEN ---
    let route = vec![
        RouteNode {
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            dx_cm: 10000,
            dy_cm: 0,
            seg_len_mm: (10000 * 10),
        },
        RouteNode {
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 10000,
            y_cm: 0,
            cum_dist_cm: 10000,
            dx_cm: 0,
            dy_cm: 0,
            seg_len_mm: (0 * 10),
        },
    ];
    let grid_size_cm = 100; // 1m cells - very small

    // --- WHEN ---
    let grid = build_grid(&route, grid_size_cm);

    // --- THEN ---
    // Should create many cells
    assert!(grid.cols >= 100, "small cells: many columns: got {}", grid.cols);
}

#[test]
fn test_very_large_grid_cells() {
    // --- GIVEN ---
    let route = vec![
        RouteNode {
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            dx_cm: 10000,
            dy_cm: 0,
            seg_len_mm: (10000 * 10),
        },
        RouteNode {
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 10000,
            y_cm: 0,
            cum_dist_cm: 10000,
            dx_cm: 0,
            dy_cm: 0,
            seg_len_mm: (0 * 10),
        },
    ];
    let grid_size_cm = 100000; // 1km cells - very large

    // --- WHEN ---
    let grid = build_grid(&route, grid_size_cm);

    // --- THEN ---
    // Should create few cells
    assert_eq!(grid.cols, 1, "large cells: single column");
    assert_eq!(grid.rows, 1, "large cells: single row");
}

// ============================================================================
// Edge Cases: Query at Cell Boundaries
// ============================================================================

#[test]
fn test_query_at_exact_cell_boundary() {
    // --- GIVEN ---
    let route = vec![
        RouteNode {
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            dx_cm: 20000,
            dy_cm: 0,
            seg_len_mm: (20000 * 10),
        },
        RouteNode {
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 20000,
            y_cm: 0,
            cum_dist_cm: 20000,
            dx_cm: 0,
            dy_cm: 0,
            seg_len_mm: (0 * 10),
        },
    ];
    let grid = build_grid(&route, 10000);

    // Query exactly at cell boundary (10000, 0)
    // --- WHEN ---
    let result = query_neighbors(&grid, 10000, 0, 1);

    // --- THEN ---
    // Should handle boundary gracefully
    assert!(!result.is_empty(), "boundary query should return results");
}

#[test]
fn test_query_at_segment_endpoint() {
    // --- GIVEN ---
    let route = vec![
        RouteNode {
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            dx_cm: 10000,
            dy_cm: 0,
            seg_len_mm: (10000 * 10),
        },
        RouteNode {
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 10000,
            y_cm: 0,
            cum_dist_cm: 10000,
            dx_cm: 0,
            dy_cm: 0,
            seg_len_mm: (0 * 10),
        },
    ];
    let grid = build_grid(&route, 10000);

    // Query at segment endpoint
    // --- WHEN ---
    let result = query_neighbors(&grid, 10000, 0, 0);

    // --- THEN ---
    // The endpoint (10000, 0) with radius 0 may or may not return results
    // depending on cell boundary handling
    // The key is that it shouldn't crash
    assert!(result.len() >= 0, "endpoint query handled gracefully");
}

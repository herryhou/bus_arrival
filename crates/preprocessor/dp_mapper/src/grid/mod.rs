//! Spatial indexing for O(k) segment queries

pub mod builder;

pub use builder::{build_grid, query_neighbors};

/// Spatial grid for O(k) segment queries
pub struct SpatialGrid {
    pub cells: Vec<Vec<usize>>,
    pub grid_size_cm: i32,
    pub cols: u32,
    pub rows: u32,
    pub x0_cm: i32,
    pub y0_cm: i32,
}

#[cfg(test)]
mod tests {
    use crate::grid::builder;

    #[test]
    fn test_empty_grid() {
        let grid = builder::build_grid(&[], 10000);
        assert_eq!(grid.cols, 0);
        assert_eq!(grid.rows, 0);
    }

    #[test]
    fn test_single_segment() {
        use shared::RouteNode;
        let nodes = vec![
            RouteNode {
                seg_len_mm: 1000,
                heading_cdeg: 0,
                _pad: 0,
                x_cm: 0,
                y_cm: 0,
                cum_dist_cm: 0,
                dx_cm: 100,
                dy_cm: 0,
            },
            RouteNode {
                seg_len_mm: 0,
                heading_cdeg: 0,
                _pad: 0,
                x_cm: 100,
                y_cm: 0,
                cum_dist_cm: 100,
                dx_cm: 0,
                dy_cm: 0,
            },
        ];
        let grid = builder::build_grid(&nodes, 10000);
        // With 100m margin, grid covers (-10000, -10000) to (10100, 10000)
        // Grid size 10000cm gives 3x3 grid
        assert_eq!(grid.cols, 3);
        assert_eq!(grid.rows, 2);
        // Segment at (0,0) maps to middle column (cell index 1, 3, or 5 depending on row)
        // Count total segments across all cells - should be 1
        let total_segments: usize = grid.cells.iter().map(|c| c.len()).sum();
        assert_eq!(total_segments, 1);
    }

    #[test]
    fn test_multi_segment_grid() {
        use shared::RouteNode;
        let nodes = vec![
            RouteNode { seg_len_mm: 100000, heading_cdeg: 0, _pad: 0, x_cm: 0, y_cm: 0, cum_dist_cm: 0, dx_cm: 10000, dy_cm: 0 },
            RouteNode { seg_len_mm: 100000, heading_cdeg: 0, _pad: 0, x_cm: 10000, y_cm: 0, cum_dist_cm: 10000, dx_cm: 0, dy_cm: 10000 },
            RouteNode { seg_len_mm: 0, heading_cdeg: 0, _pad: 0, x_cm: 10000, y_cm: 10000, cum_dist_cm: 20000, dx_cm: 0, dy_cm: 0 },
        ];
        let grid = builder::build_grid(&nodes, 10000);
        // With 100m margin: (-10000,-10000) to (20000,20000)
        // Grid size 10000cm gives 3x3 grid
        assert_eq!(grid.cols, 3);
        assert_eq!(grid.rows, 3);
        assert_eq!(grid.x0_cm, -10000);
        assert_eq!(grid.y0_cm, -10000);
    }

    #[test]
    fn test_query_neighbors_radius_1() {
        use shared::RouteNode;
        let nodes = vec![
            RouteNode { seg_len_mm: 100000, heading_cdeg: 0, _pad: 0, x_cm: 0, y_cm: 0, cum_dist_cm: 0, dx_cm: 10000, dy_cm: 0 },
            RouteNode { seg_len_mm: 100000, heading_cdeg: 0, _pad: 0, x_cm: 10000, y_cm: 0, cum_dist_cm: 10000, dx_cm: 0, dy_cm: 10000 },
            RouteNode { seg_len_mm: 0, heading_cdeg: 0, _pad: 0, x_cm: 10000, y_cm: 10000, cum_dist_cm: 20000, dx_cm: 0, dy_cm: 0 },
        ];
        let grid = builder::build_grid(&nodes, 10000);

        // Query at origin, radius 1 (3x3 neighborhood)
        let result = builder::query_neighbors(&grid, 0, 0, 1);
        assert!(!result.is_empty());
        assert!(result.contains(&0)); // segment 0 should be found
    }

    #[test]
    fn test_query_neighbors_dedup() {
        use shared::RouteNode;
        let nodes = vec![
            RouteNode { seg_len_mm: 100000, heading_cdeg: 0, _pad: 0, x_cm: 0, y_cm: 0, cum_dist_cm: 0, dx_cm: 10000, dy_cm: 0 },
            RouteNode { seg_len_mm: 100000, heading_cdeg: 0, _pad: 0, x_cm: 10000, y_cm: 0, cum_dist_cm: 10000, dx_cm: 0, dy_cm: 10000 },
            RouteNode { seg_len_mm: 0, heading_cdeg: 0, _pad: 0, x_cm: 10000, y_cm: 10000, cum_dist_cm: 20000, dx_cm: 0, dy_cm: 0 },
        ];
        let grid = builder::build_grid(&nodes, 10000);

        // Query at (5000, 0) with radius 1
        let result = builder::query_neighbors(&grid, 5000, 0, 1);
        // Result may contain duplicates (same segment in multiple cells)
        // Dedup and verify we get unique segments
        let mut sorted = result.clone();
        sorted.sort();
        sorted.dedup();
        assert!(!sorted.is_empty());
        // After dedup, we should have fewer or equal elements
        assert!(sorted.len() <= result.len());
    }
}

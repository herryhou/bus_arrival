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
                len2_cm2: 10000,
                heading_cdeg: 0,
                _pad: 0,
                x_cm: 0,
                y_cm: 0,
                cum_dist_cm: 0,
                dx_cm: 100,
                dy_cm: 0,
                seg_len_cm: 100,
            },
            RouteNode {
                len2_cm2: 0,
                heading_cdeg: 0,
                _pad: 0,
                x_cm: 100,
                y_cm: 0,
                cum_dist_cm: 100,
                dx_cm: 0,
                dy_cm: 0,
                seg_len_cm: 0,
            },
        ];
        let grid = builder::build_grid(&nodes, 10000);
        assert_eq!(grid.cols, 1);
        assert_eq!(grid.rows, 1);
        assert_eq!(grid.cells[0].len(), 1); // segment 0 in cell 0
    }

    #[test]
    fn test_multi_segment_grid() {
        use shared::RouteNode;
        let nodes = vec![
            RouteNode { len2_cm2: 100000000, heading_cdeg: 0, _pad: 0, x_cm: 0, y_cm: 0, cum_dist_cm: 0, dx_cm: 10000, dy_cm: 0, seg_len_cm: 10000 },
            RouteNode { len2_cm2: 100000000, heading_cdeg: 0, _pad: 0, x_cm: 10000, y_cm: 0, cum_dist_cm: 10000, dx_cm: 0, dy_cm: 10000, seg_len_cm: 10000 },
            RouteNode { len2_cm2: 0, heading_cdeg: 0, _pad: 0, x_cm: 10000, y_cm: 10000, cum_dist_cm: 20000, dx_cm: 0, dy_cm: 0, seg_len_cm: 0 },
        ];
        let grid = builder::build_grid(&nodes, 10000);
        // Should have 2x1 grid (x: 0-10000, y: 0-10000)
        assert_eq!(grid.cols, 1);
        assert_eq!(grid.rows, 1);
        assert_eq!(grid.x0_cm, 0);
        assert_eq!(grid.y0_cm, 0);
    }

    #[test]
    fn test_query_neighbors_radius_1() {
        use shared::RouteNode;
        let nodes = vec![
            RouteNode { len2_cm2: 100000000, heading_cdeg: 0, _pad: 0, x_cm: 0, y_cm: 0, cum_dist_cm: 0, dx_cm: 10000, dy_cm: 0, seg_len_cm: 10000 },
            RouteNode { len2_cm2: 100000000, heading_cdeg: 0, _pad: 0, x_cm: 10000, y_cm: 0, cum_dist_cm: 10000, dx_cm: 0, dy_cm: 10000, seg_len_cm: 10000 },
            RouteNode { len2_cm2: 0, heading_cdeg: 0, _pad: 0, x_cm: 10000, y_cm: 10000, cum_dist_cm: 20000, dx_cm: 0, dy_cm: 0, seg_len_cm: 0 },
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
            RouteNode { len2_cm2: 100000000, heading_cdeg: 0, _pad: 0, x_cm: 0, y_cm: 0, cum_dist_cm: 0, dx_cm: 10000, dy_cm: 0, seg_len_cm: 10000 },
            RouteNode { len2_cm2: 100000000, heading_cdeg: 0, _pad: 0, x_cm: 10000, y_cm: 0, cum_dist_cm: 10000, dx_cm: 0, dy_cm: 10000, seg_len_cm: 10000 },
            RouteNode { len2_cm2: 0, heading_cdeg: 0, _pad: 0, x_cm: 10000, y_cm: 10000, cum_dist_cm: 20000, dx_cm: 0, dy_cm: 0, seg_len_cm: 0 },
        ];
        let grid = builder::build_grid(&nodes, 10000);

        // Same cell query should return deduplicated results
        let result = builder::query_neighbors(&grid, 5000, 0, 1);
        // No duplicates in result
        let mut sorted = result.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(result.len(), sorted.len());
    }
}

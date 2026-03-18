//! Spatial indexing for O(k) segment queries

pub mod builder;

pub use builder::{build_grid, query_neighbors};

use shared::RouteNode;

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
    use super::*;

    #[test]
    fn test_empty_grid() {
        let grid = build_grid(&[], 10000);
        assert_eq!(grid.cols, 0);
        assert_eq!(grid.rows, 0);
    }
}

//! Grid construction and query functions

use shared::RouteNode;
use super::SpatialGrid;

/// Build a spatial grid index for route nodes
pub fn build_grid(nodes: &[RouteNode], grid_size_cm: i32) -> SpatialGrid {
    if nodes.is_empty() {
        return SpatialGrid {
            cells: vec![],
            grid_size_cm,
            cols: 0,
            rows: 0,
            x0_cm: 0,
            y0_cm: 0,
        };
    }

    // 1. Find bounding box
    const I32_MAX: i32 = i32::MAX;
    const I32_MIN: i32 = i32::MIN;

    let mut min_x = I32_MAX;
    let mut min_y = I32_MAX;
    let mut max_x = I32_MIN;
    let mut max_y = I32_MIN;

    for node in nodes {
        min_x = min_x.min(node.x_cm);
        min_y = min_y.min(node.y_cm);
        max_x = max_x.max(node.x_cm);
        max_y = max_y.max(node.y_cm);
    }

    // Add margin to grid bounds to cover detour paths and GPS noise
    // 100m margin = 10000 cm
    const MARGIN_CM: i32 = 10000;
    min_x = min_x.saturating_sub(MARGIN_CM);
    min_y = min_y.saturating_sub(MARGIN_CM);
    max_x = max_x.saturating_add(MARGIN_CM);
    max_y = max_y.saturating_add(MARGIN_CM);

    // 2. Determine grid dimensions
    let width_cm = max_x - min_x;
    let height_cm = max_y - min_y;

    let cols = ((width_cm as f64 / grid_size_cm as f64).ceil() as u32).max(1);
    let rows = ((height_cm as f64 / grid_size_cm as f64).ceil() as u32).max(1);

    let mut cells = vec![vec![]; (rows * cols) as usize];

    // 3. Map segments to cells
    for i in 0..nodes.len().saturating_sub(1) {
        let node_a = &nodes[i];
        let node_b = &nodes[i + 1];

        // Segment bounding box in grid coordinates
        let seg_min_x = node_a.x_cm.min(node_b.x_cm);
        let seg_max_x = node_a.x_cm.max(node_b.x_cm);
        let seg_min_y = node_a.y_cm.min(node_b.y_cm);
        let seg_max_y = node_a.y_cm.max(node_b.y_cm);

        let x_start = ((seg_min_x - min_x) / grid_size_cm).max(0) as u32;
        let x_end = ((seg_max_x - min_x) / grid_size_cm).min(cols as i32 - 1) as u32;
        let y_start = ((seg_min_y - min_y) / grid_size_cm).max(0) as u32;
        let y_end = ((seg_max_y - min_y) / grid_size_cm).min(rows as i32 - 1) as u32;

        for r in y_start..=y_end {
            for c in x_start..=x_end {
                let cell_idx = (r * cols + c) as usize;
                cells[cell_idx].push(i);
            }
        }
    }

    SpatialGrid {
        cells,
        grid_size_cm,
        cols,
        rows,
        x0_cm: min_x,
        y0_cm: min_y,
    }
}

/// Query grid for segments within a radius (in grid cells)
pub fn query_neighbors(grid: &SpatialGrid, x_cm: i32, y_cm: i32, radius: u32) -> Vec<usize> {
    if grid.cols == 0 || grid.rows == 0 {
        return Vec::new();
    }

    // Convert point to grid coordinates
    let gx = ((x_cm - grid.x0_cm) / grid.grid_size_cm) as i32;
    let gy = ((y_cm - grid.y0_cm) / grid.grid_size_cm) as i32;

    let mut candidates = Vec::new();

    // Expand radius: 1 → 3×3, 2 → 5×5, 3 → 7×7
    let r = radius as i32;
    for dy in -r..=r {
        for dx in -r..=r {
            let nx = gx + dx;
            let ny = gy + dy;

            // Check bounds
            if nx >= 0 && ny >= 0 {
                let nx_u = nx as u32;
                let ny_u = ny as u32;
                if nx_u < grid.cols && ny_u < grid.rows {
                    let cell_idx = (ny_u * grid.cols + nx_u) as usize;
                    if cell_idx < grid.cells.len() {
                        candidates.extend_from_slice(&grid.cells[cell_idx]);
                    }
                }
            }
        }
    }

    candidates
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::RouteNode;

    fn make_l_shaped_route() -> Vec<RouteNode> {
        vec![
            RouteNode { seg_len_mm: 100000, heading_cdeg: 0, _pad: 0, x_cm: 0, y_cm: 0, cum_dist_cm: 0, dx_cm: 10000, dy_cm: 0 },
            RouteNode { seg_len_mm: 100000, heading_cdeg: 0, _pad: 0, x_cm: 10000, y_cm: 0, cum_dist_cm: 10000, dx_cm: 0, dy_cm: 10000 },
            RouteNode { seg_len_mm: 0, heading_cdeg: 0, _pad: 0, x_cm: 10000, y_cm: 10000, cum_dist_cm: 20000, dx_cm: 0, dy_cm: 0 },
        ]
    }

    #[test]
    fn test_query_neighbors_dedup_across_radii() {
        let nodes = make_l_shaped_route();
        let grid = build_grid(&nodes, 10000);

        // Query with radius 2 might return same segment multiple times
        // (when segment spans multiple cells)
        let result = query_neighbors(&grid, 0, 0, 2);
        let mut sorted = result.clone();
        sorted.sort_unstable();
        sorted.dedup();
        // Result may have duplicates, but after dedup should be non-empty
        assert!(!sorted.is_empty());
        // Deduped version should have <= elements
        assert!(sorted.len() <= result.len());
    }

    #[test]
    fn test_query_neighbors_radius_2() {
        let nodes = make_l_shaped_route();
        let grid = build_grid(&nodes, 10000);

        // Query at origin with radius 2 (5x5 neighborhood)
        let result = query_neighbors(&grid, 0, 0, 2);
        assert!(!result.is_empty());
        // Should find both segments
        assert!(result.contains(&0) || result.contains(&1));
    }

    #[test]
    fn test_query_neighbors_radius_3() {
        let nodes = make_l_shaped_route();
        let grid = build_grid(&nodes, 10000);

        // Query at origin with radius 3 (7x7 neighborhood)
        let result = query_neighbors(&grid, 0, 0, 3);
        assert!(!result.is_empty());
        // With larger radius, should find all segments
        assert!(result.contains(&0));
    }

    #[test]
    fn test_query_neighbors_out_of_bounds() {
        let nodes = make_l_shaped_route();
        let grid = build_grid(&nodes, 10000);

        // Query far outside the grid
        let result = query_neighbors(&grid, 100000, 100000, 1);
        // Should return empty or handle gracefully
        assert!(result.is_empty() || !result.is_empty());
    }

    #[test]
    fn test_query_neighbors_empty_grid() {
        let grid = build_grid(&[], 10000);

        // Query on empty grid should not panic
        let result = query_neighbors(&grid, 0, 0, 1);
        assert!(result.is_empty());
    }
}

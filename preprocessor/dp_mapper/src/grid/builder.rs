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

pub fn query_neighbors(_grid: &SpatialGrid, _x_cm: i32, _y_cm: i32, _radius: u32) -> Vec<usize> {
    vec![]
}

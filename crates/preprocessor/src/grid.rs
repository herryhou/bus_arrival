// Spatial Grid Index construction
//
// Partitions the route into 100m x 100m cells to enable O(k) map matching.
// Each cell stores indices of segments that pass through or are near it.

use shared::{RouteNode, SpatialGrid, DistCm};

/// Build a spatial grid index for a set of route nodes
///
/// # Arguments
/// * `nodes` - Linearized route nodes
/// * `grid_size_cm` - Cell size (default 10000 cm / 100m)
pub fn build_grid(nodes: &[RouteNode], grid_size_cm: DistCm) -> SpatialGrid {
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
    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;

    for node in nodes {
        min_x = min_x.min(node.x_cm);
        min_y = min_y.min(node.y_cm);
        max_x = max_x.max(node.x_cm);
        max_y = max_y.max(node.y_cm);
    }

    // 2. Determine grid dimensions
    let cols = (((max_x - min_x) as f64 / grid_size_cm as f64).ceil() as u32).max(1);
    let rows = (((max_y - min_y) as f64 / grid_size_cm as f64).ceil() as u32).max(1);

    let mut cells = vec![vec![]; (rows * cols) as usize];

    // 3. Map segments to cells
    for i in 0..nodes.len() - 1 {
        let node_a = &nodes[i];
        let node_b = &nodes[i + 1];

        // Segment bounding box in grid coordinates
        let x_start = ((node_a.x_cm.min(node_b.x_cm) - min_x) / grid_size_cm).max(0) as u32;
        let x_end = ((node_a.x_cm.max(node_b.x_cm) - min_x) / grid_size_cm).min(cols as i32 - 1) as u32;
        let y_start = ((node_a.y_cm.min(node_b.y_cm) - min_y) / grid_size_cm).max(0) as u32;
        let y_end = ((node_a.y_cm.max(node_b.y_cm) - min_y) / grid_size_cm).min(rows as i32 - 1) as u32;

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

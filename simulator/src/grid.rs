//! Spatial grid index for O(k) map matching

use shared::{RouteNode, GridOrigin, DistCm, SpatialGrid};

pub const GRID_SIZE_CM: DistCm = 10000;  // 100m

/// Build spatial grid from route nodes
pub fn build_spatial_grid(nodes: &[RouteNode], origin: &GridOrigin) -> SpatialGrid {
    if nodes.is_empty() {
        return SpatialGrid::empty();
    }

    let x_min = origin.x0_cm;
    let y_min = origin.y0_cm;

    // Calculate grid dimensions
    let x_max = nodes.iter().map(|n| n.x_cm).max().unwrap();
    let y_max = nodes.iter().map(|n| n.y_cm).max().unwrap();

    let cols = ((x_max - x_min) / GRID_SIZE_CM + 1) as u32;
    let rows = ((y_max - y_min) / GRID_SIZE_CM + 1) as u32;
    let total_cells = (cols * rows) as usize;

    let mut cells: Vec<Vec<usize>> = vec![Vec::new(); total_cells];

    // For each segment, add to intersecting cells
    for i in 0..nodes.len().saturating_sub(1) {
        let p0 = &nodes[i];
        let p1 = &nodes[i + 1];

        // Segment bounding box
        let x_start = ((p0.x_cm.min(p1.x_cm) - x_min) / GRID_SIZE_CM) as usize;
        let x_end = ((p0.x_cm.max(p1.x_cm) - x_min) / GRID_SIZE_CM) as usize;
        let y_start = ((p0.y_cm.min(p1.y_cm) - y_min) / GRID_SIZE_CM) as usize;
        let y_end = ((p0.y_cm.max(p1.y_cm) - y_min) / GRID_SIZE_CM) as usize;

        // Add segment to each cell in bounding box
        for gy in y_start..=y_end.min(rows as usize - 1) {
            for gx in x_start..=x_end.min(cols as usize - 1) {
                let idx = gy * (cols as usize) + gx;
                cells[idx].push(i);
            }
        }
    }

    SpatialGrid {
        cells,
        grid_size_cm: GRID_SIZE_CM,
        cols,
        rows,
        x0_cm: x_min,
        y0_cm: y_min,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_grid() {
        let grid = SpatialGrid::empty();
        assert!(grid.query(0, 0).is_empty());
    }

    #[test]
    fn grid_single_segment() {
        let origin = GridOrigin { x0_cm: 0, y0_cm: 0 };
        let nodes = vec![
            RouteNode {
                len2_cm2: 100000000,
                line_c: 0,
                x_cm: 0,
                y_cm: 0,
                cum_dist_cm: 0,
                dx_cm: 10000,
                dy_cm: 0,
                seg_len_cm: 10000,
                line_a: 0,
                line_b: 10000,
                heading_cdeg: 0,
                _pad: 0,
            },
            RouteNode {
                len2_cm2: 0,
                line_c: 0,
                x_cm: 10000,
                y_cm: 0,
                cum_dist_cm: 10000,
                dx_cm: 0,
                dy_cm: 0,
                seg_len_cm: 0,
                line_a: 0,
                line_b: 0,
                heading_cdeg: 0,
                _pad: 0,
            },
        ];

        let grid = build_spatial_grid(&nodes, &origin);
        assert!(!grid.query(5000, 0).is_empty());
    }
}

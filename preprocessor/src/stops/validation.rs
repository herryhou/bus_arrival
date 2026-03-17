// Stop sequence validation with path-constrained grid search
//
// Ensures stops project to monotonically increasing progress values
// along the route, preserving input order from stops.json.

use shared::{RouteNode, SpatialGrid};

/// Result of quick validation pass
#[derive(Debug)]
pub struct ValidationResult {
    /// Validated stop progress values in input order
    pub progress_values: Vec<i32>,
    /// If validation failed, contains info for retry
    pub reversal_info: Option<ReversalInfo>,
}

/// Information about a detected sequence reversal
#[derive(Debug)]
pub struct ReversalInfo {
    /// Index in stops array where reversal was detected
    pub stop_index: usize,
    /// The problematic progress value (smaller than previous)
    pub problem_progress: i32,
    /// Previous stop's progress (larger)
    pub previous_progress: i32,
    /// Approximate route region to re-simplify (segment indices)
    pub affected_region: (usize, usize),
    /// Suggested epsilon for retry (binary search step)
    pub suggested_epsilon: f64,
    /// Retry attempt count
    pub retry_count: u32,
}

/// Query grid with progressive window expansion
///
/// Returns segments within specified radius (in grid cells) from point.
/// Radius 1 = 3×3 cells, radius 2 = 5×5 cells, radius 3 = 7×7 cells.
fn query_grid_radius(
    grid: &SpatialGrid,
    x_cm: i64,
    y_cm: i64,
    radius: usize,
) -> Vec<usize> {
    if grid.cols == 0 || grid.rows == 0 {
        return Vec::new();
    }

    let gx = ((x_cm - grid.x0_cm as i64) / grid.grid_size_cm as i64) as usize;
    let gy = ((y_cm - grid.y0_cm as i64) / grid.grid_size_cm as i64) as usize;

    let mut candidates = Vec::new();
    let diameter = radius * 2 + 1; // radius 1 → 3×3

    for dy in 0..diameter {
        for dx in 0..diameter {
            let ny = gy as i32 + dy as i32 - radius as i32;
            let nx = gx as i32 + dx as i32 - radius as i32;

            if ny >= 0 && ny < grid.rows as i32 && nx >= 0 && nx < grid.cols as i32 {
                let idx = ny as usize * (grid.cols as usize) + nx as usize;
                if idx < grid.cells.len() {
                    candidates.extend_from_slice(&grid.cells[idx]);
                }
            }
        }
    }

    candidates
}

/// Find closest segment to a point with path constraint
///
/// Only searches segments with index >= min_segment_idx (enforces monotonicity).
/// Uses progressive grid search expansion (3×3 → 5×5 → 7×7 → linear fallback).
///
/// Returns: (segment_index, t_value)
fn find_closest_segment_constrained(
    point: &(i64, i64),
    nodes: &[RouteNode],
    grid: &SpatialGrid,
    min_segment_idx: usize,
) -> (usize, f64) {
    // Try progressive window expansion
    for radius in 1..=3 {
        let candidates: Vec<usize> = query_grid_radius(grid, point.0, point.1, radius)
            .into_iter()
            .filter(|&seg_idx| seg_idx >= min_segment_idx)
            .collect();

        if !candidates.is_empty() {
            return find_closest_in_candidates(point, nodes, &candidates);
        }
    }

    // Fallback: linear search from min_segment_idx
    let linear_candidates: Vec<usize> = (min_segment_idx..nodes.len().saturating_sub(1))
        .filter(|&i| nodes[i].len2_cm2 != 0)
        .collect();

    find_closest_in_candidates(point, nodes, &linear_candidates)
}

/// Find closest segment among candidates
fn find_closest_in_candidates(
    point: &(i64, i64),
    nodes: &[RouteNode],
    candidates: &[usize],
) -> (usize, f64) {
    let mut best_idx = candidates[0];
    let mut best_t = 0.0;
    let mut best_dist2 = i64::MAX;

    for &seg_idx in candidates {
        if seg_idx >= nodes.len() {
            continue;
        }

        let node = &nodes[seg_idx];
        if node.len2_cm2 == 0 {
            continue; // Last node has no outgoing segment
        }

        let dx = point.0 - node.x_cm as i64;
        let dy = point.1 - node.y_cm as i64;

        // Project point onto segment
        let t_num = dx * node.dx_cm as i64 + dy * node.dy_cm as i64;
        let t = (t_num as f64 / node.len2_cm2 as f64).clamp(0.0, 1.0);

        // Closest point on segment
        let px = node.x_cm as f64 + t * node.dx_cm as f64;
        let py = node.y_cm as f64 + t * node.dy_cm as f64;

        let dist_x = point.0 as f64 - px;
        let dist_y = point.1 as f64 - py;
        let dist2 = (dist_x * dist_x + dist_y * dist_y) as i64;

        if dist2 < best_dist2 {
            best_dist2 = dist2;
            best_idx = seg_idx;
            best_t = t;
        }
    }

    (best_idx, best_t)
}

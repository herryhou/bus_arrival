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

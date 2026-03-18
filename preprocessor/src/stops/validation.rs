// Stop sequence validation with path-constrained grid search
//
// This module implements sequence-constrained stop projection to ensure
// that the input order from stops.json is preserved after RDP simplification.
//
// Key algorithms:
// - Path-constrained grid search: each stop can only match segments >= previous stop's segment
// - Progressive window expansion: 3×3 → 5×5 → 7×7 → linear fallback
// - T-constraint: when same segment, must have t > previous_t
// - Monotonicity validation: progress must strictly increase (or equal for same location)

use shared::{RouteNode, SpatialGrid};

/// Result of validation pass with complete segment mapping
#[derive(Debug)]
pub struct ValidationResult {
    /// Validated stop progress values in input order
    pub progress_values: Vec<i32>,
    /// Segment index for each stop (for debugging)
    pub segment_indices: Vec<usize>,
    /// T-value (0.0-1.0) for each stop (for debugging)
    pub t_values: Vec<f64>,
    /// If validation failed, contains info for diagnostics
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
}

/// Query grid with progressive window expansion
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
    let diameter = radius * 2 + 1;

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
/// Constraint: (seg_idx > min_segment_idx) OR (seg_idx == min_segment_idx AND t > min_t)
/// For first stop (min_t = None), accepts seg_idx >= min_segment_idx.
///
/// Uses a single-pass search that checks validity of each candidate immediately.
fn find_closest_segment_constrained(
    point: &(i64, i64),
    nodes: &[RouteNode],
    grid: &SpatialGrid,
    min_segment_idx: usize,
    min_t: Option<f64>,
) -> (usize, f64) {
    let mut best_seg = min_segment_idx;
    let mut best_t = 0.0;
    let mut best_dist2 = i64::MAX;
    let mut found = false;

    // Check if a candidate (seg_idx, t) satisfies the path constraint
    let is_valid = |seg_idx: usize, t: f64| -> bool {
        if seg_idx < min_segment_idx {
            return false;
        }
        if seg_idx == min_segment_idx {
            if let Some(min_t_val) = min_t {
                return t > min_t_val;
            }
        }
        true
    };

    // Try grid search with progressive expansion
    for radius in 1..=3 {
        let candidates = query_grid_radius(grid, point.0, point.1, radius);
        for &seg_idx in &candidates {
            if seg_idx >= nodes.len() || nodes[seg_idx].len2_cm2 == 0 {
                continue;
            }

            let node = &nodes[seg_idx];
            let dx = point.0 - node.x_cm as i64;
            let dy = point.1 - node.y_cm as i64;

            let t_num = dx * node.dx_cm as i64 + dy * node.dy_cm as i64;
            let t = (t_num as f64 / node.len2_cm2 as f64).clamp(0.0, 1.0);

            if !is_valid(seg_idx, t) {
                continue;
            }

            let px = node.x_cm as f64 + t * node.dx_cm as f64;
            let py = node.y_cm as f64 + t * node.dy_cm as f64;

            let dist_x = point.0 as f64 - px;
            let dist_y = point.1 as f64 - py;
            let dist2 = (dist_x * dist_x + dist_y * dist_y) as i64;

            if dist2 < best_dist2 {
                best_dist2 = dist2;
                best_seg = seg_idx;
                best_t = t;
                found = true;
            }
        }

        if found {
            return (best_seg, best_t);
        }
    }

    // Fallback: linear search
    for seg_idx in min_segment_idx..nodes.len().saturating_sub(1) {
        if nodes[seg_idx].len2_cm2 == 0 {
            continue;
        }

        let node = &nodes[seg_idx];
        let dx = point.0 - node.x_cm as i64;
        let dy = point.1 - node.y_cm as i64;

        let t_num = dx * node.dx_cm as i64 + dy * node.dy_cm as i64;
        let t = (t_num as f64 / node.len2_cm2 as f64).clamp(0.0, 1.0);

        if !is_valid(seg_idx, t) {
            continue;
        }

        let px = node.x_cm as f64 + t * node.dx_cm as f64;
        let py = node.y_cm as f64 + t * node.dy_cm as f64;

        let dist_x = point.0 as f64 - px;
        let dist_y = point.1 as f64 - py;
        let dist2 = (dist_x * dist_x + dist_y * dist_y) as i64;

        if dist2 < best_dist2 {
            best_dist2 = dist2;
            best_seg = seg_idx;
            best_t = t;
            found = true;
        }
    }

    if found {
        (best_seg, best_t)
    } else {
        // Last resort: next segment with t=0.0 (shouldn't happen with valid route)
        let next_seg = (min_segment_idx + 1).min(nodes.len().saturating_sub(1));
        (next_seg, 0.0)
    }
}

/// Validate stop sequence for monotonicity
pub fn validate_stop_sequence(
    stops_cm: &[(i64, i64)],
    route_nodes: &[RouteNode],
    grid: &SpatialGrid,
) -> ValidationResult {
    if stops_cm.is_empty() {
        return ValidationResult {
            progress_values: vec![],
            segment_indices: vec![],
            t_values: vec![],
            reversal_info: None,
        };
    }

    let mut progress_values = Vec::with_capacity(stops_cm.len());
    let mut segment_indices = Vec::with_capacity(stops_cm.len());
    let mut t_values = Vec::with_capacity(stops_cm.len());
    let mut min_segment_idx = 0;
    let mut min_t: Option<f64> = None;
    let mut previous_progress = i32::MIN;
    let mut reversal_info = None;

    for (input_idx, stop_pt) in stops_cm.iter().enumerate() {
        let (seg_idx, t) = find_closest_segment_constrained(
            stop_pt,
            route_nodes,
            grid,
            min_segment_idx,
            min_t,
        );

        let node = &route_nodes[seg_idx];
        let progress_cm = node.cum_dist_cm + (t * node.seg_len_cm as f64).round() as i32;

        progress_values.push(progress_cm);
        segment_indices.push(seg_idx);
        t_values.push(t);

        // Allow equal progress for stops at same location
        if progress_cm < previous_progress && reversal_info.is_none() {
            reversal_info = Some(ReversalInfo {
                stop_index: input_idx,
                problem_progress: progress_cm,
                previous_progress,
            });
        }

        previous_progress = progress_cm;

        // Update path constraint for next stop
        // Constraint: next stop must have segment > current_segment OR (same segment AND t > current_t)
        if seg_idx > min_segment_idx {
            min_segment_idx = seg_idx;
            min_t = Some(t); // Set min_t to prevent going backwards on this new segment
        } else {
            // Same segment - update t constraint to be strictly greater
            min_t = Some(t);
        }
    }

    ValidationResult {
        progress_values,
        segment_indices,
        t_values,
        reversal_info,
    }
}

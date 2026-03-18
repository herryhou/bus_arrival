//! Candidate generation functions

use super::Candidate;
use shared::RouteNode;
use crate::grid::{SpatialGrid, query_neighbors};

// Constants
const GRID_RADIUS_MAX: u32 = 3;

/// Generate candidates for a stop (without snap - for first stop)
pub fn generate_candidates(
    stop: (i64, i64),
    route_nodes: &[RouteNode],
    grid: &SpatialGrid,
    k: usize,
) -> Vec<Candidate> {
    let mut candidates = Vec::new();

    // Grid query with expanding radius
    for radius in 1..=GRID_RADIUS_MAX {
        let seg_indices = query_neighbors(grid, stop.0 as i32, stop.1 as i32, radius);

        for &seg_idx in &seg_indices {
            if seg_idx >= route_nodes.len().saturating_sub(1) {
                continue;
            }

            let node = &route_nodes[seg_idx];

            // Skip zero-length segments
            if node.seg_len_cm == 0 || node.len2_cm2 == 0 {
                continue;
            }

            // Project stop onto segment
            let dx = stop.0 - node.x_cm as i64;
            let dy = stop.1 - node.y_cm as i64;

            // t = [(P - A) · (B - A)] / |B - A|²
            let t_num = dx * node.dx_cm as i64 + dy * node.dy_cm as i64;
            let t = (t_num as f64 / node.len2_cm2 as f64).clamp(0.0, 1.0);

            // Closest point on segment
            let px = node.x_cm as f64 + t * node.dx_cm as f64;
            let py = node.y_cm as f64 + t * node.dy_cm as f64;

            // Squared distance
            let dist_x = stop.0 as f64 - px;
            let dist_y = stop.1 as f64 - py;
            let dist_sq_cm2 = (dist_x * dist_x + dist_y * dist_y) as i64;

            // Progress along route
            let progress_cm = node.cum_dist_cm + (t * node.seg_len_cm as f64).round() as i32;

            candidates.push(Candidate {
                seg_idx,
                t,
                dist_sq_cm2,
                progress_cm,
            });
        }

        // If we found candidates in this radius, stop expanding
        if !candidates.is_empty() {
            break;
        }
    }

    if candidates.is_empty() {
        return candidates;
    }

    // Deduplicate by (seg_idx, t)
    candidates.sort_by_key(|c| (c.seg_idx, c.t.to_bits()));
    candidates.dedup_by(|a, b| a.seg_idx == b.seg_idx && a.t == b.t);

    // Sort by distance and keep top-K
    candidates.sort_by_key(|c| c.dist_sq_cm2);
    candidates.truncate(k);

    candidates
}

// Snap penalty: ~316 km² (100× larger than worst legitimate projection)
const SNAP_PENALTY_CM2: i64 = 1_000_000_000_000;

/// Generate candidates with snap-forward fallback
pub fn generate_candidates_with_snap(
    stop: (i64, i64),
    route_nodes: &[RouteNode],
    grid: &SpatialGrid,
    k: usize,
    max_prev_progress_cm: i32,
) -> Vec<Candidate> {
    // Generate normal candidates
    let mut candidates = generate_candidates(stop, route_nodes, grid, k);

    // Find first segment whose END is past max_prev_progress_cm
    let snap_seg_idx = route_nodes
        .iter()
        .position(|n| n.cum_dist_cm + n.seg_len_cm >= max_prev_progress_cm)
        .unwrap_or(route_nodes.len().saturating_sub(2));

    // Create snap candidate at segment start
    let snap_candidate = Candidate {
        seg_idx: snap_seg_idx,
        t: 0.0,
        dist_sq_cm2: SNAP_PENALTY_CM2,
        progress_cm: route_nodes[snap_seg_idx].cum_dist_cm,
    };

    candidates.push(snap_candidate);
    candidates
}

#[cfg(test)]
mod tests {
    include!("generator_tests.rs");
}

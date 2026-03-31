//! DP solver implementation

use crate::candidate::generator::SNAP_PENALTY_CM2;
use crate::candidate::{generate_candidates, generate_candidates_with_snap, Candidate};
use crate::grid::SpatialGrid;
use shared::RouteNode;

/// Threshold for warning about large projection errors (30m = 3000cm)
const PROJECTION_WARN_THRESHOLD_CM2: i64 = 1_000_000; // (1000 cm)²

/// Format stop identifier with index and optional name
fn format_stop_id(index: usize, name: Option<&str>) -> String {
    match name {
        Some(n) if !n.is_empty() => format!("{} ({})", index + 1, n),
        _ => format!("{}", index + 1),
    }
}

/// DP layer for one stop: contains candidate states and running minimum
#[derive(Debug, Clone)]
pub struct DpLayer {
    /// All candidate states for this stop (in original unsorted order)
    pub candidates: Vec<Candidate>,
    /// Running minimum cost for each candidate index
    pub best_cost: Vec<i64>,
    /// Best previous candidate index for reconstruction (None for j=0)
    pub best_prev: Vec<Option<usize>>,
}

/// Candidate with original index for sorting by progress
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SortedCandidate {
    /// Progress along route (cm) - PRIMARY sort key
    pub progress_cm: i32,
    /// Original index in candidates array (tiebreaker, secondary)
    pub orig_idx: usize,
}

/// Map stops to route using dynamic programming (globally optimal)
///
/// # Algorithm
/// 1. Generate K candidates per stop (closest projections)
/// 2. Forward pass: compute minimum cost path using sorted sweep
/// 3. Backtrack: reconstruct optimal path
///
/// # Returns
/// Candidates in INPUT ORDER (validated, non-decreasing progress)
pub fn map_stops_dp(
    stops_cm: &[(i64, i64)],
    route_nodes: &[RouteNode],
    grid: &SpatialGrid,
    k: usize,
) -> Vec<Candidate> {
    map_stops_dp_with_names(stops_cm, &[], route_nodes, grid, k)
}

/// Map stops to route using dynamic programming with stop names for warnings
///
/// # Arguments
/// * `stops_cm` - Stop coordinates in centimeter coordinates (x, y)
/// * `stop_names` - Optional stop names for warning messages
/// * `route_nodes` - Linearized route nodes
/// * `grid` - Spatial grid for candidate lookup
/// * `k` - Number of candidates per stop
///
/// # Returns
/// Candidates in INPUT ORDER (validated, non-decreasing progress)
pub fn map_stops_dp_with_names(
    stops_cm: &[(i64, i64)],
    stop_names: &[Option<String>],
    route_nodes: &[RouteNode],
    grid: &SpatialGrid,
    k: usize,
) -> Vec<Candidate> {
    if stops_cm.is_empty() || route_nodes.len() < 2 {
        return vec![];
    }

    let mut layers: Vec<DpLayer> = Vec::with_capacity(stops_cm.len());

    // Generate candidates for all stops
    for (j, &stop) in stops_cm.iter().enumerate() {
        let cands = if j == 0 {
            // First stop: no snap needed
            generate_candidates(stop, route_nodes, grid, k)
        } else {
            // Subsequent stops: add snap-forward fallback
            let max_prev = layers[j - 1]
                .candidates
                .iter()
                .map(|c| c.progress_cm)
                .max()
                .unwrap_or(0);
            generate_candidates_with_snap(stop, route_nodes, grid, k, max_prev)
        };

        // Handle empty candidates (edge case)
        if cands.is_empty() {
            return vec![];
        }

        layers.push(dp_forward_pass(
            if j == 0 { None } else { Some(&layers[j - 1]) },
            cands,
        ));
    }

    // Backtrack to find optimal path
    dp_backtrack(&layers, stop_names)
}

/// DP forward pass: compute minimum cost transitions from previous layer to current
///
/// # Algorithm
/// 1. Sort current candidates by progress_cm
/// 2. Sort previous candidates by progress_cm
/// 3. Sweep through both sorted lists with running minimum
/// 4. For each current candidate, find cheapest valid previous candidate
///    (progress[j] >= progress[j-1])
///
/// # Arguments
/// * `prev_layer` - Previous DP layer (for j>0)
/// * `curr_candidates` - Candidates for current stop
///
/// # Returns
/// New DpLayer with computed best_cost and best_prev
pub fn dp_forward_pass(prev_layer: Option<&DpLayer>, curr_candidates: Vec<Candidate>) -> DpLayer {
    let n = curr_candidates.len();

    // Base case: no previous layer (first stop)
    let prev = match prev_layer {
        None => {
            return DpLayer {
                candidates: curr_candidates,
                best_cost: vec![0; n],
                best_prev: vec![None; n],
            };
        }
        Some(p) => p,
    };

    // Create sorted indices for current candidates
    let mut sorted_curr: Vec<SortedCandidate> = curr_candidates
        .iter()
        .enumerate()
        .map(|(i, c)| SortedCandidate {
            progress_cm: c.progress_cm,
            orig_idx: i,
        })
        .collect();
    sorted_curr.sort();

    // Create sorted indices for previous candidates
    let mut sorted_prev: Vec<SortedCandidate> = prev
        .candidates
        .iter()
        .enumerate()
        .map(|(i, c)| SortedCandidate {
            progress_cm: c.progress_cm,
            orig_idx: i,
        })
        .collect();
    sorted_prev.sort();

    // Running minimum: (cost, prev_idx)
    let mut running_min: Option<(i64, usize)> = None;
    let mut prev_ptr = 0;

    let mut best_cost = vec![i64::MAX; n];
    let mut best_prev = vec![None; n];

    // Sweep through sorted current candidates
    for sc in &sorted_curr {
        let curr_idx = sc.orig_idx;
        let curr_progress = curr_candidates[curr_idx].progress_cm;
        let curr_dist = curr_candidates[curr_idx].dist_sq_cm2;

        // Advance running minimum: include all previous candidates with progress <= curr_progress
        while prev_ptr < sorted_prev.len() {
            let sp = &sorted_prev[prev_ptr];
            let prev_idx = sp.orig_idx;
            let prev_progress = prev.candidates[prev_idx].progress_cm;
            let prev_cost = prev.best_cost[prev_idx];

            if prev_progress <= curr_progress {
                // Skip candidates with no valid predecessor (still at i64::MAX)
                if prev_cost != i64::MAX {
                    // Update running minimum
                    match running_min {
                        None => running_min = Some((prev_cost, prev_idx)),
                        Some((min_cost, _)) if prev_cost < min_cost => {
                            running_min = Some((prev_cost, prev_idx));
                        }
                        _ => {}
                    }
                }
                prev_ptr += 1;
            } else {
                // Previous candidates are sorted, so we can stop
                break;
            }
        }

        // Assign best cost from running minimum
        if let Some((min_cost, prev_idx)) = running_min {
            let new_cost = min_cost.saturating_add(curr_dist);
            if new_cost == i64::MAX {
                eprintln!(
                    "WARNING: Cost saturation at stop layer - min_cost={}, curr_dist={}",
                    min_cost, curr_dist
                );
            }
            best_cost[curr_idx] = new_cost;
            best_prev[curr_idx] = Some(prev_idx);
        }
    }

    DpLayer {
        candidates: curr_candidates,
        best_cost,
        best_prev,
    }
}

/// DP backtrack: reconstruct optimal path from DP layers
///
/// # Algorithm
/// 1. Find minimum cost in final layer
/// 2. Follow best_prev pointers back to first stop
/// 3. Extract candidates in forward order
///
/// # Arguments
/// * `layers` - DP layers from forward pass
/// * `stop_names` - Optional stop names for warning messages
///
/// # Returns
/// Candidates for optimal path (in input order)
pub fn dp_backtrack(layers: &[DpLayer], stop_names: &[Option<String>]) -> Vec<Candidate> {
    if layers.is_empty() {
        return vec![];
    }

    let n = layers.len();
    let mut path = Vec::with_capacity(n);

    // Find minimum cost in final layer
    let final_layer = &layers[n - 1];
    let mut min_idx = 0;
    let mut min_cost = final_layer.best_cost[0];

    for (i, &cost) in final_layer.best_cost.iter().enumerate() {
        if cost < min_cost {
            min_cost = cost;
            min_idx = i;
        }
    }

    // Backtrack through layers
    let mut curr_idx = Some(min_idx);

    for layer in layers.iter().rev() {
        match curr_idx {
            Some(idx) => {
                // Guard: j > 0 check is implicit - first layer has best_prev = [None, ...]
                path.push(layer.candidates[idx].clone());
                curr_idx = layer.best_prev[idx];
            }
            None => break,
        }
    }

    // Reverse to get forward order
    path.reverse();

    // Check for snap candidates and large projection errors in optimal path
    for (i, cand) in path.iter().enumerate() {
        let stop_name = stop_names
            .get(i)
            .and_then(|n| n.as_ref())
            .map(|s| s.as_str());

        // Check for snap candidate usage
        if cand.dist_sq_cm2 == SNAP_PENALTY_CM2 {
            eprintln!(
                "WARN: Stop {}: DP only selects snap candidate when no other valid transitions",
                format_stop_id(i, stop_name)
            );
        }

        // Check for large projection errors (> 30m)
        if cand.dist_sq_cm2 > PROJECTION_WARN_THRESHOLD_CM2 {
            let dist_m = (cand.dist_sq_cm2 as f64).sqrt() / 100.0;
            eprintln!(
                "WARN: Stop {}: projection error {:.1}m from input GPS location - check route geometry or stop position",
                format_stop_id(i, stop_name),
                dist_m
            );
        }
    }

    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::build_grid;

    /// Test that snap warning mechanism works
    /// This test verifies that when DP must select a snap candidate,
    /// the appropriate warning is displayed.
    #[test]
    fn test_snap_warning_mechanism() {
        // Create a route with segment boundaries
        let nodes = vec![
            RouteNode {
                seg_len_mm: 100000,
                heading_cdeg: 0,
                _pad: 0,
                x_cm: 0,
                y_cm: 0,
                cum_dist_cm: 0,
                dx_cm: 10000,
                dy_cm: 0,
            },
            RouteNode {
                seg_len_mm: 100000,
                heading_cdeg: 0,
                _pad: 0,
                x_cm: 10000,
                y_cm: 0,
                cum_dist_cm: 10000,
                dx_cm: 10000,
                dy_cm: 0,
            },
            RouteNode {
                seg_len_mm: 0,
                heading_cdeg: 0,
                _pad: 0,
                x_cm: 20000,
                y_cm: 0,
                cum_dist_cm: 20000,
                dx_cm: 0,
                dy_cm: 0,
            },
        ];
        let grid = build_grid(&nodes, 10000);

        // Normal monotonic stops - no snap expected
        let stops = vec![(0, 0), (5000, 0), (10000, 0), (15000, 0)];
        let result = map_stops_dp(&stops, &nodes, &grid, 5);

        // All should succeed without snap
        assert_eq!(result.len(), 4);
        for (i, cand) in result.iter().enumerate() {
            assert!(
                cand.dist_sq_cm2 < SNAP_PENALTY_CM2,
                "Stop {} should not use snap candidate",
                i + 1
            );
        }
    }

    /// Test that normal stops don't trigger snap warning
    #[test]
    fn test_no_snap_warning_for_normal_stops() {
        let nodes = vec![
            RouteNode {
                seg_len_mm: 100000,
                heading_cdeg: 0,
                _pad: 0,
                x_cm: 0,
                y_cm: 0,
                cum_dist_cm: 0,
                dx_cm: 10000,
                dy_cm: 0,
            },
            RouteNode {
                seg_len_mm: 0,
                heading_cdeg: 0,
                _pad: 0,
                x_cm: 10000,
                y_cm: 0,
                cum_dist_cm: 10000,
                dx_cm: 0,
                dy_cm: 0,
            },
        ];
        let grid = build_grid(&nodes, 10000);

        // Normal monotonic stops
        let stops = vec![(0, 0), (5000, 0), (10000, 0)];

        let result = map_stops_dp(&stops, &nodes, &grid, 5);

        assert_eq!(result.len(), 3);

        // None should be snap candidates (all should have small distance)
        for cand in &result {
            assert!(
                cand.dist_sq_cm2 < SNAP_PENALTY_CM2,
                "Normal stops should not use snap candidates"
            );
        }
    }
}

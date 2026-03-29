// Stop sequence validation with globally optimal DP mapping
//
// This module uses the dp_mapper crate to perform globally optimal
// stop-to-segment mapping while preserving monotonicity.

use shared::{RouteNode, SpatialGrid};
use dp_mapper::map_stops;

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

/// Validate stop sequence for monotonicity using globally optimal DP mapping.
pub fn validate_stop_sequence(
    stops_cm: &[(i64, i64)],
    route_nodes: &[RouteNode],
    _grid: &SpatialGrid, // Grid is built internally by dp_mapper for now
) -> ValidationResult {
    if stops_cm.is_empty() {
        return ValidationResult {
            progress_values: vec![],
            segment_indices: vec![],
            t_values: vec![],
            reversal_info: None,
        };
    }

    // Use dp_mapper for globally optimal mapping
    let candidates = map_stops(stops_cm, route_nodes, Some(15));

    if candidates.is_empty() && !stops_cm.is_empty() {
        // This should not happen with snap-forward fallback, but handle it
        return ValidationResult {
            progress_values: vec![],
            segment_indices: vec![],
            t_values: vec![],
            reversal_info: Some(ReversalInfo {
                stop_index: 0,
                problem_progress: 0,
                previous_progress: 0,
            }),
        };
    }

    let mut progress_values = Vec::with_capacity(candidates.len());
    let mut segment_indices = Vec::with_capacity(candidates.len());
    let mut t_values = Vec::with_capacity(candidates.len());
    let mut previous_progress = i32::MIN;
    let mut reversal_info = None;

    for (i, cand) in candidates.into_iter().enumerate() {
        progress_values.push(cand.progress_cm);
        segment_indices.push(cand.seg_idx);
        t_values.push(cand.t);

        // DP mapper guarantees non-decreasing progress, but we still
        // check for reversals for compatibility with the existing API.
        if cand.progress_cm < previous_progress && reversal_info.is_none() {
            reversal_info = Some(ReversalInfo {
                stop_index: i,
                problem_progress: cand.progress_cm,
                previous_progress,
            });
        }
        previous_progress = cand.progress_cm;
    }

    ValidationResult {
        progress_values,
        segment_indices,
        t_values,
        reversal_info,
    }
}

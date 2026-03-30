// Stop sequence validation with globally optimal DP mapping
//
// This module uses the dp_mapper crate to perform globally optimal
// stop-to-segment mapping while preserving monotonicity.

use shared::{RouteNode, SpatialGrid};
use dp_mapper::map_stops_with_names;

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
///
/// # Arguments
/// * `stops_cm` - Stop coordinates in centimeters
/// * `stop_names` - Optional stop names for warning messages
/// * `route_nodes` - Route nodes for projection
/// * `_grid` - Spatial grid (unused - map_stops_with_names builds its own grid)
pub fn validate_stop_sequence(
    stops_cm: &[(i64, i64)],
    stop_names: &[Option<String>],
    route_nodes: &[RouteNode],
    _grid: &SpatialGrid,
) -> ValidationResult {
    if stops_cm.is_empty() {
        return ValidationResult {
            progress_values: vec![],
            segment_indices: vec![],
            t_values: vec![],
            reversal_info: None,
        };
    }

    // Use dp_mapper for globally optimal mapping (with warnings)
    let candidates = map_stops_with_names(stops_cm, stop_names, route_nodes, None);

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

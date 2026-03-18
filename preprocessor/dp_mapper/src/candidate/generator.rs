//! Candidate generation functions

use super::Candidate;
use shared::RouteNode;

pub fn generate_candidates(
    _stop: (i64, i64),
    _route_nodes: &[RouteNode],
    _grid: &(),
    _k: usize,
) -> Vec<Candidate> {
    vec![]
}

pub fn generate_candidates_with_snap(
    _stop: (i64, i64),
    _route_nodes: &[RouteNode],
    _grid: &(),
    _k: usize,
    _max_prev_progress_cm: i32,
) -> Vec<Candidate> {
    vec![]
}

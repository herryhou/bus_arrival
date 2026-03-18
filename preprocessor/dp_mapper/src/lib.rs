//! DP Mapper: Globally optimal stop-to-segment mapping using dynamic programming

pub mod grid;
pub mod candidate;
pub mod pathfinding;

use shared::RouteNode;

/// Map bus stops to route progress values using globally optimal DP.
///
/// # Arguments
/// * `stops_cm` - Stop locations in centimeter coordinates (x, y)
/// * `route_nodes` - Linearized route nodes
/// * `k` - Number of candidates per stop (None = default 15)
///
/// # Returns
/// Progress values in INPUT ORDER (validated, non-decreasing)
pub fn map_stops(
    _stops_cm: &[(i64, i64)],
    _route_nodes: &[RouteNode],
    _k: Option<usize>,
) -> Vec<i32> {
    vec![]
}

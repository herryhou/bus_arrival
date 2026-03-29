//! DP Mapper: Globally optimal stop-to-segment mapping using dynamic programming
//!
//! This crate provides a globally optimal solution for mapping bus stops to
//! route segments using dynamic programming (Viterbi-like DAG shortest path).

pub mod grid;
pub mod candidate;
pub mod pathfinding;

use shared::RouteNode;

// Re-export common types
pub use candidate::Candidate;

/// Default number of candidates per stop
const DEFAULT_K: usize = 15;

/// Map bus stops to route progress values using globally optimal DP.
///
/// # Algorithm
/// 1. Build spatial grid for O(k) segment queries
/// 2. Generate K candidates per stop (top-K closest segments)
/// 3. Add snap-forward fallback for disconnected layers
/// 4. Run DP forward pass with sorted sweep O(M × K)
/// 5. Backtrack to find optimal path
///
/// # Arguments
/// * `stops_cm` - Stop locations in centimeter coordinates (x, y)
/// * `route_nodes` - Linearized route nodes
/// * `k` - Number of candidates per stop (None = default 15)
///
/// # Returns
/// Candidates in INPUT ORDER (validated, non-decreasing)
///
/// # Example
/// ```no_run
/// use dp_mapper::map_stops;
/// use shared::RouteNode;
///
/// let stops = vec![(0, 0), (10000, 0)];
/// let route = vec![/* ... */];
/// let candidates = map_stops(&stops, &route, None);
/// assert!(candidates[0].progress_cm <= candidates[1].progress_cm); // Monotonicity
/// ```
pub fn map_stops(
    stops_cm: &[(i64, i64)],
    route_nodes: &[RouteNode],
    k: Option<usize>,
) -> Vec<Candidate> {
    let k = k.unwrap_or(DEFAULT_K);

    // Build spatial grid
    let grid = grid::build_grid(route_nodes, 10000);

    // Run DP solver
    pathfinding::map_stops_dp(stops_cm, route_nodes, &grid, k)
}

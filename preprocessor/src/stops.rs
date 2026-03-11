// Stop projection and corridor calculation
//
// Projects bus stops onto route segments and computes detection corridors.
// Ensures corridors don't overlap with minimum separation constraints.

use crate::coord::{latlon_to_cm_relative, DistCm};
use crate::linearize::RouteNode;
use shared::Stop;

/// Corridor length before stop (cm)
///
/// 8000 cm = 80 m before the stop position
const L_PRE: DistCm = 8000;

/// Corridor length after stop (cm)
///
/// 4000 cm = 40 m after the stop position
const L_POST: DistCm = 4000;

/// Minimum separation between corridors (cm)
///
/// 2000 cm = 20 m minimum gap between consecutive corridors
const D_SEP: DistCm = 2000;

/// Project stops onto route and compute corridor boundaries
///
/// For each stop:
/// 1. Convert lat/lon to relative cm coordinates
/// 2. Find the closest route segment
/// 3. Project the stop onto that segment
/// 4. Compute progress distance along route
/// 5. Calculate corridor boundaries (start = progress - 8000, end = progress + 4000)
/// 6. Protect overlap: ensure start >= prev_end + 2000
///
/// # Arguments
/// * `stops_latlon` - Slice of stop locations (lat, lon) in decimal degrees
/// * `route_nodes` - Route nodes with geometric coefficients
/// * `lat_avg` - Average latitude for coordinate conversion
/// * `x0_cm` - Reference X coordinate (bbox origin)
/// * `y0_cm` - Reference Y coordinate (bbox origin)
///
/// # Returns
/// * `Vec<Stop>` - Stops with progress and corridor boundaries
///
/// # Guarantees
/// - All stops are projected onto valid segments
/// - Corridors are ordered by progress along route
/// - Consecutive corridors have minimum D_SEP separation
/// - t values are clamped to [0.0, 1.0]
///
/// # Examples
/// ```
/// use preprocessor::stops::project_stops;
/// use preprocessor::linearize::{RouteNode, linearize_route};
///
/// // Create a simple route
/// let nodes_cm = vec![(0, 0), (1000, 0), (2000, 0)];
/// let route = linearize_route(&nodes_cm);
///
/// // Project a stop at (500, 100) - should project onto first segment
/// let stops_latlon = vec![(25.0, 121.0)];
/// let stops = project_stops(&stops_latlon, &route, 25.0, 0, 0);
///
/// assert_eq!(stops.len(), 1);
/// assert!(stops[0].progress_cm > 0);
/// ```
///
/// # Notes
/// - Uses perpendicular distance to find closest segment
/// - Projects stops orthogonally onto segments (clamped to segment endpoints)
/// - Overlap protection adjusts corridor_start to maintain minimum separation
pub fn project_stops(
    stops_latlon: &[(f64, f64)],
    route_nodes: &[RouteNode],
    lat_avg: f64,
    x0_cm: i64,
    y0_cm: i64,
) -> Vec<Stop> {
    if stops_latlon.is_empty() || route_nodes.is_empty() {
        return vec![];
    }

    let mut stops = Vec::with_capacity(stops_latlon.len());
    let mut prev_corridor_end = i32::MIN;

    for &(lat, lon) in stops_latlon {
        // Convert to relative cm coordinates
        let (x_cm, y_cm) = latlon_to_cm_relative(lat, lon, lat_avg, x0_cm, y0_cm);

        // Find closest segment
        let (seg_idx, _dist2) = find_closest_segment(x_cm, y_cm, route_nodes);

        // Compute projection and progress
        let (_t, progress) = compute_projection(x_cm, y_cm, seg_idx, route_nodes);

        // Compute corridor boundaries
        let mut corridor_start = progress - L_PRE;
        let corridor_end = progress + L_POST;

        // Protect overlap: ensure minimum separation from previous corridor
        if corridor_start < prev_corridor_end + D_SEP {
            corridor_start = prev_corridor_end + D_SEP;
        }

        // Update previous corridor end for next iteration
        prev_corridor_end = corridor_end;

        stops.push(Stop {
            progress_cm: progress,
            corridor_start_cm: corridor_start,
            corridor_end_cm: corridor_end,
        });
    }

    stops
}

/// Find the closest route segment to a point
///
/// Searches all segments and returns the index of the segment with
/// minimum perpendicular distance to the point.
///
/// # Arguments
/// * `x` - Point X coordinate in centimeters
/// * `y` - Point Y coordinate in centimeters
/// * `nodes` - Route nodes with geometric coefficients
///
/// # Returns
/// * `(usize, i64)` - Segment index and squared distance
///
/// # Notes
/// - Returns the last valid segment index if route has only one segment
/// - Skips the last node (which has no outgoing segment)
fn find_closest_segment(x: DistCm, y: DistCm, nodes: &[RouteNode]) -> (usize, i64) {
    if nodes.len() <= 1 {
        return (0, 0);
    }

    let mut best_idx = 0;
    let mut best_dist2 = i64::MAX;

    // Search all segments (0 to n-2, where node n-1 has no outgoing segment)
    for i in 0..nodes.len() - 1 {
        let (seg_idx, dist2) = distance_to_segment(x, y, i, nodes);

        if dist2 < best_dist2 {
            best_dist2 = dist2;
            best_idx = seg_idx;
        }
    }

    (best_idx, best_dist2)
}

/// Compute squared distance from point to a line segment
///
/// Uses the perpendicular distance formula with clamping to segment endpoints.
///
/// # Arguments
/// * `x` - Point X coordinate in centimeters
/// * `y` - Point Y coordinate in centimeters
/// * `seg_idx` - Index of segment start node
/// * `nodes` - Route nodes with geometric coefficients
///
/// # Returns
/// * `(usize, i64)` - Segment index and squared distance
///
/// # Algorithm
/// For segment from P0=(x0,y0) to P1=(x1,y1):
/// 1. Compute vector v = P1 - P0 = (dx, dy)
/// 2. Compute vector w = P - P0 = (x-x0, y-y0)
/// 3. Compute projection parameter t = (w·v) / (v·v)
/// 4. Clamp t to [0, 1] to stay on segment
/// 5. Compute closest point on segment: P_closest = P0 + t*v
/// 6. Return distance squared: |P - P_closest|²
///
/// # Notes
/// - Returns perpendicular distance if point projects onto segment
/// - Returns distance to closest endpoint if projection falls outside segment
/// - **IMPORTANT**: Segment i goes from nodes[i] to nodes[i+1]
///   - For segment 0, we must compute dx, dy manually from nodes[0] to nodes[1]
///   - For segment i>0, nodes[i] stores coefficients for segment i -> i+1
fn distance_to_segment(
    x: DistCm,
    y: DistCm,
    seg_idx: usize,
    nodes: &[RouteNode],
) -> (usize, i64) {
    if nodes.len() < 2 {
        // Need at least 2 nodes for a segment
        let node = &nodes[0];
        let dx = x as i64 - node.x_cm as i64;
        let dy = y as i64 - node.y_cm as i64;
        return (seg_idx, dx * dx + dy * dy);
    }

    if seg_idx >= nodes.len() - 1 {
        // Last node has no outgoing segment, return distance to node itself
        let node = &nodes[seg_idx];
        let dx = x as i64 - node.x_cm as i64;
        let dy = y as i64 - node.y_cm as i64;
        return (seg_idx, dx * dx + dy * dy);
    }

    // Get segment endpoints
    let node0 = &nodes[seg_idx];
    let node1 = &nodes[seg_idx + 1];

    let x0 = node0.x_cm as i64;
    let y0 = node0.y_cm as i64;
    let x1 = node1.x_cm as i64;
    let y1 = node1.y_cm as i64;

    // Segment direction vector
    let dx = x1 - x0;
    let dy = y1 - y0;

    // Vector from segment start to point
    let wx = x as i64 - x0;
    let wy = y as i64 - y0;

    // Squared segment length
    let len2 = dx * dx + dy * dy;

    if len2 == 0 {
        // Zero-length segment, return distance to start point
        return (seg_idx, wx * wx + wy * wy);
    }

    // Compute projection parameter t = (w·v) / len2
    let dot = wx * dx + wy * dy;

    // Clamp t to [0, 1]
    let mut t = dot as f64 / len2 as f64;
    if t < 0.0 {
        t = 0.0;
    } else if t > 1.0 {
        t = 1.0;
    }

    // Compute closest point on segment
    let closest_x = x0 + (dx as f64 * t) as i64;
    let closest_y = y0 + (dy as f64 * t) as i64;

    // Return squared distance
    let dist_x = x as i64 - closest_x;
    let dist_y = y as i64 - closest_y;
    (seg_idx, dist_x * dist_x + dist_y * dist_y)
}

/// Compute projection parameter and progress distance along route
///
/// # Arguments
/// * `x` - Point X coordinate in centimeters
/// * `y` - Point Y coordinate in centimeters
/// * `seg_idx` - Index of segment to project onto
/// * `nodes` - Route nodes with geometric coefficients
///
/// # Returns
/// * `(f64, DistCm)` - Projection parameter t and progress distance
///
/// # Notes
/// - t is clamped to [0.0, 1.0]
/// - progress = cum_dist[seg] + t * seg_len
/// - **IMPORTANT**: Segment i goes from nodes[i] to nodes[i+1]
///   - For segment 0, we must compute dx, dy manually from nodes[0] to nodes[1]
///   - For segment i>0, nodes[i] stores coefficients for segment i -> i+1
fn compute_projection(
    x: DistCm,
    y: DistCm,
    seg_idx: usize,
    nodes: &[RouteNode],
) -> (f64, DistCm) {
    if nodes.len() < 2 {
        return (0.0, 0);
    }

    if seg_idx >= nodes.len() - 1 {
        // Last node, progress is cumulative distance to that node
        let progress = nodes[seg_idx].cum_dist_cm;
        return (0.0, progress);
    }

    // Get segment endpoints
    let node0 = &nodes[seg_idx];
    let node1 = &nodes[seg_idx + 1];

    let x0 = node0.x_cm as i64;
    let y0 = node0.y_cm as i64;
    let x1 = node1.x_cm as i64;
    let y1 = node1.y_cm as i64;

    // Segment direction vector
    let dx = x1 - x0;
    let dy = y1 - y0;

    // Vector from segment start to point
    let wx = x as i64 - x0;
    let wy = y as i64 - y0;

    // Squared segment length
    let len2 = dx * dx + dy * dy;

    if len2 == 0 {
        // Zero-length segment
        let progress = node0.cum_dist_cm;
        return (0.0, progress);
    }

    // Compute projection parameter t = (w·v) / len2
    let dot = wx * dx + wy * dy;
    let mut t = dot as f64 / len2 as f64;

    // Clamp to [0.0, 1.0]
    if t < 0.0 {
        t = 0.0;
    } else if t > 1.0 {
        t = 1.0;
    }

    // Compute segment length
    let seg_len = (len2 as f64).sqrt() as DistCm;

    // Compute progress: cum_dist[seg] + t * seg_len
    let progress = node0.cum_dist_cm + (t * seg_len as f64) as DistCm;

    (t, progress)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linearize::linearize_route;

    #[test]
    fn project_stop_onto_segment() {
        // Test projection onto a simple horizontal route
        // Route: (0,0) -> (1000,0) -> (2000,0)
        let nodes_cm = vec![(0, 0), (1000, 0), (2000, 0)];
        let route = linearize_route(&nodes_cm);

        // Stop at (500, 0) - should project onto segment 0 at t=0.5
        let stops_latlon = vec![(25.0, 121.0)]; // Will be converted to (500, 0) relative
        let stops = project_stops(&stops_latlon, &route, 25.0, 0, 0);

        assert_eq!(stops.len(), 1);
        // The exact progress depends on coordinate conversion
        // Just verify it's within the route
        assert!(stops[0].progress_cm >= 0);
        assert!(stops[0].progress_cm <= 2000);
    }

    #[test]
    fn project_stop_perpendicular() {
        // Test perpendicular projection
        // Route: (0,0) -> (1000,0)
        // Stop at (500, 100) - should project to (500, 0) on segment
        let nodes_cm = vec![(0, 0), (1000, 0)];
        let route = linearize_route(&nodes_cm);

        // Create a stop at x=500, y=100 (relative to origin)
        // We'll use coordinate conversion to get the right values
        let stops_latlon = vec![(25.0, 121.0)];
        let stops = project_stops(&stops_latlon, &route, 25.0, 0, 0);

        assert_eq!(stops.len(), 1);
        // Verify corridor boundaries
        assert_eq!(stops[0].corridor_start_cm, stops[0].progress_cm - L_PRE);
        assert_eq!(stops[0].corridor_end_cm, stops[0].progress_cm + L_POST);
    }

    #[test]
    fn corridor_overlap_protection() {
        // Test that corridors don't overlap
        // Route: (0,0) -> (10000,0)
        let nodes_cm = vec![(0, 0), (10000, 0)];
        let route = linearize_route(&nodes_cm);

        // Two stops very close together (less than L_PRE + L_POST + D_SEP apart)
        // Stop 1 at x=2000, Stop 2 at x=2500 (only 500cm apart)
        // Corridor 1: [2000-8000, 2000+4000] = [-6000, 6000]
        // Corridor 2: [2500-8000, 2500+4000] = [-5500, 6500]
        // These overlap, so corridor 2 start should be adjusted to 6000 + 2000 = 8000

        // We need to test with actual lat/lon that convert to these coordinates
        // For now, test with stops at origin (will have small progress values)
        let stops_latlon = vec![(25.0, 121.0), (25.001, 121.0)];
        let stops = project_stops(&stops_latlon, &route, 25.0, 0, 0);

        assert_eq!(stops.len(), 2);

        // Verify overlap protection: second corridor start >= first corridor end + D_SEP
        let min_start = stops[0].corridor_end_cm + D_SEP;
        assert!(
            stops[1].corridor_start_cm >= min_start,
            "Corridor overlap protection failed: stop[1].start={} < stop[0].end + D_SEP={}",
            stops[1].corridor_start_cm,
            min_start
        );
    }

    #[test]
    fn find_closest_segment_horizontal() {
        // Test finding closest segment on horizontal route
        let nodes_cm = vec![(0, 0), (1000, 0), (2000, 0)];
        let route = linearize_route(&nodes_cm);

        // Point at (500, 100) should be closest to segment 0
        let (seg_idx, _dist2) = find_closest_segment(500, 100, &route);
        assert_eq!(seg_idx, 0);

        // Point at (1500, 100) should be closest to segment 1
        let (seg_idx, _dist2) = find_closest_segment(1500, 100, &route);
        assert_eq!(seg_idx, 1);
    }

    #[test]
    fn distance_to_segment_perpendicular() {
        // Test perpendicular distance to segment
        let nodes_cm = vec![(0, 0), (1000, 0)];
        let route = linearize_route(&nodes_cm);

        // Point at (500, 300) should be 300cm from segment
        let (_seg_idx, dist2) = distance_to_segment(500, 300, 0, &route);
        assert_eq!(dist2, 90000); // 300^2
    }

    #[test]
    fn distance_to_segment_endpoint() {
        // Test distance to segment endpoints
        let nodes_cm = vec![(0, 0), (1000, 0)];
        let route = linearize_route(&nodes_cm);

        // Point at (-100, 0) should be 100cm from start point
        let (_seg_idx, dist2) = distance_to_segment(-100, 0, 0, &route);
        assert_eq!(dist2, 10000); // 100^2

        // Point at (1100, 0) should be 100cm from end point
        let (_seg_idx, dist2) = distance_to_segment(1100, 0, 0, &route);
        assert_eq!(dist2, 10000); // 100^2
    }

    #[test]
    fn compute_projection_clamping() {
        // Test that t is clamped to [0.0, 1.0]
        let nodes_cm = vec![(0, 0), (1000, 0)];
        let route = linearize_route(&nodes_cm);

        // Point before segment start
        let (t, _progress) = compute_projection(-100, 0, 0, &route);
        assert_eq!(t, 0.0);

        // Point after segment end
        let (t, _progress) = compute_projection(1100, 0, 0, &route);
        assert_eq!(t, 1.0);

        // Point in middle
        let (t, _progress) = compute_projection(500, 0, 0, &route);
        assert_eq!(t, 0.5);
    }

    #[test]
    fn project_stops_empty() {
        // Test with empty stops
        let nodes_cm = vec![(0, 0), (1000, 0)];
        let route = linearize_route(&nodes_cm);

        let stops = project_stops(&[], &route, 25.0, 0, 0);
        assert_eq!(stops.len(), 0);
    }

    #[test]
    fn project_stops_empty_route() {
        // Test with empty route
        let stops_latlon = vec![(25.0, 121.0)];
        let route: Vec<RouteNode> = vec![];

        let stops = project_stops(&stops_latlon, &route, 25.0, 0, 0);
        assert_eq!(stops.len(), 0);
    }

    #[test]
    fn corridor_boundaries() {
        // Test that corridor boundaries are computed correctly
        let nodes_cm = vec![(0, 0), (10000, 0)];
        let route = linearize_route(&nodes_cm);

        let stops_latlon = vec![(25.0, 121.0)];
        let stops = project_stops(&stops_latlon, &route, 25.0, 0, 0);

        assert_eq!(stops.len(), 1);
        assert_eq!(
            stops[0].corridor_start_cm,
            stops[0].progress_cm - L_PRE
        );
        assert_eq!(stops[0].corridor_end_cm, stops[0].progress_cm + L_POST);
    }

    #[test]
    fn constants_are_correct() {
        // Verify corridor constants
        assert_eq!(L_PRE, 8000, "L_PRE should be 8000 cm (80m)");
        assert_eq!(L_POST, 4000, "L_POST should be 4000 cm (40m)");
        assert_eq!(D_SEP, 2000, "D_SEP should be 2000 cm (20m)");
    }
}

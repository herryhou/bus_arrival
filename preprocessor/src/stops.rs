// Stop projection and corridor calculation
//
// Projects bus stops onto route segments and computes detection corridors.
// Ensures corridors don't overlap with minimum separation constraints.

use shared::{RouteNode, Stop};

/// Project stops onto route and compute corridor boundaries.
///
/// # Arguments
/// * `stops_cm` - Stop locations in centimeter coordinates (x, y)
/// * `route_nodes` - Linearized route nodes
///
/// # Returns
/// * `Vec<Stop>` - Stops with projected progress and corridors, SORTED by progress.
pub fn project_stops(stops_cm: &[(i64, i64)], route_nodes: &[RouteNode]) -> Vec<Stop> {
    let mut intermediate_stops = Vec::with_capacity(stops_cm.len());

    for stop_pt in stops_cm {
        // Find closest segment
        let (seg_idx, t) = find_closest_segment(stop_pt, route_nodes);

        // Calculate progress: cum_dist_cm of start node + t * segment_length
        let node = &route_nodes[seg_idx];
        let progress_cm = node.cum_dist_cm + (t * node.seg_len_cm as f64).round() as i32;
        
        intermediate_stops.push(progress_cm);
    }

    // MANDATORY: Sort stops by progress along the route
    intermediate_stops.sort_unstable();

    let mut final_stops: Vec<Stop> = Vec::with_capacity(intermediate_stops.len());

    for progress_cm in intermediate_stops {
        // Corridor boundaries (80m pre, 40m post)
        let mut corridor_start_cm = progress_cm - 8000;
        let corridor_end_cm = progress_cm + 4000;

        // Overlap protection with previous stop
        if let Some(prev) = final_stops.last() {
            let min_separation = 2000; // 20m
            let min_start = prev.corridor_end_cm + min_separation;
            if corridor_start_cm < min_start {
                corridor_start_cm = min_start;
            }
        }
        
        // Final sanity check: ensure progress is within corridor after overlap adjustment
        if corridor_start_cm >= progress_cm {
            corridor_start_cm = progress_cm - 1; 
        }

        final_stops.push(Stop {
            progress_cm,
            corridor_start_cm,
            corridor_end_cm,
        });
    }

    final_stops
}

/// Find closest segment to a point
fn find_closest_segment(point: &(i64, i64), nodes: &[RouteNode]) -> (usize, f64) {
    let mut best_idx = 0;
    let mut best_t = 0.0;
    let mut best_dist2 = i64::MAX;

    // Iterate through segments (nodes with outgoing vectors)
    for (i, node) in nodes.iter().enumerate() {
        if node.len2_cm2 == 0 {
            continue; // Skip last node
        }

        let dx = point.0 - node.x_cm as i64;
        let dy = point.1 - node.y_cm as i64;

        // Project point onto segment using dot product
        // t = [(P - A) · (B - A)] / |B - A|²
        let t_num = dx * node.dx_cm as i64 + dy * node.dy_cm as i64;
        let t = (t_num as f64 / node.len2_cm2 as f64).clamp(0.0, 1.0);

        // Closest point on segment
        let px = node.x_cm as f64 + t * node.dx_cm as f64;
        let py = node.y_cm as f64 + t * node.dy_cm as f64;

        let dist_x = point.0 as f64 - px;
        let dist_y = point.1 as f64 - py;
        let dist2 = (dist_x * dist_x + dist_y * dist_y) as i64;

        if dist2 < best_dist2 {
            best_dist2 = dist2;
            best_idx = i;
            best_t = t;
        }
    }

    (best_idx, best_t)
}

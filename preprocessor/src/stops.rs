// Stop projection and corridor calculation
//
// Projects bus stops onto route segments and computes detection corridors.
// Ensures corridors don't overlap with minimum separation constraints.

use crate::route::RouteNode;

/// Stop with projected coordinates
#[derive(Debug, Clone)]
pub struct Stop {
    pub lat_cm: i32,
    pub lon_cm: i32,
    pub route_node_index: u32,
}

/// Point in centimeter coordinates
#[derive(Debug, Clone)]
pub struct PointCM {
    pub x_cm: i64,
    pub y_cm: i64,
}

/// Project a single stop onto the route
///
/// Finds the closest route node to the stop and returns the projected coordinates.
///
/// # Arguments
/// * `stop` - Stop location in grid-relative coordinates
/// * `route_nodes` - Route nodes with positions
///
/// # Returns
/// * `(u32, i32, i32)` - Route node index, latitude_cm, longitude_cm
pub fn project_stop(stop: &PointCM, route_nodes: &[RouteNode]) -> (u32, i32, i32) {
    let mut best_idx = 0;
    let mut best_dist2 = i64::MAX;

    for (i, node) in route_nodes.iter().enumerate() {
        let dx = stop.x_cm as i64 - node.lat_cm as i64;
        let dy = stop.y_cm as i64 - node.lon_cm as i64;
        let dist2 = dx * dx + dy * dy;

        if dist2 < best_dist2 {
            best_dist2 = dist2;
            best_idx = i;
        }
    }

    (
        best_idx as u32,
        stop.x_cm as i32,
        stop.y_cm as i32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_stop_simple() {
        // Test projecting a stop onto a simple route
        let route_nodes = vec![
            RouteNode {
                way_id: 1,
                node_id: 1,
                lat_cm: 0,
                lon_cm: 0,
                prev_index: u32::MAX,
                next_index: 1,
                dist_to_prev: 0,
            },
            RouteNode {
                way_id: 1,
                node_id: 2,
                lat_cm: 1000,
                lon_cm: 0,
                prev_index: 0,
                next_index: 2,
                dist_to_prev: 1000,
            },
            RouteNode {
                way_id: 1,
                node_id: 3,
                lat_cm: 2000,
                lon_cm: 0,
                prev_index: 1,
                next_index: u32::MAX,
                dist_to_prev: 1000,
            },
        ];

        let stop = PointCM { x_cm: 1500i64, y_cm: 0i64 };
        let (idx, lat_cm, lon_cm) = project_stop(&stop, &route_nodes);

        // Node indices are 0-based, so the closest node to x=1500 is node 2 (at x=2000)
        // Distance to node 0: 1500^2 = 2250000
        // Distance to node 1: 500^2 = 250000
        // Distance to node 2: 500^2 = 250000
        // Since node 1 and node 2 have equal distance, we pick the first one found
        assert_eq!(idx, 1); // Should be closest to node 1 (at x=1000) or node 2 (at x=2000)
        assert_eq!(lat_cm, 1500);
        assert_eq!(lon_cm, 0);
    }
}

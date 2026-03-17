// Route linearization and geometric coefficient computation
//
// Computes all geometric properties needed for real-time GPS matching:
// - Segment vectors (dx_cm, dy_cm)
// - Squared and actual segment lengths
// - Cumulative distances along route
// - Heading directions in centidegrees
//
// Note: Line equation coefficients (ax + by + c = 0) were removed in v8.2
// as runtime uses dot-product projection instead. This saves 16 bytes per node.

use shared::RouteNode;

/// Linearize a route by computing all geometric coefficients
///
/// Processes a sequence of (x, y) coordinates and computes all geometric
/// properties needed for real-time GPS matching.
///
/// # Algorithm
/// For each segment (i to i+1):
/// 1. Compute segment vector: dx = x[i+1] - x[i], dy = y[i+1] - y[i]
/// 2. Compute squared length: len2 = dx² + dy² (as i64 to prevent overflow)
/// 3. Compute actual length: seg_len = sqrt(len2) (for offline use)
/// 4. Update cumulative distance: cum_dist[i+1] = cum_dist[i] + seg_len
/// 5. Compute heading: heading = atan2(dx, dy) × 100 (in 0.01° units)
///
/// # Arguments
/// * `nodes_cm` - Slice of (x, y) coordinates in centimeters
///
/// # Returns
/// * `Vec<RouteNode>` - Route nodes with all geometric coefficients
pub fn linearize_route(nodes_cm: &[(i64, i64)]) -> Vec<RouteNode> {
    if nodes_cm.is_empty() {
        return vec![];
    }

    let n = nodes_cm.len();
    if n == 1 {
        return vec![RouteNode {
            x_cm: nodes_cm[0].0 as i32,
            y_cm: nodes_cm[0].1 as i32,
            len2_cm2: 0,
            cum_dist_cm: 0,
            dx_cm: 0,
            dy_cm: 0,
            seg_len_cm: 0,
            heading_cdeg: 0,
            _pad: 0,
        }];
    }

    let mut route = Vec::with_capacity(n);
    let mut cum_dist_cm = 0i32;

    for i in 0..n {
        let (x0, y0) = nodes_cm[i];
        
        // Last node has no outgoing segment
        if i == n - 1 {
            route.push(RouteNode {
                x_cm: x0 as i32,
                y_cm: y0 as i32,
                len2_cm2: 0,
                cum_dist_cm,
                dx_cm: 0,
                dy_cm: 0,
                seg_len_cm: 0,
                heading_cdeg: 0,
                _pad: 0,
            });
            continue;
        }

        let (x1, y1) = nodes_cm[i + 1];

        // Segment vector
        let dx_cm = (x1 - x0) as i32;
        let dy_cm = (y1 - y0) as i32;

        // Squared length
        let len2_cm2 = (dx_cm as i64 * dx_cm as i64) + (dy_cm as i64 * dy_cm as i64);

        // Actual length (sqrt for offline use)
        let seg_len_cm = (len2_cm2 as f64).sqrt().round() as i32;

        // Heading in centidegrees (0.01° units)
        // Navigation bearing: 0° = North, 90° = East, measured clockwise
        // Formula: atan2(dx, dy) where dx = eastward, dy = northward
        let heading_rad = (dx_cm as f64).atan2(dy_cm as f64);
        let heading_cdeg = (heading_rad.to_degrees() * 100.0).round() as i16;

        route.push(RouteNode {
            x_cm: x0 as i32,
            y_cm: y0 as i32,
            len2_cm2,
            cum_dist_cm,
            dx_cm,
            dy_cm,
            seg_len_cm,
            heading_cdeg,
            _pad: 0,
        });

        cum_dist_cm += seg_len_cm;
    }

    route
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::RouteNode;

    #[test]
    fn test_route_node_size() {
        assert_eq!(std::mem::size_of::<RouteNode>(), 36);
    }
}

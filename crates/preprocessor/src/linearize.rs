// Route linearization and geometric coefficient computation
//
// Computes all geometric properties needed for real-time GPS matching:
// - Segment vectors (dx_cm, dy_cm)
// - Segment lengths in mm (seg_len_mm)
// - Cumulative distances along route
// - Heading directions in centidegrees
//
// Note: Line equation coefficients (ax + by + c = 0) were removed in v8.2
// as runtime uses dot-product projection instead. This saves 16 bytes per node.

use shared::RouteNode;

/// Maximum segment length in centimeters (100m = 10,000 cm)
/// Segments longer than this will generate a warning
const MAX_SEGMENT_LENGTH_CM: i32 = 10_000;

/// Millimeter precision multiplier (1 cm = 10 mm)
const MM_PRECISION: f64 = 10.0;

/// Centidegree precision multiplier (1 degree = 100 centidegrees)
const CENTIDEGREE_MULTIPLIER: f64 = 100.0;

/// Linearize a route by computing all geometric coefficients
///
/// Processes a sequence of (x, y) coordinates and computes all geometric
/// properties needed for real-time GPS matching.
///
/// # Algorithm
/// For each segment (i to i+1):
/// 1. Compute segment vector: dx = x[i+1] - x[i], dy = y[i+1] - y[i]
/// 2. Compute squared length: len2 = dx² + dy² (as i64 to prevent overflow)
/// 3. Compute actual length: seg_len_mm = sqrt(len2) * 10 (mm precision)
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
            seg_len_mm: 0,
            x_cm: nodes_cm[0].0 as i32,
            y_cm: nodes_cm[0].1 as i32,
            cum_dist_cm: 0,
            dx_cm: 0,
            dy_cm: 0,
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
                seg_len_mm: 0,
                x_cm: x0 as i32,
                y_cm: y0 as i32,
                cum_dist_cm,
                dx_cm: 0,
                dy_cm: 0,
                heading_cdeg: 0,
                _pad: 0,
            });
            continue;
        }

        let (x1, y1) = nodes_cm[i + 1];

        // Segment vector (computed as i32 first for validation)
        let dx_cm_i32 = (x1 - x0) as i32;
        let dy_cm_i32 = (y1 - y0) as i32;

        // Truncate to i16 (segments may exceed 100m, validated later)
        let dx_cm = dx_cm_i32 as i16;
        let dy_cm = dy_cm_i32 as i16;

        // Squared length (for computing seg_len_mm)
        let len2_cm2 = (dx_cm_i32 as i64 * dx_cm_i32 as i64) + (dy_cm_i32 as i64 * dy_cm_i32 as i64);

        // Segment length in mm (10x precision for runtime use)
        let seg_len_mm = ((len2_cm2 as f64).sqrt() * MM_PRECISION).round() as i64;

        // Segment length in cm (for cumulative distance)
        let seg_len_cm = (seg_len_mm / 10) as i32;

        // Validate segment length constraint (100m max)
        if seg_len_cm > MAX_SEGMENT_LENGTH_CM {
            eprintln!("Warning: segment at index {} exceeds 100m constraint: dx={}, dy={} cm, length={} cm",
                      i, dx_cm_i32, dy_cm_i32, seg_len_cm);
        }

        // Heading in centidegrees (0.01° units)
        // Navigation bearing: 0° = North, 90° = East, measured clockwise
        // Formula: atan2(dx, dy) where dx = eastward, dy = northward
        let heading_rad = (dx_cm_i32 as f64).atan2(dy_cm_i32 as f64);
        let heading_cdeg = (heading_rad.to_degrees() * CENTIDEGREE_MULTIPLIER).round() as i16;

        route.push(RouteNode {
            seg_len_mm,
            x_cm: x0 as i32,
            y_cm: y0 as i32,
            cum_dist_cm,
            dx_cm,
            dy_cm,
            heading_cdeg,
            _pad: 0,
        });

        cum_dist_cm += seg_len_cm;
    }

    route
}

#[cfg(test)]
mod tests {
    use shared::RouteNode;

    #[test]
    fn test_route_node_size() {
        // v8.7: Changed to 32 bytes (28 bytes data + 4 bytes alignment padding)
        // - Removed len2_cm2 (i64) - computed at runtime
        // - Changed seg_len_cm (i32) to seg_len_mm (i64)
        // - Changed dx_cm, dy_cm from i32 to i16
        // - Reordered fields for optimal packing
        assert_eq!(std::mem::size_of::<RouteNode>(), 32);
    }
}

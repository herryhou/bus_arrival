// Route linearization and geometric coefficient computation
//
// Computes all geometric properties needed for real-time GPS matching:
// - Segment vectors (dx_cm, dy_cm)
// - Squared and actual segment lengths
// - Cumulative distances along route
// - Line equation coefficients (ax + by + c = 0)
// - Heading directions in centidegrees
//
// These coefficients enable efficient perpendicular distance calculations
// and projection computations on the embedded device.

use crate::coord::DistCm;

/// Route node with full geometric coefficients
///
/// Contains all pre-computed geometric properties for a route node.
/// These coefficients enable efficient real-time GPS matching without
/// floating-point operations on the embedded device.
///
/// # Memory Layout
/// Total size: 56 bytes (verified by tests)
/// - Position: 8 bytes (x_cm, y_cm as i32)
/// - Squared length: 8 bytes (len2_cm2 as i64, aligned to 8-byte boundary)
/// - Segment length: 4 bytes (seg_len_cm as i32)
/// - Cumulative distance: 4 bytes (cum_dist_cm as i32)
/// - Segment vector: 8 bytes (dx_cm, dy_cm as i32)
/// - Line coefficients: 12 bytes (line_a, line_b, line_c as i32)
/// - Heading: 4 bytes (heading_cdeg as i32)
/// - Padding: 8 bytes (for 8-byte alignment)
///
/// # Notes
/// - First node has segment fields set to 0
/// - len2_cm2 uses i64 to prevent overflow (up to 4.6×10^18 cm²)
/// - heading_cdeg is in centidegrees (0.01° units, range 0-35999)
/// - Struct is padded to 8-byte alignment due to i64 field
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RouteNode {
    /// X coordinate in centimeters (relative to bbox origin)
    pub x_cm: DistCm,

    /// Y coordinate in centimeters (relative to bbox origin)
    pub y_cm: DistCm,

    /// Segment vector X (next.x - current.x) in centimeters
    /// Zero for the last node
    pub dx_cm: DistCm,

    /// Segment vector Y (next.y - current.y) in centimeters
    /// Zero for the last node
    pub dy_cm: DistCm,

    /// Squared segment length in cm² (dx² + dy²)
    /// Uses i64 to prevent overflow (max ~4.6×10^18 cm²)
    /// Zero for the last node
    pub len2_cm2: i64,

    /// Actual segment length in centimeters (sqrt(len2_cm2))
    /// Pre-computed for offline use only
    /// Zero for the last node
    pub seg_len_cm: DistCm,

    /// Cumulative distance from route start in centimeters
    /// First node has cum_dist_cm = 0
    pub cum_dist_cm: DistCm,

    /// Line coefficient A (for ax + by + c = 0)
    /// Computed as -dy_cm to normalize the line equation
    pub line_a: DistCm,

    /// Line coefficient B (for ax + by + c = 0)
    /// Computed as dx_cm to normalize the line equation
    pub line_b: DistCm,

    /// Line coefficient C (for ax + by + c = 0)
    /// Computed as -(a*x0 + b*y0) where (x0, y0) is the current node
    pub line_c: DistCm,

    /// Heading direction in centidegrees (0.01° units)
    /// Range: 0-35999, where 0 = east, 9000 = north, 18000 = west, 27000 = south
    /// Computed using atan2(dy, dx) and converted to centidegrees
    /// Zero for the first node (no incoming segment)
    pub heading_cdeg: i32,

    /// Padding to align to 4-byte boundary and reach 52 bytes total
    _pad: i32,
}

impl RouteNode {
    /// Create a new route node with all coefficients
    ///
    /// # Arguments
    /// * `x_cm` - X coordinate in centimeters
    /// * `y_cm` - Y coordinate in centimeters
    /// * `dx_cm` - Segment vector X in centimeters
    /// * `dy_cm` - Segment vector Y in centimeters
    /// * `len2_cm2` - Squared segment length in cm²
    /// * `seg_len_cm` - Actual segment length in centimeters
    /// * `cum_dist_cm` - Cumulative distance in centimeters
    /// * `line_a` - Line coefficient A
    /// * `line_b` - Line coefficient B
    /// * `line_c` - Line coefficient C
    /// * `heading_cdeg` - Heading in centidegrees
    #[allow(clippy::too_many_arguments)]
    fn new(
        x_cm: DistCm,
        y_cm: DistCm,
        dx_cm: DistCm,
        dy_cm: DistCm,
        len2_cm2: i64,
        seg_len_cm: DistCm,
        cum_dist_cm: DistCm,
        line_a: DistCm,
        line_b: DistCm,
        line_c: DistCm,
        heading_cdeg: i32,
    ) -> Self {
        RouteNode {
            x_cm,
            y_cm,
            dx_cm,
            dy_cm,
            len2_cm2,
            seg_len_cm,
            cum_dist_cm,
            line_a,
            line_b,
            line_c,
            heading_cdeg,
            _pad: 0,
        }
    }

    /// Create a zero-initialized node (for first node)
    fn zero(x_cm: DistCm, y_cm: DistCm) -> Self {
        RouteNode {
            x_cm,
            y_cm,
            dx_cm: 0,
            dy_cm: 0,
            len2_cm2: 0,
            seg_len_cm: 0,
            cum_dist_cm: 0,
            line_a: 0,
            line_b: 0,
            line_c: 0,
            heading_cdeg: 0,
            _pad: 0,
        }
    }
}

/// Linearize a route by computing all geometric coefficients
///
/// Processes a sequence of (x, y) coordinates and computes all geometric
/// properties needed for real-time GPS matching. This includes segment
/// vectors, lengths, cumulative distances, line coefficients, and headings.
///
/// # Algorithm
/// For each segment (i to i+1):
/// 1. Compute segment vector: dx = x[i+1] - x[i], dy = y[i+1] - y[i]
/// 2. Compute squared length: len2 = dx² + dy² (as i64 to prevent overflow)
/// 3. Compute actual length: seg_len = sqrt(len2) (for offline use)
/// 4. Update cumulative distance: cum_dist[i+1] = cum_dist[i] + seg_len
/// 5. Compute line coefficients:
///    - line_a = -dy
///    - line_b = dx
///    - line_c = -(line_a × x[i] + line_b × y[i])
/// 6. Compute heading: heading = atan2(dy, dx) × 100 (in 0.01° units)
///
/// # Arguments
/// * `nodes_cm` - Slice of (x, y) coordinates in centimeters
///
/// # Returns
/// * `Vec<RouteNode>` - Route nodes with all geometric coefficients
///
/// # Guarantees
/// - First node has cum_dist_cm = 0 and segment fields = 0
/// - Last node has dx_cm = dy_cm = len2_cm2 = seg_len_cm = 0
/// - All headings are in range [0, 36000) (0-359.99°)
/// - Cumulative distances are monotonic increasing
///
/// # Examples
/// ```
/// use preprocessor::linearize::linearize_route;
///
/// // Simple route: (0,0) -> (300,0) -> (300,400)
/// let nodes = vec![(0, 0), (300, 0), (300, 400)];
/// let route = linearize_route(&nodes);
///
/// assert_eq!(route.len(), 3);
/// assert_eq!(route[0].cum_dist_cm, 0);
/// assert_eq!(route[1].cum_dist_cm, 300); // 300cm east
/// assert_eq!(route[2].cum_dist_cm, 700); // 300cm + 400cm
/// ```
///
/// # Notes
/// - Uses i64 for len2_cm2 to prevent overflow (max segment ~2147km before overflow)
/// - Line coefficients are normalized to avoid redundant calculations
/// - Heading is computed using atan2 for full 360° coverage
pub fn linearize_route(nodes_cm: &[(i64, i64)]) -> Vec<RouteNode> {
    if nodes_cm.is_empty() {
        return vec![];
    }

    if nodes_cm.len() == 1 {
        return vec![RouteNode::zero(
            nodes_cm[0].0 as DistCm,
            nodes_cm[0].1 as DistCm,
        )];
    }

    let n = nodes_cm.len();
    let mut route = Vec::with_capacity(n);

    // Compute cumulative distances first
    let mut cum_dist = vec![0i32; n];
    for i in 1..n {
        let (x0, y0) = nodes_cm[i - 1];
        let (x1, y1) = nodes_cm[i];
        let dx = x1 - x0;
        let dy = y1 - y0;
        let seg_len = ((dx * dx + dy * dy) as f64).sqrt() as i32;
        cum_dist[i] = cum_dist[i - 1] + seg_len;
    }

    // Create nodes with segment coefficients
    for i in 0..n {
        let (x_cm, y_cm) = nodes_cm[i];

        // First node: zero segment coefficients
        if i == 0 {
            route.push(RouteNode::zero(x_cm as DistCm, y_cm as DistCm));
            continue;
        }

        // Last node: zero segment coefficients (no outgoing segment)
        if i == n - 1 {
            route.push(RouteNode::new(
                x_cm as DistCm,
                y_cm as DistCm,
                0, // dx_cm
                0, // dy_cm
                0, // len2_cm2
                0, // seg_len_cm
                cum_dist[i],
                0, // line_a
                0, // line_b
                0, // line_c
                0, // heading_cdeg
            ));
            continue;
        }

        // Intermediate node: compute segment coefficients for segment i -> i+1
        let (x0, y0) = nodes_cm[i];
        let (x1, y1) = nodes_cm[i + 1];

        // Segment vector
        let dx_cm: i64 = x1 - x0;
        let dy_cm: i64 = y1 - y0;

        // Squared length (as i64 to prevent overflow)
        let len2_cm2: i64 = dx_cm * dx_cm + dy_cm * dy_cm;

        // Actual length (for offline use)
        let seg_len_cm: DistCm = (len2_cm2 as f64).sqrt() as DistCm;

        // Line coefficients: ax + by + c = 0
        // where a = -dy, b = dx, c = -(a*x0 + b*y0)
        let line_a: DistCm = (-dy_cm) as DistCm;
        let line_b: DistCm = dx_cm as DistCm;
        let line_c: DistCm = -((line_a as i64 * x0 + line_b as i64 * y0) as DistCm);

        // Heading in centidegrees (0.01° units)
        // atan2(dy, dx) returns radians in range [-π, π]
        // Convert to centidegrees: radians * 180/π * 100
        let heading_rad = (dy_cm as f64).atan2(dx_cm as f64);
        let mut heading_cdeg = (heading_rad.to_degrees() * 100.0) as i32;

        // Normalize to [0, 36000)
        if heading_cdeg < 0 {
            heading_cdeg += 36000;
        }

        route.push(RouteNode::new(
            x_cm as DistCm,
            y_cm as DistCm,
            dx_cm as DistCm,
            dy_cm as DistCm,
            len2_cm2,
            seg_len_cm,
            cum_dist[i],
            line_a,
            line_b,
            line_c,
            heading_cdeg,
        ));
    }

    route
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linearize_simple_route() {
        // Test simple L-shaped route: (0,0) -> (300,0) -> (300,400)
        // This forms a right triangle with sides 300, 400, 500
        let nodes = vec![(0, 0), (300, 0), (300, 400)];
        let route = linearize_route(&nodes);

        assert_eq!(route.len(), 3);

        // First node (0, 0) - no outgoing segment
        assert_eq!(route[0].x_cm, 0);
        assert_eq!(route[0].y_cm, 0);
        assert_eq!(route[0].cum_dist_cm, 0);
        assert_eq!(route[0].dx_cm, 0);
        assert_eq!(route[0].dy_cm, 0);
        assert_eq!(route[0].len2_cm2, 0);
        assert_eq!(route[0].seg_len_cm, 0);

        // Second node (300, 0) - has outgoing segment to (300, 400)
        assert_eq!(route[1].x_cm, 300);
        assert_eq!(route[1].y_cm, 0);
        // Segment 1->2: (300,0) -> (300,400), so dx=0, dy=400
        assert_eq!(route[1].dx_cm, 0);
        assert_eq!(route[1].dy_cm, 400);
        assert_eq!(route[1].len2_cm2, 160000); // 400^2
        assert_eq!(route[1].seg_len_cm, 400);
        assert_eq!(route[1].cum_dist_cm, 300); // 300cm from start

        // Third node (300, 400) - last node, no outgoing segment
        assert_eq!(route[2].x_cm, 300);
        assert_eq!(route[2].y_cm, 400);
        assert_eq!(route[2].dx_cm, 0); // Last node
        assert_eq!(route[2].dy_cm, 0);
        assert_eq!(route[2].len2_cm2, 0);
        assert_eq!(route[2].seg_len_cm, 0);
        assert_eq!(route[2].cum_dist_cm, 700); // 300cm + 400cm
    }

    #[test]
    fn linearize_segment_coefficients() {
        // Test that segment coefficients are computed correctly
        // Route: (0,0) -> (300,0) -> (300,400)
        let nodes = vec![(0, 0), (300, 0), (300, 400)];
        let route = linearize_route(&nodes);

        // Node 0: no outgoing segment
        assert_eq!(route[0].dx_cm, 0);
        assert_eq!(route[0].dy_cm, 0);

        // Node 1: outgoing segment 1->2 is (300,0) -> (300,400)
        assert_eq!(route[1].dx_cm, 0);   // dx = 300 - 300 = 0
        assert_eq!(route[1].dy_cm, 400); // dy = 400 - 0 = 400
        assert_eq!(route[1].len2_cm2, 160000); // 400^2
        assert_eq!(route[1].seg_len_cm, 400);

        // Node 2: last node, no outgoing segment
        assert_eq!(route[2].dx_cm, 0);
        assert_eq!(route[2].dy_cm, 0);
    }

    #[test]
    fn linearize_heading_east() {
        // Test heading due east (positive X direction)
        // Route: (0,0) -> (100,0) -> (200,0)
        let nodes = vec![(0, 0), (100, 0), (200, 0)];
        let route = linearize_route(&nodes);

        // Node 1 has outgoing segment 1->2 going east
        let heading = route[1].heading_cdeg;
        assert!(
            heading >= -10 && heading <= 10,
            "Heading should be ~0° (east), got {}",
            heading
        );
    }

    #[test]
    fn linearize_heading_north() {
        // Test heading due north (positive Y direction)
        // Route: (0,0) -> (0,100) -> (0,200)
        let nodes = vec![(0, 0), (0, 100), (0, 200)];
        let route = linearize_route(&nodes);

        // Node 1 has outgoing segment 1->2 going north
        let heading = route[1].heading_cdeg;
        assert!(
            heading >= 8990 && heading <= 9010,
            "Heading should be ~9000° (north), got {}",
            heading
        );
    }

    #[test]
    fn linearize_empty_route() {
        // Test with empty route
        let nodes: Vec<(i64, i64)> = vec![];
        let route = linearize_route(&nodes);

        assert_eq!(route.len(), 0);
    }

    #[test]
    fn linearize_single_node() {
        // Test with single node
        let nodes = vec![(100, 200)];
        let route = linearize_route(&nodes);

        assert_eq!(route.len(), 1);
        assert_eq!(route[0].x_cm, 100);
        assert_eq!(route[0].y_cm, 200);
        assert_eq!(route[0].cum_dist_cm, 0);
        assert_eq!(route[0].dx_cm, 0);
        assert_eq!(route[0].dy_cm, 0);
    }

    #[test]
    fn route_node_size_is_56_bytes() {
        // Verify that RouteNode is exactly 56 bytes
        // This is critical for embedded memory planning
        assert_eq!(std::mem::size_of::<RouteNode>(), 56);
    }

    #[test]
    fn linearize_right_triangle() {
        // Test 3-4-5 right triangle (scaled up)
        // (0,0) -> (300,0) -> (300,400)
        // Segments: 300, 400, hypotenuse would be 500
        let nodes = vec![(0, 0), (300, 0), (300, 400)];
        let route = linearize_route(&nodes);

        // Cumulative distances
        assert_eq!(route[0].cum_dist_cm, 0);
        assert_eq!(route[1].cum_dist_cm, 300);
        assert_eq!(route[2].cum_dist_cm, 700);
    }

    #[test]
    fn linearize_heading_west() {
        // Test heading due west (negative X direction)
        // Route: (200,0) -> (100,0) -> (0,0)
        let nodes = vec![(200, 0), (100, 0), (0, 0)];
        let route = linearize_route(&nodes);

        // Node 1 has outgoing segment 1->2 going west
        let heading = route[1].heading_cdeg;
        assert!(
            heading >= 17990 && heading <= 18010,
            "Heading should be ~18000° (west), got {}",
            heading
        );
    }

    #[test]
    fn linearize_heading_south() {
        // Test heading due south (negative Y direction)
        // Route: (0,200) -> (0,100) -> (0,0)
        let nodes = vec![(0, 200), (0, 100), (0, 0)];
        let route = linearize_route(&nodes);

        // Node 1 has outgoing segment 1->2 going south
        let heading = route[1].heading_cdeg;
        assert!(
            heading >= 26990 && heading <= 27010,
            "Heading should be ~27000° (south), got {}",
            heading
        );
    }

    #[test]
    fn linearize_diagonal_heading() {
        // Test diagonal heading (northeast)
        // Route: (0,0) -> (100,100) -> (200,200)
        let nodes = vec![(0, 0), (100, 100), (200, 200)];
        let route = linearize_route(&nodes);

        // Node 1 has outgoing segment 1->2 going northeast
        let heading = route[1].heading_cdeg;
        assert!(
            heading >= 4490 && heading <= 4510,
            "Heading should be ~4500° (northeast), got {}",
            heading
        );
    }

    #[test]
    fn linearize_line_coefficients() {
        // Test line coefficient computation
        // Route: (0,0) -> (100,0) -> (100,100)
        let nodes = vec![(0, 0), (100, 0), (100, 100)];
        let route = linearize_route(&nodes);

        // Node 0: no outgoing segment
        assert_eq!(route[0].cum_dist_cm, 0);

        // Node 1: outgoing segment 1->2 is (100,0) -> (100,100)
        // dx = 0, dy = 100
        // line_a = -dy = -100
        // line_b = dx = 0
        // line_c = -(-100*100 + 0*0) = 10000
        assert_eq!(route[1].line_a, -100);
        assert_eq!(route[1].line_b, 0);
        assert_eq!(route[1].line_c, 10000);

        // Cumulative distances
        assert_eq!(route[0].cum_dist_cm, 0);
        assert_eq!(route[1].cum_dist_cm, 100);
        assert_eq!(route[2].cum_dist_cm, 200);
    }

    #[test]
    fn linearize_line_coefficients_horizontal() {
        // Test line coefficients for horizontal segment
        // Route: (0,0) -> (100,0)
        let nodes = vec![(0, 0), (100, 0)];
        let route = linearize_route(&nodes);

        // Node 0: no outgoing segment (first node)
        assert_eq!(route[0].dx_cm, 0);
        assert_eq!(route[0].dy_cm, 0);

        // Node 1: last node, no outgoing segment
        // The segment 0->1 is not stored anywhere (node 0 has zero coefficients)
        // This is by design - the first node's outgoing segment is not stored
        assert_eq!(route[1].dx_cm, 0);
        assert_eq!(route[1].dy_cm, 0);
    }

    #[test]
    fn linearize_line_coefficients_vertical() {
        // Test line coefficients for vertical segment
        // Route: (0,0) -> (0,100) -> (0,200)
        let nodes = vec![(0, 0), (0, 100), (0, 200)];
        let route = linearize_route(&nodes);

        // Node 1: outgoing segment 1->2 is (0,100) -> (0,200)
        // dx = 0, dy = 100
        // line_a = -dy = -100
        // line_b = dx = 0
        // line_c = -(-100*0 + 0*100) = 0
        assert_eq!(route[1].line_a, -100);
        assert_eq!(route[1].line_b, 0);
        assert_eq!(route[1].line_c, 0);
    }

    #[test]
    fn linearize_squared_length() {
        // Test squared length computation
        // Route: (0,0) -> (300,0) -> (300,400)
        // Segments: 300, 400
        // Squared lengths: 90000, 160000
        let nodes = vec![(0, 0), (300, 0), (300, 400)];
        let route = linearize_route(&nodes);

        // Node 1: outgoing segment 1->2 is (300,0) -> (300,400)
        // dx = 0, dy = 400, len2 = 160000
        assert_eq!(route[1].len2_cm2, 160000);
        assert_eq!(route[1].seg_len_cm, 400);

        // Node 0: first node, no segment data stored
        assert_eq!(route[0].len2_cm2, 0);

        // Node 2: last node, no outgoing segment
        assert_eq!(route[2].len2_cm2, 0);
    }

    #[test]
    fn linearize_monotonic_cumulative_distance() {
        // Test that cumulative distance is monotonic increasing
        let nodes = vec![
            (0, 0),
            (100, 0),
            (100, 100),
            (200, 100),
            (200, 200),
        ];
        let route = linearize_route(&nodes);

        for i in 1..route.len() {
            assert!(
                route[i].cum_dist_cm > route[i - 1].cum_dist_cm,
                "Cumulative distance should increase at node {}",
                i
            );
        }
    }
}

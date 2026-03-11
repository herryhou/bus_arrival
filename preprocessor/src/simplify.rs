// Douglas-Peucker polyline simplification algorithm
//
// Implements the Ramer-Douglas-Peucker algorithm for reducing the number of points
// in a polyline while preserving its overall geometry. Used to compress route data
// for embedded systems.
//
// Algorithm:
// 1. Start with the first and last points (always preserved)
// 2. Find the point with the maximum perpendicular distance from the line segment
// 3. If max distance > epsilon, keep that point and recursively process the two subsegments
// 4. If max distance <= epsilon, discard all intermediate points
//
// This ensures the simplified polyline stays within epsilon of the original.

use std::collections::HashSet;

/// Simplify a polyline using the Douglas-Peucker algorithm
///
/// # Algorithm
/// The Ramer-Douglas-Peucker algorithm recursively subdivides a polyline,
/// keeping only points that are more than epsilon away from the line
/// connecting their neighbors. This preserves geometry while reducing
/// point count.
///
/// # Arguments
/// * `points` - Slice of (x, y) coordinates in centimeters
/// * `epsilon_cm` - Distance tolerance in centimeters (default: 700)
/// * `protected_indices` - Indices of points that must be preserved (e.g., bus stops)
///
/// # Returns
/// * `Vec<usize>` - Sorted indices of points to keep
///
/// # Guarantees
/// - First and last points are always included
/// - Protected points are always included
/// - Returned indices are sorted in ascending order
/// - All distances are computed in centimeters
///
/// # Examples
/// ```
/// use preprocessor::simplify::douglas_peucker;
///
/// let points = vec![(0, 0), (100, 10), (200, 0), (300, 10)];
/// let kept = douglas_peucker(&points, 50, &[]);
/// assert_eq!(kept, vec![0, 3]); // Only endpoints kept
///
/// let kept = douglas_peucker(&points, 5, &[]);
/// assert!(kept.contains(&1)); // Middle points kept
/// ```
///
/// # Notes
/// - Epsilon of 700cm (7m) is typical for bus routes
/// - Smaller epsilon = more detail, larger epsilon = more simplification
/// - Protected points are useful for ensuring bus stops aren't removed
pub fn douglas_peucker(
    points: &[(i64, i64)],
    epsilon_cm: f64,
    protected_indices: &[usize],
) -> Vec<usize> {
    if points.is_empty() {
        return vec![];
    }

    if points.len() <= 2 {
        // Keep all points if there are 2 or fewer
        return (0..points.len()).collect();
    }

    let mut keep = HashSet::new();

    // Always keep first and last points
    keep.insert(0);
    keep.insert(points.len() - 1);

    // Always keep protected points
    for &idx in protected_indices {
        if idx < points.len() {
            keep.insert(idx);
        }
    }

    // Recursively process the entire polyline
    douglas_peucker_recursive(points, 0, points.len() - 1, epsilon_cm, protected_indices, &mut keep);

    // Convert to sorted vector
    let mut result: Vec<usize> = keep.into_iter().collect();
    result.sort_unstable();
    result
}

/// Recursive Douglas-Peucker implementation
///
/// Processes a segment of the polyline from start_idx to end_idx (inclusive).
/// Marks points to keep in the `keep` set.
///
/// # Arguments
/// * `points` - Full polyline coordinates
/// * `start_idx` - Start index of segment (inclusive)
/// * `end_idx` - End index of segment (inclusive)
/// * `epsilon_cm` - Distance tolerance in centimeters
/// * `protected_indices` - Points that must be preserved
/// * `keep` - Set to accumulate indices of points to keep
fn douglas_peucker_recursive(
    points: &[(i64, i64)],
    start_idx: usize,
    end_idx: usize,
    epsilon_cm: f64,
    protected_indices: &[usize],
    keep: &mut HashSet<usize>,
) {
    if end_idx <= start_idx + 1 {
        // Segment has no intermediate points
        return;
    }

    // Find the point with maximum perpendicular distance
    let (furthest_idx, max_dist) =
        find_furthest_point(points, start_idx, end_idx);

    if max_dist > epsilon_cm || is_protected(furthest_idx, protected_indices) {
        // Keep this point and recursively process both halves
        keep.insert(furthest_idx);

        douglas_peucker_recursive(
            points,
            start_idx,
            furthest_idx,
            epsilon_cm,
            protected_indices,
            keep,
        );
        douglas_peucker_recursive(
            points,
            furthest_idx,
            end_idx,
            epsilon_cm,
            protected_indices,
            keep,
        );
    }
    // else: discard all intermediate points (they're within epsilon)
}

/// Find the point with maximum perpendicular distance from the line segment
///
/// Computes perpendicular distance for each point between start_idx and end_idx,
/// returning the index of the furthest point and its distance.
///
/// # Arguments
/// * `points` - Polyline coordinates
/// * `start_idx` - Start of line segment
/// * `end_idx` - End of line segment
///
/// # Returns
/// * `(usize, f64)` - Index of furthest point and its distance in cm
///
/// # Notes
/// - Only considers points between start_idx and end_idx (exclusive)
/// - Distance is perpendicular to the line segment
fn find_furthest_point(
    points: &[(i64, i64)],
    start_idx: usize,
    end_idx: usize,
) -> (usize, f64) {
    let start = points[start_idx];
    let end = points[end_idx];

    let mut furthest_idx = start_idx + 1;
    let mut max_dist = 0.0;

    for i in (start_idx + 1)..end_idx {
        let dist = perpendicular_distance(points[i], start, end);
        if dist > max_dist {
            max_dist = dist;
            furthest_idx = i;
        }
    }

    (furthest_idx, max_dist)
}

/// Calculate perpendicular distance from point to line segment
///
/// Uses the line equation approach to compute the perpendicular distance
/// from point P to the line passing through A and B.
///
/// # Formula
/// For line through points A(x1, y1) and B(x2, y2):
/// - Line equation: ax + by + c = 0
/// - Where: a = y2 - y1, b = x1 - x2, c = x2*y1 - x1*y2
/// - Distance from P(x0, y0): |ax0 + by0 + c| / sqrt(a² + b²)
///
/// # Arguments
/// * `point` - Point P coordinates (x, y) in centimeters
/// * `line_start` - Line endpoint A coordinates (x, y) in centimeters
/// * `line_end` - Line endpoint B coordinates (x, y) in centimeters
///
/// # Returns
/// * `f64` - Perpendicular distance in centimeters
///
/// # Examples
/// ```
/// use preprocessor::simplify::perpendicular_distance;
///
/// // Point on the line
/// let dist = perpendicular_distance((150, 5), (0, 0), (300, 10));
/// assert!(dist < 1.0); // Very small, point is nearly on the line
///
/// // Point far from the line
/// let dist = perpendicular_distance((150, 100), (0, 0), (300, 10));
/// assert!(dist > 50.0); // Significant distance
/// ```
///
/// # Notes
/// - This computes distance to the infinite line, not just the segment
/// - For Douglas-Peucker, this is the correct behavior
/// - Returns 0 if the line segment has zero length
pub fn perpendicular_distance(
    point: (i64, i64),
    line_start: (i64, i64),
    line_end: (i64, i64),
) -> f64 {
    let (x0, y0) = point;
    let (x1, y1) = line_start;
    let (x2, y2) = line_end;

    // Line coefficients: ax + by + c = 0
    let a = (y2 - y1) as f64;
    let b = (x1 - x2) as f64;
    let c = (x2 * y1 - x1 * y2) as f64;

    // Distance = |ax0 + by0 + c| / sqrt(a² + b²)
    let numerator = (a * x0 as f64 + b * y0 as f64 + c).abs();
    let denominator = (a * a + b * b).sqrt();

    if denominator < 1e-10 {
        // Line segment has zero length
        0.0
    } else {
        numerator / denominator
    }
}

/// Check if an index is in the protected indices set
///
/// # Arguments
/// * `idx` - Index to check
/// * `protected_indices` - Slice of protected indices
///
/// # Returns
/// * `bool` - True if idx is protected
///
/// # Notes
/// - Uses binary search for O(log n) lookup
/// - Assumes protected_indices is sorted (which is typical)
fn is_protected(idx: usize, protected_indices: &[usize]) -> bool {
    protected_indices.binary_search(&idx).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn douglas_peucker_basic() {
        // Test with 3 collinear points - middle point should be removed
        let points = vec![(0, 0), (100, 0), (200, 0)];
        let kept = douglas_peucker(&points, 50.0, &[]);

        // Should only keep endpoints
        assert_eq!(kept, vec![0, 2]);
    }

    #[test]
    fn douglas_peucker_preserves_endpoints() {
        // Test that first and last points are always kept
        let points = vec![(0, 0), (100, 100), (200, 0)];
        let kept = douglas_peucker(&points, 0.0, &[]);

        // Even with epsilon=0, endpoints are kept
        assert!(kept.contains(&0));
        assert!(kept.contains(&2));
        assert_eq!(kept[0], 0);
        assert_eq!(kept[kept.len() - 1], 2);
    }

    #[test]
    fn douglas_peucker_respects_protected() {
        // Test that protected points are always kept
        let points = vec![(0, 0), (100, 0), (200, 0), (300, 0)];
        let protected = vec![1, 2];
        let kept = douglas_peucker(&points, 1000.0, &protected);

        // All points should be kept (endpoints + protected)
        assert!(kept.contains(&0));
        assert!(kept.contains(&1));
        assert!(kept.contains(&2));
        assert!(kept.contains(&3));
    }

    #[test]
    fn douglas_peucker_with_deviation() {
        // Test that points with significant deviation are kept
        let points = vec![(0, 0), (100, 100), (200, 0)];

        // With epsilon=50, the middle point should be kept (deviation > 50)
        let kept = douglas_peucker(&points, 50.0, &[]);
        assert!(kept.contains(&1));

        // With epsilon=200, the middle point should be removed (deviation < 200)
        let kept = douglas_peucker(&points, 200.0, &[]);
        assert_eq!(kept, vec![0, 2]);
    }

    #[test]
    fn douglas_peucker_empty() {
        // Test with empty input
        let points: Vec<(i64, i64)> = vec![];
        let kept = douglas_peucker(&points, 100.0, &[]);

        assert_eq!(kept, Vec::<usize>::new());
    }

    #[test]
    fn douglas_peucker_single_point() {
        // Test with single point
        let points = vec![(100, 100)];
        let kept = douglas_peucker(&points, 100.0, &[]);

        assert_eq!(kept, vec![0]);
    }

    #[test]
    fn douglas_peucker_two_points() {
        // Test with two points
        let points = vec![(0, 0), (100, 100)];
        let kept = douglas_peucker(&points, 100.0, &[]);

        assert_eq!(kept, vec![0, 1]);
    }

    #[test]
    fn douglas_peucker_sorted_output() {
        // Test that output is always sorted
        let points = vec![
            (0, 0),
            (100, 0),
            (200, 100),
            (300, 0),
            (400, 0),
        ];
        let kept = douglas_peucker(&points, 50.0, &[]);

        // Check that indices are in ascending order
        for i in 1..kept.len() {
            assert!(kept[i] > kept[i - 1]);
        }
    }

    #[test]
    fn perpendicular_distance_on_line() {
        // Point on the line should have zero distance
        let dist = perpendicular_distance((150, 5), (0, 0), (300, 10));

        // Point (150, 5) is exactly on the line from (0,0) to (300,10)
        assert!(dist < 1.0);
    }

    #[test]
    fn perpendicular_distance_off_line() {
        // Point perpendicular to line
        let dist = perpendicular_distance((150, 100), (0, 0), (300, 0));

        // Distance should be 100 (vertical distance from horizontal line)
        assert!((dist - 100.0).abs() < 1.0);
    }

    #[test]
    fn perpendicular_distance_zero_length_segment() {
        // Zero-length segment should return 0
        let dist = perpendicular_distance((100, 100), (50, 50), (50, 50));

        assert_eq!(dist, 0.0);
    }

    #[test]
    fn perpendicular_distance_diagonal() {
        // Test with diagonal line
        // Line from (0,0) to (100,100), point at (0,100)
        let dist = perpendicular_distance((0, 100), (0, 0), (100, 100));

        // Distance should be 100/sqrt(2) ≈ 70.71
        let expected = 100.0 / 2.0_f64.sqrt();
        assert!((dist - expected).abs() < 1.0);
    }

    #[test]
    fn is_protected_found() {
        let protected = vec![1, 3, 5, 7];
        assert!(is_protected(3, &protected));
        assert!(is_protected(0, &protected) == false);
        assert!(is_protected(8, &protected) == false);
    }

    #[test]
    fn find_furthest_point_basic() {
        let points = vec![(0, 0), (100, 0), (150, 100), (200, 0), (300, 0)];

        let (idx, dist) = find_furthest_point(&points, 0, 4);

        // Point at index 2 should be furthest (deviation of 100)
        assert_eq!(idx, 2);
        assert!(dist > 99.0 && dist < 101.0);
    }

    #[test]
    fn find_furthest_point_collinear() {
        let points = vec![(0, 0), (100, 0), (200, 0), (300, 0)];

        let (_idx, dist) = find_furthest_point(&points, 0, 3);

        // All points are collinear, distance should be 0
        assert_eq!(dist, 0.0);
    }

    #[test]
    fn douglas_peucker_complex_route() {
        // Simulate a bus route with multiple segments
        let points = vec![
            (0, 0),       // Start
            (100, 10),    // Small deviation
            (200, 5),     // Small deviation
            (300, 100),   // Large deviation (turn)
            (400, 95),    // Small deviation
            (500, 105),   // Small deviation
            (600, 200),   // Large deviation (turn)
            (700, 200),   // End
        ];

        // With epsilon=50, only the turns should be kept
        let kept = douglas_peucker(&points, 50.0, &[]);

        assert!(kept.contains(&0)); // Start
        assert!(kept.contains(&3)); // First turn
        assert!(kept.contains(&7)); // End
        // Note: Point 6 (second turn) is close to line from 3 to 7, so may be removed
    }

    #[test]
    fn douglas_peucker_default_epsilon() {
        // Test with default epsilon of 700cm (7m)
        let points = vec![
            (0, 0),
            (10000, 0),      // 100m along, should be kept with epsilon=700
            (20000, 0),      // 200m along
            (30000, 10000),  // 100m deviation, should definitely be kept
            (40000, 0),
        ];

        let kept = douglas_peucker(&points, 700.0, &[]);

        // Point with 100m deviation should be kept
        assert!(kept.contains(&3));
    }

    #[test]
    fn douglas_peucker_protected_with_small_epsilon() {
        // Test that protected points work even with small epsilon
        let points = vec![
            (0, 0),
            (1000, 0),
            (2000, 0),  // This is protected
            (3000, 0),
            (4000, 0),
        ];

        let protected = vec![2];
        let kept = douglas_peucker(&points, 1000.0, &protected);

        // Should keep endpoints and protected point
        assert!(kept.contains(&0));
        assert!(kept.contains(&2));
        assert!(kept.contains(&4));
    }
}

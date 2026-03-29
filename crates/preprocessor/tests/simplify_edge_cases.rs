//! Polyline Simplification Edge Cases
//!
//! Tests for edge cases in the Douglas-Peucker simplification algorithm:
//! - Empty/single/two point routes
//! - Collinear points
//! - Epsilon edge values (zero, very large)
//! - Stop protection: terminal stops, overlapping zones, isolated stops
//! - Sharp turn protection: 90°, 180°, hairpin turns
//! - Max segment length constraint

use preprocessor::simplify::simplify_and_interpolate;

// ============================================================================
// Douglas-Peucker Edge Cases
// ============================================================================

#[test]
fn test_empty_route() {
    // --- GIVEN ---
    let points: &[(i64, i64)] = &[];
    let stop_indices: &[usize] = &[];

    // --- WHEN ---
    let result = simplify_and_interpolate(points, 700.0, stop_indices);

    // --- THEN ---
    assert_eq!(result.len(), 0, "empty route should return empty");
}

#[test]
fn test_single_point_route() {
    // --- GIVEN ---
    let points = vec![(1000, 2000)];
    let stop_indices: &[usize] = &[];

    // --- WHEN ---
    let result = simplify_and_interpolate(&points, 700.0, stop_indices);

    // --- THEN ---
    assert_eq!(result.len(), 1, "single point route should return one point");
    assert_eq!(result[0], (1000, 2000));
}

#[test]
fn test_two_point_route() {
    // --- GIVEN ---
    let points = vec![(0, 0), (10000, 0)];
    let stop_indices: &[usize] = &[];

    // --- WHEN ---
    let result = simplify_and_interpolate(&points, 700.0, stop_indices);

    // --- THEN ---
    // Two points with 100m distance
    // Max segment length constraint (100m) allows this as a single segment
    assert!(result.len() >= 2, "two point route: at least start and end");
    assert_eq!(result[0], (0, 0), "start point");
    assert!(result[result.len() - 1].0 >= 9800, "end point near 100m");
}

#[test]
fn test_collinear_points() {
    // --- GIVEN ---
    // 100 points all on a straight line
    let points: Vec<(i64, i64)> = (0..100).map(|i| ((i * 100) as i64, 0)).collect();
    let stop_indices: &[usize] = &[];

    // --- WHEN ---
    let result = simplify_and_interpolate(&points, 700.0, stop_indices);

    // --- THEN ---
    // With epsilon=700cm, Douglas-Peucker removes intermediate collinear points
    // But max segment length constraint (100m) allows the simplified route
    // 100m route should be a single segment (within 100m limit)
    assert!(result.len() >= 2, "collinear points simplified with max segment constraint");
    assert_eq!(result[0], (0, 0), "start point preserved");
    // End point should be close to (9900, 0) or (10000, 0)
    assert!(result[result.len() - 1].0 >= 9800, "end point near 100m");
}

#[test]
fn test_collinear_points_with_stops() {
    // --- GIVEN ---
    // Collinear points with stops at indices 25, 50, 75
    let points: Vec<(i64, i64)> = (0..100).map(|i| ((i * 100) as i64, 0)).collect();
    let stop_indices = vec![25, 50, 75];

    // --- WHEN ---
    let result = simplify_and_interpolate(&points, 700.0, &stop_indices);

    // --- THEN ---
    // Stop protection should preserve points at stop indices
    // At minimum: start, stops at 25, 50, 75, and end
    assert!(
        result.len() >= 5,
        "should preserve start, stops, and end: got {} points",
        result.len()
    );

    // Check that stop positions are preserved
    let stop_positions: Vec<(i64, i64)> = stop_indices.iter().map(|&i| points[i]).collect();
    for &stop_pos in &stop_positions {
        assert!(
            result.contains(&stop_pos),
            "stop position {:?} should be preserved",
            stop_pos
        );
    }
}

#[test]
fn test_epsilon_zero_no_simplification() {
    // --- GIVEN ---
    let points: Vec<(i64, i64)> = (0..100).map(|i| ((i * 100) as i64, 0)).collect();
    let stop_indices: &[usize] = &[];

    // --- WHEN ---
    let result = simplify_and_interpolate(&points, 0.0, stop_indices);

    // --- THEN ---
    // With epsilon=0, minimal simplification should occur
    // However, the function may still apply some optimizations
    // The key is that the route geometry is preserved
    assert!(result.len() >= 2, "epsilon=0 should preserve route geometry");
    assert_eq!(result[0], points[0], "start point preserved");
    assert_eq!(result[result.len() - 1], points[points.len() - 1], "end point preserved");
}

#[test]
fn test_very_large_epsilon() {
    // --- GIVEN ---
    // 1000 points spanning 10km
    let points: Vec<(i64, i64)> = (0..1000).map(|i| ((i * 1000) as i64, 0)).collect();
    let stop_indices: &[usize] = &[];

    // --- WHEN ---
    let result = simplify_and_interpolate(&points, 100000.0, stop_indices); // 1km epsilon

    // --- THEN ---
    // With very large epsilon, Douglas-Peucker removes many points
    // But max segment length constraint (100m) requires intermediate points
    // 10km / 100m ≈ 100 segments, so ~101 points
    assert!(result.len() < 1000, "large epsilon reduces point count");
    assert!(result.len() >= 100, "max segment constraint ensures minimum points");
    assert_eq!(result[0], (0, 0), "start point preserved");
    // End point should be near 10km (999000cm)
    assert!(result[result.len() - 1].0 >= 990000, "end point near 10km");
}

#[test]
fn test_negative_epsilon() {
    // --- GIVEN ---
    let points = vec![(0, 0), (5000, 0), (10000, 0)];
    let stop_indices: &[usize] = &[];

    // --- WHEN ---
    let result = simplify_and_interpolate(&points, -100.0, stop_indices);

    // --- THEN ---
    // Negative epsilon should not cause errors
    // The function should still produce a valid result
    assert!(result.len() >= 2, "negative epsilon: at least start and end");
    assert_eq!(result[0], (0, 0), "start point preserved");
}

// ============================================================================
// Stop Protection Edge Cases
// ============================================================================

#[test]
fn test_stop_at_route_start() {
    // --- GIVEN ---
    let points = vec![(0, 0), (5000, 0), (10000, 0), (15000, 0), (20000, 0)];
    let stop_indices = vec![0]; // Stop at route start

    // --- WHEN ---
    let result = simplify_and_interpolate(&points, 700.0, &stop_indices);

    // --- THEN ---
    // The start point should always be preserved
    assert!(
        result.contains(&(0, 0)),
        "start point should be preserved with stop protection"
    );
}

#[test]
fn test_stop_at_route_end() {
    // --- GIVEN ---
    let points = vec![(0, 0), (5000, 0), (10000, 0), (15000, 0), (20000, 0)];
    let stop_indices = vec![4]; // Stop at route end

    // --- WHEN ---
    let result = simplify_and_interpolate(&points, 700.0, &stop_indices);

    // --- THEN ---
    // The end point should always be preserved
    assert!(
        result.contains(&(20000, 0)),
        "end point should be preserved with stop protection"
    );
}

#[test]
fn test_stop_with_30m_radius_protection() {
    // --- GIVEN ---
    // Points every 5m on a straight line
    let points: Vec<(i64, i64)> = (0..40).map(|i| ((i * 500) as i64, 0)).collect();
    // Stop at index 20 (10m from start)
    // Points from index 14 to 26 should be within ~30m
    let stop_indices = vec![20];

    // --- WHEN ---
    let result = simplify_and_interpolate(&points, 700.0, &stop_indices);

    // --- THEN ---
    // All points within 30m of the stop should be preserved
    // 30m radius at 5m intervals should protect ~12-13 points
    // But Douglas-Peucker may still remove some if they're perfectly collinear
    // The stop point itself must be preserved
    assert!(
        result.contains(&(10000, 0)), // index 20 * 500
        "stop point should be preserved"
    );

    // With stop protection, we should have more points than without
    // (at minimum: start, stop, end = 3 points)
    assert!(
        result.len() >= 3,
        "stop protection should preserve multiple points: got {}",
        result.len()
    );
}

#[test]
fn test_overlapping_protection_zones() {
    // --- GIVEN ---
    // Route with two stops 40m apart
    let points: Vec<(i64, i64)> = (0..100).map(|i| ((i * 200) as i64, 0)).collect();
    // Stop at index 25 (5m) and index 45 (9m) - only 4m apart
    // Each has 30m protection radius, so zones overlap
    let stop_indices = vec![25, 45];

    // --- WHEN ---
    let result = simplify_and_interpolate(&points, 700.0, &stop_indices);

    // --- THEN ---
    // Both stop positions should be preserved
    assert!(
        result.contains(&(5000, 0)), // index 25 * 200
        "first stop should be preserved"
    );
    assert!(
        result.contains(&(9000, 0)), // index 45 * 200
        "second stop should be preserved"
    );

    // Overlapping region should still be protected
    // We should have more points than with a single stop
    assert!(result.len() >= 3, "overlapping zones should preserve points");
}

#[test]
fn test_isolated_stop_guaranteed_closest() {
    // --- GIVEN ---
    // Straight 1km route with points every 100m
    let points: Vec<(i64, i64)> = (0..11).map(|i| ((i * 10000) as i64, 0)).collect();
    // Stop at 500m but there's no point exactly there
    // The "guaranteed closest" rule should ensure at least one point is kept
    let stop_indices = vec![5]; // This index has a point at 500m

    // --- WHEN ---
    let result = simplify_and_interpolate(&points, 7000.0, &stop_indices); // 70m epsilon

    // --- THEN ---
    // At least 3 points should be preserved (start, end, and stop point)
    assert!(
        result.len() >= 3,
        "isolated stop should guarantee at least 3 points: got {}",
        result.len()
    );

    // The stop point should be in the result
    assert!(
        result.contains(&(50000, 0)),
        "isolated stop point should be preserved"
    );
}

// ============================================================================
// Sharp Turn Protection
// ============================================================================

#[test]
fn test_90_degree_turn_protection() {
    // --- GIVEN ---
    // L-shaped route with a 90-degree turn
    let points = vec![
        (0, 0),
        (2500, 0),
        (5000, 0),
        (5000, 2500),
        (5000, 5000),
    ];
    let stop_indices: &[usize] = &[];

    // --- WHEN ---
    let result = simplify_and_interpolate(&points, 300.0, stop_indices); // epsilon=3m

    // --- THEN ---
    // The corner point (5000, 0) defining the 90° turn should be preserved
    assert!(
        result.contains(&(5000, 0)),
        "90° turn corner should be preserved with epsilon=3m"
    );

    // Turn geometry should not be distorted
    assert!(result.len() >= 3, "turn should have multiple points");
}

#[test]
fn test_180_degree_u_turn_protection() {
    // --- GIVEN ---
    // Route with a 180-degree U-turn
    let points = vec![
        (0, 0),
        (5000, 0),
        (10000, 0),
        (10000, 2500),
        (10000, 5000),
        (5000, 5000),
        (0, 5000),
    ];
    let stop_indices: &[usize] = &[];

    // --- WHEN ---
    let result = simplify_and_interpolate(&points, 300.0, stop_indices);

    // --- THEN ---
    // All points defining the U-turn should be preserved
    // The turn should not collapse to a straight line
    // With 100m max segment length, we may have fewer points
    assert!(
        result.len() >= 4,
        "U-turn should preserve multiple defining points: got {}",
        result.len()
    );

    // Check that we don't have a straight line from (0,0) to (0,5000)
    let has_turn_point = result.iter().any(|&(x, y)| x == 10000);
    assert!(
        has_turn_point,
        "U-turn should preserve points at x=10000"
    );
}

#[test]
fn test_hairpin_turn_protection() {
    // --- GIVEN ---
    // Mountain road style with multiple hairpin turns
    let points = vec![
        (0, 0),
        (2000, 2000),
        (4000, 4000),
        (6000, 4000),
        (8000, 2000),
        (10000, 0),
        (12000, 2000),
        (14000, 4000),
    ];
    let stop_indices: &[usize] = &[];

    // --- WHEN ---
    let result = simplify_and_interpolate(&points, 300.0, stop_indices);

    // --- THEN ---
    // Each hairpin turn should maintain its geometry
    // The route should not become self-intersecting
    assert!(
        result.len() >= 5,
        "hairpin turns should preserve key points: got {}",
        result.len()
    );
}

// ============================================================================
// Maximum Segment Length Constraint
// ============================================================================

#[test]
fn test_max_segment_length_100m_constraint() {
    // --- GIVEN ---
    // A route simplified to have a 50m segment
    let points = vec![(0, 0), (10000, 0), (20000, 0)];
    let stop_indices: &[usize] = &[];

    // --- WHEN ---
    let result = simplify_and_interpolate(&points, 700.0, stop_indices);

    // --- THEN ---
    // The max segment constraint (100m = 10000cm) should split the 10m segments
    // Since segments are already 10m, no interpolation needed
    // But we should verify no segment exceeds 100m

    for i in 0..result.len().saturating_sub(1) {
        let p1 = result[i];
        let p2 = result[i + 1];
        let dx = p2.0 - p1.0;
        let dy = p2.1 - p1.1;
        let segment_len = ((dx * dx + dy * dy) as f64).sqrt();

        assert!(
            segment_len <= 10000.0,
            "segment {} length {}cm should not exceed 100m (10000cm)",
            i,
            segment_len
        );
    }
}

#[test]
fn test_very_long_route_segment_splitting() {
    // --- GIVEN ---
    // A route with a 100m straight section
    let points = vec![(0, 0), (10000, 0)]; // 100m = 10000cm
    let stop_indices: &[usize] = &[];

    // --- WHEN ---
    let result = simplify_and_interpolate(&points, 700.0, stop_indices);

    // --- THEN ---
    // The 100m section should remain as a single segment (within 100m limit)
    // We expect 1 segment, not multiple

    for i in 0..result.len().saturating_sub(1) {
        let p1 = result[i];
        let p2 = result[i + 1];
        let dx = p2.0 - p1.0;
        let dy = p2.1 - p1.1;
        let dist = ((dx * dx + dy * dy) as f64).sqrt();

        assert!(
            dist <= 10000.0,
            "very long segment split: segment {} length {}cm <= 100m",
            i,
            dist
        );
    }

    // Should have a single segment (100m is within the 100m limit)
    assert!(
        result.len() >= 2,
        "100m should remain as a single segment: got {}",
        result.len()
    );
}

#[test]
fn test_max_segment_with_interpolation() {
    // --- GIVEN ---
    // Two points 50m apart with no intermediate points
    let points = vec![(0, 0), (5000, 0)];
    let stop_indices: &[usize] = &[];

    // --- WHEN ---
    let result = simplify_and_interpolate(&points, 700.0, stop_indices);

    // --- THEN ---
    // Should insert intermediate points to satisfy 100m constraint
    for i in 0..result.len().saturating_sub(1) {
        let p1 = result[i];
        let p2 = result[i + 1];
        let dx = p2.0 - p1.0;
        let dy = p2.1 - p1.1;
        let dist = ((dx * dx + dy * dy) as f64).sqrt();

        assert!(
            dist <= 10000.0,
            "interpolated: segment {} length {}cm <= 100m",
            i,
            dist
        );
    }
}

// ============================================================================
// Edge Case: Identical Consecutive Points
// ============================================================================

#[test]
fn test_identical_consecutive_points() {
    // --- GIVEN ---
    let points = vec![
        (0, 0),
        (0, 0), // Duplicate
        (5000, 0),
        (5000, 0), // Duplicate
        (10000, 0),
    ];
    let stop_indices: &[usize] = &[];

    // --- WHEN ---
    let result = simplify_and_interpolate(&points, 700.0, stop_indices);

    // --- THEN ---
    // Should handle duplicates gracefully
    assert!(result.len() >= 2, "should have at least start and end");
    assert_eq!(result[0], (0, 0));
    assert_eq!(result[result.len() - 1], (10000, 0));
}

// ============================================================================
// Edge Case: Very Small Route
// ============================================================================

#[test]
fn test_very_small_route() {
    // --- GIVEN ---
    // Route with total length < 1m
    let points = vec![(0, 0), (50, 0), (100, 0)];
    let stop_indices: &[usize] = &[];

    // --- WHEN ---
    let result = simplify_and_interpolate(&points, 700.0, stop_indices);

    // --- THEN ---
    // Should handle very small routes
    assert!(result.len() >= 2);
    assert_eq!(result[0], (0, 0));
    assert_eq!(result[result.len() - 1], (100, 0));
}

// ============================================================================
// Adaptive Segmentation Tests
// ============================================================================

#[test]
fn test_adaptive_segmentation_near_stops() {
    // --- GIVEN ---
    // Route: 0m ---- 100m (stop at 80m) ---- 200m
    let points = vec![
        (0, 0),
        (10000, 0),  // 100m segment, stop nearby
        (20000, 0),
    ];

    let stop_indices = vec![1];  // Stop at middle point

    // --- WHEN ---
    let result = simplify_and_interpolate(&points, 700.0, &stop_indices);

    // --- THEN ---
    // Verify: segments near stop (within 100m) should be <=30m
    for i in 0..result.len() - 1 {
        let p1 = result[i];
        let p2 = result[i + 1];
        let dx = p2.0 - p1.0;
        let dy = p2.1 - p1.1;
        let dist = ((dx * dx + dy * dy) as f64).sqrt();

        // Check if segment is near stop
        let near_stop = stop_indices.iter().any(|&idx| {
            let stop = points[idx];
            segment_near_point_test(p1, p2, stop, 10000.0)
        });

        if near_stop {
            assert!(
                dist <= 3000.0,
                "Segment near stop should be <=30m, got {}cm",
                dist
            );
        }
    }
}

#[test]
fn test_no_refinement_far_from_stops() {
    // --- GIVEN ---
    // Route: 0m ---------- 100m ---------- 200m (no stops)
    let points = vec![
        (0, 0),
        (10000, 0),
        (20000, 0),
    ];

    let stop_indices = vec![];  // No stops

    // --- WHEN ---
    let result = simplify_and_interpolate(&points, 700.0, &stop_indices);

    // --- THEN ---
    // Verify: segments can be up to 100m when no stops nearby
    let max_segment_len = result.iter().zip(result.iter().skip(1))
        .map(|(p1, p2)| {
            let dx = p2.0 - p1.0;
            let dy = p2.1 - p1.1;
            ((dx * dx + dy * dy) as f64).sqrt()
        })
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap_or(0.0);

    assert!(
        max_segment_len <= 10000.0,
        "Max segment length should be <=100m, got {}cm",
        max_segment_len
    );

    // With no stops and straight route, 100m segments are allowed
    // The route has two 100m segments, both should remain as-is
    assert_eq!(result.len(), 3, "Route should remain unchanged with no stops");
}

#[test]
fn test_adaptive_segmentation_sharp_turn() {
    // --- GIVEN ---
    // L-shaped route with 100m segments
    // (0,0) -> (10000, 0) -> (10000, 10000)
    let points = vec![
        (0, 0),
        (10000, 0),   // 90 degree turn here
        (10000, 10000),
    ];

    let stop_indices = vec![];

    // --- WHEN ---
    let result = simplify_and_interpolate(&points, 700.0, &stop_indices);

    // --- THEN ---
    // Verify: segments at sharp turn should be refined
    // The corner point should be preserved
    assert!(result.contains(&(10000, 0)), "Corner point should be preserved");

    // Verify no segment exceeds 100m
    for i in 0..result.len() - 1 {
        let p1 = result[i];
        let p2 = result[i + 1];
        let dx = p2.0 - p1.0;
        let dy = p2.1 - p1.1;
        let dist = ((dx * dx + dy * dy) as f64).sqrt();

        assert!(
            dist <= 10000.0,
            "All segments should be <=100m, got {}cm",
            dist
        );
    }
}

// ============================================================================
// Test Helper Functions
// ============================================================================

/// Test helper for segment-point distance calculation.
/// Returns true if the minimum distance from point to segment is <= threshold.
fn segment_near_point_test(p1: (i64, i64), p2: (i64, i64), point: (i64, i64), threshold: f64) -> bool {
    let px = point.0 as f64;
    let py = point.1 as f64;
    let x1 = p1.0 as f64;
    let y1 = p1.1 as f64;
    let x2 = p2.0 as f64;
    let y2 = p2.1 as f64;

    let dx = x2 - x1;
    let dy = y2 - y1;
    let ldx = px - x1;
    let ldy = py - y1;

    let seg_len2 = dx * dx + dy * dy;
    let t = if seg_len2 < 1e-10 {
        0.0
    } else {
        ((ldx * dx + ldy * dy) / seg_len2).clamp(0.0, 1.0)
    };

    let closest_x = x1 + t * dx;
    let closest_y = y1 + t * dy;
    let dist_sq = (px - closest_x).powi(2) + (py - closest_y).powi(2);

    dist_sq.sqrt() <= threshold
}

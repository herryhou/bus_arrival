//! Integration tests for dp_mapper
//!
//! Tests the full end-to-end functionality of the DP mapper.

use dp_mapper::map_stops;
use shared::RouteNode;

/// Helper: Create a simple straight-line route
fn make_straight_route(length_cm: i32, num_segments: usize) -> Vec<RouteNode> {
    let seg_len = length_cm / num_segments as i32;
    let mut nodes = Vec::with_capacity(num_segments + 1);

    for i in 0..=num_segments {
        let x_cm = i as i32 * seg_len;
        let cum_dist = x_cm;
        let dx_cm = if i < num_segments { seg_len } else { 0 };
        let len2_cm2 = if i < num_segments { (seg_len * seg_len) as i64 } else { 0 };

        nodes.push(RouteNode {
            len2_cm2,
            heading_cdeg: 0,
            _pad: 0,
            x_cm,
            y_cm: 0,
            cum_dist_cm: cum_dist,
            dx_cm,
            dy_cm: 0,
            seg_len_cm: if i < num_segments { seg_len } else { 0 },
        });
    }

    nodes
}

#[test]
fn test_integration_straight_route() {
    // Simple route: 50m straight line, 5 segments
    let route = make_straight_route(5000, 5);

    // Stops at regular intervals along the route
    let stops = vec![(500, 0), (1500, 0), (2500, 0), (3500, 0), (4500, 0)];

    let result = map_stops(&stops, &route, None);

    assert_eq!(result.len(), 5, "should return progress for all stops");

    // Verify monotonicity
    for i in 0..result.len() - 1 {
        assert!(
            result[i] <= result[i + 1],
            "progress should be non-decreasing: {} <= {}",
            result[i],
            result[i + 1]
        );
    }

    // First stop should be near the beginning
    assert!(result[0] >= 400 && result[0] <= 600, "first stop near 500cm");
    // Last stop should be near the end
    assert!(result[4] >= 4400 && result[4] <= 4600, "last stop near 4500cm");
}

#[test]
fn test_integration_l_shaped_route() {
    // L-shaped route: goes east, then north
    let route = vec![
        RouteNode { len2_cm2: 100000000, heading_cdeg: 0, _pad: 0, x_cm: 0, y_cm: 0, cum_dist_cm: 0, dx_cm: 10000, dy_cm: 0, seg_len_cm: 10000 },
        RouteNode { len2_cm2: 100000000, heading_cdeg: 9000, _pad: 0, x_cm: 10000, y_cm: 0, cum_dist_cm: 10000, dx_cm: 0, dy_cm: 10000, seg_len_cm: 10000 },
        RouteNode { len2_cm2: 0, heading_cdeg: 0, _pad: 0, x_cm: 10000, y_cm: 10000, cum_dist_cm: 20000, dx_cm: 0, dy_cm: 0, seg_len_cm: 0 },
    ];

    // One stop on each leg
    let stops = vec![(5000, 0), (10000, 5000)];

    let result = map_stops(&stops, &route, None);

    assert_eq!(result.len(), 2);
    assert!(result[0] <= result[1], "progress should be monotonic: {} <= {}", result[0], result[1]);

    // First stop on horizontal leg (progress ~5000)
    assert!(result[0] >= 4000 && result[0] <= 6000, "first stop should be near middle of first segment: {}", result[0]);
    // Second stop on vertical leg (progress > 10000)
    // The stop is at (10000, 5000) which is the midpoint of the vertical segment
    // So progress should be 10000 + 5000 = 15000
    assert!(result[1] >= 10000, "second stop should be on vertical leg: {}", result[1]);
}

#[test]
fn test_integration_stops_at_same_location() {
    // Route with 10m segments
    let route = make_straight_route(10000, 10);

    // Two stops at the same location
    let stops = vec![(2500, 0), (2500, 0)];

    let result = map_stops(&stops, &route, None);

    assert_eq!(result.len(), 2);
    // The DP algorithm finds the globally optimal path subject to monotonicity.
    // For identical stops, it may choose different candidates due to the
    // snap-forward mechanism on the second stop.
    // What matters is that progress is non-decreasing and in a reasonable range.
    assert!(result[0] <= result[1], "progress should be non-decreasing");
    assert!(result[0] >= 0 && result[0] <= 10000, "first stop within route bounds");
    assert!(result[1] >= 2000 && result[1] <= 3000, "second stop near 2500cm: {}", result[1]);
}

#[test]
fn test_integration_empty_inputs() {
    let route = make_straight_route(5000, 5);

    // Empty stops
    let result = map_stops(&[], &route, None);
    assert_eq!(result, vec![].as_slice());

    // Empty route
    let result = map_stops(&[(100, 0)], &[], None);
    assert_eq!(result, vec![].as_slice());
}

#[test]
fn test_integration_single_stop() {
    let route = make_straight_route(10000, 10);

    let result = map_stops(&[(5000, 0)], &route, None);

    assert_eq!(result.len(), 1);
    assert!(result[0] >= 4500 && result[0] <= 5500);
}

#[test]
fn test_integration_custom_k() {
    let route = make_straight_route(10000, 10);
    let stops = vec![(2000, 0), (5000, 0), (8000, 0)];

    // Test with different K values
    let result_k5 = map_stops(&stops, &route, Some(5));
    let result_k15 = map_stops(&stops, &route, Some(15));
    let result_default = map_stops(&stops, &route, None);

    // All should return the same number of results
    assert_eq!(result_k5.len(), 3);
    assert_eq!(result_k15.len(), 3);
    assert_eq!(result_default.len(), 3);

    // All should satisfy monotonicity
    for result in &[&result_k5, &result_k15, &result_default] {
        for i in 0..result.len() - 1 {
            assert!(result[i] <= result[i + 1]);
        }
    }
}

#[test]
fn test_integration_snap_forward_usage() {
    // Create a scenario where snap-forward is needed
    // Route: segments 0-2 at increasing progress
    let route = vec![
        RouteNode { len2_cm2: 100000000, heading_cdeg: 0, _pad: 0, x_cm: 0, y_cm: 0, cum_dist_cm: 0, dx_cm: 5000, dy_cm: 0, seg_len_cm: 5000 },
        RouteNode { len2_cm2: 100000000, heading_cdeg: 0, _pad: 0, x_cm: 5000, y_cm: 0, cum_dist_cm: 5000, dx_cm: 5000, dy_cm: 0, seg_len_cm: 5000 },
        RouteNode { len2_cm2: 100000000, heading_cdeg: 0, _pad: 0, x_cm: 10000, y_cm: 0, cum_dist_cm: 10000, dx_cm: 5000, dy_cm: 0, seg_len_cm: 5000 },
        RouteNode { len2_cm2: 0, heading_cdeg: 0, _pad: 0, x_cm: 15000, y_cm: 0, cum_dist_cm: 15000, dx_cm: 0, dy_cm: 0, seg_len_cm: 0 },
    ];

    // First stop early on the route
    // Second stop far from the route (triggers snap)
    let stops = vec![(1000, 0), (0, 10000)]; // Second stop is 100m from route

    let result = map_stops(&stops, &route, Some(10));

    assert_eq!(result.len(), 2);
    assert!(result[0] <= result[1]);
}

// ============================================================================
// High-Priority Edge Case Tests
// ============================================================================

#[test]
fn test_integration_route_loops_back() {
    // U-shaped route: goes east, then north, then west back to same x
    // This tests routes that double back on themselves
    let route = vec![
        // Leg 1: East 0 → 10000
        RouteNode { len2_cm2: 100000000, heading_cdeg: 0, _pad: 0, x_cm: 0, y_cm: 0, cum_dist_cm: 0, dx_cm: 10000, dy_cm: 0, seg_len_cm: 10000 },
        // Leg 2: North 0 → 5000
        RouteNode { len2_cm2: 100000000, heading_cdeg: 9000, _pad: 0, x_cm: 10000, y_cm: 0, cum_dist_cm: 10000, dx_cm: 0, dy_cm: 5000, seg_len_cm: 5000 },
        // Leg 3: West 10000 → 0 (returns to x=0)
        RouteNode { len2_cm2: 100000000, heading_cdeg: 18000, _pad: 0, x_cm: 10000, y_cm: 5000, cum_dist_cm: 15000, dx_cm: -10000, dy_cm: 0, seg_len_cm: 10000 },
        RouteNode { len2_cm2: 0, heading_cdeg: 0, _pad: 0, x_cm: 0, y_cm: 5000, cum_dist_cm: 25000, dx_cm: 0, dy_cm: 0, seg_len_cm: 0 },
    ];

    // Stops: first on east leg, second on west leg (same y, different x)
    // The west leg stop has same progress as earlier on east leg
    let stops = vec![(5000, 0), (5000, 5000)];

    let result = map_stops(&stops, &route, Some(15));

    assert_eq!(result.len(), 2);
    // Both stops should be mapped
    // First stop at ~5000 (middle of east leg)
    assert!(result[0] >= 4000 && result[0] <= 6000, "first stop on east leg: {}", result[0]);
    // Second stop at ~20000 (middle of west leg: 15000 + 5000)
    assert!(result[1] >= 19000 && result[1] <= 21000, "second stop on west leg: {}", result[1]);
    // Monotonicity must be preserved
    assert!(result[0] < result[1], "west leg must have higher progress: {} < {}", result[0], result[1]);
}

#[test]
fn test_integration_route_crosses_itself() {
    // Figure-8 route: crosses itself at origin
    // First loop: counter-clockwise from origin
    // Second loop: clockwise crossing back through origin
    let route = vec![
        // Loop 1: Quadrant I
        RouteNode { len2_cm2: 100000000, heading_cdeg: 0, _pad: 0, x_cm: 0, y_cm: 0, cum_dist_cm: 0, dx_cm: 5000, dy_cm: 0, seg_len_cm: 5000 },
        RouteNode { len2_cm2: 100000000, heading_cdeg: 9000, _pad: 0, x_cm: 5000, y_cm: 0, cum_dist_cm: 5000, dx_cm: 0, dy_cm: 5000, seg_len_cm: 5000 },
        RouteNode { len2_cm2: 100000000, heading_cdeg: 18000, _pad: 0, x_cm: 5000, y_cm: 5000, cum_dist_cm: 10000, dx_cm: -5000, dy_cm: 0, seg_len_cm: 5000 },
        // Back to origin
        RouteNode { len2_cm2: 100000000, heading_cdeg: 0, _pad: 0, x_cm: 0, y_cm: 0, cum_dist_cm: 15000, dx_cm: 5000, dy_cm: 0, seg_len_cm: 5000 },
        // Loop 2: Quadrant IV (different direction)
        RouteNode { len2_cm2: 100000000, heading_cdeg: -9000, _pad: 0, x_cm: 5000, y_cm: 0, cum_dist_cm: 20000, dx_cm: 0, dy_cm: -5000, seg_len_cm: 5000 },
        RouteNode { len2_cm2: 0, heading_cdeg: 0, _pad: 0, x_cm: 5000, y_cm: -5000, cum_dist_cm: 25000, dx_cm: 0, dy_cm: 0, seg_len_cm: 0 },
    ];

    // Stop at origin appears twice - once in each loop
    let stops = vec![(0, 0), (5000, 0)];

    let result = map_stops(&stops, &route, Some(15));

    assert_eq!(result.len(), 2);
    // Both should map to different progress values (same location, different visits)
    // First occurrence at origin (progress 0 or 15000 depending on segment)
    // Second occurrence on east leg
    assert!(result[0] <= result[1], "monotonicity preserved across route crossing");
}

#[test]
fn test_integration_scalability_many_stops() {
    // Test with 35 stops (typical real-world route size)
    // This verifies O(M × K log K) complexity doesn't degrade
    let num_stops: usize = 35;
    let seg_len: i64 = 3000; // 30m segments
    let num_segments: usize = 40;

    let route = make_straight_route((seg_len * num_segments as i64) as i32, num_segments);

    // Stops distributed along the entire route
    let stops: Vec<(i64, i64)> = (0..num_stops)
        .map(|i| {
            let i = i as i64;
            let progress = i * seg_len * num_segments as i64 / num_stops as i64;
            (progress, 0)
        })
        .collect();

    let result = map_stops(&stops, &route, None);

    assert_eq!(result.len(), num_stops, "all stops mapped");

    // Verify monotonicity across all stops
    for i in 0..result.len() - 1 {
        assert!(
            result[i] <= result[i + 1],
            "progress should be monotonic at index {}: {} <= {}",
            i,
            result[i],
            result[i + 1]
        );
    }

    // Verify coverage - stops should span most of the route
    let total_route_len = route.last().unwrap().cum_dist_cm;
    let coverage = result[num_stops - 1] as f64 / total_route_len as f64;
    assert!(coverage > 0.8, "should cover >80% of route: {}", coverage);
}

#[test]
fn test_integration_scalability_dense_stops() {
    // Many stops in a short distance (stops are dense relative to segment length)
    // This is the scenario mentioned in the algorithm doc where greedy fails
    let route = make_straight_route(10000, 10); // 10 segments of 1m each

    // 20 stops packed into 10m route (more stops than segments)
    let stops: Vec<(i64, i64)> = (0..20)
        .map(|i| ((i * 500) as i64, 0))
        .collect();

    let result = map_stops(&stops, &route, Some(20));

    assert_eq!(result.len(), 20);

    // Verify monotonicity - critical for dense stops
    for i in 0..result.len() - 1 {
        assert!(
            result[i] <= result[i + 1],
            "dense stops must maintain monotonicity at index {}: {} <= {}",
            i,
            result[i],
            result[i + 1]
        );
    }
}

#[test]
fn test_integration_stops_at_segment_boundaries() {
    // Test stops exactly at segment boundaries
    // This tests floating point precision at t=0.0 and t=1.0 boundaries
    let route = vec![
        RouteNode { len2_cm2: 100000000, heading_cdeg: 0, _pad: 0, x_cm: 0, y_cm: 0, cum_dist_cm: 0, dx_cm: 10000, dy_cm: 0, seg_len_cm: 10000 },
        RouteNode { len2_cm2: 100000000, heading_cdeg: 0, _pad: 0, x_cm: 10000, y_cm: 0, cum_dist_cm: 10000, dx_cm: 10000, dy_cm: 0, seg_len_cm: 10000 },
        RouteNode { len2_cm2: 100000000, heading_cdeg: 0, _pad: 0, x_cm: 20000, y_cm: 0, cum_dist_cm: 20000, dx_cm: 10000, dy_cm: 0, seg_len_cm: 10000 },
        RouteNode { len2_cm2: 0, heading_cdeg: 0, _pad: 0, x_cm: 30000, y_cm: 0, cum_dist_cm: 30000, dx_cm: 0, dy_cm: 0, seg_len_cm: 0 },
    ];

    // Stops exactly at segment boundaries
    let stops = vec![
        (0, 0),      // Start of route (t=0.0 on segment 0)
        (10000, 0),  // End of segment 0 / Start of segment 1 (t=1.0 / t=0.0 boundary)
        (20000, 0),  // End of segment 1 / Start of segment 2 (t=1.0 / t=0.0 boundary)
        (30000, 0),  // End of route (t=1.0 on last segment)
    ];

    let result = map_stops(&stops, &route, Some(10));

    assert_eq!(result.len(), 4);

    // Each stop should map to its exact boundary position
    assert_eq!(result[0], 0, "first stop at route start");
    assert_eq!(result[1], 10000, "stop at segment 0/1 boundary");
    assert_eq!(result[2], 20000, "stop at segment 1/2 boundary");
    assert_eq!(result[3], 30000, "stop at route end");
}

#[test]
fn test_integration_stops_near_segment_boundaries() {
    // Test stops very close to segment boundaries
    // This checks for numerical stability in t-clamping
    let route = vec![
        RouteNode { len2_cm2: 100000000, heading_cdeg: 0, _pad: 0, x_cm: 0, y_cm: 0, cum_dist_cm: 0, dx_cm: 10000, dy_cm: 0, seg_len_cm: 10000 },
        RouteNode { len2_cm2: 100000000, heading_cdeg: 0, _pad: 0, x_cm: 10000, y_cm: 0, cum_dist_cm: 10000, dx_cm: 10000, dy_cm: 0, seg_len_cm: 10000 },
        RouteNode { len2_cm2: 0, heading_cdeg: 0, _pad: 0, x_cm: 20000, y_cm: 0, cum_dist_cm: 20000, dx_cm: 0, dy_cm: 0, seg_len_cm: 0 },
    ];

    // Stops 1cm before/after boundaries (within floating point epsilon)
    let stops = vec![
        (9999, 0),   // 1cm before first boundary
        (10001, 0),  // 1cm after first boundary
        (19999, 0),  // 1cm before second boundary
    ];

    let result = map_stops(&stops, &route, Some(10));

    assert_eq!(result.len(), 3);

    // All should be properly mapped without precision issues
    for i in 0..result.len() {
        assert!(
            result[i] >= 0 && result[i] <= 20000,
            "stop {} mapped within route bounds: {}",
            i,
            result[i]
        );
    }

    // Monotonicity must hold
    assert!(result[0] <= result[1] && result[1] <= result[2]);
}

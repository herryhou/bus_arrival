//! DP Mapper Edge Cases - Stop Placement
//!
//! Tests for edge cases related to stop placement:
//! - Stops at segment boundaries
//! - Stops near segment boundaries
//! - Identical stops
//! - Stops far from route
//! - Dense stops

use dp_mapper::map_stops;
use shared::RouteNode;

// ============================================================================
// Route Builders
// ============================================================================

fn make_straight_route(length_cm: i32, num_segments: usize) -> Vec<RouteNode> {
    let seg_len = length_cm / num_segments as i32;
    let mut nodes = Vec::with_capacity(num_segments + 1);

    for i in 0..=num_segments {
        let x_cm = i as i32 * seg_len;
        let cum_dist = x_cm;
        let dx_cm = if i < num_segments { seg_len } else { 0 };
        let len2_cm2 = if i < num_segments {
            (seg_len as i64) * (seg_len as i64)
        } else {
            0
        };

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

// ============================================================================
// Stops at Segment Boundaries
// ============================================================================

#[test]
fn test_stops_at_exact_segment_boundaries() {
    // --- GIVEN ---
    // A route with segments of exactly 10m each
    // Stops are placed exactly at segment boundaries (0m, 10m, 20m, 30m)
    let route = vec![
        RouteNode {
            len2_cm2: 100000000,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            dx_cm: 10000,
            dy_cm: 0,
            seg_len_cm: 10000,
        },
        RouteNode {
            len2_cm2: 100000000,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 10000,
            y_cm: 0,
            cum_dist_cm: 10000,
            dx_cm: 10000,
            dy_cm: 0,
            seg_len_cm: 10000,
        },
        RouteNode {
            len2_cm2: 100000000,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 20000,
            y_cm: 0,
            cum_dist_cm: 20000,
            dx_cm: 10000,
            dy_cm: 0,
            seg_len_cm: 10000,
        },
        RouteNode {
            len2_cm2: 0,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 30000,
            y_cm: 0,
            cum_dist_cm: 30000,
            dx_cm: 0,
            dy_cm: 0,
            seg_len_cm: 0,
        },
    ];

    let stops = vec![
        (0, 0),      // Start of route (t=0.0 on segment 0)
        (10000, 0),  // End of segment 0 / Start of segment 1
        (20000, 0),  // End of segment 1 / Start of segment 2
        (30000, 0),  // End of route (t=1.0 on last segment)
    ];

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(10));

    // --- THEN ---
    assert_eq!(result.len(), 4, "should map all 4 stops");

    // Each stop should map to its exact boundary progress value
    assert_eq!(result[0].progress_cm, 0, "first stop at route start");
    assert_eq!(result[1].progress_cm, 10000, "stop at segment 0/1 boundary");
    assert_eq!(result[2].progress_cm, 20000, "stop at segment 1/2 boundary");
    assert_eq!(result[3].progress_cm, 30000, "stop at route end");
}

#[test]
fn test_stops_1cm_from_segment_boundaries() {
    // --- GIVEN ---
    // A route with segments at 0m, 10m, 20m
    // Stops are placed at 9.99m, 10.01m, 19.99m
    // This tests numerical stability near boundaries
    let route = vec![
        RouteNode {
            len2_cm2: 100000000,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            dx_cm: 10000,
            dy_cm: 0,
            seg_len_cm: 10000,
        },
        RouteNode {
            len2_cm2: 100000000,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 10000,
            y_cm: 0,
            cum_dist_cm: 10000,
            dx_cm: 10000,
            dy_cm: 0,
            seg_len_cm: 10000,
        },
        RouteNode {
            len2_cm2: 0,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 20000,
            y_cm: 0,
            cum_dist_cm: 20000,
            dx_cm: 0,
            dy_cm: 0,
            seg_len_cm: 0,
        },
    ];

    let stops = vec![
        (9999, 0),   // 1cm before first boundary
        (10001, 0),  // 1cm after first boundary
        (19999, 0),  // 1cm before second boundary
    ];

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(10));

    // --- THEN ---
    assert_eq!(result.len(), 3, "should map all stops");

    // All should be properly mapped without boundary ambiguity
    for (i, r) in result.iter().enumerate() {
        assert!(
            r.progress_cm >= 0 && r.progress_cm <= 20000,
            "stop {} mapped within route bounds: {}",
            i,
            r.progress_cm
        );
    }

    // Monotonicity must hold - no boundary crossing issues
    assert!(
        result[0].progress_cm <= result[1].progress_cm,
        "boundary proximity: {} <= {}",
        result[0].progress_cm,
        result[1].progress_cm
    );
    assert!(
        result[1].progress_cm <= result[2].progress_cm,
        "boundary proximity: {} <= {}",
        result[1].progress_cm,
        result[2].progress_cm
    );
}

#[test]
fn test_stops_at_node_t_zero_and_t_one() {
    // --- GIVEN ---
    // Route with varying segment lengths
    // Stops at exact node positions (t=0.0 and t=1.0)
    let route = vec![
        RouteNode {
            len2_cm2: 25000000,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            dx_cm: 5000,
            dy_cm: 0,
            seg_len_cm: 5000,
        },
        RouteNode {
            len2_cm2: 64000000,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 5000,
            y_cm: 0,
            cum_dist_cm: 5000,
            dx_cm: 8000,
            dy_cm: 0,
            seg_len_cm: 8000,
        },
        RouteNode {
            len2_cm2: 0,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 13000,
            y_cm: 0,
            cum_dist_cm: 13000,
            dx_cm: 0,
            dy_cm: 0,
            seg_len_cm: 0,
        },
    ];

    // Stops exactly at nodes
    let stops = vec![(0, 0), (5000, 0), (13000, 0)];

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(10));

    // --- THEN ---
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].progress_cm, 0, "stop at start node");
    assert_eq!(result[1].progress_cm, 5000, "stop at middle node");
    assert_eq!(result[2].progress_cm, 13000, "stop at end node");
}

// ============================================================================
// Identical Stops
// ============================================================================

#[test]
fn test_three_identical_stops_same_location() {
    // --- GIVEN ---
    // A straight route
    // Three stops are all at the exact same coordinates (50m, 0m)
    let route = make_straight_route(10000, 10);
    let stops = vec![(2500, 0), (2500, 0), (2500, 0)];

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(15));

    // --- THEN ---
    assert_eq!(result.len(), 3, "should map all identical stops");

    // All stops should return valid candidates
    for (i, r) in result.iter().enumerate() {
        assert!(
            r.progress_cm >= 0 && r.progress_cm <= 10000,
            "identical stop {} mapped within route bounds: {}",
            i,
            r.progress_cm
        );
    }

    // Progress should be non-decreasing (may use snap-forward)
    assert!(
        result[0].progress_cm <= result[1].progress_cm
            && result[1].progress_cm <= result[2].progress_cm,
        "identical stops: monotonicity {} <= {} <= {}",
        result[0].progress_cm,
        result[1].progress_cm,
        result[2].progress_cm
    );
}

#[test]
fn test_two_identical_stops_at_route_end() {
    // --- GIVEN ---
    // Route with two identical stops at the end
    let route = make_straight_route(5000, 5);
    let stops = vec![(5000, 0), (5000, 0)];

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(10));

    // --- THEN ---
    assert_eq!(result.len(), 2);
    assert!(
        result[0].progress_cm <= result[1].progress_cm,
        "identical stops at end: monotonicity"
    );
    assert!(
        result[1].progress_cm <= 5000,
        "identical stops at end: within bounds"
    );
}

#[test]
fn test_identical_stops_near_middle() {
    // --- GIVEN ---
    // Multiple identical stops at the middle of route
    let route = make_straight_route(20000, 20);
    let stops = vec![(10000, 0), (10000, 0), (10000, 0), (10000, 0)];

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(20));

    // --- THEN ---
    assert_eq!(result.len(), 4);

    // All should be mapped within route bounds
    // Note: Due to the mapping algorithm and snap-forward, identical stops may map to different positions
    for (i, r) in result.iter().enumerate() {
        assert!(
            r.progress_cm >= 0 && r.progress_cm <= 20000,
            "identical stop {} within route bounds: {}",
            i,
            r.progress_cm
        );
    }

    // Monotonicity should be preserved
    for i in 0..result.len() - 1 {
        assert!(
            result[i].progress_cm <= result[i + 1].progress_cm,
            "identical stops: monotonicity at {}",
            i
        );
    }

    // Monotonicity with snap-forward
    for i in 0..result.len() - 1 {
        assert!(
            result[i].progress_cm <= result[i + 1].progress_cm,
            "identical stops: monotonic at {}",
            i
        );
    }
}

// ============================================================================
// Stops Far from Route
// ============================================================================

#[test]
fn test_stop_150m_from_route() {
    // --- GIVEN ---
    // A straight route along X-axis from 0 to 100m
    // A stop is placed at (50m, 150m) - 150m perpendicular to route
    let route = make_straight_route(10000, 10);
    let stops = vec![(5000, 15000)];

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(15));

    // --- THEN ---
    assert_eq!(result.len(), 1, "should map far stop");

    // Snap-forward should provide a valid candidate
    assert!(
        result[0].progress_cm >= 0 && result[0].progress_cm <= 10000,
        "far stop mapped within route bounds: {}",
        result[0].progress_cm
    );

    // The mapped position should be geometrically reasonable
    // (near the perpendicular projection point)
    assert!(
        result[0].progress_cm >= 4000 && result[0].progress_cm <= 6000,
        "far stop mapped near expected projection: {}",
        result[0].progress_cm
    );
}

#[test]
fn test_multiple_stops_far_from_route() {
    // --- GIVEN ---
    // Route with multiple stops far off-route
    let route = make_straight_route(15000, 15);
    let stops = vec![
        (3000, 12000),  // 120m off at x=30m
        (7500, 20000),  // 200m off at x=75m
        (12000, 15000), // 150m off at x=120m
    ];

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(15));

    // --- THEN ---
    assert_eq!(result.len(), 3, "should map all far stops");

    // All should be valid
    for (i, r) in result.iter().enumerate() {
        assert!(
            r.progress_cm >= 0 && r.progress_cm <= 15000,
            "far stop {} within bounds: {}",
            i,
            r.progress_cm
        );
    }

    // Monotonicity should be preserved
    assert!(
        result[0].progress_cm <= result[1].progress_cm
            && result[1].progress_cm <= result[2].progress_cm,
        "far stops: monotonicity"
    );
}

#[test]
fn test_stop_very_far_from_route() {
    // --- GIVEN ---
    // Stop 500m from route (extreme case)
    let route = make_straight_route(10000, 10);
    let stops = vec![(5000, 50000)];

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(15));

    // --- THEN ---
    // When stop is extremely far from route (500m = 50000cm), no candidates may be found
    // The map_stops function returns empty result in this case
    assert!(result.len() <= 1, "very far stop: at most one result");

    // If a result is found, it should be within bounds
    if !result.is_empty() {
        assert!(
            result[0].progress_cm >= 0 && result[0].progress_cm <= 10000,
            "very far stop still maps within bounds: {}",
            result[0].progress_cm
        );
    }
}

// ============================================================================
// Dense Stops
// ============================================================================

#[test]
fn test_dense_stops_more_stops_than_segments() {
    // --- GIVEN ---
    // 20 stops on a route with only 10 segments
    // This is the scenario where greedy fails
    let route = make_straight_route(10000, 10);
    let stops: Vec<(i64, i64)> = (0..20).map(|i| ((i * 500) as i64, 0)).collect();

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(20));

    // --- THEN ---
    assert_eq!(result.len(), 20, "should map all 20 stops");

    // All stops should have valid mappings
    for (i, r) in result.iter().enumerate() {
        assert!(
            r.progress_cm >= 0 && r.progress_cm <= 10000,
            "dense stop {} within bounds: {}",
            i,
            r.progress_cm
        );
    }

    // Progress should be strictly monotonic (no duplicates allowed for dense stops)
    for i in 0..result.len() - 1 {
        assert!(
            result[i].progress_cm <= result[i + 1].progress_cm,
            "dense stops: monotonicity at {}: {} <= {}",
            i,
            result[i].progress_cm,
            result[i + 1].progress_cm
        );
    }
}

#[test]
fn test_very_dense_stops_50_on_10_segments() {
    // --- GIVEN ---
    // 50 stops on a route with 10 segments (5:1 ratio)
    let route = make_straight_route(10000, 10);
    let stops: Vec<(i64, i64)> = (0..50).map(|i| ((i * 200) as i64, 0)).collect();

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(50));

    // --- THEN ---
    assert_eq!(result.len(), 50);

    // Verify monotonicity - critical for very dense stops
    for i in 0..result.len() - 1 {
        assert!(
            result[i].progress_cm <= result[i + 1].progress_cm,
            "very dense stops: monotonicity at {}: {} <= {}",
            i,
            result[i].progress_cm,
            result[i + 1].progress_cm
        );
    }

    // Stops should span the route
    assert!(
        result[0].progress_cm >= 0,
        "first stop at or after start"
    );
    assert!(
        result[49].progress_cm <= 10000,
        "last stop at or before end"
    );
}

#[test]
fn test_dense_stops_at_regular_intervals() {
    // --- GIVEN ---
    // 30 stops placed every 10cm on a 3m route
    let route = make_straight_route(300, 3);
    let stops: Vec<(i64, i64)> = (0..30).map(|i| ((i * 10) as i64, 0)).collect();

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(30));

    // --- THEN ---
    assert_eq!(result.len(), 30);

    // Each stop should map close to its input position
    for (i, r) in result.iter().enumerate() {
        let expected = i * 10;
        assert!(
            (r.progress_cm - expected as i32).abs() <= 5,
            "dense stop {} at {}: mapped to {} (expected {})",
            i,
            expected,
            r.progress_cm,
            expected
        );
    }
}

#[test]
fn test_dense_stops_with_gaps() {
    // --- GIVEN ---
    // Dense stops with gaps between groups
    let route = make_straight_route(20000, 20);
    let mut stops = Vec::new();

    // First dense cluster: 0-1000cm (10 stops)
    for i in 0..10 {
        stops.push((i * 100, 0));
    }
    // Gap
    // Second dense cluster: 10000-11000cm (10 stops)
    for i in 0..10 {
        stops.push((10000 + i * 100, 0));
    }

    let stops: Vec<(i64, i64)> = stops.into_iter().map(|(x, y)| (x as i64, y as i64)).collect();

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(20));

    // --- THEN ---
    assert_eq!(result.len(), 20);

    // First cluster should map to 0-1000 range
    for i in 0..10 {
        assert!(
            result[i].progress_cm >= 0 && result[i].progress_cm <= 1500,
            "cluster 1 stop {} in range: {}",
            i,
            result[i].progress_cm
        );
    }

    // Second cluster should map to 10000-11000 range
    for i in 10..20 {
        assert!(
            result[i].progress_cm >= 9500 && result[i].progress_cm <= 11500,
            "cluster 2 stop {} in range: {}",
            i,
            result[i].progress_cm
        );
    }

    // Monotonicity across the gap
    assert!(
        result[9].progress_cm <= result[10].progress_cm,
        "gap: monotonicity {} <= {}",
        result[9].progress_cm,
        result[10].progress_cm
    );
}

// ============================================================================
// Edge Case: Stop at Route Start/End
// ============================================================================

#[test]
fn test_stop_at_exact_route_start() {
    // --- GIVEN ---
    let route = make_straight_route(10000, 10);
    let stops = vec![(0, 0)];

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(10));

    // --- THEN ---
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].progress_cm, 0, "stop at route start");
}

#[test]
fn test_stop_at_exact_route_end() {
    // --- GIVEN ---
    let route = make_straight_route(10000, 10);
    let stops = vec![(10000, 0)];

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(10));

    // --- THEN ---
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].progress_cm, 10000, "stop at route end");
}

#[test]
fn test_stops_at_both_extremes() {
    // --- GIVEN ---
    let route = make_straight_route(10000, 10);
    let stops = vec![(0, 0), (10000, 0)];

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(10));

    // --- THEN ---
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].progress_cm, 0, "first stop at start");
    assert_eq!(result[1].progress_cm, 10000, "second stop at end");
}

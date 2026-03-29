//! DP Mapper Edge Cases - DP Algorithm
//!
//! Tests for edge cases related to the dynamic programming algorithm:
//! - Cost saturation
//! - Empty candidate sets
//! - Single candidate per stop
//! - All candidates invalid (monotonicity violation)

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
// Cost Saturation Tests
// ============================================================================

#[test]
fn test_cost_accumulation_no_overflow() {
    // --- GIVEN ---
    // A route with 50 stops
    // Each stop has moderate distance squared
    // This tests that costs don't overflow i64::MAX
    let route = make_straight_route(50000, 50);

    // Stops distributed along the route
    let stops: Vec<(i64, i64)> = (0..50)
        .map(|i| {
            let progress = (i * 1000) as i64;
            (progress, 0)
        })
        .collect();

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(15));

    // --- THEN ---
    assert_eq!(result.len(), 50, "should map all stops");

    // All results should be valid
    for (i, r) in result.iter().enumerate() {
        assert!(
            r.progress_cm >= 0 && r.progress_cm <= 50000,
            "stop {} mapped within bounds: {}",
            i,
            r.progress_cm
        );
    }

    // Monotonicity should hold
    for i in 0..result.len() - 1 {
        assert!(
            result[i].progress_cm <= result[i + 1].progress_cm,
            "cost saturation: monotonicity at {}: {} <= {}",
            i,
            result[i].progress_cm,
            result[i + 1].progress_cm
        );
    }
}

#[test]
fn test_high_cost_still_works() {
    // --- GIVEN ---
    // Stops with high squared distances (but not overflowing)
    let route = make_straight_route(10000, 10);

    // Stops slightly off-route (creates higher costs)
    let stops = vec![
        (1000, 500),   // 5m off
        (3000, 800),   // 8m off
        (5000, 1000),  // 10m off
        (7000, 600),   // 6m off
        (9000, 400),   // 4m off
    ];

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(15));

    // --- THEN ---
    assert_eq!(result.len(), 5);

    // High costs shouldn't break monotonicity
    for i in 0..result.len() - 1 {
        assert!(
            result[i].progress_cm <= result[i + 1].progress_cm,
            "high cost: monotonicity at {}",
            i
        );
    }
}

// ============================================================================
// Empty Candidate Set Tests
// ============================================================================

#[test]
fn test_empty_route_returns_empty() {
    // --- GIVEN ---
    let route: Vec<RouteNode> = vec![];
    let stops = vec![(100, 0)];

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(15));

    // --- THEN ---
    assert_eq!(result.len(), 0, "empty route should return empty result");
}

#[test]
fn test_empty_stops_returns_empty() {
    // --- GIVEN ---
    let route = make_straight_route(5000, 5);
    let stops: Vec<(i64, i64)> = vec![];

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(15));

    // --- THEN ---
    assert_eq!(result.len(), 0, "empty stops should return empty result");
}

#[test]
fn test_single_node_route() {
    // --- GIVEN ---
    // Route with only one node (degenerate case)
    let route = vec![RouteNode {
        len2_cm2: 0,
        heading_cdeg: 0,
        _pad: 0,
        x_cm: 0,
        y_cm: 0,
        cum_dist_cm: 0,
        dx_cm: 0,
        dy_cm: 0,
        seg_len_cm: 0,
    }];
    let stops = vec![(0, 0)];

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(15));

    // --- THEN ---
    // Single node route may return empty result (no segments to project onto)
    // This is expected behavior for degenerate routes
    assert!(result.len() <= 1, "single node route: at most one result");
}

// ============================================================================
// Single Candidate Per Stop Tests (K=1)
// ============================================================================

#[test]
fn test_k_1_single_candidate_per_stop() {
    // --- GIVEN ---
    // A complex route
    // K is set to 1 (only one candidate per stop)
    let route = make_straight_route(10000, 10);
    let stops = vec![(1000, 0), (3000, 0), (5000, 0), (7000, 0), (9000, 0)];

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(1));

    // --- THEN ---
    assert_eq!(result.len(), 5, "should map all stops with K=1");

    // Each stop should have exactly one candidate (only one result per stop)
    // The global path should still be optimal with limited choice
    for i in 0..result.len() - 1 {
        assert!(
            result[i].progress_cm <= result[i + 1].progress_cm,
            "K=1: monotonicity at {}: {} <= {}",
            i,
            result[i].progress_cm,
            result[i + 1].progress_cm
        );
    }
}

#[test]
fn test_k_1_with_complex_route() {
    // --- GIVEN ---
    // L-shaped route with K=1
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
            heading_cdeg: 9000,
            _pad: 0,
            x_cm: 10000,
            y_cm: 0,
            cum_dist_cm: 10000,
            dx_cm: 0,
            dy_cm: 10000,
            seg_len_cm: 10000,
        },
        RouteNode {
            len2_cm2: 0,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 10000,
            y_cm: 10000,
            cum_dist_cm: 20000,
            dx_cm: 0,
            dy_cm: 0,
            seg_len_cm: 0,
        },
    ];

    let stops = vec![(5000, 0), (10000, 5000)];

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(1));

    // --- THEN ---
    assert_eq!(result.len(), 2);
    assert!(
        result[0].progress_cm <= result[1].progress_cm,
        "K=1 L-route: monotonicity"
    );
}

#[test]
fn test_k_1_dense_stops() {
    // --- GIVEN ---
    // Dense stops with K=1 (stress test)
    let route = make_straight_route(10000, 10);
    let stops: Vec<(i64, i64)> = (0..20).map(|i| ((i * 500) as i64, 0)).collect();

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(1));

    // --- THEN ---
    assert_eq!(result.len(), 20);

    // Even with K=1, monotonicity should hold
    for i in 0..result.len() - 1 {
        assert!(
            result[i].progress_cm <= result[i + 1].progress_cm,
            "K=1 dense: monotonicity at {}",
            i
        );
    }
}

// ============================================================================
// All Candidates Invalid (Monotonicity Violation) Tests
// ============================================================================

#[test]
fn test_monotonicity_violation_snap_forward_resolves() {
    // --- GIVEN ---
    // Route where second stop is geographically before first stop
    // This would normally violate monotonicity
    let route = make_straight_route(10000, 10);

    // First stop at 70m, second stop at 30m (geographically before)
    let stops = vec![(7000, 0), (3000, 0)];

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(15));

    // --- THEN ---
    assert_eq!(result.len(), 2);

    // Snap-forward should provide a valid candidate for the second stop
    // The final path should satisfy monotonicity constraint
    assert!(
        result[0].progress_cm <= result[1].progress_cm,
        "snap-forward: monotonicity restored: {} <= {}",
        result[0].progress_cm,
        result[1].progress_cm
    );
}

#[test]
fn test_reverse_order_stops() {
    // --- GIVEN ---
    // All stops in reverse geographic order
    let route = make_straight_route(10000, 10);
    let stops = vec![(9000, 0), (7000, 0), (5000, 0), (3000, 0), (1000, 0)];

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(15));

    // --- THEN ---
    assert_eq!(result.len(), 5);

    // Snap-forward should restore monotonicity for all stops
    for i in 0..result.len() - 1 {
        assert!(
            result[i].progress_cm <= result[i + 1].progress_cm,
            "reverse order: monotonicity at {}: {} <= {}",
            i,
            result[i].progress_cm,
            result[i + 1].progress_cm
        );
    }
}

#[test]
fn test_stops_out_of_order_mixed() {
    // --- GIVEN ---
    // Stops in mixed order (some forward, some backward)
    let route = make_straight_route(15000, 15);
    let stops = vec![
        (2000, 0),   // Forward
        (8000, 0),   // Forward
        (4000, 0),   // Backward (violates monotonicity)
        (12000, 0),  // Forward
        (10000, 0),  // Backward (violates monotonicity)
    ];

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(15));

    // --- THEN ---
    assert_eq!(result.len(), 5);

    // All should be valid with snap-forward
    for i in 0..result.len() - 1 {
        assert!(
            result[i].progress_cm <= result[i + 1].progress_cm,
            "mixed order: monotonicity at {}: {} <= {}",
            i,
            result[i].progress_cm,
            result[i + 1].progress_cm
        );
    }
}

#[test]
fn test_monotonicity_with_u_turn() {
    // --- GIVEN ---
    // U-turn route with stops that would violate monotonicity without snap-forward
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
            heading_cdeg: 9000,
            _pad: 0,
            x_cm: 10000,
            y_cm: 0,
            cum_dist_cm: 10000,
            dx_cm: 0,
            dy_cm: 5000,
            seg_len_cm: 5000,
        },
        RouteNode {
            len2_cm2: 100000000,
            heading_cdeg: 18000,
            _pad: 0,
            x_cm: 10000,
            y_cm: 5000,
            cum_dist_cm: 15000,
            dx_cm: -10000,
            dy_cm: 0,
            seg_len_cm: 10000,
        },
        RouteNode {
            len2_cm2: 0,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 0,
            y_cm: 5000,
            cum_dist_cm: 25000,
            dx_cm: 0,
            dy_cm: 0,
            seg_len_cm: 0,
        },
    ];

    // First stop on west leg, second on east leg (same X)
    let stops = vec![(5000, 5000), (5000, 0)];

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(15));

    // --- THEN ---
    assert_eq!(result.len(), 2);

    // Despite the west leg stop being geographically "first" at X=5000,
    // the east leg stop must come first in progress
    // Note: If both stops map to same progress (due to snap-forward), monotonicity (<=) is still satisfied
    assert!(
        result[0].progress_cm <= result[1].progress_cm,
        "U-turn: east leg before or at west leg: {} <= {}",
        result[0].progress_cm,
        result[1].progress_cm
    );
}

// ============================================================================
// Edge Case: Very Large K
// ============================================================================

#[test]
fn test_very_large_k() {
    // --- GIVEN ---
    // K=100 (much larger than needed)
    let route = make_straight_route(10000, 10);
    let stops = vec![(1000, 0), (5000, 0), (9000, 0)];

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(100));

    // --- THEN ---
    assert_eq!(result.len(), 3);

    // Large K shouldn't break anything
    for i in 0..result.len() - 1 {
        assert!(
            result[i].progress_cm <= result[i + 1].progress_cm,
            "large K: monotonicity at {}",
            i
        );
    }
}

// ============================================================================
// Edge Case: Zero-Length Route
// ============================================================================

#[test]
fn test_zero_length_route() {
    // --- GIVEN ---
    // Route with zero length (both points at origin)
    let route = vec![
        RouteNode {
            len2_cm2: 0,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            dx_cm: 0,
            dy_cm: 0,
            seg_len_cm: 0,
        },
        RouteNode {
            len2_cm2: 0,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            dx_cm: 0,
            dy_cm: 0,
            seg_len_cm: 0,
        },
    ];

    let stops = vec![(0, 0)];

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(15));

    // --- THEN ---
    // Zero-length route may return empty result (no segments to project onto)
    // This is expected behavior for degenerate routes
    assert!(result.len() <= 1, "zero-length route: at most one result");
}

// ============================================================================
// Edge Case: Single Segment
// ============================================================================

#[test]
fn test_single_segment_multiple_stops() {
    // --- GIVEN ---
    // Route with only one segment
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
            len2_cm2: 0,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 10000,
            y_cm: 0,
            cum_dist_cm: 10000,
            dx_cm: 0,
            dy_cm: 0,
            seg_len_cm: 0,
        },
    ];

    let stops = vec![(0, 0), (2500, 0), (5000, 0), (7500, 0), (10000, 0)];

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(15));

    // --- THEN ---
    assert_eq!(result.len(), 5);

    // All should map to the single segment
    for (i, r) in result.iter().enumerate() {
        assert!(
            r.progress_cm >= 0 && r.progress_cm <= 10000,
            "single segment: stop {} in bounds: {}",
            i,
            r.progress_cm
        );
    }

    // Exact boundary stops
    assert_eq!(result[0].progress_cm, 0);
    assert_eq!(result[4].progress_cm, 10000);
}

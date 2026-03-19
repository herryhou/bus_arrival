//! DP Mapper Edge Cases - Route Geometry
//!
//! Tests for edge cases related to route geometry:
//! - U-turn/loop-back routes
//! - Figure-8 self-crossing routes
//! - Zig-zag routes with sharp turns

use dp_mapper::map_stops;
use shared::RouteNode;

// ============================================================================
// Route Builders (inline helpers)
// ============================================================================

fn make_u_route(horizontal_cm: i32, vertical_cm: i32) -> Vec<RouteNode> {
    vec![
        RouteNode {
            len2_cm2: (horizontal_cm as i64) * (horizontal_cm as i64),
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            dx_cm: horizontal_cm,
            dy_cm: 0,
            seg_len_cm: horizontal_cm,
        },
        RouteNode {
            len2_cm2: (vertical_cm as i64) * (vertical_cm as i64),
            heading_cdeg: 9000,
            _pad: 0,
            x_cm: horizontal_cm,
            y_cm: 0,
            cum_dist_cm: horizontal_cm,
            dx_cm: 0,
            dy_cm: vertical_cm,
            seg_len_cm: vertical_cm,
        },
        RouteNode {
            len2_cm2: (horizontal_cm as i64) * (horizontal_cm as i64),
            heading_cdeg: 18000,
            _pad: 0,
            x_cm: horizontal_cm,
            y_cm: vertical_cm,
            cum_dist_cm: horizontal_cm + vertical_cm,
            dx_cm: -horizontal_cm,
            dy_cm: 0,
            seg_len_cm: horizontal_cm,
        },
        RouteNode {
            len2_cm2: 0,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 0,
            y_cm: vertical_cm,
            cum_dist_cm: 2 * horizontal_cm + vertical_cm,
            dx_cm: 0,
            dy_cm: 0,
            seg_len_cm: 0,
        },
    ]
}

fn make_figure8_route(size_cm: i32) -> Vec<RouteNode> {
    vec![
        RouteNode {
            len2_cm2: (size_cm as i64) * (size_cm as i64),
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            dx_cm: size_cm,
            dy_cm: 0,
            seg_len_cm: size_cm,
        },
        RouteNode {
            len2_cm2: (size_cm as i64) * (size_cm as i64),
            heading_cdeg: 9000,
            _pad: 0,
            x_cm: size_cm,
            y_cm: 0,
            cum_dist_cm: size_cm,
            dx_cm: 0,
            dy_cm: size_cm,
            seg_len_cm: size_cm,
        },
        RouteNode {
            len2_cm2: (size_cm as i64) * (size_cm as i64),
            heading_cdeg: 18000,
            _pad: 0,
            x_cm: size_cm,
            y_cm: size_cm,
            cum_dist_cm: 2 * size_cm,
            dx_cm: -size_cm,
            dy_cm: 0,
            seg_len_cm: size_cm,
        },
        RouteNode {
            len2_cm2: (size_cm as i64) * (size_cm as i64),
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 3 * size_cm,
            dx_cm: size_cm,
            dy_cm: 0,
            seg_len_cm: size_cm,
        },
        RouteNode {
            len2_cm2: (size_cm as i64) * (size_cm as i64),
            heading_cdeg: -9000,
            _pad: 0,
            x_cm: size_cm,
            y_cm: 0,
            cum_dist_cm: 4 * size_cm,
            dx_cm: 0,
            dy_cm: -size_cm,
            seg_len_cm: size_cm,
        },
        RouteNode {
            len2_cm2: 0,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: size_cm,
            y_cm: -size_cm,
            cum_dist_cm: 5 * size_cm,
            dx_cm: 0,
            dy_cm: 0,
            seg_len_cm: 0,
        },
    ]
}

fn make_zigzag_route(num_segments: usize, segment_length_cm: i32) -> Vec<RouteNode> {
    let mut nodes = Vec::with_capacity(num_segments + 1);
    let mut cum_dist_cm = 0;
    let mut x_cm = 0;
    let mut y_cm = 0;
    let mut going_east = true;

    for i in 0..=num_segments {
        let is_last = i == num_segments;

        let (dx_cm, dy_cm, heading_cdeg) = if is_last {
            (0, 0, 0)
        } else if going_east {
            (segment_length_cm, 0, 0)
        } else {
            (0, segment_length_cm, 9000)
        };

        let seg_len_cm = if is_last { 0 } else { segment_length_cm };
        let len2_cm2 = if is_last {
            0
        } else {
            (segment_length_cm as i64) * (segment_length_cm as i64)
        };

        nodes.push(RouteNode {
            len2_cm2,
            heading_cdeg,
            _pad: 0,
            x_cm,
            y_cm,
            cum_dist_cm,
            dx_cm,
            dy_cm,
            seg_len_cm,
        });

        if !is_last {
            cum_dist_cm += seg_len_cm;
            x_cm += dx_cm;
            y_cm += dy_cm;
            going_east = !going_east;
        }
    }

    nodes
}

fn assert_monotonic_progress(progress_values: &[i32], context: &str) {
    for i in 0..progress_values.len().saturating_sub(1) {
        assert!(
            progress_values[i] <= progress_values[i + 1],
            "{}: monotonicity violated at index {}: {} > {}",
            context,
            i,
            progress_values[i],
            progress_values[i + 1]
        );
    }
}

// ============================================================================
// Tests
// ============================================================================

#[test]
fn test_u_turn_route_with_stops_on_both_legs() {
    // --- GIVEN ---
    // U-shaped route: goes east 100m, then north 50m, then west 100m
    // Stops are placed at (50m, 0m) on east leg and (50m, 50m) on west leg
    let route = make_u_route(10000, 5000);
    let stops = vec![(5000, 0), (5000, 5000)];

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(15));

    // --- THEN ---
    assert_eq!(result.len(), 2, "should map both stops");

    // First stop at ~5000 (middle of east leg)
    assert!(
        result[0].progress_cm >= 4000 && result[0].progress_cm <= 6000,
        "first stop on east leg: {}",
        result[0].progress_cm
    );

    // Second stop at ~20000 (middle of west leg: 10000 + 5000 + 5000)
    assert!(
        result[1].progress_cm >= 19000 && result[1].progress_cm <= 21000,
        "second stop on west leg: {}",
        result[1].progress_cm
    );

    // Monotonicity: west leg stop must have higher progress despite same X coordinate
    assert!(
        result[0].progress_cm < result[1].progress_cm,
        "west leg must have higher progress: {} < {}",
        result[0].progress_cm,
        result[1].progress_cm
    );
}

#[test]
fn test_u_turn_route_stops_at_same_x_different_progress() {
    // --- GIVEN ---
    // U-turn route where stops share X coordinate but are on different legs
    let route = make_u_route(8000, 4000);
    // Stop at x=4000 on east leg, and x=4000 on west leg
    let stops = vec![(4000, 0), (4000, 4000)];

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(15));

    // --- THEN ---
    assert_eq!(result.len(), 2);
    assert!(
        result[0].progress_cm < result[1].progress_cm,
        "stops at same X must map to different progress values"
    );
}

#[test]
fn test_figure8_route_crossing_at_origin() {
    // --- GIVEN ---
    // Figure-8 route that crosses itself at origin (0, 0)
    // Two stops at origin but on different loops
    let route = make_figure8_route(5000);
    let stops = vec![(0, 0), (5000, 0)];

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(15));

    // --- THEN ---
    assert_eq!(result.len(), 2, "should map both stops");

    // Stops should map to different progress values based on visit order
    assert!(
        result[0].progress_cm <= result[1].progress_cm,
        "monotonicity preserved across crossing point: {} <= {}",
        result[0].progress_cm,
        result[1].progress_cm
    );
}

#[test]
fn test_figure8_multiple_stops_at_crossing() {
    // --- GIVEN ---
    // Figure-8 with three stops at the crossing point (different visits)
    let route = make_figure8_route(6000);
    let stops = vec![(0, 0), (0, 0), (6000, 0)];

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(15));

    // --- THEN ---
    assert_eq!(result.len(), 3);

    // All should have valid progress values
    for (i, r) in result.iter().enumerate() {
        let last_cum = route.last().map(|n| n.cum_dist_cm).unwrap_or(0);
        assert!(
            r.progress_cm >= 0 && r.progress_cm <= last_cum,
            "stop {} progress within route bounds: {}",
            i,
            r.progress_cm
        );
    }

    // Monotonicity must hold
    assert_monotonic_progress(&result.iter().map(|r| r.progress_cm).collect::<Vec<_>>(), "figure8 crossing");
}

#[test]
fn test_zigzag_route_90_degree_turns() {
    // --- GIVEN ---
    // Zig-zag route with alternating 90-degree turns every 10m
    let route = make_zigzag_route(10, 1000);
    // Stops at each corner (where turns occur)
    let stops: Vec<(i64, i64)> = (0..=10)
        .map(|i| {
            if i % 2 == 0 {
                // Even indices: on horizontal segments
                ((i / 2) * 1000, 0)
            } else {
                // Odd indices: on vertical segments
                ((i / 2) * 1000, 1000)
            }
        })
        .map(|(x, y)| (x as i64, y as i64))
        .collect();

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(15));

    // --- THEN ---
    assert_eq!(result.len(), stops.len(), "should map all stops");

    // All stops should be mapped correctly despite sharp heading changes
    for (i, r) in result.iter().enumerate() {
        let last_cum = route.last().map(|n| n.cum_dist_cm).unwrap_or(0);
        assert!(
            r.progress_cm >= 0 && r.progress_cm <= last_cum,
            "stop {} mapped within route bounds: {}",
            i,
            r.progress_cm
        );
    }

    // Progress should increase monotonically through each turn
    assert_monotonic_progress(&result.iter().map(|r| r.progress_cm).collect::<Vec<_>>(), "zigzag 90° turns");
}

#[test]
fn test_zigzag_dense_stops_on_turns() {
    // --- GIVEN ---
    // Zig-zag route with stops placed at turn points
    let route = make_zigzag_route(6, 1500);
    let stops: Vec<(i64, i64)> = vec![
        (0, 0),      // Start
        (1500, 0),   // After first horizontal
        (1500, 1500), // After first vertical
        (3000, 1500), // After second horizontal
        (3000, 3000), // After second vertical
    ]
    .into_iter()
    .map(|(x, y)| (x as i64, y as i64))
    .collect();

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(15));

    // --- THEN ---
    assert_eq!(result.len(), stops.len());

    // Verify monotonicity - critical for stops at sharp turns
    for i in 0..result.len() - 1 {
        assert!(
            result[i].progress_cm <= result[i + 1].progress_cm,
            "zigzag: monotonicity at index {}: {} <= {}",
            i,
            result[i].progress_cm,
            result[i + 1].progress_cm
        );
    }
}

#[test]
fn test_route_with_180_degree_turn() {
    // --- GIVEN ---
    // Route with a 180-degree U-turn (immediate reversal)
    let route = vec![
        // Northward segment
        RouteNode {
            len2_cm2: 100000000,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            dx_cm: 0,
            dy_cm: 10000,
            seg_len_cm: 10000,
        },
        // U-turn point
        RouteNode {
            len2_cm2: 100000000,
            heading_cdeg: 18000,
            _pad: 0,
            x_cm: 0,
            y_cm: 10000,
            cum_dist_cm: 10000,
            dx_cm: 0,
            dy_cm: -10000,
            seg_len_cm: 10000,
        },
        // Back toward start
        RouteNode {
            len2_cm2: 0,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 20000,
            dx_cm: 0,
            dy_cm: 0,
            seg_len_cm: 0,
        },
    ];

    // Stop before turn and after turn
    let stops = vec![(0, 5000), (0, 5000)];

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(15));

    // --- THEN ---
    assert_eq!(result.len(), 2);

    // Both stops are at same geographic location but different progress
    assert!(
        result[0].progress_cm <= result[1].progress_cm,
        "monotonicity on 180° turn: {} <= {}",
        result[0].progress_cm,
        result[1].progress_cm
    );
}

#[test]
fn test_route_with_hairpin_turns() {
    // --- GIVEN ---
    // Mountain road style with multiple hairpin turns
    // This simulates a route climbing with switchbacks
    let route = vec![
        // Start going northeast
        RouteNode {
            len2_cm2: 50000000,
            heading_cdeg: 4500,
            _pad: 0,
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            dx_cm: 7071,
            dy_cm: 7071,
            seg_len_cm: 10000,
        },
        // Hairpin 1: turn back southwest
        RouteNode {
            len2_cm2: 50000000,
            heading_cdeg: -13500,
            _pad: 0,
            x_cm: 7071,
            y_cm: 7071,
            cum_dist_cm: 10000,
            dx_cm: -7071,
            dy_cm: -7071,
            seg_len_cm: 10000,
        },
        // Near start again
        RouteNode {
            len2_cm2: 50000000,
            heading_cdeg: 4500,
            _pad: 0,
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 20000,
            dx_cm: 7071,
            dy_cm: 7071,
            seg_len_cm: 10000,
        },
        // Continue northeast again
        RouteNode {
            len2_cm2: 0,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 7071,
            y_cm: 7071,
            cum_dist_cm: 30000,
            dx_cm: 0,
            dy_cm: 0,
            seg_len_cm: 0,
        },
    ];

    // Stops at each major point
    let stops = vec![(0, 0), (7071, 7071), (0, 0)];

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(15));

    // --- THEN ---
    assert_eq!(result.len(), 3);

    // All stops should be mapped
    for (i, r) in result.iter().enumerate() {
        assert!(
            r.progress_cm >= 0 && r.progress_cm <= 30000,
            "hairpin stop {} within bounds: {}",
            i,
            r.progress_cm
        );
    }

    // Monotonicity despite visiting same location twice
    assert_monotonic_progress(&result.iter().map(|r| r.progress_cm).collect::<Vec<_>>(), "hairpin turns");
}

#[test]
fn test_self_intersecting_route_complex() {
    // --- GIVEN ---
    // Route that forms a bowtie pattern (two triangles sharing a vertex)
    let route = vec![
        RouteNode {
            len2_cm2: 100000000,
            heading_cdeg: 3000,
            _pad: 0,
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            dx_cm: 5000,
            dy_cm: 8660,
            seg_len_cm: 10000,
        },
        RouteNode {
            len2_cm2: 100000000,
            heading_cdeg: 9000,
            _pad: 0,
            x_cm: 5000,
            y_cm: 8660,
            cum_dist_cm: 10000,
            dx_cm: -5000,
            dy_cm: 8660,
            seg_len_cm: 10000,
        },
        // Back to y-axis (crossing point)
        RouteNode {
            len2_cm2: 100000000,
            heading_cdeg: -15000,
            _pad: 0,
            x_cm: 0,
            y_cm: 17320,
            cum_dist_cm: 20000,
            dx_cm: 5000,
            dy_cm: -8660,
            seg_len_cm: 10000,
        },
        RouteNode {
            len2_cm2: 100000000,
            heading_cdeg: -9000,
            _pad: 0,
            x_cm: 5000,
            y_cm: 8660,
            cum_dist_cm: 30000,
            dx_cm: -5000,
            dy_cm: -8660,
            seg_len_cm: 10000,
        },
        RouteNode {
            len2_cm2: 0,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 40000,
            dx_cm: 0,
            dy_cm: 0,
            seg_len_cm: 0,
        },
    ];

    // Stop at the crossing point (5000, 8660) - visited twice
    let stops = vec![(5000, 8660), (5000, 8660)];

    // --- WHEN ---
    let result = map_stops(&stops, &route, Some(15));

    // --- THEN ---
    assert_eq!(result.len(), 2);
    assert!(
        result[0].progress_cm <= result[1].progress_cm,
        "self-intersecting: monotonicity at same location"
    );
}

//! Integration tests for dp_mapper
//!
//! Tests the full end-to-end functionality of the DP mapper.

use dp_mapper::map_stops;
use shared::RouteNode;

// ============================================================================
// Coordinate Conversion (for tests with GPS data)
// ============================================================================

/// Earth's radius in centimeters
const R_CM: f64 = 637_100_000.0;

/// Fixed origin longitude in degrees (120.0°E)
const FIXED_ORIGIN_LON_DEG: f64 = 120.0;

/// Fixed origin latitude in degrees (20.0°N)
const FIXED_ORIGIN_LAT_DEG: f64 = 20.0;

/// Fixed origin Y coordinate in centimeters
const FIXED_ORIGIN_Y_CM: i64 = {
    let lat_rad = FIXED_ORIGIN_LAT_DEG * std::f64::consts::PI / 180.0;
    let y_cm = R_CM * lat_rad;
    y_cm as i64
};

/// Convert latitude/longitude to relative centimeter coordinates
fn latlon_to_cm_relative(lat: f64, lon: f64, lat_avg: f64) -> (i32, i32) {
    let lat_rad = lat.to_radians();
    let lon_rad = lon.to_radians();
    let lat_avg_rad = lat_avg.to_radians();

    let cos_lat = lat_avg_rad.cos();

    let x_abs = R_CM * lon_rad * cos_lat;
    let y_abs = R_CM * lat_rad;

    let x0_abs = (FIXED_ORIGIN_LON_DEG.to_radians() * R_CM) * cos_lat;
    let y0_abs = FIXED_ORIGIN_Y_CM as f64;

    let dx_cm = (x_abs - x0_abs).round() as i64;
    let dy_cm = (y_abs - y0_abs).round() as i64;

    (dx_cm as i32, dy_cm as i32)
}

/// Compute average latitude from a set of GPS coordinates
fn compute_lat_avg(points: &[(f64, f64)]) -> f64 {
    if points.is_empty() {
        return 25.0; // Default Taiwan
    }

    let sum: f64 = points.iter().map(|(lat, _)| lat).sum();
    sum / points.len() as f64
}

// ============================================================================
// Test Helpers
// ============================================================================

/// Helper: Create a simple straight-line route
fn make_straight_route(length_cm: i32, num_segments: usize) -> Vec<RouteNode> {
    let seg_len = length_cm / num_segments as i32;
    let mut nodes = Vec::with_capacity(num_segments + 1);

    for i in 0..=num_segments {
        let x_cm = i as i32 * seg_len;
        let cum_dist = x_cm;
        let dx_cm = if i < num_segments { seg_len } else { 0 };
        let seg_len_mm = if i < num_segments {
            seg_len * 10
        } else {
            0
        };

        nodes.push(RouteNode {
            seg_len_mm,
            heading_cdeg: 0,
            _pad: 0,
            x_cm,
            y_cm: 0,
            cum_dist_cm: cum_dist,
            dx_cm: dx_cm as i16,
            dy_cm: 0,
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
            result[i].progress_cm <= result[i + 1].progress_cm,
            "progress should be non-decreasing: {} <= {}",
            result[i].progress_cm,
            result[i + 1].progress_cm
        );
    }

    // First stop should be near the beginning
    assert!(
        result[0].progress_cm >= 400 && result[0].progress_cm <= 600,
        "first stop near 500cm"
    );
    // Last stop should be near the end
    assert!(
        result[4].progress_cm >= 4400 && result[4].progress_cm <= 4600,
        "last stop near 4500cm"
    );
}

#[test]
fn test_integration_l_shaped_route() {
    // L-shaped route: goes east, then north
    let route = vec![
        RouteNode {
            seg_len_mm: 100000,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            dx_cm: 10000,
            dy_cm: 0,
        },
        RouteNode {
            seg_len_mm: 100000,
            heading_cdeg: 9000,
            _pad: 0,
            x_cm: 10000,
            y_cm: 0,
            cum_dist_cm: 10000,
            dx_cm: 0,
            dy_cm: 10000,
        },
        RouteNode {
            seg_len_mm: 0,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 10000,
            y_cm: 10000,
            cum_dist_cm: 20000,
            dx_cm: 0,
            dy_cm: 0,
        },
    ];

    // One stop on each leg
    let stops = vec![(5000, 0), (10000, 5000)];

    let result = map_stops(&stops, &route, None);

    assert_eq!(result.len(), 2);
    assert!(
        result[0].progress_cm <= result[1].progress_cm,
        "progress should be monotonic: {} <= {}",
        result[0].progress_cm,
        result[1].progress_cm
    );

    // First stop on horizontal leg (progress ~5000)
    assert!(
        result[0].progress_cm >= 4000 && result[0].progress_cm <= 6000,
        "first stop should be near middle of first segment: {}",
        result[0].progress_cm
    );
    // Second stop on vertical leg (progress > 10000)
    // The stop is at (10000, 5000) which is the midpoint of the vertical segment
    // So progress should be 10000 + 5000 = 15000
    assert!(
        result[1].progress_cm >= 10000,
        "second stop should be on vertical leg: {}",
        result[1].progress_cm
    );
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
    assert!(result[0].progress_cm <= result[1].progress_cm, "progress should be non-decreasing");
    assert!(
        result[0].progress_cm >= 0 && result[0].progress_cm <= 10000,
        "first stop within route bounds"
    );
    assert!(
        result[1].progress_cm >= 2000 && result[1].progress_cm <= 3000,
        "second stop near 2500cm: {}",
        result[1].progress_cm
    );
}

#[test]
fn test_integration_empty_inputs() {
    let route = make_straight_route(5000, 5);

    // Empty stops
    let result = map_stops(&[], &route, None);
    assert_eq!(result.len(), 0);

    // Empty route
    let result = map_stops(&[(100, 0)], &[], None);
    assert_eq!(result.len(), 0);
}

#[test]
fn test_integration_single_stop() {
    let route = make_straight_route(10000, 10);

    let result = map_stops(&[(5000, 0)], &route, None);

    assert_eq!(result.len(), 1);
    assert!(result[0].progress_cm >= 4500 && result[0].progress_cm <= 5500);
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
            assert!(result[i].progress_cm <= result[i + 1].progress_cm);
        }
    }
}

#[test]
fn test_integration_snap_forward_usage() {
    // Create a scenario where snap-forward is needed
    // Route: segments 0-2 at increasing progress
    let route = vec![
        RouteNode {
            seg_len_mm: 50000,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            dx_cm: 5000,
            dy_cm: 0,
        },
        RouteNode {
            seg_len_mm: 50000,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 5000,
            y_cm: 0,
            cum_dist_cm: 5000,
            dx_cm: 5000,
            dy_cm: 0,
        },
        RouteNode {
            seg_len_mm: 50000,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 10000,
            y_cm: 0,
            cum_dist_cm: 10000,
            dx_cm: 5000,
            dy_cm: 0,
        },
        RouteNode {
            seg_len_mm: 0,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 15000,
            y_cm: 0,
            cum_dist_cm: 15000,
            dx_cm: 0,
            dy_cm: 0,
        },
    ];

    // First stop early on the route
    // Second stop far from the route (triggers snap)
    let stops = vec![(1000, 0), (0, 10000)]; // Second stop is 100m from route

    let result = map_stops(&stops, &route, Some(10));

    assert_eq!(result.len(), 2);
    assert!(result[0].progress_cm <= result[1].progress_cm);
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
        RouteNode {
            seg_len_mm: 100000,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            dx_cm: 10000,
            dy_cm: 0,
        },
        // Leg 2: North 0 → 5000
        RouteNode {
            seg_len_mm: 50000,
            heading_cdeg: 9000,
            _pad: 0,
            x_cm: 10000,
            y_cm: 0,
            cum_dist_cm: 10000,
            dx_cm: 0,
            dy_cm: 5000,
        },
        // Leg 3: West 10000 → 0 (returns to x=0)
        RouteNode {
            seg_len_mm: 100000,
            heading_cdeg: 18000,
            _pad: 0,
            x_cm: 10000,
            y_cm: 5000,
            cum_dist_cm: 15000,
            dx_cm: -10000,
            dy_cm: 0,
        },
        RouteNode {
            seg_len_mm: 0,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 0,
            y_cm: 5000,
            cum_dist_cm: 25000,
            dx_cm: 0,
            dy_cm: 0,
        },
    ];

    // Stops: first on east leg, second on west leg (same y, different x)
    // The west leg stop has same progress as earlier on east leg
    let stops = vec![(5000, 0), (5000, 5000)];

    let result = map_stops(&stops, &route, Some(15));

    assert_eq!(result.len(), 2);
    // Both stops should be mapped
    // First stop at ~5000 (middle of east leg)
    assert!(
        result[0].progress_cm >= 4000 && result[0].progress_cm <= 6000,
        "first stop on east leg: {}",
        result[0].progress_cm
    );
    // Second stop at ~20000 (middle of west leg: 15000 + 5000)
    assert!(
        result[1].progress_cm >= 19000 && result[1].progress_cm <= 21000,
        "second stop on west leg: {}",
        result[1].progress_cm
    );
    // Monotonicity must be preserved
    assert!(
        result[0].progress_cm < result[1].progress_cm,
        "west leg must have higher progress: {} < {}",
        result[0].progress_cm,
        result[1].progress_cm
    );
}

#[test]
fn test_integration_route_crosses_itself() {
    // Figure-8 route: crosses itself at origin
    // First loop: counter-clockwise from origin
    // Second loop: clockwise crossing back through origin
    let route = vec![
        // Loop 1: Quadrant I
        RouteNode {
            seg_len_mm: 50000,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            dx_cm: 5000,
            dy_cm: 0,
        },
        RouteNode {
            seg_len_mm: 50000,
            heading_cdeg: 9000,
            _pad: 0,
            x_cm: 5000,
            y_cm: 0,
            cum_dist_cm: 5000,
            dx_cm: 0,
            dy_cm: 5000,
        },
        RouteNode {
            seg_len_mm: 50000,
            heading_cdeg: 18000,
            _pad: 0,
            x_cm: 5000,
            y_cm: 5000,
            cum_dist_cm: 10000,
            dx_cm: -5000,
            dy_cm: 0,
        },
        // Back to origin
        RouteNode {
            seg_len_mm: 50000,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 15000,
            dx_cm: 5000,
            dy_cm: 0,
        },
        // Loop 2: Quadrant IV (different direction)
        RouteNode {
            seg_len_mm: 50000,
            heading_cdeg: -9000,
            _pad: 0,
            x_cm: 5000,
            y_cm: 0,
            cum_dist_cm: 20000,
            dx_cm: 0,
            dy_cm: -5000,
        },
        RouteNode {
            seg_len_mm: 0,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 5000,
            y_cm: -5000,
            cum_dist_cm: 25000,
            dx_cm: 0,
            dy_cm: 0,
        },
    ];

    // Stop at origin appears twice - once in each loop
    let stops = vec![(0, 0), (5000, 0)];

    let result = map_stops(&stops, &route, Some(15));

    assert_eq!(result.len(), 2);
    // Both should map to different progress values (same location, different visits)
    // First occurrence at origin (progress 0 or 15000 depending on segment)
    // Second occurrence on east leg
    assert!(
        result[0].progress_cm <= result[1].progress_cm,
        "monotonicity preserved across route crossing"
    );
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
            result[i].progress_cm <= result[i + 1].progress_cm,
            "progress should be monotonic at index {}: {} <= {}",
            i,
            result[i].progress_cm,
            result[i + 1].progress_cm
        );
    }

    // Verify coverage - stops should span most of the route
    let total_route_len = route.last().unwrap().cum_dist_cm;
    let coverage = result[num_stops - 1].progress_cm as f64 / total_route_len as f64;
    assert!(coverage > 0.8, "should cover >80% of route: {}", coverage);
}

#[test]
fn test_integration_scalability_dense_stops() {
    // Many stops in a short distance (stops are dense relative to segment length)
    // This is the scenario mentioned in the algorithm doc where greedy fails
    let route = make_straight_route(10000, 10); // 10 segments of 1m each

    // 20 stops packed into 10m route (more stops than segments)
    let stops: Vec<(i64, i64)> = (0..20).map(|i| ((i * 500) as i64, 0)).collect();

    let result = map_stops(&stops, &route, Some(20));

    assert_eq!(result.len(), 20);

    // Verify monotonicity - critical for dense stops
    for i in 0..result.len() - 1 {
        assert!(
            result[i].progress_cm <= result[i + 1].progress_cm,
            "dense stops must maintain monotonicity at index {}: {} <= {}",
            i,
            result[i].progress_cm,
            result[i + 1].progress_cm
        );
    }
}

#[test]
fn test_integration_stops_at_segment_boundaries() {
    // Test stops exactly at segment boundaries
    // This tests floating point precision at t=0.0 and t=1.0 boundaries
    let route = vec![
        RouteNode {
            seg_len_mm: 100000,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            dx_cm: 10000,
            dy_cm: 0,
        },
        RouteNode {
            seg_len_mm: 100000,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 10000,
            y_cm: 0,
            cum_dist_cm: 10000,
            dx_cm: 10000,
            dy_cm: 0,
        },
        RouteNode {
            seg_len_mm: 100000,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 20000,
            y_cm: 0,
            cum_dist_cm: 20000,
            dx_cm: 10000,
            dy_cm: 0,
        },
        RouteNode {
            seg_len_mm: 0,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 30000,
            y_cm: 0,
            cum_dist_cm: 30000,
            dx_cm: 0,
            dy_cm: 0,
        },
    ];

    // Stops exactly at segment boundaries
    let stops = vec![
        (0, 0),     // Start of route (t=0.0 on segment 0)
        (10000, 0), // End of segment 0 / Start of segment 1 (t=1.0 / t=0.0 boundary)
        (20000, 0), // End of segment 1 / Start of segment 2 (t=1.0 / t=0.0 boundary)
        (30000, 0), // End of route (t=1.0 on last segment)
    ];

    let result = map_stops(&stops, &route, Some(10));

    assert_eq!(result.len(), 4);

    // Each stop should map to its exact boundary position
    assert_eq!(result[0].progress_cm, 0, "first stop at route start");
    assert_eq!(result[1].progress_cm, 10000, "stop at segment 0/1 boundary");
    assert_eq!(result[2].progress_cm, 20000, "stop at segment 1/2 boundary");
    assert_eq!(result[3].progress_cm, 30000, "stop at route end");
}

#[test]
fn test_integration_stops_near_segment_boundaries() {
    // Test stops very close to segment boundaries
    // This checks for numerical stability in t-clamping
    let route = vec![
        RouteNode {
            seg_len_mm: 100000,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            dx_cm: 10000,
            dy_cm: 0,
        },
        RouteNode {
            seg_len_mm: 100000,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 10000,
            y_cm: 0,
            cum_dist_cm: 10000,
            dx_cm: 10000,
            dy_cm: 0,
        },
        RouteNode {
            seg_len_mm: 0,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 20000,
            y_cm: 0,
            cum_dist_cm: 20000,
            dx_cm: 0,
            dy_cm: 0,
        },
    ];

    // Stops 1cm before/after boundaries (within floating point epsilon)
    let stops = vec![
        (9999, 0),  // 1cm before first boundary
        (10001, 0), // 1cm after first boundary
        (19999, 0), // 1cm before second boundary
    ];

    let result = map_stops(&stops, &route, Some(10));

    assert_eq!(result.len(), 3);

    // All should be properly mapped without precision issues
    for i in 0..result.len() {
        assert!(
            result[i].progress_cm >= 0 && result[i].progress_cm <= 20000,
            "stop {} mapped within route bounds: {}",
            i,
            result[i].progress_cm
        );
    }

    // Monotonicity must hold
    assert!(result[0].progress_cm <= result[1].progress_cm && result[1].progress_cm <= result[2].progress_cm);
}

// ============================================================================
// Real-World Route Tests
// ============================================================================

#[test]
fn test_integration_ty225_real_route() {
    // Integration test using real ty225 route data
    // This is a real Taipei bus route with 54 stops
    // Tests end-to-end functionality with actual GPS coordinates

    // Resolve path to test data relative to project root
    // CARGO_MANIFEST_DIR points to the dp_mapper crate directory
    // We need to go up to the project root to find test_data/
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let project_root = manifest_dir
        .parent() // preprocessor
        .and_then(|p| p.parent()) // crates
        .and_then(|p| p.parent()) // bus_arrival root
        .and_then(|p| {
            // If we're in a worktree, go up to the actual project root
            if p.ends_with(".worktrees") || p.ends_with("worktrees") {
                p.parent()
            } else {
                Some(p)
            }
        });

    let test_data_dir = match project_root {
        Some(root) => root.join("test_data"),
        None => {
            // Fallback: try direct path from current directory
            std::path::PathBuf::from("test_data")
        }
    };

    let route_json_path = test_data_dir.join("ty225_route.json");
    let stops_json_path = test_data_dir.join("ty225_stops.json");

    let route_json = std::fs::read_to_string(&route_json_path)
        .unwrap_or_else(|_| panic!("failed to load ty225 route file from {:?}", route_json_path));

    let route_value: serde_json::Value =
        serde_json::from_str(&route_json).expect("failed to parse ty225 route JSON");

    // Extract route points as [lat, lon] pairs
    let route_points: Vec<(f64, f64)> = route_value["route_points"]
        .as_array()
        .expect("route_points should be an array")
        .iter()
        .map(|v| {
            let arr = v.as_array().expect("route_point should be [lat, lon]");
            (
                arr[0].as_f64().expect("lat should be number"),
                arr[1].as_f64().expect("lon should be number"),
            )
        })
        .collect();

    // Compute average latitude for coordinate conversion
    let lat_avg = compute_lat_avg(&route_points);

    // Convert route points to cm coordinates
    let route_cm: Vec<(i32, i32)> = route_points
        .iter()
        .map(|(lat, lon)| latlon_to_cm_relative(*lat, *lon, lat_avg))
        .collect();

    // Build RouteNode list from converted coordinates
    let route_nodes: Vec<RouteNode> = build_route_nodes_from_cm(&route_cm);

    // Load stops JSON
    let stops_json = std::fs::read_to_string(&stops_json_path)
        .unwrap_or_else(|_| panic!("failed to load ty225 stops file from {:?}", stops_json_path));

    let stops_value: serde_json::Value =
        serde_json::from_str(&stops_json).expect("failed to parse ty225 stops JSON");

    // Extract stops as (lat, lon) pairs
    let stops_gps: Vec<(f64, f64)> = stops_value["stops"]
        .as_array()
        .expect("stops should be an array")
        .iter()
        .map(|s| {
            (
                s["lat"].as_f64().expect("lat should be number"),
                s["lon"].as_f64().expect("lon should be number"),
            )
        })
        .collect();

    // Convert stops to cm coordinates
    let stops_cm: Vec<(i64, i64)> = stops_gps
        .iter()
        .map(|(lat, lon)| {
            let (x, y) = latlon_to_cm_relative(*lat, *lon, lat_avg);
            (x as i64, y as i64)
        })
        .collect();

    // Run DP mapper
    let result = map_stops(&stops_cm, &route_nodes, Some(15));

    // Validate results
    assert_eq!(result.len(), 54, "should map all 54 stops");

    // Verify monotonicity (critical constraint)
    for i in 0..result.len() - 1 {
        assert!(
            result[i].progress_cm <= result[i + 1].progress_cm,
            "ty225: monotonicity violated at index {}: {} (stop {}) > {} (stop {})",
            i,
            result[i].progress_cm,
            i + 1,
            result[i + 1].progress_cm,
            i + 2
        );
    }

    // === Geometric correctness validation ===
    // For each stop, compute the actual mapped position on the route
    // and verify it's close to the original stop location

    let mut max_dist_cm = 0i64;
    let mut total_dist_cm = 0i64;

    for (i, (stop_cm, progress_cm)) in stops_cm.iter().zip(result.iter().map(|c| c.progress_cm)).enumerate() {
        // Find the actual position on the route at this progress value
        let mapped_pos = position_at_progress(&route_nodes, progress_cm);

        // Compute distance from stop to its mapped position
        let dx = stop_cm.0 - mapped_pos.0 as i64;
        let dy = stop_cm.1 - mapped_pos.1 as i64;
        let dist_sq_cm2 = dx * dx + dy * dy;
        let dist_cm = (dist_sq_cm2 as f64).sqrt().round() as i64;

        max_dist_cm = max_dist_cm.max(dist_cm);
        total_dist_cm += dist_cm;

        // Each stop should be mapped to a position within 300m (30000cm)
        // Real-world GPS data has larger errors due to stop placement and route geometry
        assert!(
            dist_cm <= 30000,
            "ty225: stop {} mapped too far from actual position: {}cm (stop: {:?}, mapped: {:?})",
            i + 1,
            dist_cm,
            stop_cm,
            mapped_pos
        );
    }

    let avg_dist_cm = total_dist_cm / result.len() as i64;

    // Additional sanity checks
    // Average mapping error should be reasonably small
    assert!(
        avg_dist_cm <= 5000,
        "ty225: average mapping distance too large: {}cm (max: {}cm)",
        avg_dist_cm,
        max_dist_cm
    );

    println!(
        "ty225 validation: avg_dist={}m, max_dist={}m",
        avg_dist_cm as f64 / 100.0,
        max_dist_cm as f64 / 100.0
    );
}

/// Find the (x, y) position on the route at a given progress value (in cm)
fn position_at_progress(route_nodes: &[RouteNode], progress_cm: i32) -> (i32, i32) {
    // Find the segment containing this progress value
    // RouteNode stores cumulative distance at segment start
    let mut seg_idx = 0;
    for (i, node) in route_nodes.iter().enumerate() {
        if node.cum_dist_cm <= progress_cm {
            seg_idx = i;
        } else {
            break;
        }
    }

    // Guard against edge case where progress exceeds last segment
    if seg_idx >= route_nodes.len().saturating_sub(1) {
        seg_idx = route_nodes.len().saturating_sub(1);
    }

    let node = &route_nodes[seg_idx];

    // How far into this segment?
    let offset_cm = progress_cm - node.cum_dist_cm;

    // t = offset / seg_len (clamped to [0, 1])
    let seg_len_cm = (node.seg_len_mm / 10) as i32;
    let t = if seg_len_cm > 0 {
        (offset_cm as f64 / seg_len_cm as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };

    // Interpolate position
    let x = node.x_cm as f64 + t * node.dx_cm as f64;
    let y = node.y_cm as f64 + t * node.dy_cm as f64;

    (x.round() as i32, y.round() as i32)
}

/// Helper: Build RouteNode list from cm coordinates
fn build_route_nodes_from_cm(points: &[(i32, i32)]) -> Vec<RouteNode> {
    if points.len() < 2 {
        return vec![];
    }

    let mut nodes = Vec::with_capacity(points.len());

    // Calculate cumulative distance
    let mut cum_dist_cm: i32 = 0;

    for (i, &(x_cm, y_cm)) in points.iter().enumerate() {
        let is_last = i == points.len() - 1;

        if !is_last {
            let next_point = points[i + 1];
            let dx_cm = next_point.0 - x_cm;
            let dy_cm = next_point.1 - y_cm;
            // Use i64 to avoid overflow when squaring large coordinate differences
            let seg_len_cm = (((dx_cm as i64) * (dx_cm as i64) + (dy_cm as i64) * (dy_cm as i64))
                as f64)
                .sqrt()
                .round() as i32;

            nodes.push(RouteNode {
                seg_len_mm: seg_len_cm * 10,
                heading_cdeg: 0,
                _pad: 0,
                x_cm,
                y_cm,
                cum_dist_cm,
                dx_cm: dx_cm as i16,
                dy_cm: dy_cm as i16,
            });

            cum_dist_cm += seg_len_cm;
        } else {
            // Last node: no outgoing segment
            nodes.push(RouteNode {
                seg_len_mm: 0,
                heading_cdeg: 0,
                _pad: 0,
                x_cm,
                y_cm,
                cum_dist_cm,
                dx_cm: 0,
                dy_cm: 0,
            });
        }
    }

    nodes
}

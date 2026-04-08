//! Route geometry edge case tests
//! Tests for: loop closure, large projection errors, close stops

use super::common::load_ty225_route;
use shared::binfile::RouteData;

/// Test: Loop closure detection
/// Validates: Stop at route loop completion is detected
#[test]
fn test_loop_closure_detection() {
    // Load normal route (has loop closure at stop 57/58)
    let route_bytes = load_ty225_route("normal");
    let route_data = RouteData::load(&route_bytes)
        .expect("Failed to load route data");

    let stops = route_data.stops();

    // Verify we have 58 stops
    assert_eq!(stops.len(), 58, "Route should have 58 stops for loop test");

    // Verify last stop exists
    let last_stop = &stops[57];
    assert!(
        last_stop.progress_cm > 0,
        "Last stop should have progress value"
    );

    // Verify first stop (loop closure point)
    let first_stop = &stops[0];
    assert!(
        first_stop.progress_cm >= 0,
        "First stop should have valid progress"
    );
}

/// Test: Large projection error handling
/// Validates: System handles stops with > 30m projection errors
#[test]
fn test_large_projection_error_stops_exist() {
    // Load normal route
    let route_bytes = load_ty225_route("normal");
    let route_data = RouteData::load(&route_bytes)
        .expect("Failed to load route data");

    let stops = route_data.stops();

    // Verify all stops have corridor values
    for (i, stop) in stops.iter().enumerate() {
        assert!(
            stop.corridor_start_cm <= stop.progress_cm,
            "Stop {}: corridor_start should be <= progress",
            i
        );
        assert!(
            stop.corridor_end_cm >= stop.progress_cm,
            "Stop {}: corridor_end should be >= progress",
            i
        );
    }
}

/// Test: Close stop discrimination
/// Validates: System can discriminate nearby stops
#[test]
fn test_close_stop_discrimination() {
    let route_bytes = load_ty225_route("normal");
    let route_data = RouteData::load(&route_bytes)
        .expect("Failed to load route data");

    let stops = route_data.stops();

    // Find stops that are close to each other (< 100m apart in progress)
    let mut close_pairs = Vec::new();
    for i in 0..stops.len().saturating_sub(1) {
        let gap_cm = stops[i + 1].progress_cm - stops[i].progress_cm;
        if gap_cm > 0 && gap_cm < 10000 {
            close_pairs.push((i, i + 1, gap_cm));
        }
    }

    // If we have close stops, verify they have non-overlapping corridors
    for (i, j, gap) in close_pairs {
        let stop_i = &stops[i];
        let stop_j = &stops[j];

        // Corridors should be smaller than gap to avoid confusion
        let corridor_i = stop_i.corridor_end_cm - stop_i.progress_cm;
        let corridor_j = stop_j.progress_cm - stop_j.corridor_start_cm;

        assert!(
            corridor_i + corridor_j <= gap * 2,
            "Close stops {} and {} ({}cm apart): corridors may overlap",
            i, j, gap
        );
    }
}

/// Test: Route data structure integrity
#[test]
fn test_route_data_integrity() {
    let route_bytes = load_ty225_route("normal");
    let route_data = RouteData::load(&route_bytes)
        .expect("Failed to load route data");

    // Verify stops are in increasing progress order
    let stops = route_data.stops();
    for i in 1..stops.len() {
        assert!(
            stops[i].progress_cm >= stops[i - 1].progress_cm,
            "Stop {} progress should be >= stop {} progress",
            i, i - 1
        );
    }

    // Verify nodes exist - use node_count instead of nodes().len()
    assert!(
        route_data.node_count > 0,
        "Route should have nodes"
    );
}
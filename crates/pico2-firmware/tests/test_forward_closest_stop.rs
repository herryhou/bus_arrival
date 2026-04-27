//! Test forward closest stop index search
//! Run with: cargo test -p pico2-firmware find_forward_closest --features dev

use pico2_firmware::state::State;
use shared::{binfile::RouteData, Stop, RouteNode, SpatialGrid};

/// Create test route data with multiple stops for forward search testing
fn create_route_with_stops() -> RouteData<'static> {
    // Create a simple straight route along X-axis with stops at various positions
    // Route: 0cm → 10,000cm → 20,000cm → 30,000cm → 40,000cm
    // Stops at: 5,000cm (idx 0), 15,000cm (idx 1), 25,000cm (idx 2), 35,000cm (idx 3)
    let nodes = vec![
        RouteNode {
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            seg_len_mm: 100000, // 100m in mm
            dx_cm: 10000,       // 100m
            dy_cm: 0,
            heading_cdeg: 9000, // 90 degrees (East)
            _pad: 0,
        },
        RouteNode {
            x_cm: 10000,
            y_cm: 0,
            cum_dist_cm: 10000,
            seg_len_mm: 100000, // 100m in mm
            dx_cm: 10000,       // 100m
            dy_cm: 0,
            heading_cdeg: 9000,
            _pad: 0,
        },
        RouteNode {
            x_cm: 20000,
            y_cm: 0,
            cum_dist_cm: 20000,
            seg_len_mm: 100000, // 100m in mm
            dx_cm: 10000,       // 100m
            dy_cm: 0,
            heading_cdeg: 9000,
            _pad: 0,
        },
        RouteNode {
            x_cm: 30000,
            y_cm: 0,
            cum_dist_cm: 30000,
            seg_len_mm: 100000, // 100m in mm
            dx_cm: 10000,       // 100m
            dy_cm: 0,
            heading_cdeg: 9000,
            _pad: 0,
        },
        RouteNode {
            x_cm: 40000,
            y_cm: 0,
            cum_dist_cm: 40000,
            seg_len_mm: 0, // Last node
            dx_cm: 0,
            dy_cm: 0,
            heading_cdeg: 9000,
            _pad: 0,
        },
    ];

    let stops = vec![
        Stop {
            progress_cm: 5000,   // Stop 0 at 50m
            corridor_start_cm: 3000,
            corridor_end_cm: 7000,
        },
        Stop {
            progress_cm: 15000,  // Stop 1 at 150m
            corridor_start_cm: 13000,
            corridor_end_cm: 17000,
        },
        Stop {
            progress_cm: 25000,  // Stop 2 at 250m
            corridor_start_cm: 23000,
            corridor_end_cm: 27000,
        },
        Stop {
            progress_cm: 35000,  // Stop 3 at 350m
            corridor_start_cm: 33000,
            corridor_end_cm: 37000,
        },
    ];

    let grid = SpatialGrid {
        cells: vec![vec![0, 1, 2, 3, 4], vec![0, 1, 2, 3, 4]],
        grid_size_cm: 10000,
        cols: 2,
        rows: 2,
        x0_cm: 0,
        y0_cm: 0,
    };

    // Pack route data
    let mut buffer = Vec::new();
    shared::binfile::pack_route_data(&nodes, &stops, &grid, 0.0, &mut buffer)
        .expect("Failed to pack test route data");

    let leaked_buffer = Box::leak(buffer.into_boxed_slice());
    RouteData::load(leaked_buffer).expect("Failed to load route data")
}

#[test]
fn test_find_forward_closest_stop_index_basic() {
    // Test basic forward search: position is closer to stop 1, but we search from stop 2
    let route_data = create_route_with_stops();
    let state = State::new(&route_data, None);

    // Position at 18,000cm (180m)
    // Distance to stops:
    // - Stop 0 (5,000cm): |18000 - 5000| = 13000cm
    // - Stop 1 (15,000cm): |18000 - 15000| = 3000cm  <- CLOSEST overall
    // - Stop 2 (25,000cm): |18000 - 25000| = 7000cm
    // - Stop 3 (35,000cm): |18000 - 35000| = 17000cm

    // Search forward from stop 2 (index 2)
    // Should only consider stops 2 and 3, so stop 2 should be selected
    let result = state.find_forward_closest_stop_index(18000, 2);

    assert_eq!(result, 2, "Forward search from index 2 should find stop 2, not stop 1");
}

#[test]
fn test_find_forward_closest_stop_index_edge_case() {
    // Test edge case: position is EXACTLY at last_idx
    let route_data = create_route_with_stops();
    let state = State::new(&route_data, None);

    // Position at 15,000cm (exactly at stop 1)
    // Search forward from stop 1 (index 1)
    // Should select stop 1 since it's at the search start
    let result = state.find_forward_closest_stop_index(15000, 1);

    assert_eq!(result, 1, "Forward search from index 1 at stop 1 should find stop 1");
}

#[test]
fn test_find_forward_closest_stop_index_last_stop() {
    // Test searching from the last stop
    let route_data = create_route_with_stops();
    let state = State::new(&route_data, None);

    // Position at 40,000cm (end of route)
    // Search forward from last stop (index 3)
    let result = state.find_forward_closest_stop_index(40000, 3);

    assert_eq!(result, 3, "Forward search from last stop should return last stop");
}

#[test]
fn test_find_forward_closest_stop_index_mid_range() {
    // Test forward search in the middle of the route
    let route_data = create_route_with_stops();
    let state = State::new(&route_data, None);

    // Position at 28,000cm (280m)
    // Distance to stops from index 1:
    // - Stop 1 (15,000cm): |28000 - 15000| = 13000cm
    // - Stop 2 (25,000cm): |28000 - 25000| = 3000cm  <- CLOSEST from index 1
    // - Stop 3 (35,000cm): |28000 - 35000| = 7000cm

    let result = state.find_forward_closest_stop_index(28000, 1);

    assert_eq!(result, 2, "Forward search from index 1 should find stop 2");
}

#[test]
fn test_find_forward_closest_stop_index_prevents_backward_selection() {
    // Test that forward search NEVER selects a stop before last_idx
    let route_data = create_route_with_stops();
    let state = State::new(&route_data, None);

    // Position at 6,000cm (60m)
    // Distance to all stops:
    // - Stop 0 (5,000cm): |6000 - 5000| = 1000cm  <- CLOSEST overall
    // - Stop 1 (15,000cm): |6000 - 15000| = 9000cm
    // - Stop 2 (25,000cm): |6000 - 25000| = 19000cm
    // - Stop 3 (35,000cm): |6000 - 35000| = 29000cm

    // Search forward from stop 1 (index 1)
    // Should NOT select stop 0 (which is closest overall)
    // Should select stop 1 (the first stop in the search range)
    let result = state.find_forward_closest_stop_index(6000, 1);

    assert_eq!(result, 1, "Forward search from index 1 should NOT select stop 0 (before last_idx)");
}

#[test]
fn test_find_forward_closest_vs_full_search() {
    // Compare forward search with full search to verify different behavior
    let route_data = create_route_with_stops();
    let state = State::new(&route_data, None);

    // Position at 8,000cm (80m)
    // Distance to all stops:
    // - Stop 0 (5,000cm): |8000 - 5000| = 3000cm   <- CLOSEST overall
    // - Stop 1 (15,000cm): |8000 - 15000| = 7000cm
    // - Stop 2 (25,000cm): |8000 - 25000| = 17000cm
    // - Stop 3 (35,000cm): |8000 - 35000| = 27000cm

    // Full search should find stop 0 (closest overall)
    let full_result = state.find_closest_stop_index(8000);
    assert_eq!(full_result, 0, "Full search should find stop 0 (closest overall)");

    // Forward search from index 1 should find stop 1 (not stop 0)
    let forward_result = state.find_forward_closest_stop_index(8000, 1);
    assert_eq!(forward_result, 1, "Forward search from index 1 should find stop 1, not stop 0");
}

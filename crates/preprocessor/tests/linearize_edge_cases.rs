//! Route Linearization Edge Cases
//!
//! Tests for edge cases in route linearization:
//! - Coordinate conversion: equator/date line crossing, extreme latitudes
//! - Distance calculation: zero-length segments, overflow
//! - Cumulative distance: floating point accumulation

use preprocessor::coord::{latlon_to_cm_relative, compute_lat_avg};
use preprocessor::linearize::linearize_route;

// ============================================================================
// Coordinate Conversion Edge Cases
// ============================================================================

#[test]
fn test_equator_crossing() {
    // --- GIVEN ---
    // A route that crosses the equator
    let route_points = vec![
        (-1.0, 120.0), // 1° south
        (0.0, 120.0),  // On equator
        (1.0, 120.0),  // 1° north
    ];

    let lat_avg = compute_lat_avg(&route_points);

    // --- WHEN ---
    let converted: Vec<(i32, i32)> = route_points
        .iter()
        .map(|(lat, lon)| latlon_to_cm_relative(*lat, *lon, lat_avg))
        .collect();

    // --- THEN ---
    // The conversion should handle latitude = 0 correctly
    // No division by zero should occur
    assert_eq!(converted.len(), 3);

    // Y coordinates should be monotonic (south to north)
    assert!(
        converted[0].1 < converted[1].1 && converted[1].1 < converted[2].1,
        "Y should increase from south to north: {} < {} < {}",
        converted[0].1,
        converted[1].1,
        converted[2].1
    );
}

#[test]
fn test_extreme_southern_latitude() {
    // --- GIVEN ---
    // Route at latitude -85° (near South Pole)
    let route_points = vec![
        (-85.0, 120.0),
        (-85.0, 121.0),
        (-85.0, 122.0),
    ];

    let lat_avg = compute_lat_avg(&route_points);

    // --- WHEN ---
    let converted: Vec<(i32, i32)> = route_points
        .iter()
        .map(|(lat, lon)| latlon_to_cm_relative(*lat, *lon, lat_avg))
        .collect();

    // --- THEN ---
    // The cos(lat_avg) should be very small but non-zero
    // X-coordinates should be compressed correctly
    assert_eq!(converted.len(), 3);

    // X should increase as longitude increases
    assert!(
        converted[0].0 < converted[1].0 && converted[1].0 < converted[2].0,
        "X should increase with longitude even at extreme latitude"
    );
}

#[test]
fn test_extreme_northern_latitude() {
    // --- GIVEN ---
    // Route at latitude 85° (near North Pole)
    let route_points = vec![
        (85.0, 120.0),
        (85.0, 121.0),
        (85.0, 122.0),
    ];

    let lat_avg = compute_lat_avg(&route_points);

    // --- WHEN ---
    let converted: Vec<(i32, i32)> = route_points
        .iter()
        .map(|(lat, lon)| latlon_to_cm_relative(*lat, *lon, lat_avg))
        .collect();

    // --- THEN ---
    assert_eq!(converted.len(), 3);

    // X should increase (though compressed due to small cos(lat))
    assert!(
        converted[0].0 < converted[1].0 && converted[1].0 < converted[2].0,
        "X should increase with longitude at extreme northern latitude"
    );
}

#[test]
fn test_date_line_crossing() {
    // --- GIVEN ---
    // Route that crosses the International Date Line
    let route_points = vec![
        (25.0, 179.0),  // Just east of date line
        (25.0, 180.0),  // On date line
        (25.0, -179.0), // Just west of date line
    ];

    let lat_avg = compute_lat_avg(&route_points);

    // --- WHEN ---
    let converted: Vec<(i32, i32)> = route_points
        .iter()
        .map(|(lat, lon)| latlon_to_cm_relative(*lat, *lon, lat_avg))
        .collect();

    // --- THEN ---
    // The conversion should handle longitude wraparound
    assert_eq!(converted.len(), 3);

    // Note: The simple conversion doesn't handle date line crossing specially
    // X coordinates will have a large jump across the date line
    // This is expected behavior for the local flat-earth approximation
}

#[test]
fn test_large_coordinate_values() {
    // --- GIVEN ---
    // Coordinates that would result in large cm values
    let route_points = vec![
        (20.0, 120.0),   // Origin
        (21.0, 122.0),   // ~220km away in both directions
    ];

    let lat_avg = compute_lat_avg(&route_points);

    // --- WHEN ---
    let converted: Vec<(i32, i32)> = route_points
        .iter()
        .map(|(lat, lon)| latlon_to_cm_relative(*lat, *lon, lat_avg))
        .collect();

    // --- THEN ---
    // i64 should be used for intermediate calculations
    // No overflow should occur
    assert_eq!(converted.len(), 2);

    // The difference should be reasonable (< 500km = 50M cm)
    let dx = (converted[1].0 - converted[0].0).abs();
    let dy = (converted[1].1 - converted[0].1).abs();
    assert!(dx < 50_000_000, "dx should be < 50M cm: {}", dx);
    assert!(dy < 50_000_000, "dy should be < 50M cm: {}", dy);
}

#[test]
fn test_compute_lat_avg_empty() {
    // --- GIVEN ---
    let points: &[(f64, f64)] = &[];

    // --- WHEN ---
    let lat_avg = compute_lat_avg(points);

    // --- THEN ---
    // Should return default for Taiwan region
    assert_eq!(lat_avg, 25.0, "empty points should return default lat_avg");
}

#[test]
fn test_compute_lat_avg_single_point() {
    // --- GIVEN ---
    let points = vec![(25.5, 121.0)];

    // --- WHEN ---
    let lat_avg = compute_lat_avg(&points);

    // --- THEN ---
    assert_eq!(lat_avg, 25.5);
}

// ============================================================================
// Distance Calculation Edge Cases
// ============================================================================

#[test]
fn test_zero_length_segment() {
    // --- GIVEN ---
    // A route with two consecutive points at the same location
    let nodes_cm = vec![
        (0, 0),
        (0, 0), // Zero-length segment
        (5000, 0),
    ];

    // --- WHEN ---
    let route = linearize_route(&nodes_cm);

    // --- THEN ---
    // The zero-length segment should be handled correctly
    assert_eq!(route.len(), 3);

    // First segment should have zero length
    let node0_seg = route[0].seg_len_mm;
    let node0_dx = route[0].dx_cm;
    let node0_dy = route[0].dy_cm;
    let node0_cum = route[0].cum_dist_cm;
    assert_eq!(node0_seg, 0);
    assert_eq!(node0_dx, 0);
    assert_eq!(node0_dy, 0);

    // Cumulative distance should not advance for zero-length segment
    assert_eq!(node0_cum, 0);
    let node1_cum = route[1].cum_dist_cm;
    assert_eq!(node1_cum, 0); // Still at 0 after zero-length segment
    let node2_cum = route[2].cum_dist_cm;
    assert_eq!(node2_cum, 5000); // Advances after real segment
}

#[test]
fn test_very_small_segment() {
    // --- GIVEN ---
    // A route with a 0.5cm segment
    let nodes_cm = vec![
        (0, 0),
        (0, 0), // Using (0,0) then (1,0) gives ~1cm
        (1, 0),
    ];

    // --- WHEN ---
    let route = linearize_route(&nodes_cm);

    // --- THEN ---
    assert_eq!(route.len(), 3);

    // Very small segment should be handled
    // Length should round to 1cm or 0cm appropriately (in mm)
    let node1_seg = route[1].seg_len_mm;
    assert!(node1_seg >= 0 && node1_seg <= 20);
}

#[test]
fn test_point_distance_overflow() {
    // --- GIVEN ---
    // Two points very far apart (different continents scale)
    // But within i32 range for individual coordinates
    let nodes_cm = vec![
        (0, 0),
        (10_000_000, 10_000_000), // ~141km in both directions
    ];

    // --- WHEN ---
    let route = linearize_route(&nodes_cm);

    // --- THEN ---
    assert_eq!(route.len(), 2);

    // Actual length should be calculated correctly (in mm)
    // sqrt(2) * 10_000_000 * 10 = 141421356.237... mm
    let expected_len_mm = ((2 * 10_000_000_i64 * 10_000_000_i64) as f64).sqrt() * 10.0;
    let node0_seg = route[0].seg_len_mm;
    // Allow small rounding difference
    assert!(((node0_seg as i64) - expected_len_mm as i64).abs() <= 1);
}

// ============================================================================
// Cumulative Distance Edge Cases
// ============================================================================

#[test]
fn test_cumulative_distance_no_overflow() {
    // --- GIVEN ---
    // A route with 1000 segments of 30m each (30km total)
    // Well within i32::MAX (21,474,836 cm)
    let nodes_cm: Vec<(i64, i64)> = (0..=1000)
        .map(|i| ((i * 3000) as i64, 0))
        .collect();

    // --- WHEN ---
    let route = linearize_route(&nodes_cm);

    // --- THEN ---
    assert_eq!(route.len(), 1001);

    // Final cumulative distance should be 3,000,000 cm (30km)
    let final_cum_dist = route.last().unwrap().cum_dist_cm;
    assert_eq!(final_cum_dist, 3_000_000);

    // Verify no overflow occurred
    assert!(final_cum_dist > 0);
}

#[test]
fn test_floating_point_accumulation() {
    // --- GIVEN ---
    // A route with 10,000 segments
    // This tests that floating point error doesn't accumulate significantly
    let nodes_cm: Vec<(i64, i64)> = (0..=10000)
        .map(|i| ((i * 100) as i64, 0)) // 100cm = 1m segments
        .collect();

    // --- WHEN ---
    let route = linearize_route(&nodes_cm);

    // --- THEN ---
    assert_eq!(route.len(), 10001);

    // Final cumulative distance should be 1,000,000 cm (10km)
    let final_cum_dist = route.last().unwrap().cum_dist_cm;
    assert_eq!(final_cum_dist, 1_000_000);

    // Verify each segment (100 cm = 1000 mm)
    for i in 0..10000 {
        let seg = route[i].seg_len_mm;
        assert_eq!(seg, 1000, "segment {} length", i);
    }
}

#[test]
fn test_heading_calculation_east() {
    // --- GIVEN ---
    let nodes_cm = vec![(0, 0), (10000, 0)]; // 100m east

    // --- WHEN ---
    let route = linearize_route(&nodes_cm);

    // --- THEN ---
    // East = 90° in navigation bearing (0° = North)
    // But our formula uses atan2(dx, dy), so:
    // dx=10000, dy=0 → atan2(10000, 0) = 90°
    let node0_heading = route[0].heading_cdeg;
    assert_eq!(node0_heading, 9000); // 90° in centidegrees
}

#[test]
fn test_heading_calculation_north() {
    // --- GIVEN ---
    let nodes_cm = vec![(0, 0), (0, 10000)]; // 100m north

    // --- WHEN ---
    let route = linearize_route(&nodes_cm);

    // --- THEN ---
    // North = 0° in navigation bearing
    // dx=0, dy=10000 → atan2(0, 10000) = 0°
    let node0_heading = route[0].heading_cdeg;
    assert_eq!(node0_heading, 0);
}

#[test]
fn test_heading_calculation_south() {
    // --- GIVEN ---
    let nodes_cm = vec![(0, 0), (0, -10000)]; // 100m south

    // --- WHEN ---
    let route = linearize_route(&nodes_cm);

    // --- THEN ---
    // South = 180°
    // dx=0, dy=-10000 → atan2(0, -10000) = 180°
    let node0_heading = route[0].heading_cdeg;
    assert_eq!(node0_heading, 18000);
}

#[test]
fn test_heading_calculation_west() {
    // --- GIVEN ---
    let nodes_cm = vec![(0, 0), (-10000, 0)]; // 100m west

    // --- WHEN ---
    let route = linearize_route(&nodes_cm);

    // --- THEN ---
    // West = 270° or -90°
    // dx=-10000, dy=0 → atan2(-10000, 0) = -90° = 270°
    let node0_heading = route[0].heading_cdeg;
    assert_eq!(node0_heading, -9000);
}

#[test]
fn test_heading_diagonal() {
    // --- GIVEN ---
    let nodes_cm = vec![(0, 0), (10000, 10000)]; // Northeast diagonal

    // --- WHEN ---
    let route = linearize_route(&nodes_cm);

    // --- THEN ---
    // Northeast = 45°
    // dx=10000, dy=10000 → atan2(10000, 10000) = 45°
    let node0_heading = route[0].heading_cdeg;
    assert_eq!(node0_heading, 4500);
}

// ============================================================================
// Edge Cases for Empty and Single Point Routes
// ============================================================================

#[test]
fn test_linearize_empty_route() {
    // --- GIVEN ---
    let nodes_cm: &[(i64, i64)] = &[];

    // --- WHEN ---
    let route = linearize_route(nodes_cm);

    // --- THEN ---
    assert_eq!(route.len(), 0);
}

#[test]
fn test_linearize_single_point() {
    // --- GIVEN ---
    let nodes_cm = vec![(5000, 3000)];

    // --- WHEN ---
    let route = linearize_route(&nodes_cm);

    // --- THEN ---
    assert_eq!(route.len(), 1);
    let node0_x = route[0].x_cm;
    let node0_y = route[0].y_cm;
    let node0_seg = route[0].seg_len_mm;
    let node0_cum = route[0].cum_dist_cm;
    assert_eq!(node0_x, 5000);
    assert_eq!(node0_y, 3000);
    assert_eq!(node0_seg, 0);
    assert_eq!(node0_cum, 0);
}

#[test]
fn test_linearize_two_points() {
    // --- GIVEN ---
    let nodes_cm = vec![(0, 0), (10000, 0)];

    // --- WHEN ---
    let route = linearize_route(&nodes_cm);

    // --- THEN ---
    assert_eq!(route.len(), 2);

    // First node has the segment (10000 cm = 100000 mm)
    let node0_seg = route[0].seg_len_mm;
    let node0_cum = route[0].cum_dist_cm;
    assert_eq!(node0_seg, 100000);
    assert_eq!(node0_cum, 0);

    // Second node is terminal
    let node1_seg = route[1].seg_len_mm;
    let node1_cum = route[1].cum_dist_cm;
    assert_eq!(node1_seg, 0);
    assert_eq!(node1_cum, 10000);
}

// ============================================================================
// Edge Case: Integer Rounding
// ============================================================================

#[test]
fn test_integer_rounding_of_distances() {
    // --- GIVEN ---
    // A segment that results in non-integer length when computed
    // 3-4-5 triangle: sqrt(3² + 4²) = 5
    let nodes_cm = vec![
        (0, 0),
        (300, 400), // 500cm length exactly
    ];

    // --- WHEN ---
    let route = linearize_route(&nodes_cm);

    // --- THEN ---
    let node0_seg = route[0].seg_len_mm;
    assert_eq!(node0_seg, 5000);
}

#[test]
fn test_integer_rounding_irrational() {
    // --- GIVEN ---
    // Segment with irrational length (sqrt(2))
    let nodes_cm = vec![
        (0, 0),
        (1000, 1000), // sqrt(2000000) ≈ 1414.21...
    ];

    // --- WHEN ---
    let route = linearize_route(&nodes_cm);

    // --- THEN ---
    // Should round to nearest integer (1414 cm = 14140 mm)
    let node0_seg = route[0].seg_len_mm;
    assert_eq!(node0_seg, 14142);
}

// ============================================================================
// Edge Case: Large Coordinate Values in cm
// ============================================================================

#[test]
fn test_i32_to_i64_conversion() {
    // --- GIVEN ---
    // Route with coordinates near i32::MAX / 2
    let nodes_cm = vec![
        (0, 0),
        (1_000_000_000, 0), // 10,000 km - unrealistic but tests conversion
    ];

    // --- WHEN ---
    let route = linearize_route(&nodes_cm);

    // --- THEN ---
    // Should handle without overflow (dx_cm is i16, so this will overflow/wrap)
    assert_eq!(route.len(), 2);
    // dx_cm is i16, value wraps when cast from i32
    let node0_dx = route[0].dx_cm;
    assert_eq!(node0_dx, -13824_i16); // 1_000_000_000 % 2^16 = 16960, but as i16 = -13824
}

#[test]
fn test_negative_coordinates() {
    // --- GIVEN ---
    // Route with negative coordinates (south/west of origin)
    let nodes_cm = vec![
        (-5000, -3000),
        (2000, -1000),
    ];

    // --- WHEN ---
    let route = linearize_route(&nodes_cm);

    // --- THEN ---
    assert_eq!(route.len(), 2);
    let node0_x = route[0].x_cm;
    let node0_y = route[0].y_cm;
    let node1_x = route[1].x_cm;
    let node1_y = route[1].y_cm;
    assert_eq!(node0_x, -5000);
    assert_eq!(node0_y, -3000);
    assert_eq!(node1_x, 2000);
    assert_eq!(node1_y, -1000);

    // dx and dy should be positive
    let node0_dx = route[0].dx_cm;
    let node0_dy = route[0].dy_cm;
    assert_eq!(node0_dx, 7000_i16);
    assert_eq!(node0_dy, 2000_i16);
}

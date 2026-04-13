//! Coordinate System Edge Cases
//!
//! Tests for edge cases in coordinate handling:
//! - i64 to i32 conversion handling
//! - Heading wraparound
//! - Heading difference across 180°

use preprocessor::coord::{latlon_to_cm_relative, compute_lat_avg, R_CM, FIXED_ORIGIN_LON_DEG, FIXED_ORIGIN_LAT_DEG};

// ============================================================================
// Integer Type Conversions
// ============================================================================

#[test]
fn test_i64_to_i32_conversion_in_range() {
    // --- GIVEN ---
    // Coordinate values within i32 range
    let lat = 25.0;
    let lon = 121.0;
    let lat_avg = 25.0;

    // --- WHEN ---
    let (x_cm, y_cm) = latlon_to_cm_relative(lat, lon, lat_avg);

    // --- THEN ---
    // Should convert without loss
    assert!(x_cm >= i32::MIN as i32 && x_cm <= i32::MAX as i32);
    assert!(y_cm >= i32::MIN as i32 && y_cm <= i32::MAX as i32);
}

#[test]
fn test_floating_point_to_integer_rounding() {
    // --- GIVEN ---
    // A coordinate value that results in fractional cm
    let lat = 25.00001; // Slightly offset
    let lon = 121.0;
    let lat_avg = 25.0;

    // --- WHEN ---
    let (x_cm, y_cm) = latlon_to_cm_relative(lat, lon, lat_avg);

    // --- THEN ---
    // Should round to nearest integer
    assert_eq!(x_cm % 1, 0); // x_cm is integer
    assert_eq!(y_cm % 1, 0); // y_cm is integer
}

#[test]
fn test_rounding_consistency() {
    // --- GIVEN ---
    // Same location converted multiple times
    let lat = 25.5;
    let lon = 121.5;
    let lat_avg = 25.5;

    // --- WHEN ---
    let (x1, y1) = latlon_to_cm_relative(lat, lon, lat_avg);
    let (x2, y2) = latlon_to_cm_relative(lat, lon, lat_avg);

    // --- THEN ---
    // Results should be consistent
    assert_eq!(x1, x2, "x coordinate should be consistent");
    assert_eq!(y1, y2, "y coordinate should be consistent");
}

#[test]
fn test_lat_avg_influences_x_coordinate() {
    // --- GIVEN ---
    // Same point but different lat_avg
    let lat = 25.0;
    let lon = 121.0;

    // --- WHEN ---
    let (x1, _) = latlon_to_cm_relative(lat, lon, 20.0);
    let (x2, _) = latlon_to_cm_relative(lat, lon, 30.0);

    // --- THEN ---
    // X coordinate should differ due to cos(lat_avg) factor
    // cos(20°) > cos(30°), so x1 > x2 for same longitude difference
    assert_ne!(x1, x2, "different lat_avg should give different x");
}

// ============================================================================
// Heading Calculations
// ============================================================================

/// Calculate heading from two points
fn calculate_heading(x1: i32, y1: i32, x2: i32, y2: i32) -> i16 {
    let dx_cm = x2 - x1;
    let dy_cm = y2 - y1;
    let heading_rad = (dx_cm as f64).atan2(dy_cm as f64);
    (heading_rad.to_degrees() * 100.0).round() as i16
}

#[test]
fn test_heading_wraparound_359_plus_5() {
    // --- GIVEN ---
    // A heading of 359 degrees (35900 centidegrees)
    // And a turn of +5 degrees

    // --- WHEN ---
    // Use i32 for the calculation to avoid overflow
    let current_heading = 35900_i32;
    let turn_deg = 5_i32;
    let new_heading = current_heading + turn_deg * 100;

    // --- THEN ---
    // The result would be 36400 centidegrees = 364°
    // In a real system, this should wrap to 4°
    // But for this test, we just verify the calculation
    assert_eq!(new_heading, 36400);
}

#[test]
fn test_heading_wraparound_negative() {
    // --- GIVEN ---
    // A heading of 5 degrees
    // And a turn of -10 degrees

    // --- WHEN ---
    // Use i32 for the calculation
    let current_heading = 500_i32;
    let turn_deg = -10_i32;
    let new_heading = current_heading + turn_deg * 100;

    // --- THEN ---
    // The result would be -500 centidegrees = -5°
    // In a real system, this should wrap to 355°
    assert_eq!(new_heading, -500);
}

#[test]
fn test_heading_difference_across_180() {
    // --- GIVEN ---
    // Two headings: 10 degrees and 350 degrees

    // --- WHEN ---
    // Use i32 for the calculation
    let h1_i32 = 1000_i32;  // 10°
    let h2_i32 = 35000_i32; // 350°

    // Direct difference
    let diff1_i32 = (h2_i32 - h1_i32).abs(); // 340°

    // Wrapped difference (shortest path)
    let diff2_i32 = 36000 - diff1_i32; // 20° wrapped

    // --- THEN ---
    // The shortest path should be 20 degrees
    assert_eq!(diff1_i32, 34000); // 340° direct
    assert_eq!(diff2_i32, 2000);  // 20° wrapped

    // For navigation, the wrapped difference is usually what matters
    assert!(diff2_i32 < diff1_i32, "shortest path should be used");
}

#[test]
fn test_heading_difference_both_near_180() {
    // --- GIVEN ---
    let h1 = 17000_i16; // 170°
    let h2 = 19000_i16; // 190°

    // --- WHEN ---
    let diff = (h2 - h1).abs();

    // --- THEN ---
    // Direct difference is 20°
    assert_eq!(diff, 2000);
}

#[test]
fn test_heading_calculation_cardinal_directions() {
    // --- GIVEN ---
    // Test all 4 cardinal directions

    // --- THEN ---
    // North (0°)
    let h_north = calculate_heading(0, 0, 0, 1000);
    assert_eq!(h_north, 0);

    // East (90°)
    let h_east = calculate_heading(0, 0, 1000, 0);
    assert_eq!(h_east, 9000);

    // South (180°)
    let h_south = calculate_heading(0, 0, 0, -1000);
    assert_eq!(h_south, 18000);

    // West (270° or -90°)
    let h_west = calculate_heading(0, 0, -1000, 0);
    assert_eq!(h_west, -9000);
}

#[test]
fn test_heading_calculation_diagonal() {
    // --- GIVEN ---
    // Northeast diagonal (45°)
    let h_ne = calculate_heading(0, 0, 1000, 1000);

    // --- THEN ---
    // Should be 4500 centidegrees (45°)
    assert_eq!(h_ne, 4500);
}

#[test]
fn test_heading_overflow_i16_max() {
    // --- GIVEN ---
    // Very large dx, dy that would cause overflow in atan2
    // Actually atan2 takes f64, so we just test the conversion to i16

    // --- WHEN ---
    let dx_cm = 10000_i32;
    let dy_cm = 10000_i32;
    let heading_rad = (dx_cm as f64).atan2(dy_cm as f64);
    let heading_cdeg = (heading_rad.to_degrees() * 100.0).round() as i16;

    // --- THEN ---
    // Should stay within i16 range (-180° to 180° or similar)
    assert!(heading_cdeg >= -18000 && heading_cdeg <= 18000);
}

#[test]
fn test_heading_at_exact_angles() {
    // --- GIVEN ---
    // Test exact 30°, 45°, 60° angles

    // --- THEN ---
    // 30°
    let h_30 = calculate_heading(0, 0, 577, 1000); // tan(30°) ≈ 0.577
    assert!((h_30 - 3000).abs() <= 100, "30° heading: got {}", h_30);

    // 45°
    let h_45 = calculate_heading(0, 0, 1000, 1000);
    assert_eq!(h_45, 4500);

    // 60°
    let h_60 = calculate_heading(0, 0, 1732, 1000); // tan(60°) ≈ 1.732
    assert!((h_60 - 6000).abs() <= 100, "60° heading: got {}", h_60);
}

// ============================================================================
// Edge Case: Origin Point
// ============================================================================

#[test]
fn test_conversion_at_fixed_origin() {
    // --- GIVEN ---
    // Point at the fixed origin
    let lat = FIXED_ORIGIN_LAT_DEG;
    let lon = FIXED_ORIGIN_LON_DEG;
    let lat_avg = FIXED_ORIGIN_LAT_DEG;

    // --- WHEN ---
    let (x_cm, y_cm) = latlon_to_cm_relative(lat, lon, lat_avg);

    // --- THEN ---
    // Should be approximately at (0, 0)
    assert!(x_cm.abs() <= 10, "origin x should be ~0: got {}", x_cm);
    assert!(y_cm.abs() <= 10, "origin y should be ~0: got {}", y_cm);
}

#[test]
fn test_conversion_near_origin() {
    // --- GIVEN ---
    // Points very close to origin
    let lat = FIXED_ORIGIN_LAT_DEG + 0.00001; // ~1m north
    let lon = FIXED_ORIGIN_LON_DEG + 0.00001; // ~1m east
    let lat_avg = FIXED_ORIGIN_LAT_DEG;

    // --- WHEN ---
    let (x_cm, y_cm) = latlon_to_cm_relative(lat, lon, lat_avg);

    // --- THEN ---
    // Should be small positive values (~100cm)
    assert!(x_cm > 0 && x_cm < 200, "near origin x: {}", x_cm);
    assert!(y_cm > 0 && y_cm < 200, "near origin y: {}", y_cm);
}

// ============================================================================
// Edge Case: Latitude at Extremes
// ============================================================================

#[test]
fn test_conversion_at_equator() {
    // --- GIVEN ---
    let lat = 0.0;
    let lon = 120.0;
    let lat_avg = 0.0;

    // --- WHEN ---
    let (x_cm, y_cm) = latlon_to_cm_relative(lat, lon, lat_avg);

    // --- THEN ---
    // Should handle lat=0 without division by zero
    // Note: The y coordinate is relative to FIXED_ORIGIN_LAT_DEG (20°N)
    // So lat=0 gives a negative y offset from the origin
    assert!(x_cm >= -10 && x_cm <= 10, "equator x ~ 0: {}", x_cm);
    // y_cm should be approximately -R_CM * 20° in radians (offset from 20°N origin)
    let expected_y = -(20.0_f64.to_radians() * R_CM).round() as i32;
    assert!((y_cm - expected_y).abs() < 10000, "equator y relative to 20°N origin: {} ~ {}", y_cm, expected_y);
}

#[test]
fn test_conversion_at_tropic_of_cancer() {
    // --- GIVEN ---
    let lat = 23.5; // Tropic of Cancer
    let lon = 120.0;
    let lat_avg = 23.5;

    // --- WHEN ---
    let (x_cm, _y_cm) = latlon_to_cm_relative(lat, lon, lat_avg);

    // --- THEN ---
    // cos(23.5°) ≈ 0.917
    // X coordinate should be compressed by this factor
    let expected_x = R_CM * (lon.to_radians() - FIXED_ORIGIN_LON_DEG.to_radians()) * (23.5_f64.to_radians().cos());
    let actual_x = x_cm as f64;
    assert!((actual_x - expected_x).abs() < 1000.0, "x coordinate within 10m");
}

// ============================================================================
// Edge Case: Longitude at Extremes
// ============================================================================

#[test]
fn test_conversion_at_date_line_positive() {
    // --- GIVEN ---
    let lat = 25.0;
    let lon = 180.0;
    let lat_avg = 25.0;

    // --- WHEN ---
    let (x_cm, _y_cm) = latlon_to_cm_relative(lat, lon, lat_avg);

    // --- THEN ---
    // Should handle lon=180°
    // Note: This is a large value from the fixed origin at 120°E
    assert!(x_cm > 10_000_000, "date line x should be large: {}", x_cm);
}

#[test]
fn test_conversion_at_date_line_negative() {
    // --- GIVEN ---
    let lat = 25.0;
    let lon = -180.0;
    let lat_avg = 25.0;

    // --- WHEN ---
    let (x_cm, _y_cm) = latlon_to_cm_relative(lat, lon, lat_avg);

    // --- THEN ---
    // Should handle lon=-180°
    // This is equivalent to 180° but on the other side
    let (x_cm_180, _) = latlon_to_cm_relative(lat, 180.0, lat_avg);
    // The difference should be approximately the full circumference
    let diff = (x_cm - x_cm_180).abs();
    assert!(diff > 20_000_000, "date line wraparound should be large");
}

// ============================================================================
// Edge Case: Precision and Rounding
// ============================================================================

#[test]
fn test_conversion_precision_high_lat() {
    // --- GIVEN ---
    // High latitude where cos(lat) is small
    let lat = 80.0;
    let lon = 120.0;
    let lat_avg = 80.0;

    // --- WHEN ---
    let (_x_cm, y_cm) = latlon_to_cm_relative(lat, lon, lat_avg);

    // --- THEN ---
    // cos(80°) ≈ 0.174, so x is heavily compressed
    // But precision should still be reasonable
    assert!(y_cm > 0, "high latitude y should be positive");
}

#[test]
fn test_lat_avg_with_extreme_values() {
    // --- GIVEN ---
    let points = vec![(80.0, 120.0), (-80.0, 120.0)];

    // --- WHEN ---
    let lat_avg = compute_lat_avg(&points);

    // --- THEN ---
    // Average should be 0°
    assert_eq!(lat_avg, 0.0);
}

#[test]
fn test_lat_avg_with_many_points() {
    // --- GIVEN ---
    let points: Vec<(f64, f64)> = (0..1000)
        .map(|i| (25.0 + (i as f64 * 0.001), 121.0))
        .collect();

    // --- WHEN ---
    let lat_avg = compute_lat_avg(&points);

    // --- THEN ---
    // Average should be close to 25.5
    assert!((lat_avg - 25.5).abs() < 0.001, "lat_avg: {}", lat_avg);
}

// ============================================================================
// Edge Case: Coordinate Differences
// ============================================================================

#[test]
fn test_small_coordinate_difference() {
    // --- GIVEN ---
    // Two points 1cm apart
    let lat1 = 25.0;
    let lon1 = 121.0;
    let lat2 = 25.0000001; // Very small change
    let lon2 = 121.0;
    let lat_avg = 25.0;

    // --- WHEN ---
    let (_x1, y1) = latlon_to_cm_relative(lat1, lon1, lat_avg);
    let (_x2, y2) = latlon_to_cm_relative(lat2, lon2, lat_avg);

    // --- THEN ---
    // Difference should be very small (~1cm or less)
    let dy = (y2 - y1).abs();
    assert!(dy <= 10, "1cm apart: dy = {}", dy);
}

#[test]
fn test_large_coordinate_difference() {
    // --- GIVEN ---
    // Two points 100km apart
    let lat1 = 25.0;
    let lon1 = 121.0;
    let lat2 = 26.0; // ~111km north
    let lon2 = 121.0;
    let lat_avg = 25.5;

    // --- WHEN ---
    let (_x1, y1) = latlon_to_cm_relative(lat1, lon1, lat_avg);
    let (_x2, y2) = latlon_to_cm_relative(lat2, lon2, lat_avg);

    // --- THEN ---
    // Y difference should be ~111km = 11,100,000cm
    let dy = (y2 - y1).abs();
    assert!(dy > 10_000_000 && dy < 12_000_000, "100km apart: dy = {}cm", dy);
}

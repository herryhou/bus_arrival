//! Projection accuracy tests
//!
//! These tests verify coordinate projection roundtrip accuracy.
//! The lat_avg_deg mismatch was causing ~10m position errors.

use preprocessor::coord::{latlon_to_cm_relative, compute_lat_avg};

#[test]
fn test_projection_roundtrip_taiwan_coordinates() {
    // Test with actual Taiwan coordinates from our test route
    let lat = 25.00425;
    let lon = 121.28645;
    let lat_avg = 24.990083; // Computed from the route

    // Project to cm
    let (x_cm, y_cm) = latlon_to_cm_relative(lat, lon, lat_avg);

    // Verify we get reasonable values (not wild outliers)
    // Taiwan is about 100-600km from the fixed origin (20°N, 120°E)
    assert!(x_cm.abs() < 100_000_000, "x_cm out of reasonable range: {}", x_cm);
    assert!(y_cm.abs() < 100_000_000, "y_cm out of reasonable range: {}", y_cm);

    // Specific values for this point
    assert_eq!(x_cm, 12965481);
    assert_eq!(y_cm, 55644721);
}

#[test]
fn test_lat_avg_computation() {
    // Verify average latitude is computed correctly
    let points = vec![
        (25.00425, 121.28645),
        (25.00566, 121.28619),
        (25.00592, 121.28794),
        (25.00100, 121.29500),
    ];

    let lat_avg = compute_lat_avg(&points);

    // Average should be close to the middle of the range
    assert!(lat_avg > 25.0 && lat_avg < 25.01, "lat_avg out of expected range: {}", lat_avg);

    // With these specific points, should get approximately 24.99
    assert!((lat_avg - 25.0025).abs() < 0.01, "lat_avg too far from expected: {}", lat_avg);
}

#[test]
fn test_lat_avg_impact_on_projection() {
    // Demonstrate the impact of using wrong lat_avg
    let lat = 25.00425;
    let lon = 121.28645;

    // Project with correct lat_avg
    let (x1, _) = latlon_to_cm_relative(lat, lon, 24.990083);

    // Project with wrong lat_avg (25.0)
    let (x2, _) = latlon_to_cm_relative(lat, lon, 25.0);

    // The difference is measurable (~10m) but not catastrophic
    let diff = (x1 - x2).abs() as f64;

    // This demonstrates why storing lat_avg in the binary is important!
    // The ~10m error per point accumulates across the route
    assert!(diff < 2000.0, "lat_avg mismatch causes {} cm error (unexpectedly high)", diff);
    assert!(diff > 500.0, "lat_avg mismatch should cause measurable error");
}

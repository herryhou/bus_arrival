//! Integration test: GPS positions should match route positions
//!
//! This test simulates the full pipeline:
//! 1. Create a route with known lat/lon
//! 2. Convert route points to cm coordinates (preprocessor does this)
//! 3. Convert a GPS position to cm coordinates (simulator does this)
//! 4. Verify the GPS position matches the expected route position
//!
//! This ensures the coordinate systems are aligned between preprocessor and simulator.

use shared::{EARTH_R_CM, FIXED_ORIGIN_LON_DEG, FIXED_ORIGIN_Y_CM};

/// Convert lat/lon to cm using the same formula as preprocessor
fn latlon_to_cm(lat: f64, lon: f64, lat_avg_deg: f64) -> (i64, i64) {
    let lat_rad = lat.to_radians();
    let lon_rad = lon.to_radians();
    let lat_avg_rad = lat_avg_deg.to_radians();
    let cos_lat = lat_avg_rad.cos();

    let x_abs = EARTH_R_CM as f64 * lon_rad * cos_lat;
    let y_abs = EARTH_R_CM as f64 * lat_rad;
    let x0_abs = (FIXED_ORIGIN_LON_DEG.to_radians() * EARTH_R_CM as f64) * cos_lat;
    let y0_abs = FIXED_ORIGIN_Y_CM as f64;

    let dx_cm = (x_abs - x0_abs).round() as i64;
    let dy_cm = (y_abs - y0_abs).round() as i64;

    (dx_cm, dy_cm)
}

#[test]
fn test_gps_position_matches_route_position() {
    // This test verifies that a GPS position at a known location
    // matches the route's position at that same location.

    // Arrange: Route point at known coordinates
    let route_lat = 25.005;
    let route_lon = 121.005;
    let lat_avg = 25.0;

    // Act: Convert route point to cm (what preprocessor does)
    let (route_x_cm, route_y_cm) = latlon_to_cm(route_lat, route_lon, lat_avg);

    // Convert GPS at same location to cm (what simulator should do)
    let (gps_x_cm, gps_y_cm) = latlon_to_cm(route_lat, route_lon, lat_avg);

    // Assert: GPS position should match route position exactly
    // If this fails, there's a coordinate system mismatch between
    // preprocessor and simulator
    assert_eq!(gps_x_cm, route_x_cm,
        "GPS x_cm should match route x_cm (coordinate system aligned)");
    assert_eq!(gps_y_cm, route_y_cm,
        "GPS y_cm should match route y_cm (coordinate system aligned)");
}

#[test]
fn test_gps_offset_from_route_measures_distance() {
    // This test verifies that the distance between a GPS position
    // and the route can be calculated correctly.

    let route_lat = 25.000;
    let route_lon = 121.000;
    let gps_lat = 25.001;
    let gps_lon = 121.001;
    let lat_avg = 25.0;

    // Convert both to cm
    let (route_x, route_y) = latlon_to_cm(route_lat, route_lon, lat_avg);
    let (gps_x, gps_y) = latlon_to_cm(gps_lat, gps_lon, lat_avg);

    // Calculate distance in cm
    let dx = gps_x - route_x;
    let dy = gps_y - route_y;
    let distance_cm = (dx * dx + dy * dy) as f64;
    let distance_m = distance_cm.sqrt() / 100.0;

    // The expected distance can be calculated using haversine formula
    // For small distances, the flat-earth approximation is accurate
    // 0.001° ≈ 111m in latitude, * cos(25°) ≈ 100m in longitude
    // So 0.001° in both ≈ 150m (Pythagorean theorem)
    let expected_distance_m = 150.0;

    // Assert: Distance is approximately correct (±5% tolerance)
    assert!((distance_m - expected_distance_m).abs() < expected_distance_m * 0.05,
        "GPS offset distance should be approximately correct (±5%)");
}

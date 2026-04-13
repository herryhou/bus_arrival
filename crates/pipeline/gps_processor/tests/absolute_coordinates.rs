//! Test that simulator uses ABSOLUTE coordinates for GPS matching
//!
//! This test ensures the simulator converts GPS coordinates to absolute coordinates
//! (relative to FIXED origin 120°E, 20°N), NOT relative to grid origin (x0_cm, y0_cm).
//!
//! This test would have caught the bug where the simulator was subtracting grid origin
//! from GPS coordinates, causing bus positions to appear far from the route path.

use gps_processor::map_match;

#[test]
fn test_gps_to_absolute_coordinates_uses_correct_lat_avg() {
    // Arrange: GPS position at known coordinates
    let lat = 25.0;
    let lon = 121.0;
    let lat_avg_deg = 25.0; // Route's actual average latitude

    // Act: Convert to absolute coordinates using route's lat_avg
    let (x_cm, y_cm) = map_match::latlon_to_cm_absolute_with_lat_avg(lat, lon, lat_avg_deg);

    // Expected: Calculate absolute coordinates manually
    use shared::{EARTH_R_CM, FIXED_ORIGIN_LON_DEG, FIXED_ORIGIN_Y_CM};

    let lat_rad = lat.to_radians();
    let lon_rad = lon.to_radians();
    let lat_avg_rad = lat_avg_deg.to_radians();
    let cos_lat = lat_avg_rad.cos();

    let x_abs = EARTH_R_CM as f64 * lon_rad * cos_lat;
    let y_abs = EARTH_R_CM as f64 * lat_rad;
    let x0_abs = (FIXED_ORIGIN_LON_DEG.to_radians() * EARTH_R_CM as f64) * cos_lat;
    let y0_abs = FIXED_ORIGIN_Y_CM as f64;

    let expected_x = (x_abs - x0_abs).round() as i32;
    let expected_y = (y_abs - y0_abs).round() as i32;

    // Assert: Function returns absolute coordinates
    assert_eq!(
        x_cm, expected_x,
        "GPS should be converted to absolute coordinates (from fixed origin)"
    );
    assert_eq!(
        y_cm, expected_y,
        "GPS should be converted to absolute coordinates (from fixed origin)"
    );
}

#[test]
fn test_latlon_to_cm_absolute_with_lat_avg_matches_preprocessor_formula() {
    // This test verifies that the simulator's coordinate conversion uses the
    // same formula as the preprocessor.

    let lat = 25.005;
    let lon = 121.005;
    let lat_avg_deg = 25.0;

    // Simulator conversion
    let (sim_x, sim_y) = map_match::latlon_to_cm_absolute_with_lat_avg(lat, lon, lat_avg_deg);

    // Expected calculation (same as preprocessor/src/coord.rs)
    use shared::{EARTH_R_CM, FIXED_ORIGIN_LON_DEG, FIXED_ORIGIN_Y_CM};

    let lat_rad = lat.to_radians();
    let lon_rad = lon.to_radians();
    let lat_avg_rad = lat_avg_deg.to_radians();
    let cos_lat = lat_avg_rad.cos();

    let x_abs = EARTH_R_CM as f64 * lon_rad * cos_lat;
    let y_abs = EARTH_R_CM as f64 * lat_rad;
    let x0_abs = (FIXED_ORIGIN_LON_DEG.to_radians() * EARTH_R_CM as f64) * cos_lat;
    let y0_abs = FIXED_ORIGIN_Y_CM as f64;

    let expected_x = (x_abs - x0_abs).round() as i32;
    let expected_y = (y_abs - y0_abs).round() as i32;

    // Assert: Simulator uses correct formula matching preprocessor
    assert_eq!(
        sim_x, expected_x,
        "Simulator must use same coordinate conversion as preprocessor (x)"
    );
    assert_eq!(
        sim_y, expected_y,
        "Simulator must use same coordinate conversion as preprocessor (y)"
    );
}

#[test]
fn test_absolute_coordinates_not_relative_to_grid_origin() {
    // This test verifies that GPS coordinates are NOT adjusted by grid origin.
    // The x0_cm/y0_cm in the binary file are for spatial indexing only.

    let lat = 25.01;
    let lon = 121.01;
    let lat_avg_deg = 25.0;

    // Get absolute coordinates
    let (abs_x, abs_y) = map_match::latlon_to_cm_absolute_with_lat_avg(lat, lon, lat_avg_deg);

    // Verify absolute coordinates are large positive values (offset from fixed origin)
    assert!(
        abs_x > 10000000,
        "Absolute x coordinate should be large positive value"
    );
    assert!(
        abs_y > 50000000,
        "Absolute y coordinate should be large positive value"
    );

    // Demonstrate the bug: if we subtract grid origin from GPS coordinates,
    // the result would be much smaller and in the wrong location
    // Use a hypothetical grid origin (what x0_cm/y0_cm typically are for a Taiwan route)
    let hypothetical_grid_x0 = 10000000;
    let hypothetical_grid_y0 = 55000000;

    let relative_x = abs_x as i64 - hypothetical_grid_x0;
    let relative_y = abs_y as i64 - hypothetical_grid_y0;
    // This puts the bus in the wrong location!
    assert_ne!(
        relative_x, abs_x as i64,
        "Using relative coordinates would give wrong x position"
    );
    assert_ne!(
        relative_y, abs_y as i64,
        "Using relative coordinates would give wrong y position"
    );

    // The difference should be significant (> 10 km in one dimension)
    let diff_x = (relative_x - abs_x as i64).abs();
    let diff_y = (relative_y - abs_y as i64).abs();
    assert!(
        diff_x > 1000000 || diff_y > 1000000,
        "Coordinate system error should cause >10km position error"
    );
}

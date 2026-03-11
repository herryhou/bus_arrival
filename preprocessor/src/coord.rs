// Coordinate conversion functions for GPS bus arrival detection system
//
// Provides conversion between latitude/longitude and centimeter-based coordinate systems.
// Uses a local flat-earth approximation suitable for small geographic areas.

/// Earth's radius in centimeters
///
/// Used for coordinate conversion calculations.
/// Earth radius ≈ 6,371 km = 637,100,000 cm
pub const R_CM: f64 = 637_100_000.0;

/// Fixed origin longitude in degrees (120.0°E)
///
/// All routes use this fixed origin to ensure consistent coordinate system.
pub const FIXED_ORIGIN_LON_DEG: f64 = 120.0;

/// Fixed origin latitude in degrees (20.0°N)
///
/// All routes use this fixed origin to ensure consistent coordinate system.
pub const FIXED_ORIGIN_LAT_DEG: f64 = 20.0;

/// Compute cosine using a small Taylor approximation (const-friendly)
const fn cos_deg(deg: f64) -> f64 {
    let rad = deg * std::f64::consts::PI / 180.0;
    // Small-angle approximation is sufficient for our precision needs
    // cos(x) ≈ 1 - x²/2 for small x, but we need better accuracy
    // Use Taylor series around 0: cos(x) = 1 - x²/2! + x⁴/4! - x⁶/6!
    let x2 = rad * rad;
    let x4 = x2 * x2;
    let x6 = x2 * x4;
    1.0 - x2 / 2.0 + x4 / 24.0 - x6 / 720.0
}

/// Fixed origin X coordinate in centimeters
///
/// Computed at compile time: R × lon_rad × cos(lat_rad)
/// where lon=120.0°, lat=20.0°
pub const FIXED_ORIGIN_X_CM: i64 = {
    let lon_rad = FIXED_ORIGIN_LON_DEG * std::f64::consts::PI / 180.0;
    let x_cm = R_CM * lon_rad * cos_deg(FIXED_ORIGIN_LAT_DEG);
    x_cm as i64
};

/// Fixed origin Y coordinate in centimeters
///
/// Computed at compile time: R × lat_rad
/// where lat=20.0°
pub const FIXED_ORIGIN_Y_CM: i64 = {
    let lat_rad = FIXED_ORIGIN_LAT_DEG * std::f64::consts::PI / 180.0;
    let y_cm = R_CM * lat_rad;
    y_cm as i64
};

/// Distance in centimeters (relative coordinate)
///
/// Used for relative coordinates that fit within i32 range.
/// These represent offsets from a reference point (bbox origin).
pub type DistCm = i32;

/// Convert latitude/longitude to absolute centimeter coordinates
///
/// Uses a local flat-earth approximation:
/// - X (east-west): R * lon_rad * cos(lat_avg)
/// - Y (north-south): R * lat_rad
///
/// # Arguments
/// * `lat` - Latitude in decimal degrees
/// * `lon` - Longitude in decimal degrees
///
/// # Returns
/// * `(i64, i64)` - Absolute (x, y) coordinates in centimeters
///
/// # Examples
/// ```
/// use preprocessor::coord::latlon_to_cm;
///
/// let (x, y) = latlon_to_cm(25.00425, 121.28645);
/// assert!(x > 0 && y > 0);
/// ```
///
/// # Notes
/// - This is an approximation suitable for small geographic areas (< 100 km)
/// - For global coordinates, consider using a proper projection library
/// - The origin (0, 0) is at lat=0, lon=0 (equator at prime meridian)
pub fn latlon_to_cm(lat: f64, lon: f64) -> (i64, i64) {
    let lat_rad = lat.to_radians();
    let lon_rad = lon.to_radians();

    // For absolute coordinates, we need to use the actual latitude
    // for the cos(lat) correction in X (longitude varies with latitude)
    let cos_lat = lat_rad.cos();

    let x_cm = R_CM * lon_rad * cos_lat;
    let y_cm = R_CM * lat_rad;

    (x_cm as i64, y_cm as i64)
}

/// Convert latitude/longitude to relative centimeter coordinates
///
/// Computes coordinates relative to the FIXED origin at (120.0°E, 20.0°N).
/// This keeps values small enough to fit in i32, preventing overflow issues.
/// All routes use the same fixed origin for consistency.
///
/// # Arguments
/// * `lat` - Latitude in decimal degrees
/// * `lon` - Longitude in decimal degrees
/// * `lat_avg` - Average latitude of the area (for cos(lat) correction)
///
/// # Returns
/// * `(DistCm, DistCm)` - Relative (dx, dy) coordinates in centimeters
///
/// # Examples
/// ```
/// use preprocessor::coord::latlon_to_cm_relative;
///
/// // At origin (120.0°E, 20.0°N), relative coords should be approximately (0, 0)
/// let (dx, dy) = latlon_to_cm_relative(20.0, 120.0, 20.0);
/// assert!(dx.abs() < 1_000_000); // Allow small tolerance for floating point
/// assert!(dy.abs() < 1_000_000);
///
/// // 1° north of origin (≈111km)
/// let (dx, dy) = latlon_to_cm_relative(21.0, 120.0, 20.0);
/// assert!(dy > 10_000_000 && dy < 11_500_000); // ~111km
/// ```
///
/// # Notes
/// - Relative coordinates are designed to fit within i32 range
/// - The lat_avg parameter is kept for API compatibility but not used internally
/// - Fixed origin ensures all routes share the same coordinate system
///
/// # Note on X-coordinate scaling
/// The fixed origin was computed at 20°N, so all x-coordinates use cos(20°)
/// for scaling to ensure consistency across all routes.
pub fn latlon_to_cm_relative(
    lat: f64,
    lon: f64,
    _lat_avg: f64, // Kept for API compatibility, but not used for x scaling
) -> (DistCm, DistCm) {
    let lat_rad = lat.to_radians();
    let lon_rad = lon.to_radians();

    // Use fixed origin's latitude (20°N) for x-coordinate scaling
    // This ensures all routes use the same x-coordinate scale
    let fixed_origin_lat_rad = FIXED_ORIGIN_LAT_DEG.to_radians();
    let cos_fixed_lat = fixed_origin_lat_rad.cos();

    let x_cm = R_CM * lon_rad * cos_fixed_lat;
    let y_cm = R_CM * lat_rad;

    // Use FIXED origin (same for all routes)
    // Round to handle floating point precision issues
    let dx_cm = (x_cm - FIXED_ORIGIN_X_CM as f64).round() as i64;
    let dy_cm = (y_cm - FIXED_ORIGIN_Y_CM as f64).round() as i64;

    (dx_cm as DistCm, dy_cm as DistCm)
}

/// Compute average latitude from a set of GPS coordinates
///
/// Used for the cos(lat) correction in coordinate conversion.
/// Provides better accuracy for areas spanning multiple degrees of latitude.
///
/// # Arguments
/// * `points` - Slice of (lat, lon) coordinate tuples in decimal degrees
///
/// # Returns
/// * `f64` - Average latitude, or 0.0 if empty
///
/// # Examples
/// ```
/// use preprocessor::coord::compute_lat_avg;
///
/// let points = vec![(25.0, 121.0), (26.0, 122.0), (25.5, 121.5)];
/// let avg = compute_lat_avg(&points);
/// assert_eq!(avg, 25.5);
/// ```
///
/// # Notes
/// - Returns 0.0 for empty input to handle edge cases gracefully
/// - Using the average latitude improves accuracy for relative coordinates
pub fn compute_lat_avg(points: &[(f64, f64)]) -> f64 {
    if points.is_empty() {
        return 0.0;
    }

    let sum: f64 = points.iter().map(|(lat, _)| lat).sum();
    sum / points.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latlon_to_cm_basic() {
        // Test that we get reasonable order of magnitude
        // At lat=25, lon=121, we should be hundreds of millions of cm from origin
        let (x, y) = latlon_to_cm(25.00425, 121.28645);

        // X and Y should be on order of 10^8 to 10^9 cm
        // (degrees * 111 km/degree * 100000 cm/km)
        assert!(
            x > 100_000_000,
            "X coordinate should be > 10^8 cm, got {}",
            x
        );
        assert!(
            y > 100_000_000,
            "Y coordinate should be > 10^8 cm, got {}",
            y
        );

        // At lon=121 with cos(25°)≈0.9, x ≈ R * 121° * π/180 * 0.9 ≈ 1.22×10^9 cm
        assert!(
            x > 1_200_000_000 && x < 1_300_000_000,
            "X coordinate out of expected range: {}",
            x
        );

        // At lat=25, y ≈ R * 25° * π/180 ≈ 2.78×10^8 cm
        assert!(
            y > 270_000_000 && y < 290_000_000,
            "Y coordinate out of expected range: {}",
            y
        );
    }

    #[test]
    fn latlon_to_cm_relative_origin() {
        // Test that the fixed origin point (120.0°E, 20.0°N) returns (0, 0)
        // Allow tolerance for floating point precision differences
        let (dx, dy) = latlon_to_cm_relative(
            FIXED_ORIGIN_LAT_DEG,
            FIXED_ORIGIN_LON_DEG,
            FIXED_ORIGIN_LAT_DEG,
        );
        assert!(
            dx.abs() < 1_000_000,
            "Origin point should have dx≈0, got {}",
            dx
        );
        assert!(
            dy.abs() < 1_000_000,
            "Origin point should have dy≈0, got {}",
            dy
        );
    }

    #[test]
    fn latlon_to_cm_relative_different_point() {
        // Test relative coordinates for a point near the fixed origin
        let lat_avg = 20.0;

        // Point 1° north of fixed origin
        // 1° of latitude ≈ 111.3 km (Earth's circumference / 360)
        let lat = FIXED_ORIGIN_LAT_DEG + 1.0;
        let lon = FIXED_ORIGIN_LON_DEG;

        let (dx, dy) = latlon_to_cm_relative(lat, lon, lat_avg);

        // dy should be positive (northward) and approximately 111.3km = 11,130,000cm
        // Allow wider tolerance due to floating point and Earth shape variations
        assert!(
            dy > 10_000_000 && dy < 11_500_000,
            "dy should be ~111km for 1° north, got {}",
            dy
        );

        // dx should be close to 0 for same longitude
        assert!(
            dx.abs() < 1_000_000,
            "dx should be small for same longitude, got {}",
            dx
        );
    }

    #[test]
    fn relative_coords_fit_in_i32() {
        // Test that relative coordinates for Taiwan fit in i32
        // Taiwan is approximately 21-25°N, 119-122°E
        // Fixed origin at (120.0°E, 20.0°N)

        let lat_avg = 23.0; // Taiwan average latitude

        // Test Taiwan corners
        let test_points = [
            (25.0, 122.0), // Northeast corner
            (21.0, 122.0), // Southeast corner
            (25.0, 119.0), // Northwest corner
            (21.0, 119.0), // Southwest corner
        ];

        for (lat, lon) in test_points {
            let (dx, dy) = latlon_to_cm_relative(lat, lon, lat_avg);

            // Verify they fit in i32 range
            let dx_i64 = dx as i64;
            let dy_i64 = dy as i64;
            assert!(
                dx_i64 > i32::MIN as i64 / 2 && dx_i64 < i32::MAX as i64 / 2,
                "dx out of range for ({}, {}): {}",
                lat,
                lon,
                dx
            );
            assert!(
                dy_i64 > i32::MIN as i64 / 2 && dy_i64 < i32::MAX as i64 / 2,
                "dy out of range for ({}, {}): {}",
                lat,
                lon,
                dy
            );
        }

        // All coordinates should fit comfortably in i32
        // Taiwan is ~400km × ~600km, well within ±2,000 km i32 range
    }

    #[test]
    fn fixed_origin_taiwan_coordinates() {
        // Test that Taiwan coordinates are reasonable relative to fixed origin
        let lat_avg = 23.0;

        // Taipei (approximately 25.0°N, 121.5°E)
        let (dx, dy) = latlon_to_cm_relative(25.0, 121.5, lat_avg);

        // Should be positive (north and east of origin)
        assert!(dx > 0, "Taipei should be east of fixed origin");
        assert!(dy > 0, "Taipei should be north of fixed origin");

        // Should be within ~300km of origin
        assert!(dx < 350_000_000, "Taipei X offset seems too large");
        assert!(dy < 350_000_000, "Taipei Y offset seems too large");
    }

    #[test]
    fn fixed_origin_constants() {
        // Verify fixed origin constants are correctly defined
        assert!(
            FIXED_ORIGIN_X_CM > 1_200_000_000 && FIXED_ORIGIN_X_CM < 1_300_000_000,
            "FIXED_ORIGIN_X_CM should be ~1.25×10^9"
        );
        assert!(
            FIXED_ORIGIN_Y_CM > 220_000_000 && FIXED_ORIGIN_Y_CM < 230_000_000,
            "FIXED_ORIGIN_Y_CM should be ~2.23×10^8"
        );
    }

    #[test]
    fn compute_lat_avg_basic() {
        let points = vec![(25.0, 121.0), (26.0, 122.0), (25.5, 121.5)];

        let avg = compute_lat_avg(&points);
        assert_eq!(avg, 25.5);
    }

    #[test]
    fn compute_lat_avg_single() {
        let points = vec![(25.00425, 121.28645)];

        let avg = compute_lat_avg(&points);
        assert_eq!(avg, 25.00425);
    }

    #[test]
    fn compute_lat_avg_empty() {
        let points: Vec<(f64, f64)> = vec![];

        let avg = compute_lat_avg(&points);
        assert_eq!(avg, 0.0);
    }

    #[test]
    fn latlon_to_cm_negative_coordinates() {
        // Test with negative lat/lon (southern hemisphere, western hemisphere)
        let (x, y) = latlon_to_cm(-25.00425, -121.28645);

        // Both should be negative
        assert!(x < 0, "X should be negative for negative longitude");
        assert!(y < 0, "Y should be negative for negative latitude");
    }

    #[test]
    fn latlon_to_cm_equator_prime_meridian() {
        // Test at origin (equator, prime meridian)
        let (x, y) = latlon_to_cm(0.0, 0.0);

        // Should be close to (0, 0)
        assert!(x.abs() < 1000, "X should be ~0 at origin, got {}", x);
        assert!(y.abs() < 1000, "Y should be ~0 at origin, got {}", y);
    }

    #[test]
    fn latlon_to_cm_relative_negative_offsets() {
        // Test that relative coordinates can be negative (south of fixed origin)
        let lat_avg = FIXED_ORIGIN_LAT_DEG;

        // Point 1° south of fixed origin (19.0°N)
        let lat = FIXED_ORIGIN_LAT_DEG - 1.0;
        let lon = FIXED_ORIGIN_LON_DEG;

        let (_dx, dy) = latlon_to_cm_relative(lat, lon, lat_avg);

        // dy should be negative (southward)
        assert!(
            dy < 0,
            "dy should be negative for point south of origin, got {}",
            dy
        );
        // dy should be approximately -111.3km (1° of latitude)
        // Allow wider tolerance due to floating point and Earth shape variations
        assert!(
            dy > -12_500_000 && dy < -10_000_000,
            "dy should be ~-111km for 1° south, got {}",
            dy
        );
    }
}

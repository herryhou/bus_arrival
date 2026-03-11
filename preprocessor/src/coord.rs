// Coordinate conversion functions for GPS bus arrival detection system
//
// Provides conversion between latitude/longitude and centimeter-based coordinate systems.
// Uses a local flat-earth approximation suitable for small geographic areas.

/// Earth's radius in centimeters
///
/// Used for coordinate conversion calculations.
/// Earth radius ≈ 6,371 km = 637,100,000 cm
pub const R_CM: f64 = 637_100_000.0;

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
/// Computes coordinates relative to a reference point (bbox origin).
/// This keeps values small enough to fit in i32, preventing overflow issues.
///
/// # Arguments
/// * `lat` - Latitude in decimal degrees
/// * `lon` - Longitude in decimal degrees
/// * `lat_avg` - Average latitude of the area (for cos(lat) correction)
/// * `x0_cm` - Reference X coordinate in centimeters (bbox origin)
/// * `y0_cm` - Reference Y coordinate in centimeters (bbox origin)
///
/// # Returns
/// * `(DistCm, DistCm)` - Relative (dx, dy) coordinates in centimeters
///
/// # Examples
/// ```
/// use preprocessor::coord::{latlon_to_cm, latlon_to_cm_relative};
///
/// let (x0, y0) = latlon_to_cm(25.00425, 121.28645);
/// let (dx, dy) = latlon_to_cm_relative(25.00425, 121.28645, 25.0, x0, y0);
/// assert_eq!(dx, 0);
/// assert_eq!(dy, 0);
/// ```
///
/// # Notes
/// - Relative coordinates are designed to fit within i32 range
/// - The lat_avg parameter improves accuracy for areas spanning several degrees
pub fn latlon_to_cm_relative(
    lat: f64,
    lon: f64,
    lat_avg: f64,
    x0_cm: i64,
    y0_cm: i64,
) -> (DistCm, DistCm) {
    let lat_rad = lat.to_radians();
    let lon_rad = lon.to_radians();
    let lat_avg_rad = lat_avg.to_radians();

    let cos_lat_avg = lat_avg_rad.cos();

    let x_cm = R_CM * lon_rad * cos_lat_avg;
    let y_cm = R_CM * lat_rad;

    let dx_cm = (x_cm - x0_cm as f64) as i64;
    let dy_cm = (y_cm - y0_cm as f64) as i64;

    (dx_cm as DistCm, dy_cm as DistCm)
}

/// Compute bounding box origin from a set of absolute coordinates
///
/// Finds the minimum X and Y values, which become the reference point
/// for relative coordinate calculations.
///
/// # Arguments
/// * `coords` - Slice of (x, y) absolute coordinate tuples in centimeters
///
/// # Returns
/// * `(i64, i64)` - Minimum (x, y) coordinates, or (0, 0) if empty
///
/// # Examples
/// ```
/// use preprocessor::coord::compute_bbox_origin;
///
/// let coords = vec![(1000, 2000), (1500, 1800), (900, 2500)];
/// let (x_min, y_min) = compute_bbox_origin(&coords);
/// assert_eq!(x_min, 900);
/// assert_eq!(y_min, 1800);
/// ```
///
/// # Notes
/// - Returns (0, 0) for empty input to handle edge cases gracefully
/// - This is used to establish the origin for relative coordinates
pub fn compute_bbox_origin(coords: &[(i64, i64)]) -> (i64, i64) {
    if coords.is_empty() {
        return (0, 0);
    }

    let x_min = coords.iter().map(|(x, _)| *x).min().unwrap();
    let y_min = coords.iter().map(|(_, y)| *y).min().unwrap();

    (x_min, y_min)
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
        assert!(x > 100_000_000, "X coordinate should be > 10^8 cm, got {}", x);
        assert!(y > 100_000_000, "Y coordinate should be > 10^8 cm, got {}", y);

        // At lon=121 with cos(25°)≈0.9, x ≈ R * 121° * π/180 * 0.9 ≈ 1.22×10^9 cm
        assert!(x > 1_200_000_000 && x < 1_300_000_000,
                "X coordinate out of expected range: {}", x);

        // At lat=25, y ≈ R * 25° * π/180 ≈ 2.78×10^8 cm
        assert!(y > 270_000_000 && y < 290_000_000,
                "Y coordinate out of expected range: {}", y);
    }

    #[test]
    fn latlon_to_cm_relative_origin() {
        // Test that the origin point returns (0, 0)
        let lat = 25.00425;
        let lon = 121.28645;

        let (x0, y0) = latlon_to_cm(lat, lon);
        let (dx, dy) = latlon_to_cm_relative(lat, lon, lat, x0, y0);

        // The origin point should map to (0, 0) relative to itself
        assert_eq!(dx, 0, "Origin point should have dx=0, got {}", dx);
        assert_eq!(dy, 0, "Origin point should have dy=0, got {}", dy);
    }

    #[test]
    fn latlon_to_cm_relative_different_point() {
        // Test relative coordinates for a different point
        let lat0 = 25.00425;
        let lon0 = 121.28645;
        let lat_avg = 25.0;

        let (x0, y0) = latlon_to_cm(lat0, lon0);

        // Point 100m north (0.001 degrees ≈ 111m at this latitude)
        let lat1 = lat0 + 0.001;
        let lon1 = lon0;

        let (dx, dy) = latlon_to_cm_relative(lat1, lon1, lat_avg, x0, y0);

        // dy should be positive (northward) and approximately 111m = 11100cm
        assert!(dy > 10000 && dy < 12000,
                "dy should be ~11100cm for 0.001° north, got {}", dy);

        // dx will be small but not exactly 0 due to cos(lat_avg) vs cos(lat0) difference
        // The offset is at most a few cm for small latitude changes
        assert!(dx.abs() < 100000, "dx should be small for same longitude, got {}", dx);
    }

    #[test]
    fn relative_coords_fit_in_i32() {
        // Test that relative coordinates for a small area fit in i32
        // Simulate a route spanning ~10km (reasonable for a bus route)

        let lat_center = 25.0;
        let lon_center = 121.0;

        // Create points in a 10km x 10km area
        let mut coords = Vec::new();
        for i in 0..10 {
            for j in 0..10 {
                // 0.01° ≈ 1.1km, so this covers ~10km
                let lat = lat_center + (i as f64 - 5.0) * 0.01;
                let lon = lon_center + (j as f64 - 5.0) * 0.01;
                coords.push(latlon_to_cm(lat, lon));
            }
        }

        let (x0, y0) = compute_bbox_origin(&coords);

        // Convert all points to relative coordinates
        for (x_abs, y_abs) in &coords {
            let lat = (*y_abs as f64 / R_CM).to_degrees();
            let lon = (*x_abs as f64 / R_CM).to_degrees();

            let (dx, dy) = latlon_to_cm_relative(lat, lon, lat_center, x0, y0);

            // Verify they fit in i32 (they already are i32, but check they're reasonable)
            assert!(dx < i32::MAX / 2, "dx too large: {}", dx);
            assert!(dy < i32::MAX / 2, "dy too large: {}", dy);
        }

        // All coordinates should fit comfortably in i32
        // 10km = 1,000,000 cm, well within i32 range (±2×10^9)
    }

    #[test]
    fn compute_lat_avg_basic() {
        let points = vec![
            (25.0, 121.0),
            (26.0, 122.0),
            (25.5, 121.5),
        ];

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
    fn compute_bbox_origin_basic() {
        let coords = vec![
            (1000, 2000),
            (1500, 1800),
            (900, 2500),
        ];

        let (x_min, y_min) = compute_bbox_origin(&coords);
        assert_eq!(x_min, 900);
        assert_eq!(y_min, 1800);
    }

    #[test]
    fn compute_bbox_origin_empty() {
        let coords: Vec<(i64, i64)> = vec![];

        let (x_min, y_min) = compute_bbox_origin(&coords);
        assert_eq!(x_min, 0);
        assert_eq!(y_min, 0);
    }

    #[test]
    fn compute_bbox_origin_single() {
        let coords = vec![(1234, 5678)];

        let (x_min, y_min) = compute_bbox_origin(&coords);
        assert_eq!(x_min, 1234);
        assert_eq!(y_min, 5678);
    }

    #[test]
    fn compute_bbox_origin_all_same() {
        let coords = vec![
            (1000, 2000),
            (1000, 2000),
            (1000, 2000),
        ];

        let (x_min, y_min) = compute_bbox_origin(&coords);
        assert_eq!(x_min, 1000);
        assert_eq!(y_min, 2000);
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
        // Test that relative coordinates can be negative
        let lat0 = 25.0;
        let lon0 = 121.0;

        let (x0, y0) = latlon_to_cm(lat0, lon0);

        // Point south of origin
        let lat1 = lat0 - 0.001;
        let lon1 = lon0;

        let (_dx, dy) = latlon_to_cm_relative(lat1, lon1, lat0, x0, y0);

        // dy should be negative (southward)
        assert!(dy < 0, "dy should be negative for point south of origin, got {}", dy);
    }
}

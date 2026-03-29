// Coordinate conversion functions for GPS bus arrival detection system
//
// Provides conversion between latitude/longitude and centimeter-based coordinate systems.
// Uses a local flat-earth approximation suitable for small geographic areas.

/// Earth's radius in centimeters
pub const R_CM: f64 = 637_100_000.0;

/// Fixed origin longitude in degrees (120.0°E)
pub const FIXED_ORIGIN_LON_DEG: f64 = 120.0;

/// Fixed origin latitude in degrees (20.0°N)
pub const FIXED_ORIGIN_LAT_DEG: f64 = 20.0;

/// Fixed origin Y coordinate in centimeters
pub const FIXED_ORIGIN_Y_CM: i64 = {
    let lat_rad = FIXED_ORIGIN_LAT_DEG * std::f64::consts::PI / 180.0;
    let y_cm = R_CM * lat_rad;
    y_cm as i64
};

/// Distance in centimeters (relative coordinate)
pub type DistCm = i32;

/// Convert latitude/longitude to relative centimeter coordinates
pub fn latlon_to_cm_relative(
    lat: f64,
    lon: f64,
    lat_avg: f64,
) -> (DistCm, DistCm) {
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

    (dx_cm as DistCm, dy_cm as DistCm)
}

/// Compute average latitude from a set of GPS coordinates
pub fn compute_lat_avg(points: &[(f64, f64)]) -> f64 {
    if points.is_empty() {
        return 25.0; // Default Taiwan
    }

    let sum: f64 = points.iter().map(|(lat, _)| lat).sum();
    sum / points.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latlon_to_cm_relative_origin() {
        let (dx, dy) = latlon_to_cm_relative(
            FIXED_ORIGIN_LAT_DEG,
            FIXED_ORIGIN_LON_DEG,
            FIXED_ORIGIN_LAT_DEG,
        );
        assert!(dx.abs() < 10);
        assert!(dy.abs() < 10);
    }
}

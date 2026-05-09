//! Heading-constrained map matching

pub mod heading;
pub mod projection;
pub mod search;

// Re-exports for backward compatibility
pub use heading::heading_eligible;
pub use projection::{distance_to_segment_squared, project_to_route, segment_score};
pub use search::{
    find_best_segment_grid_only,
    find_best_segment_grid_only_with_min_max_s,
    find_best_segment_grid_only_with_min_s,
    find_best_segment_restricted,
    SRange,
};

use shared::DistCm;

// Import libm functions for no_std
#[cfg(not(feature = "std"))]
use libm::{cos as f64_cos, round as f64_round};

// Helper functions for floating-point operations
#[cfg(feature = "std")]
fn f64_cos(x: f64) -> f64 {
    x.cos()
}
#[cfg(feature = "std")]
fn f64_round(x: f64) -> f64 {
    x.round()
}

// Helper for to_radians
fn to_radians_compat(degrees: f64) -> f64 {
    degrees * core::f64::consts::PI / 180.0
}

/// Convert lat/lon to absolute cm coordinates with specified average latitude
/// This matches the projection used by the preprocessor
pub fn latlon_to_cm_absolute_with_lat_avg(
    lat: f64,
    lon: f64,
    lat_avg_deg: f64,
) -> (DistCm, DistCm) {
    use shared::{EARTH_R_CM, FIXED_ORIGIN_LON_DEG};

    let lat_rad = to_radians_compat(lat);
    let lon_rad = to_radians_compat(lon);
    let lat_avg_rad = to_radians_compat(lat_avg_deg);
    let cos_lat = f64_cos(lat_avg_rad);

    let x_abs = EARTH_R_CM * lon_rad * cos_lat;
    let y_abs = EARTH_R_CM * lat_rad;

    let x0_abs = (to_radians_compat(FIXED_ORIGIN_LON_DEG) * EARTH_R_CM) * cos_lat;
    let y0_abs = shared::FIXED_ORIGIN_Y_CM as f64;

    let dx_cm = f64_round(x_abs - x0_abs) as i64;
    let dy_cm = f64_round(y_abs - y0_abs) as i64;

    (dx_cm as DistCm, dy_cm as DistCm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::binfile::{BusError, RouteData};
    use shared::{RouteNode, SpatialGrid, Stop};

    /// Create minimal test route data with specified segments.
    /// Returns loaded RouteData ready for testing.
    fn create_test_route_data(segments: &[(i32, i32, i16, i32)]) -> Result<RouteData<'static>, BusError> {
        let mut nodes: Vec<RouteNode> = Vec::new();
        let mut cum_dist = 0;
        for &(x, y, heading, len_mm) in segments.iter() {
            let dx_cm = len_mm / 10; // Each segment's dx = its length in cm

            nodes.push(RouteNode {
                x_cm: x,
                y_cm: y,
                cum_dist_cm: cum_dist,
                heading_cdeg: heading,
                seg_len_mm: len_mm,
                dx_cm: dx_cm as i16,
                dy_cm: 0,
                _pad: 0,
            });

            cum_dist += dx_cm;
        }

        let stops: Vec<Stop> = vec![];

        // Create a proper grid that maps segments to cells
        // Grid size: 1000 cm (10 m), enough resolution for our test
        let grid_size_cm = 1000;

        // Calculate grid bounds
        let max_x = segments.iter().map(|(x, _, _, _)| *x).max().unwrap_or(0);
        let max_y = segments.iter().map(|(_, y, _, _)| *y).max().unwrap_or(0);
        let min_x = segments.iter().map(|(x, _, _, _)| *x).min().unwrap_or(0);
        let min_y = segments.iter().map(|(_, y, _, _)| *y).min().unwrap_or(0);

        let cols = ((max_x - min_x) / grid_size_cm + 1) as u32;
        let rows = ((max_y - min_y) / grid_size_cm + 1) as u32;

        // Populate grid cells with segment indices
        let mut cells: Vec<Vec<usize>> = vec![vec![]; (cols * rows) as usize];
        for (i, &(x, y, _, _)) in segments.iter().enumerate() {
            let gx = ((x - min_x) / grid_size_cm) as usize;
            let gy = ((y - min_y) / grid_size_cm) as usize;
            let cell_idx = gy * cols as usize + gx;
            if cell_idx < cells.len() {
                cells[cell_idx].push(i);
            }
        }

        let grid = SpatialGrid {
            cells,
            grid_size_cm,
            cols,
            rows,
            x0_cm: min_x,
            y0_cm: min_y,
        };

        let mut buffer = Vec::new();
        shared::binfile::pack_route_data(&nodes, &stops, &grid, 25.0, &mut buffer)?;

        // Leak the buffer to get 'static lifetime (safe for tests)
        let leaked: &'static [u8] = Box::leak(buffer.into_boxed_slice());
        RouteData::load(leaked)
    }

    #[test]
    fn test_segment_score_is_pure_distance() {
        let seg = RouteNode {
            x_cm: 100000,
            y_cm: 100000,
            cum_dist_cm: 0,
            heading_cdeg: 9000,
            seg_len_mm: 20000, // 200cm long, enough for our test
            dx_cm: 200,
            dy_cm: 0,
            _pad: 0,
        };

        // Same position: score should be 0 regardless of any external heading
        let score = segment_score(100000, 100000, &seg);
        assert_eq!(score, 0);

        // Different position: score is pure distance squared
        let score_far = segment_score(100500, 100000, &seg); // 500 cm away from segment start
        assert_eq!(score_far, 245_025); // Actual distance squared to segment
    }

    #[test]
    fn test_distance_to_segment_squared_on_segment() {
        let seg = RouteNode {
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            heading_cdeg: 0,
            seg_len_mm: 10_000, // 1000 cm = 10 m (1000 cm * 10 = 10,000 mm)
            dx_cm: 1000,
            dy_cm: 0,
            _pad: 0,
        };

        // Point on segment
        let d2 = distance_to_segment_squared(500, 0, &seg);
        assert_eq!(d2, 0); // Perpendicular distance is 0

        // Point at start
        let d2 = distance_to_segment_squared(0, 0, &seg);
        assert_eq!(d2, 0);
    }

    #[test]
    fn test_distance_to_segment_squared_perpendicular() {
        let seg = RouteNode {
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            heading_cdeg: 0,
            seg_len_mm: 10_000, // 10 m (1000 cm * 10 = 10,000 mm)
            dx_cm: 1000,
            dy_cm: 0,
            _pad: 0,
        };

        // Point 300 cm away perpendicular to segment
        let d2 = distance_to_segment_squared(500, 300, &seg);
        assert_eq!(d2, 90_000); // 300² = 90,000 cm²
    }

    #[test]
    fn test_distance_to_segment_squared_clamped_before() {
        let seg = RouteNode {
            x_cm: 1000,
            y_cm: 0,
            cum_dist_cm: 0,
            heading_cdeg: 0,
            seg_len_mm: 10_000, // 10 m (1000 cm * 10 = 10,000 mm)
            dx_cm: 1000,
            dy_cm: 0,
            _pad: 0,
        };

        // Point before segment start (at x=0)
        let d2 = distance_to_segment_squared(0, 0, &seg);
        assert_eq!(d2, 1_000_000); // (1000)² = 1,000,000 cm²
    }

    #[test]
    fn test_distance_to_segment_squared_zero_length() {
        let seg = RouteNode {
            x_cm: 1000,
            y_cm: 1000,
            cum_dist_cm: 0,
            heading_cdeg: 0,
            seg_len_mm: 0, // Zero length
            dx_cm: 0,
            dy_cm: 0,
            _pad: 0,
        };

        // Distance to point for zero-length segment
        let d2 = distance_to_segment_squared(1200, 1300, &seg);
        assert_eq!(d2, 200 * 200 + 300 * 300); // sqrt(200² + 300²)²
    }

    #[test]
    fn test_project_to_route_on_segment() {
        let route_data = create_test_route_data(&[
            (0, 0, 0, 100_000),   // 10 m segment, cum_dist = 0
            (10000, 0, 0, 100_000), // cum_dist = 10,000
        ])
        .unwrap();

        // Project point at start of segment 0
        let s = project_to_route(0, 0, 0, &route_data);
        assert_eq!(s, 0);

        // Project point at end of segment 0
        let s = project_to_route(10000, 0, 0, &route_data);
        assert_eq!(s, 10_000);
    }

    #[test]
    fn test_project_to_route_mid_segment() {
        let route_data = create_test_route_data(&[(0, 0, 0, 100_000)]).unwrap();

        // Project point at middle of segment
        let s = project_to_route(5000, 0, 0, &route_data);
        assert_eq!(s, 5_000); // Halfway through 10 m segment
    }

    #[test]
    fn test_latlon_to_cm_absolute_with_lat_avg() {
        // Test known conversion
        let (x, y) = latlon_to_cm_absolute_with_lat_avg(25.0, 121.0, 25.0);
        // Should produce non-zero coordinates
        assert!(x != 0 || y != 0);
    }

    #[test]
    fn test_latlon_to_cm_roundtrip() {
        // If we convert and "unconvert", we should get close to original
        // This is a sanity check, not exact roundtrip
        let (x1, y1) = latlon_to_cm_absolute_with_lat_avg(25.0, 121.0, 25.0);
        let (x2, y2) = latlon_to_cm_absolute_with_lat_avg(25.001, 121.001, 25.0);

        // Small change in lat/lon should produce change in x/y
        assert!(x2 != x1 || y2 != y1);
    }
}

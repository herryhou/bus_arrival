//! Common test utilities for preprocessor integration tests
//!
//! This module provides shared helper functions, route builders, and assertion
//! utilities used across multiple test files. Following the BDD (Given-When-Then)
//! style, these helpers support creating test scenarios with clear setup and
//! verification steps.

use shared::RouteNode;

// ============================================================================
// Constants
// ============================================================================

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

/// Default grid size for spatial indexing (100m = 10000cm)
pub const GRID_SIZE_CM: i32 = 10000;

/// Default K value for DP mapper (candidates per stop)
pub const DEFAULT_K: usize = 15;

/// Stop protection radius in cm (30m = 3000cm)
pub const STOP_PROTECTION_RADIUS_CM: f64 = 3000.0;

/// Maximum segment length constraint in cm (30m = 3000cm)
pub const MAX_SEGMENT_LENGTH_CM: i32 = 3000;

// ============================================================================
// Route Builders
// ============================================================================

/// Create a straight horizontal route
///
/// # Arguments
/// * `length_cm` - Total route length in centimeters
/// * `num_segments` - Number of segments to divide the route into
///
/// # Returns
/// A vector of RouteNodes representing a straight eastbound route
pub fn make_straight_route(length_cm: i32, num_segments: usize) -> Vec<RouteNode> {
    let seg_len = length_cm / num_segments as i32;
    let mut nodes = Vec::with_capacity(num_segments + 1);

    for i in 0..=num_segments {
        let x_cm = i as i32 * seg_len;
        let cum_dist = x_cm;
        let dx_cm = if i < num_segments { seg_len } else { 0 };
        let len2_cm2 = if i < num_segments {
            (seg_len as i64) * (seg_len as i64)
        } else {
            0
        };

        nodes.push(RouteNode {
            len2_cm2,
            heading_cdeg: 0,
            _pad: 0,
            x_cm,
            y_cm: 0,
            cum_dist_cm: cum_dist,
            dx_cm,
            dy_cm: 0,
            seg_len_cm: if i < num_segments { seg_len } else { 0 },
        });
    }

    nodes
}

/// Create an L-shaped route (east, then north)
///
/// # Arguments
/// * `horizontal_cm` - Length of horizontal segment in cm
/// * `vertical_cm` - Length of vertical segment in cm
///
/// # Returns
/// A vector of RouteNodes forming an L shape starting at origin
pub fn make_l_route(horizontal_cm: i32, vertical_cm: i32) -> Vec<RouteNode> {
    vec![
        // Start point
        RouteNode {
            len2_cm2: (horizontal_cm as i64) * (horizontal_cm as i64),
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            dx_cm: horizontal_cm,
            dy_cm: 0,
            seg_len_cm: horizontal_cm,
        },
        // Corner
        RouteNode {
            len2_cm2: (vertical_cm as i64) * (vertical_cm as i64),
            heading_cdeg: 9000,
            _pad: 0,
            x_cm: horizontal_cm,
            y_cm: 0,
            cum_dist_cm: horizontal_cm,
            dx_cm: 0,
            dy_cm: vertical_cm,
            seg_len_cm: vertical_cm,
        },
        // End point
        RouteNode {
            len2_cm2: 0,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: horizontal_cm,
            y_cm: vertical_cm,
            cum_dist_cm: horizontal_cm + vertical_cm,
            dx_cm: 0,
            dy_cm: 0,
            seg_len_cm: 0,
        },
    ]
}

/// Create a U-shaped route (east, north, west back to same x)
///
/// # Arguments
/// * `horizontal_cm` - Length of each horizontal leg in cm
/// * `vertical_cm` - Length of vertical connector in cm
///
/// # Returns
/// A vector of RouteNodes forming a U shape
pub fn make_u_route(horizontal_cm: i32, vertical_cm: i32) -> Vec<RouteNode> {
    vec![
        // Start - bottom left
        RouteNode {
            len2_cm2: (horizontal_cm as i64) * (horizontal_cm as i64),
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            dx_cm: horizontal_cm,
            dy_cm: 0,
            seg_len_cm: horizontal_cm,
        },
        // Bottom right
        RouteNode {
            len2_cm2: (vertical_cm as i64) * (vertical_cm as i64),
            heading_cdeg: 9000,
            _pad: 0,
            x_cm: horizontal_cm,
            y_cm: 0,
            cum_dist_cm: horizontal_cm,
            dx_cm: 0,
            dy_cm: vertical_cm,
            seg_len_cm: vertical_cm,
        },
        // Top right
        RouteNode {
            len2_cm2: (horizontal_cm as i64) * (horizontal_cm as i64),
            heading_cdeg: 18000,
            _pad: 0,
            x_cm: horizontal_cm,
            y_cm: vertical_cm,
            cum_dist_cm: horizontal_cm + vertical_cm,
            dx_cm: -horizontal_cm,
            dy_cm: 0,
            seg_len_cm: horizontal_cm,
        },
        // End - top left (returns to x=0)
        RouteNode {
            len2_cm2: 0,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 0,
            y_cm: vertical_cm,
            cum_dist_cm: 2 * horizontal_cm + vertical_cm,
            dx_cm: 0,
            dy_cm: 0,
            seg_len_cm: 0,
        },
    ]
}

/// Create a figure-8 route that crosses itself at the origin
///
/// # Arguments
/// * `size_cm` - Size of each loop in cm
///
/// # Returns
/// A vector of RouteNodes forming a figure-8 pattern
pub fn make_figure8_route(size_cm: i32) -> Vec<RouteNode> {
    vec![
        // Loop 1: Counter-clockwise from origin
        RouteNode {
            len2_cm2: (size_cm as i64) * (size_cm as i64),
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            dx_cm: size_cm,
            dy_cm: 0,
            seg_len_cm: size_cm,
        },
        RouteNode {
            len2_cm2: (size_cm as i64) * (size_cm as i64),
            heading_cdeg: 9000,
            _pad: 0,
            x_cm: size_cm,
            y_cm: 0,
            cum_dist_cm: size_cm,
            dx_cm: 0,
            dy_cm: size_cm,
            seg_len_cm: size_cm,
        },
        RouteNode {
            len2_cm2: (size_cm as i64) * (size_cm as i64),
            heading_cdeg: 18000,
            _pad: 0,
            x_cm: size_cm,
            y_cm: size_cm,
            cum_dist_cm: 2 * size_cm,
            dx_cm: -size_cm,
            dy_cm: 0,
            seg_len_cm: size_cm,
        },
        // Back to origin
        RouteNode {
            len2_cm2: (size_cm as i64) * (size_cm as i64),
            heading_cdeg: 0,
            _pad: 0,
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 3 * size_cm,
            dx_cm: size_cm,
            dy_cm: 0,
            seg_len_cm: size_cm,
        },
        // Loop 2: Clockwise into quadrant IV
        RouteNode {
            len2_cm2: (size_cm as i64) * (size_cm as i64),
            heading_cdeg: -9000,
            _pad: 0,
            x_cm: size_cm,
            y_cm: 0,
            cum_dist_cm: 4 * size_cm,
            dx_cm: 0,
            dy_cm: -size_cm,
            seg_len_cm: size_cm,
        },
        RouteNode {
            len2_cm2: 0,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: size_cm,
            y_cm: -size_cm,
            cum_dist_cm: 5 * size_cm,
            dx_cm: 0,
            dy_cm: 0,
            seg_len_cm: 0,
        },
    ]
}

/// Create a zig-zag route with alternating 90-degree turns
///
/// # Arguments
/// * `num_segments` - Number of segments in the route
/// * `segment_length_cm` - Length of each segment in cm
///
/// # Returns
/// A vector of RouteNodes forming a zig-zag pattern
pub fn make_zigzag_route(num_segments: usize, segment_length_cm: i32) -> Vec<RouteNode> {
    let mut nodes = Vec::with_capacity(num_segments + 1);
    let mut cum_dist_cm = 0;
    let mut x_cm = 0;
    let mut y_cm = 0;
    let mut going_east = true;

    for i in 0..=num_segments {
        let is_last = i == num_segments;

        let (dx_cm, dy_cm, heading_cdeg) = if is_last {
            (0, 0, 0)
        } else if going_east {
            (segment_length_cm, 0, 0)
        } else {
            (0, segment_length_cm, 9000)
        };

        let seg_len_cm = if is_last { 0 } else { segment_length_cm };
        let len2_cm2 = if is_last {
            0
        } else {
            (segment_length_cm as i64) * (segment_length_cm as i64)
        };

        nodes.push(RouteNode {
            len2_cm2,
            heading_cdeg,
            _pad: 0,
            x_cm,
            y_cm,
            cum_dist_cm,
            dx_cm,
            dy_cm,
            seg_len_cm,
        });

        if !is_last {
            cum_dist_cm += seg_len_cm;
            x_cm += dx_cm;
            y_cm += dy_cm;
            going_east = !going_east;
        }
    }

    nodes
}

/// Create a route from a list of (x, y) coordinates
///
/// # Arguments
/// * `points` - Slice of (x_cm, y_cm) tuples
///
/// # Returns
/// A vector of RouteNodes with computed geometric properties
pub fn make_route_from_points(points: &[(i32, i32)]) -> Vec<RouteNode> {
    if points.is_empty() {
        return vec![];
    }

    if points.len() == 1 {
        return vec![RouteNode {
            len2_cm2: 0,
            heading_cdeg: 0,
            _pad: 0,
            x_cm: points[0].0,
            y_cm: points[0].1,
            cum_dist_cm: 0,
            dx_cm: 0,
            dy_cm: 0,
            seg_len_cm: 0,
        }];
    }

    let mut nodes = Vec::with_capacity(points.len());
    let mut cum_dist_cm = 0;

    for (i, &(x_cm, y_cm)) in points.iter().enumerate() {
        let is_last = i == points.len() - 1;

        if is_last {
            nodes.push(RouteNode {
                len2_cm2: 0,
                heading_cdeg: 0,
                _pad: 0,
                x_cm,
                y_cm,
                cum_dist_cm,
                dx_cm: 0,
                dy_cm: 0,
                seg_len_cm: 0,
            });
        } else {
            let next_point = points[i + 1];
            let dx_cm = next_point.0 - x_cm;
            let dy_cm = next_point.1 - y_cm;
            let len2_cm2 = (dx_cm as i64) * (dx_cm as i64) + (dy_cm as i64) * (dy_cm as i64);
            let seg_len_cm = (len2_cm2 as f64).sqrt().round() as i32;
            let heading_rad = (dx_cm as f64).atan2(dy_cm as f64);
            let heading_cdeg = (heading_rad.to_degrees() * 100.0).round() as i16;

            nodes.push(RouteNode {
                len2_cm2,
                heading_cdeg,
                _pad: 0,
                x_cm,
                y_cm,
                cum_dist_cm,
                dx_cm,
                dy_cm,
                seg_len_cm,
            });

            cum_dist_cm += seg_len_cm;
        }
    }

    nodes
}

// ============================================================================
// Coordinate Helpers
// ============================================================================

/// Convert latitude/longitude to relative centimeter coordinates
///
/// Uses the same fixed-origin local flat-earth approximation as the
/// coordinate conversion module.
///
/// # Arguments
/// * `lat` - Latitude in degrees
/// * `lon` - Longitude in degrees
/// * `lat_avg` - Average latitude for the route (used for cos(lat) factor)
///
/// # Returns
/// A tuple of (x_cm, y_cm) relative coordinates
pub fn latlon_to_cm(lat: f64, lon: f64, lat_avg: f64) -> (i32, i32) {
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

    (dx_cm as i32, dy_cm as i32)
}

/// Compute average latitude from GPS coordinates
///
/// # Arguments
/// * `points` - Slice of (lat, lon) tuples
///
/// # Returns
/// The average latitude in degrees
pub fn compute_lat_avg(points: &[(f64, f64)]) -> f64 {
    if points.is_empty() {
        return 25.0; // Default for Taiwan region
    }

    let sum: f64 = points.iter().map(|(lat, _)| lat).sum();
    sum / points.len() as f64
}

/// Find the (x, y) position on a route at a given progress value
///
/// # Arguments
/// * `route` - Slice of RouteNodes
/// * `progress_cm` - Progress value in centimeters
///
/// # Returns
/// A tuple of (x_cm, y_cm) at the given progress
pub fn position_at_progress(route: &[RouteNode], progress_cm: i32) -> (i32, i32) {
    if route.is_empty() {
        return (0, 0);
    }

    // Find the segment containing this progress value
    let mut seg_idx = 0;
    for (i, node) in route.iter().enumerate() {
        if node.cum_dist_cm <= progress_cm {
            seg_idx = i;
        } else {
            break;
        }
    }

    // Guard against edge case where progress exceeds last segment
    if seg_idx >= route.len().saturating_sub(1) {
        seg_idx = route.len().saturating_sub(1);
    }

    let node = &route[seg_idx];

    // How far into this segment?
    let offset_cm = progress_cm - node.cum_dist_cm;

    // t = offset / seg_len (clamped to [0, 1])
    let t = if node.seg_len_cm > 0 {
        (offset_cm as f64 / node.seg_len_cm as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };

    // Interpolate position
    let x = node.x_cm as f64 + t * node.dx_cm as f64;
    let y = node.y_cm as f64 + t * node.dy_cm as f64;

    (x.round() as i32, y.round() as i32)
}

// ============================================================================
// Assertion Helpers
// ============================================================================

/// Assert that progress values are monotonically non-decreasing
///
/// # Arguments
/// * `candidates` - Slice of progress values or objects with progress_cm field
/// * `context` - Description string for error messages
pub fn assert_monotonic_progress<T>(items: &[T], context: &str)
where
    T: AsRef<i32>,
{
    for i in 0..items.len().saturating_sub(1) {
        let current = *items[i].as_ref();
        let next = *items[i + 1].as_ref();
        assert!(
            current <= next,
            "{}: monotonicity violated at index {}: {} > {}",
            context,
            i,
            current,
            next
        );
    }
}

/// Assert that all stops are mapped within a maximum distance of their route position
///
/// # Arguments
/// * `stops` - Original stop coordinates as (x, y) tuples
/// * `route` - Route nodes
/// * `results` - Mapped progress values for each stop
/// * `max_dist_cm` - Maximum allowed distance in cm
/// * `context` - Description string for error messages
pub fn assert_geometric_validity(
    stops: &[(i64, i64)],
    route: &[RouteNode],
    results: &[i32],
    max_dist_cm: i64,
    context: &str,
) {
    let mut max_actual_dist_cm = 0i64;
    let mut total_dist_cm = 0i64;

    for (i, (stop, progress_cm)) in stops.iter().zip(results.iter()).enumerate() {
        let mapped_pos = position_at_progress(route, *progress_cm);

        let dx = stop.0 - mapped_pos.0 as i64;
        let dy = stop.1 - mapped_pos.1 as i64;
        let dist_sq_cm2 = dx * dx + dy * dy;
        let dist_cm = (dist_sq_cm2 as f64).sqrt().round() as i64;

        max_actual_dist_cm = max_actual_dist_cm.max(dist_cm);
        total_dist_cm += dist_cm;

        assert!(
            dist_cm <= max_dist_cm,
            "{}: stop {} mapped too far from actual position: {}cm (max: {}cm). Stop: {:?}, mapped: {:?}",
            context,
            i,
            dist_cm,
            max_dist_cm,
            stop,
            mapped_pos
        );
    }

    let avg_dist_cm = total_dist_cm / results.len() as i64;
    println!(
        "{}: geometric validation - avg_dist: {}cm, max_dist: {}cm",
        context,
        avg_dist_cm,
        max_actual_dist_cm
    );
}

/// Assert that all progress values are within route bounds
///
/// # Arguments
/// * `progress_values` - Slice of progress values
/// * `route_length_cm` - Total route length in cm
/// * `context` - Description string for error messages
pub fn assert_within_route_bounds(
    progress_values: &[i32],
    route_length_cm: i32,
    context: &str,
) {
    for (i, &progress) in progress_values.iter().enumerate() {
        assert!(
            progress >= 0 && progress <= route_length_cm,
            "{}: progress at index {} out of bounds: {} (route length: {})",
            context,
            i,
            progress,
            route_length_cm
        );
    }
}

// ============================================================================
// Grid Helpers
// ============================================================================

/// Build a spatial grid for testing
///
/// This is a convenience wrapper around the grid builder.
///
/// # Arguments
/// * `route` - Route nodes
/// * `grid_size_cm` - Grid cell size in cm
///
/// # Returns
/// A SpatialGrid for querying segments
#[cfg(feature = "dp_mapper")]
pub fn make_test_grid(route: &[RouteNode], grid_size_cm: i32) -> dp_mapper::grid::SpatialGrid {
    dp_mapper::grid::build_grid(route, grid_size_cm)
}

// ============================================================================
// Distance Helpers
// ============================================================================

/// Calculate Euclidean distance between two points in cm
///
/// # Arguments
/// * `p1` - First point (x, y)
/// * `p2` - Second point (x, y)
///
/// # Returns
/// Distance in centimeters
pub fn distance_cm(p1: (i64, i64), p2: (i64, i64)) -> f64 {
    let dx = p2.0 - p1.0;
    let dy = p2.1 - p1.1;
    ((dx * dx + dy * dy) as f64).sqrt()
}

/// Calculate squared distance (faster for comparisons)
///
/// # Arguments
/// * `p1` - First point (x, y)
/// * `p2` - Second point (x, y)
///
/// # Returns
/// Squared distance in cm²
pub fn distance_sq_cm2(p1: (i64, i64), p2: (i64, i64)) -> i64 {
    let dx = p2.0 - p1.0;
    let dy = p2.1 - p1.1;
    dx * dx + dy * dy
}

//! Heading-constrained map matching

use shared::binfile::RouteData;
use shared::{Dist2, DistCm, HeadCdeg, RouteNode, SpeedCms};

use crate::SIGMA_GPS_CM;

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

/// Find best route segment for GPS point with preference for segments near last_idx
pub fn find_best_segment_restricted(
    gps_x: DistCm,
    gps_y: DistCm,
    gps_heading: HeadCdeg,
    gps_speed: SpeedCms,
    route_data: &RouteData,
    last_idx: usize,
) -> usize {
    // 1. First, search in a small window ahead of last_idx
    // Window: [last_idx - 2, last_idx + 10]
    const WINDOW_BACK: usize = 2; // GPS 雜訊造成的最大表觀後退路段數
    const WINDOW_FWD: usize = 10; // V_MAX(3000 cm/s) × 最大路段長(~30m) 的 10× 緩衝

    let start = last_idx.saturating_sub(WINDOW_BACK);
    let end = (last_idx + WINDOW_FWD).min(route_data.node_count.saturating_sub(1));

    let mut best_idx = last_idx;
    let mut best_score = i64::MAX;
    let mut found_in_window = false;

    for idx in start..=end {
        if let Some(seg) = route_data.get_node(idx) {
            let score = segment_score(gps_x, gps_y, gps_heading, gps_speed, &seg);
            if score < best_score {
                best_score = score;
                best_idx = idx;
                found_in_window = true;
            }
        }
    }

    // 2. If window score is acceptable, use it (e.g. distance < 50m)
    // 50m squared = 5000^2 = 25,000,000
    const MAX_DIST_SQRT: i64 = SIGMA_GPS_CM as i64 * SIGMA_GPS_CM as i64;
    if found_in_window && best_score < MAX_DIST_SQRT {
        return best_idx;
    }

    // 3. Fallback: Full grid query
    // Guard against GPS outside bounding box (cold start, GPS jump)
    if gps_x < route_data.x0_cm || gps_y < route_data.y0_cm {
        return last_idx;  // Conservative fallback
    }

    let gx = ((gps_x - route_data.x0_cm) / route_data.grid.grid_size_cm) as u32;
    let gy = ((gps_y - route_data.y0_cm) / route_data.grid.grid_size_cm) as u32;

    // Search 3x3 grid neighborhood
    for dy in 0..=2 {
        for dx in 0..=2 {
            let ny = gy as i32 + dy as i32 - 1;
            let nx = gx as i32 + dx as i32 - 1;
            if ny < 0 || nx < 0 {
                continue;
            }

            if let Ok(cell_indices) = route_data.grid.get_cell(nx as u32, ny as u32) {
                for &idx in cell_indices {
                    if let Some(seg) = route_data.get_node(idx as usize) {
                        let score = segment_score(gps_x, gps_y, gps_heading, gps_speed, &seg);
                        if score < best_score {
                            best_score = score;
                            best_idx = idx as usize;
                        }
                    }
                }
            }
        }
    }

    best_idx
}

/// Heading-weighted segment score
pub fn segment_score(
    gps_x: DistCm,
    gps_y: DistCm,
    gps_heading: HeadCdeg,
    gps_speed: SpeedCms,
    seg: &RouteNode,
) -> i64 {
    // Distance squared to segment
    let dist2 = distance_to_segment_squared(gps_x, gps_y, seg);

    // Heading penalty - skip when heading unavailable (GGA-only mode)
    let heading_penalty = if gps_heading != i16::MIN {
        let heading_diff = heading_diff_cdeg(gps_heading, seg.heading_cdeg);
        let w = heading_weight(gps_speed);
        ((heading_diff as i64).pow(2) * w as i64) >> 8
    } else {
        0  // No heading penalty when unavailable
    };

    dist2 + heading_penalty
}

/// Heading weight: 0 at v=0, 256 at v≥83 cm/s (3 km/h)
fn heading_weight(v_cms: SpeedCms) -> i32 {
    ((v_cms * 256) / 83).min(256)
}

/// Calculate heading difference (shortest around 360°)
fn heading_diff_cdeg(a: HeadCdeg, b: HeadCdeg) -> HeadCdeg {
    let diff = (a as i32 - b as i32).unsigned_abs() % 36000;
    if diff > 18000 {
        (36000 - diff) as HeadCdeg
    } else {
        diff as HeadCdeg
    }
}

/// Distance squared from point to segment (clamped projection)
/// v8.7: Computes len2 from seg_len_mm: (seg_len_mm / 10)^2
fn distance_to_segment_squared(x: DistCm, y: DistCm, seg: &RouteNode) -> Dist2 {
    let dx = x - seg.x_cm;
    let dy = y - seg.y_cm;

    // Compute len2_cm2 from seg_len_mm: (mm / 10)^2 = cm^2
    let seg_len_cm = seg.seg_len_mm / 10;
    let len2_cm2 = (seg_len_cm as i64) * (seg_len_cm as i64);

    // t = dot(point - P[i], segment) / |segment|²
    let t_num = dx as i64 * seg.dx_cm as i64 + dy as i64 * seg.dy_cm as i64;

    if len2_cm2 == 0 {
        return ((x - seg.x_cm) as i64).pow(2) + ((y - seg.y_cm) as i64).pow(2);
    }

    let t = if t_num < 0 {
        0
    } else if t_num > len2_cm2 {
        len2_cm2
    } else {
        t_num
    };

    // Projected point
    let px = seg.x_cm + ((t * seg.dx_cm as i64 / len2_cm2) as DistCm);
    let py = seg.y_cm + ((t * seg.dy_cm as i64 / len2_cm2) as DistCm);

    // Distance squared
    ((x - px) as i64).pow(2) + ((y - py) as i64).pow(2)
}

/// Project GPS point onto segment → route progress
/// v8.7: Uses seg_len_mm for length computation
pub fn project_to_route(
    gps_x: DistCm,
    gps_y: DistCm,
    seg_idx: usize,
    route_data: &RouteData,
) -> DistCm {
    let seg = route_data.get_node(seg_idx).unwrap_or_else(|| {
        // Fallback to first node if index is invalid
        route_data.get_node(0).unwrap()
    });

    let dx = gps_x - seg.x_cm;
    let dy = gps_y - seg.y_cm;
    let t_num = dx as i64 * seg.dx_cm as i64 + dy as i64 * seg.dy_cm as i64;

    // Compute len2_cm2 from seg_len_mm: (mm / 10)^2 = cm^2
    let seg_len_cm = seg.seg_len_mm / 10;
    let len2_cm2 = (seg_len_cm as i64) * (seg_len_cm as i64);

    if len2_cm2 == 0 {
        return seg.cum_dist_cm;
    }

    let t = if t_num < 0 {
        0
    } else if t_num > len2_cm2 {
        len2_cm2
    } else {
        t_num
    };

    // z = cum_dist[i] + t × seg_len_cm / len2_cm2
    let base = seg.cum_dist_cm;
    base + ((t * seg_len_cm as i64 / len2_cm2) as DistCm)
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

    #[test]
    fn test_segment_score_heading_sentinel() {
        // When heading is i16::MIN (GGA-only mode), heading penalty should be 0
        let seg = RouteNode {
            x_cm: 100000,
            y_cm: 100000,
            cum_dist_cm: 0,
            heading_cdeg: 9000, // 90 degrees
            seg_len_mm: 10000,
            dx_cm: 100,
            dy_cm: 0,
            _pad: 0,
        };

        // With valid heading
        let score_with_heading = segment_score(
            100000, 100000,  // GPS position at segment
            9000,             // Same heading
            100,              // Low speed
            &seg,
        );

        // With sentinel heading (i16::MIN) - should have no heading penalty
        let score_sentinel = segment_score(
            100000, 100000,
            i16::MIN,         // Sentinel value
            100,
            &seg,
        );

        // With sentinel, heading penalty is 0, so score should be <= score with heading
        assert!(score_sentinel <= score_with_heading,
            "Sentinel heading should not add penalty");

        // At high speed, valid heading should have significant penalty if mismatched
        let score_mismatch = segment_score(
            100000, 100000,
            0,                // Opposite heading
            500,              // High speed
            &seg,
        );

        let score_sentinel_high_speed = segment_score(
            100000, 100000,
            i16::MIN,
            500,
            &seg,
        );

        // Sentinel should have lower score than mismatched heading at high speed
        assert!(score_sentinel_high_speed < score_mismatch,
            "Sentinel should avoid heading penalty entirely");
    }
}

//! Heading-constrained map matching

use shared::{RouteNode, HeadCdeg, SpeedCms, DistCm, Dist2};

/// Find best route segment for GPS point
pub fn find_best_segment(
    gps_x: DistCm,
    gps_y: DistCm,
    gps_heading: HeadCdeg,
    gps_speed: SpeedCms,
    grid: &shared::SpatialGrid,
    nodes: &[RouteNode],
) -> usize {
    let candidates = grid.query(gps_x, gps_y);

    if candidates.is_empty() {
        return 0;
    }

    let mut best_idx = candidates[0];
    let mut best_score = i64::MAX;

    for &idx in &candidates {
        let seg = &nodes[idx];
        let score = segment_score(gps_x, gps_y, gps_heading, gps_speed, seg);
        if score < best_score {
            best_score = score;
            best_idx = idx;
        }
    }

    best_idx
}

/// Heading-weighted segment score
fn segment_score(
    gps_x: DistCm,
    gps_y: DistCm,
    gps_heading: HeadCdeg,
    gps_speed: SpeedCms,
    seg: &RouteNode,
) -> i64 {
    // Distance squared to segment
    let dist2 = distance_to_segment_squared(gps_x, gps_y, seg);

    // Heading penalty with speed ramp
    let heading_diff = heading_diff_cdeg(gps_heading, seg.heading_cdeg);
    let w = heading_weight(gps_speed);
    let penalty = (heading_diff.pow(2) as i64 * w as i64) >> 8;

    dist2 + penalty
}

/// Heading weight: 0 at v=0, 256 at v≥83 cm/s (3 km/h)
fn heading_weight(v_cms: SpeedCms) -> i32 {
    ((v_cms * 256) / 83).min(256)
}

/// Calculate heading difference (shortest around 360°)
fn heading_diff_cdeg(a: HeadCdeg, b: HeadCdeg) -> HeadCdeg {
    let diff = (a as i32 - b as i32).unsigned_abs() % 36000;
    if diff > 18000 { (36000 - diff) as HeadCdeg } else { diff as HeadCdeg }
}

/// Distance squared from point to segment (clamped projection)
fn distance_to_segment_squared(x: DistCm, y: DistCm, seg: &RouteNode) -> Dist2 {
    let dx = x - seg.x_cm;
    let dy = y - seg.y_cm;

    // t = dot(point - P[i], segment) / |segment|²
    let t_num = (dx as i64 * seg.dx_cm as i64 + dy as i64 * seg.dy_cm as i64);
    let t = if t_num < 0 { 0 } else if t_num > seg.len2_cm2 { seg.len2_cm2 } else { t_num };

    // Projected point
    let px = seg.x_cm + ((t * seg.dx_cm as i64 / seg.len2_cm2) as DistCm);
    let py = seg.y_cm + ((t * seg.dy_cm as i64 / seg.len2_cm2) as DistCm);

    // Distance squared
    ((x - px) as i64).pow(2) + ((y - py) as i64).pow(2)
}

/// Project GPS point onto segment → route progress
pub fn project_to_route(
    gps_x: DistCm,
    gps_y: DistCm,
    seg_idx: usize,
    nodes: &[RouteNode],
) -> DistCm {
    let seg = &nodes[seg_idx];

    // t = dot(gps - P[i], segment) / len2
    let dx = gps_x - seg.x_cm;
    let dy = gps_y - seg.y_cm;
    let t_num = (dx as i64 * seg.dx_cm as i64 + dy as i64 * seg.dy_cm as i64);

    // Clamp t to [0, 1]
    let t = if t_num < 0 { 0 } else if t_num > seg.len2_cm2 { seg.len2_cm2 } else { t_num };

    // z = cum_dist[i] + t × seg_len / len2
    let base = seg.cum_dist_cm - seg.seg_len_cm;
    base + ((t as i64 * seg.seg_len_cm as i64 / seg.len2_cm2) as DistCm)
}

/// Convert lat/lon to relative cm coordinates
pub fn latlon_to_cm_relative(lat: f64, lon: f64, x0_cm: i64, y0_cm: i64) -> (DistCm, DistCm) {
    const R_CM: f64 = 637_100_000.0;
    const FIXED_ORIGIN_LAT_DEG: f64 = 20.0;

    let lat_rad = lat.to_radians();
    let lon_rad = lon.to_radians();
    let lat_avg_rad = FIXED_ORIGIN_LAT_DEG.to_radians();

    let y_abs = (lat_rad * R_CM).round() as i64;
    let x_abs = (lon_rad * lat_avg_rad.cos() * R_CM).round() as i64;

    ((x_abs - x0_cm) as DistCm, (y_abs - y0_cm) as DistCm)
}

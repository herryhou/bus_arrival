//! Heading-constrained map matching

use shared::{RouteNode, HeadCdeg, SpeedCms, DistCm, Dist2};
use shared::binfile::RouteData;

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
    let start = last_idx.saturating_sub(2);
    let end = (last_idx + 10).min(route_data.node_count.saturating_sub(1));
    
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
    if found_in_window && best_score < 25_000_000 {
        return best_idx;
    }

    // 3. Fallback: Full grid query
    let gx = ((gps_x - route_data.x0_cm) / route_data.grid.grid_size_cm) as u32;
    let gy = ((gps_y - route_data.y0_cm) / route_data.grid.grid_size_cm) as u32;

    // Search 3x3 grid neighborhood
    for dy in 0..=2 {
        for dx in 0..=2 {
            let ny = gy as i32 + dy as i32 - 1;
            let nx = gx as i32 + dx as i32 - 1;
            if ny < 0 || nx < 0 { continue; }

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
    let penalty = ((heading_diff as i64).pow(2) * w as i64) >> 8;

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
    let t_num = dx as i64 * seg.dx_cm as i64 + dy as i64 * seg.dy_cm as i64;
    
    if seg.len2_cm2 == 0 {
        return ((x - seg.x_cm) as i64).pow(2) + ((y - seg.y_cm) as i64).pow(2);
    }

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
    route_data: &RouteData,
) -> DistCm {
    let seg = route_data.get_node(seg_idx).unwrap_or_else(|| {
        // Fallback to first node if index is invalid
        route_data.get_node(0).unwrap()
    });

    let dx = gps_x - seg.x_cm;
    let dy = gps_y - seg.y_cm;
    let t_num = dx as i64 * seg.dx_cm as i64 + dy as i64 * seg.dy_cm as i64;

    if seg.len2_cm2 == 0 {
        return seg.cum_dist_cm;
    }

    let t = if t_num < 0 { 0 } else if t_num > seg.len2_cm2 { seg.len2_cm2 } else { t_num };

    // z = cum_dist[i] + t × seg_len / len2
    let base = seg.cum_dist_cm;
    let seg_len = seg.seg_len_cm;
    let len2 = seg.len2_cm2;
    base + ((t as i64 * seg_len as i64 / len2) as DistCm)
}

/// Convert lat/lon to absolute cm coordinates with specified average latitude
/// This matches the projection used by the preprocessor
pub fn latlon_to_cm_absolute_with_lat_avg(lat: f64, lon: f64, lat_avg_deg: f64) -> (DistCm, DistCm) {
    use shared::{EARTH_R_CM, FIXED_ORIGIN_LON_DEG};

    let lat_rad = lat.to_radians();
    let lon_rad = lon.to_radians();
    let lat_avg_rad = lat_avg_deg.to_radians();
    let cos_lat = lat_avg_rad.cos();

    let x_abs = EARTH_R_CM * lon_rad * cos_lat;
    let y_abs = EARTH_R_CM * lat_rad;

    let x0_abs = (FIXED_ORIGIN_LON_DEG.to_radians() * EARTH_R_CM) * cos_lat;
    let y0_abs = shared::FIXED_ORIGIN_Y_CM as f64;

    let dx_cm = (x_abs - x0_abs).round() as i64;
    let dy_cm = (y_abs - y0_abs).round() as i64;

    (dx_cm as DistCm, dy_cm as DistCm)
}

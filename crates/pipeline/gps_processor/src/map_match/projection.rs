//! Distance calculation and route projection for map matching
//!
//! This module provides geometric calculations without knowledge of
//! heading, search strategies, or grid structure.

use shared::{Dist2, DistCm, RouteNode};

/// Distance squared from point to segment (clamped projection)
///
/// Heading is intentionally absent. Heading belongs in the eligibility
/// filter (`heading_eligible`), not in the ranking score. Mixing cm² and
/// cdeg² into one scalar requires an arbitrary scale factor that cannot be
/// derived from first principles.
///
/// The return type is `Dist2` (i64 cm²).
pub fn distance_to_segment_squared(x: DistCm, y: DistCm, seg: &RouteNode) -> Dist2 {
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
pub fn project_to_route(
    gps_x: DistCm,
    gps_y: DistCm,
    seg_idx: usize,
    route_data: &shared::binfile::RouteData,
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

/// Distance-squared from GPS point to segment (clamped projection).
///
/// This is `segment_score` from the original map_match.rs - kept for
/// backward compatibility. Internally delegates to `distance_to_segment_squared`.
pub fn segment_score(gps_x: DistCm, gps_y: DistCm, seg: &RouteNode) -> Dist2 {
    distance_to_segment_squared(gps_x, gps_y, seg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use shared::binfile::{BusError, RouteData};
    use shared::{SpatialGrid, Stop};

    fn create_test_route_data(segments: &[(i32, i32, i16, i32)]) -> Result<RouteData<'static>, BusError> {
        let mut nodes: Vec<RouteNode> = Vec::new();
        let mut cum_dist = 0;
        for &(x, y, heading, len_mm) in segments.iter() {
            let dx_cm = len_mm / 10;

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
        let grid_size_cm = 1000;

        let max_x = segments.iter().map(|(x, _, _, _)| *x).max().unwrap_or(0);
        let max_y = segments.iter().map(|(_, y, _, _)| *y).max().unwrap_or(0);
        let min_x = segments.iter().map(|(x, _, _, _)| *x).min().unwrap_or(0);
        let min_y = segments.iter().map(|(_, y, _, _)| *y).min().unwrap_or(0);

        let cols = ((max_x - min_x) / grid_size_cm + 1) as u32;
        let rows = ((max_y - min_y) / grid_size_cm + 1) as u32;

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
            seg_len_mm: 20000,
            dx_cm: 200,
            dy_cm: 0,
            _pad: 0,
        };

        let score = segment_score(100000, 100000, &seg);
        assert_eq!(score, 0);

        let score_far = segment_score(100500, 100000, &seg);
        assert_eq!(score_far, 245_025);
    }

    #[test]
    fn test_distance_to_segment_squared_on_segment() {
        let seg = RouteNode {
            x_cm: 0,
            y_cm: 0,
            cum_dist_cm: 0,
            heading_cdeg: 0,
            seg_len_mm: 10_000,
            dx_cm: 1000,
            dy_cm: 0,
            _pad: 0,
        };

        let d2 = distance_to_segment_squared(500, 0, &seg);
        assert_eq!(d2, 0);

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
            seg_len_mm: 10_000,
            dx_cm: 1000,
            dy_cm: 0,
            _pad: 0,
        };

        let d2 = distance_to_segment_squared(500, 300, &seg);
        assert_eq!(d2, 90_000);
    }

    #[test]
    fn test_distance_to_segment_squared_clamped_before() {
        let seg = RouteNode {
            x_cm: 1000,
            y_cm: 0,
            cum_dist_cm: 0,
            heading_cdeg: 0,
            seg_len_mm: 10_000,
            dx_cm: 1000,
            dy_cm: 0,
            _pad: 0,
        };

        let d2 = distance_to_segment_squared(0, 0, &seg);
        assert_eq!(d2, 1_000_000);
    }

    #[test]
    fn test_distance_to_segment_squared_zero_length() {
        let seg = RouteNode {
            x_cm: 1000,
            y_cm: 1000,
            cum_dist_cm: 0,
            heading_cdeg: 0,
            seg_len_mm: 0,
            dx_cm: 0,
            dy_cm: 0,
            _pad: 0,
        };

        let d2 = distance_to_segment_squared(1200, 1300, &seg);
        assert_eq!(d2, 200*200 + 300*300);
    }

    #[test]
    fn test_project_to_route_on_segment() {
        let route_data = create_test_route_data(&[
            (0, 0, 0, 100_000),
            (10000, 0, 0, 100_000),
        ]).unwrap();

        let s = project_to_route(0, 0, 0, &route_data);
        assert_eq!(s, 0);

        let s = project_to_route(10000, 0, 0, &route_data);
        assert_eq!(s, 10_000);
    }

    #[test]
    fn test_project_to_route_mid_segment() {
        let route_data = create_test_route_data(&[
            (0, 0, 0, 100_000),
        ]).unwrap();

        let s = project_to_route(5000, 0, 0, &route_data);
        assert_eq!(s, 5_000);
    }

    proptest! {
        #[test]
        fn prop_distance_non_negative(x in -10000i32..10000, y in -10000i32..10000) {
            let seg = RouteNode {
                x_cm: 0,
                y_cm: 0,
                cum_dist_cm: 0,
                heading_cdeg: 0,
                seg_len_mm: 10_000,
                dx_cm: 1000,
                dy_cm: 0,
                _pad: 0,
            };

            let d2 = distance_to_segment_squared(x, y, &seg);
            prop_assert!(d2 >= 0);
        }

        #[test]
        fn prop_distance_on_segment_is_zero(t in 0i32..1000) {
            let seg = RouteNode {
                x_cm: 0,
                y_cm: 0,
                cum_dist_cm: 0,
                heading_cdeg: 0,
                seg_len_mm: 10_000,
                dx_cm: 1000,
                dy_cm: 0,
                _pad: 0,
            };

            let d2 = distance_to_segment_squared(t, 0, &seg);
            prop_assert_eq!(d2, 0);
        }
    }
}

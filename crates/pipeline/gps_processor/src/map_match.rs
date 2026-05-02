//! Heading-constrained map matching

use shared::binfile::RouteData;
use shared::{Dist2, DistCm, HeadCdeg, RouteNode, SpeedCms};

use crate::SIGMA_GPS_CM;

/// Hard heading gate at full speed (w = 256, ≥ 3 km/h).
/// A bus in motion cannot be heading >90° from the segment direction.
/// This is the single tunable heading parameter; its units (centidegrees) are
/// directly interpretable — no hidden scale factors.
const MAX_HEADING_DIFF_CDEG: u32 = 9_000; // 90°

/// Heading filter threshold for a given speed weight.
///
/// Returns `u32::MAX` (gate disabled) when w = 0 — at a standstill GPS heading
/// is unreliable; don't reject any segment.
/// Returns `MAX_HEADING_DIFF_CDEG` (90°) at w = 256.
/// Linearly interpolates between the two, giving a progressively tighter gate
/// as the bus picks up speed.
///
/// At w = 128 (≈1.5 km/h):  threshold ≈ 22 500 cdeg (225°, nearly open)
/// At w = 256 (≥3 km/h):    threshold =  9 000 cdeg (90°, meaningful gate)
fn heading_threshold_cdeg(w: i32) -> u32 {
    if w == 0 {
        return u32::MAX;
    }
    // threshold = 36000 - (36000 - MAX_HEADING_DIFF_CDEG) × w / 256
    let range = 36_000u32 - MAX_HEADING_DIFF_CDEG; // 27 000
    36_000 - range * w as u32 / 256
}

/// Returns true if this segment is a plausible direction of travel given the
/// current GPS heading.
///
/// Heading filter strictness depends on mode:
///   - First fix/recovery (is_first_fix = true): 180° relaxed threshold
///   - Sentinel heading (i16::MIN): always eligible (GGA-only mode)
///   - Stopped (w = 0): always eligible (heading unreliable)
///   - Moving: eligible iff heading_diff ≤ threshold(speed)
///
/// Note: this is a hard gate, not a blended penalty.  A segment is either
/// physically plausible or it isn't; partial credit produces commensuration
/// problems (adding cm² to cdeg²).
fn heading_eligible(gps_heading: HeadCdeg, gps_speed: SpeedCms, seg_heading: HeadCdeg, is_first_fix: bool) -> bool {
    if gps_heading == i16::MIN {
        return true; // GGA-only: preserve existing sentinel behaviour
    }
    let w = heading_weight(gps_speed);
    let threshold = if is_first_fix {
        // Relaxed threshold for post-outage recovery: 180°
        // This allows maximum flexibility while still providing some constraint
        18_000
    } else {
        heading_threshold_cdeg(w)
    };
    let diff = heading_diff_cdeg(gps_heading, seg_heading) as u32;
    diff <= threshold
}

/// Scan a range of segment indices, returning the best eligible and best any.
///
/// When `is_first_fix` is true, the heading filter is disabled - all segments
/// are eligible based on pure distance only.
///
/// Returns:
/// - (best_eligible_idx, best_eligible_dist2, eligible_found,
///    best_any_idx, best_any_dist2)
///
/// "Best" = minimum dist2. If no segment passes the heading filter,
/// best_eligible_dist2 = Dist2::MAX and eligible_found = false.
fn best_eligible(
    gps_x: DistCm,
    gps_y: DistCm,
    gps_heading: HeadCdeg,
    gps_speed: SpeedCms,
    route_data: &RouteData,
    range: impl Iterator<Item = usize>,
    is_first_fix: bool,
) -> (usize, Dist2, bool, usize, Dist2) {
    let mut best_eligible_idx: Option<usize> = None;
    let mut best_eligible_dist2 = Dist2::MAX;
    let mut best_any_idx: Option<usize> = None;
    let mut best_any_dist2 = Dist2::MAX;

    for idx in range {
        if let Some(seg) = route_data.get_node(idx) {
            let d2 = segment_score(gps_x, gps_y, &seg);

            if d2 < best_any_dist2 {
                best_any_dist2 = d2;
                best_any_idx = Some(idx);
            }

            if heading_eligible(gps_heading, gps_speed, seg.heading_cdeg, is_first_fix)
                && d2 < best_eligible_dist2
            {
                best_eligible_dist2 = d2;
                best_eligible_idx = Some(idx);
            }
        }
    }

    let eligible_found = best_eligible_idx.is_some();
    let eligible_idx = best_eligible_idx.unwrap_or(0);
    let any_idx = best_any_idx.unwrap_or(0);

    (
        eligible_idx,
        best_eligible_dist2,
        eligible_found,
        any_idx,
        best_any_dist2,
    )
}

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
///
/// Returns (segment_index, distance_squared) where:
/// - segment_index: the best matching segment index
/// - distance_squared: the distance² from GPS point to that segment (in cm²)
///
/// The `is_first_fix` parameter controls the heading filter strictness:
/// - true: Use relaxed 180° threshold for post-outage recovery
/// - false: Use normal 90° threshold for steady-state operation
pub fn find_best_segment_restricted(
    gps_x: DistCm,
    gps_y: DistCm,
    gps_heading: HeadCdeg,
    gps_speed: SpeedCms,
    route_data: &RouteData,
    last_idx: usize,
    is_first_fix: bool,
) -> (usize, i64) {
    // Early-exit threshold: if the best eligible segment in the window is
    // within SIGMA_GPS_CM (20 m), skip the expensive grid search.
    // Now that scores are pure dist2, this comparison is physically meaningful.
    const MAX_DIST2_EARLY_EXIT: Dist2 = SIGMA_GPS_CM as i64 * SIGMA_GPS_CM as i64; // 4 000 000 cm²

    const WINDOW_BACK: usize = 2;
    const WINDOW_FWD: usize = 10;

    let start = last_idx.saturating_sub(WINDOW_BACK);
    let end = (last_idx + WINDOW_FWD).min(route_data.node_count.saturating_sub(1));

    // PHASE 1: Window search
    let (
        window_best_eligible,
        window_eligible_dist2,
        window_eligible_found,
        window_best_any,
        window_any_dist2,
    ) = best_eligible(
        gps_x,
        gps_y,
        gps_heading,
        gps_speed,
        route_data,
        start..=end,
        is_first_fix,
    );

    // Early exit if eligible segment found within threshold
    if window_eligible_found && window_eligible_dist2 < MAX_DIST2_EARLY_EXIT {
        return (window_best_eligible, window_eligible_dist2);
    }

    // Fallback: full grid search.
    if gps_x < route_data.x0_cm || gps_y < route_data.y0_cm {
        // GPS is outside grid bounds - do global search over all segments
        // This handles detour paths and GPS positions outside the route extent
        let start = 0;
        let end = route_data.node_count.saturating_sub(1);
        let (global_best_eligible, global_eligible_dist2, global_eligible_found,
             global_best_any, global_any_dist2) = best_eligible(
            gps_x, gps_y, gps_heading, gps_speed, route_data, start..=end, is_first_fix
        );
        if global_eligible_found {
            return (global_best_eligible, global_eligible_dist2);
        } else {
            return (global_best_any, global_any_dist2);
        }
    }

    let gx = ((gps_x - route_data.x0_cm) / route_data.grid.grid_size_cm) as u32;
    let gy = ((gps_y - route_data.y0_cm) / route_data.grid.grid_size_cm) as u32;

    if gx >= route_data.grid.cols || gy >= route_data.grid.rows {
        // GPS is outside grid bounds - do global search over all segments
        let start = 0;
        let end = route_data.node_count.saturating_sub(1);
        let (global_best_eligible, global_eligible_dist2, global_eligible_found,
             global_best_any, global_any_dist2) = best_eligible(
            gps_x, gps_y, gps_heading, gps_speed, route_data, start..=end, is_first_fix
        );
        if global_eligible_found {
            return (global_best_eligible, global_eligible_dist2);
        } else {
            return (global_best_any, global_any_dist2);
        }
    }

    // PHASE 2: Grid search
    // Carry over the window winner as the seed — grid search only improves on it.
    let mut best_eligible_idx = if window_eligible_found {
        window_best_eligible
    } else {
        // Safe default; will be overwritten by first eligible grid segment
        last_idx
    };
    let mut best_eligible_dist2 = if window_eligible_found {
        window_eligible_dist2
    } else {
        Dist2::MAX
    };
    let mut best_any_idx = window_best_any;
    let mut best_any_dist2 = window_any_dist2;
    let mut eligible_found = window_eligible_found;

    for dy in 0..=2i32 {
        for dx in 0..=2i32 {
            let ny = gy as i32 + dy - 1;
            let nx = gx as i32 + dx - 1;
            if ny < 0 || nx < 0 {
                continue;
            }
            let _ = route_data
                .grid
                .visit_cell(nx as u32, ny as u32, |idx: u16| {
                    if let Some(seg) = route_data.get_node(idx as usize) {
                        let d2 = segment_score(gps_x, gps_y, &seg);

                        // Update best_any tracker
                        if d2 < best_any_dist2 {
                            best_any_dist2 = d2;
                            best_any_idx = idx as usize;
                        }

                        // Update best_eligible tracker if heading matches
                        if heading_eligible(gps_heading, gps_speed, seg.heading_cdeg, is_first_fix) {
                            if d2 < best_eligible_dist2 {
                                best_eligible_dist2 = d2;
                                best_eligible_idx = idx as usize;
                                eligible_found = true;
                            }
                        }
                    }
                });
        }
    }

    // If no segment in window or grid passed the heading filter, fall back to
    // pure distance over the window. This is an explicit, logged degradation —
    // not a silent wrong answer.
    if !eligible_found {
        #[cfg(feature = "firmware")]
        defmt::warn!(
            "heading filter: no eligible segments at speed={} cdeg heading={}, \
             falling back to pure-distance selection",
            gps_speed,
            gps_heading
        );
        return (best_any_idx, best_any_dist2);
    }

    (best_eligible_idx, best_eligible_dist2)
}

/// Find best segment using grid search only (no window search around last_idx).
///
/// This is used for off-route re-entry where the bus might be at a completely
/// different part of the route than where it left off.
///
/// The `is_first_fix` parameter controls the heading filter strictness:
/// - true: Use relaxed 180° threshold for post-outage recovery
/// - false: Use normal 90° threshold for steady-state operation
pub fn find_best_segment_grid_only(
    gps_x: DistCm,
    gps_y: DistCm,
    gps_heading: HeadCdeg,
    gps_speed: SpeedCms,
    route_data: &RouteData,
    is_first_fix: bool,
) -> (usize, i64) {
    // Check bounding box first
    if gps_x < route_data.x0_cm || gps_y < route_data.y0_cm {
        // Outside bounding box - return segment 0 as fallback
        return (0, i64::MAX);
    }

    let gx = ((gps_x - route_data.x0_cm) / route_data.grid.grid_size_cm) as u32;
    let gy = ((gps_y - route_data.y0_cm) / route_data.grid.grid_size_cm) as u32;

    // Grid search over 3x3 cells
    let mut best_eligible_idx = 0;
    let mut best_eligible_dist2 = Dist2::MAX;
    let mut best_any_idx = 0;
    let mut best_any_dist2 = Dist2::MAX;
    let mut eligible_found = false;

    for dy in 0..=2i32 {
        for dx in 0..=2i32 {
            let ny = gy as i32 + dy - 1;
            let nx = gx as i32 + dx - 1;
            if ny < 0 || nx < 0 {
                continue;
            }
            let _ = route_data
                .grid
                .visit_cell(nx as u32, ny as u32, |idx: u16| {
                    if let Some(seg) = route_data.get_node(idx as usize) {
                        let d2 = segment_score(gps_x, gps_y, &seg);

                        // Update best_any tracker
                        if d2 < best_any_dist2 {
                            best_any_dist2 = d2;
                            best_any_idx = idx as usize;
                        }

                        // Update best_eligible tracker if heading matches
                        if heading_eligible(gps_heading, gps_speed, seg.heading_cdeg, is_first_fix) {
                            if d2 < best_eligible_dist2 {
                                best_eligible_dist2 = d2;
                                best_eligible_idx = idx as usize;
                                eligible_found = true;
                            }
                        }
                    }
                });
        }
    }

    // If no segment passed the heading filter, fall back to pure distance
    if !eligible_found {
        return (best_any_idx, best_any_dist2);
    }

    (best_eligible_idx, best_eligible_dist2)
}

/// Find best segment using grid search with a minimum position constraint.
///
/// This is used for off-route re-entry where we want to prevent snapping to
/// segments that project to positions before the frozen position.
///
/// The `min_s_cm` parameter constrains the search to only segments that
/// project to positions >= min_s_cm. This prevents backward snaps and
/// reduces the risk of snapping too far forward and skipping stops.
pub fn find_best_segment_grid_only_with_min_s(
    gps_x: DistCm,
    gps_y: DistCm,
    gps_heading: HeadCdeg,
    gps_speed: SpeedCms,
    route_data: &RouteData,
    is_first_fix: bool,
    min_s_cm: DistCm,
) -> (usize, i64) {
    // Check bounding box first
    if gps_x < route_data.x0_cm || gps_y < route_data.y0_cm {
        // Outside bounding box - return segment 0 as fallback
        return (0, i64::MAX);
    }

    let gx = ((gps_x - route_data.x0_cm) / route_data.grid.grid_size_cm) as u32;
    let gy = ((gps_y - route_data.x0_cm) / route_data.grid.grid_size_cm) as u32;

    // Grid search over 3x3 cells
    let mut best_eligible_idx = 0;
    let mut best_eligible_dist2 = Dist2::MAX;
    let mut best_any_idx = 0;
    let mut best_any_dist2 = Dist2::MAX;
    let mut eligible_found = false;

    for dy in 0..=2i32 {
        for dx in 0..=2i32 {
            let ny = gy as i32 + dy - 1;
            let nx = gx as i32 + dx - 1;
            if ny < 0 || nx < 0 {
                continue;
            }
            let _ = route_data
                .grid
                .visit_cell(nx as u32, ny as u32, |idx: u16| {
                    if let Some(seg) = route_data.get_node(idx as usize) {
                        // Skip segments that project to positions before min_s_cm
                        if seg.cum_dist_cm < min_s_cm {
                            return;
                        }

                        let d2 = segment_score(gps_x, gps_y, &seg);

                        // Update best_any tracker
                        if d2 < best_any_dist2 {
                            best_any_dist2 = d2;
                            best_any_idx = idx as usize;
                        }

                        // Update best_eligible tracker if heading matches
                        if heading_eligible(gps_heading, gps_speed, seg.heading_cdeg, is_first_fix) {
                            if d2 < best_eligible_dist2 {
                                best_eligible_dist2 = d2;
                                best_eligible_idx = idx as usize;
                                eligible_found = true;
                            }
                        }
                    }
                });
        }
    }

    // If no segment passed the heading filter, fall back to pure distance
    if !eligible_found {
        return (best_any_idx, best_any_dist2);
    }

    (best_eligible_idx, best_eligible_dist2)
}

/// Distance-squared from GPS point to segment (clamped projection).
///
/// Heading is intentionally absent.  Heading belongs in the eligibility
/// filter (`heading_eligible`), not in the ranking score.  Mixing cm² and
/// cdeg² into one scalar requires an arbitrary scale factor that cannot be
/// derived from first principles.
///
/// The return type is `Dist2` (i64 cm²).
pub fn segment_score(gps_x: DistCm, gps_y: DistCm, seg: &RouteNode) -> Dist2 {
    distance_to_segment_squared(gps_x, gps_y, seg)
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
    use proptest::prelude::*;
    use shared::binfile::{BusError, RouteData};
    use shared::{SpatialGrid, Stop};

    /// Create minimal test route data with specified segments.
    /// Returns loaded RouteData ready for testing.
    fn create_test_route_data(segments: &[(i32, i32, i16, i32)]) -> Result<RouteData<'static>, BusError> {
        let mut nodes: Vec<RouteNode> = Vec::new();
        let mut cum_dist = 0;
        for (i, &(x, y, heading, len_mm)) in segments.iter().enumerate() {
            let dx_cm = len_mm / 10; // Each segment's dx = its length in cm

            nodes.push(RouteNode {
                x_cm: x,
                y_cm: y,
                cum_dist_cm: cum_dist,
                heading_cdeg: heading,
                seg_len_mm: len_mm as i32,
                dx_cm: dx_cm as i16,
                dy_cm: 0,
                _pad: 0,
            });

            cum_dist += dx_cm;
        }

        let stops: Vec<Stop> = vec![];
        let grid = SpatialGrid {
            cells: vec![vec![]],
            grid_size_cm: 100_000,
            cols: 1,
            rows: 1,
            x0_cm: 0,
            y0_cm: 0,
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
    fn test_heading_eligible_sentinel() {
        let seg_heading: HeadCdeg = 9000; // 90°

        // Sentinel: always eligible regardless of segment heading or speed
        assert!(heading_eligible(i16::MIN, 500, seg_heading, false));
        assert!(heading_eligible(i16::MIN, 0, seg_heading, false));
    }

    #[test]
    fn test_heading_eligible_stopped() {
        // Stopped (w=0): always eligible — heading is unreliable
        assert!(heading_eligible(0, 0, 9000, false)); // facing opposite direction
        assert!(heading_eligible(0, 0, 18000, false)); // 180° misaligned
    }

    #[test]
    fn test_heading_eligible_moving() {
        let speed: SpeedCms = 500; // well above 83 cm/s → w=256 → threshold=9000

        // Same heading: eligible
        assert!(heading_eligible(9000, speed, 9000, false));

        // 89° off: eligible (just under 90° gate)
        assert!(heading_eligible(0, speed, 8999, false));

        // 91° off: not eligible
        assert!(!heading_eligible(0, speed, 9001, false));

        // 180° (opposite direction): not eligible at speed
        assert!(!heading_eligible(0, speed, 18000, false));
    }

    #[test]
    fn test_heading_threshold_cdeg() {
        // At w=0 (stopped): gate disabled (u32::MAX)
        assert_eq!(heading_threshold_cdeg(0), u32::MAX);

        // At w=256 (full speed): 90° gate
        assert_eq!(heading_threshold_cdeg(256), 9_000);

        // At w=128 (half speed): intermediate threshold
        let threshold = heading_threshold_cdeg(128);
        assert!(threshold > 9_000 && threshold < 36_000);

        // Threshold decreases as weight increases
        assert!(heading_threshold_cdeg(64) > heading_threshold_cdeg(128));
        assert!(heading_threshold_cdeg(128) > heading_threshold_cdeg(256));
    }

    proptest! {
        #[test]
        fn prop_heading_diff_symmetric(a in -18000i16..18000, b in -18000i16..18000) {
            let diff1 = heading_diff_cdeg(a, b);
            let diff2 = heading_diff_cdeg(b, a);
            assert_eq!(diff1, diff2);
        }

        #[test]
        fn prop_heading_diff_identity(a in -18000i16..18000) {
            let diff = heading_diff_cdeg(a, a);
            assert_eq!(diff, 0);
        }

        #[test]
        fn prop_heading_diff_max_180(a in -18000i16..18000, b in -18000i16..18000) {
            let diff = heading_diff_cdeg(a, b);
            assert!(diff <= 18000);
        }
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
        assert_eq!(d2, 200*200 + 300*300); // sqrt(200² + 300²)²
    }

    #[test]
    fn test_project_to_route_on_segment() {
        let route_data = create_test_route_data(&[
            (0, 0, 0, 100_000),    // 10 m segment, cum_dist = 0
            (10000, 0, 0, 100_000), // cum_dist = 10,000
        ]).unwrap();

        // Project point at start of segment 0
        let s = project_to_route(0, 0, 0, &route_data);
        assert_eq!(s, 0);

        // Project point at end of segment 0
        let s = project_to_route(10000, 0, 0, &route_data);
        assert_eq!(s, 10_000);
    }

    #[test]
    fn test_project_to_route_mid_segment() {
        let route_data = create_test_route_data(&[
            (0, 0, 0, 100_000), // 10 m segment
        ]).unwrap();

        // Project point at middle of segment
        let s = project_to_route(5000, 0, 0, &route_data);
        assert_eq!(s, 5_000); // Halfway through 10 m segment
    }
}

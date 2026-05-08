//! Search strategies for map matching

use super::{heading::heading_eligible, projection::segment_score};
use crate::SIGMA_GPS_CM;
use shared::binfile::RouteData;
use shared::{Dist2, DistCm, HeadCdeg, SpeedCms};

/// Scan a range of segment indices, returning the best eligible and best any.
///
/// When `is_first_fix` is true, the heading filter is disabled - all segments
/// are eligible based on pure distance only.
///
/// Returns:
/// - (best_eligible_idx, best_eligible_dist2, eligible_found,
///   best_any_idx, best_any_dist2)
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

/// Global search fallback when GPS is outside grid bounds.
/// Searches all segments and returns the best eligible (or best any if none eligible).
fn global_search_fallback(
    gps_x: DistCm,
    gps_y: DistCm,
    gps_heading: HeadCdeg,
    gps_speed: SpeedCms,
    route_data: &RouteData,
    is_first_fix: bool,
) -> (usize, Dist2) {
    let start = 0;
    let end = route_data.node_count.saturating_sub(1);
    let (best_eligible, eligible_dist2, eligible_found, best_any, any_dist2) = best_eligible(
        gps_x, gps_y, gps_heading, gps_speed, route_data, start..=end, is_first_fix,
    );
    if eligible_found {
        (best_eligible, eligible_dist2)
    } else {
        (best_any, any_dist2)
    }
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

    /// Window search looks back 2 segments and forward 10 segments from last_idx.
    ///
    /// These values are derived from:
    /// - GPS update rate: 1 Hz
    /// - Typical bus speed: 30-50 km/h (~8-14 m/s)
    /// - Segment length: ~20 m on average
    /// - In one second, a bus travels ~8-14 m, or ~0.4-0.7 segments
    /// - Window of ±10 segments provides ~20 second buffer for GPS outliers
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
        gps_x, gps_y, gps_heading, gps_speed, route_data, start..=end, is_first_fix,
    );

    // Early exit if eligible segment found within threshold
    if window_eligible_found && window_eligible_dist2 < MAX_DIST2_EARLY_EXIT {
        return (window_best_eligible, window_eligible_dist2);
    }

    // Fallback: full grid search.
    if gps_x < route_data.x0_cm || gps_y < route_data.y0_cm {
        // GPS is outside grid bounds - use global search fallback
        // This handles detour paths and GPS positions outside the route extent
        return global_search_fallback(gps_x, gps_y, gps_heading, gps_speed, route_data, is_first_fix);
    }

    let gx = ((gps_x - route_data.x0_cm) / route_data.grid.grid_size_cm) as u32;
    let gy = ((gps_y - route_data.y0_cm) / route_data.grid.grid_size_cm) as u32;

    if gx >= route_data.grid.cols || gy >= route_data.grid.rows {
        // GPS is outside grid bounds - use global search fallback
        return global_search_fallback(gps_x, gps_y, gps_heading, gps_speed, route_data, is_first_fix);
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
            let _ = route_data.grid.visit_cell(nx as u32, ny as u32, |idx: u16| {
                if let Some(seg) = route_data.get_node(idx as usize) {
                    let d2 = segment_score(gps_x, gps_y, &seg);

                    // Update best_any tracker
                    if d2 < best_any_dist2 {
                        best_any_dist2 = d2;
                        best_any_idx = idx as usize;
                    }

                    // Update best_eligible tracker if heading matches
                    if heading_eligible(gps_heading, gps_speed, seg.heading_cdeg, is_first_fix)
                        && d2 < best_eligible_dist2
                    {
                        best_eligible_dist2 = d2;
                        best_eligible_idx = idx as usize;
                        eligible_found = true;
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

/// Find best segment using grid search only (no min/max constraints).
///
/// This is used for off-route re-entry and testing where no position constraints are needed.
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
            let _ = route_data.grid.visit_cell(nx as u32, ny as u32, |idx: u16| {
                if let Some(seg) = route_data.get_node(idx as usize) {
                    let d2 = segment_score(gps_x, gps_y, &seg);

                    // Update best_any tracker
                    if d2 < best_any_dist2 {
                        best_any_dist2 = d2;
                        best_any_idx = idx as usize;
                    }

                    // Update best_eligible tracker if heading matches
                    if heading_eligible(gps_heading, gps_speed, seg.heading_cdeg, is_first_fix)
                        && d2 < best_eligible_dist2
                    {
                        best_eligible_dist2 = d2;
                        best_eligible_idx = idx as usize;
                        eligible_found = true;
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

/// Find best segment using grid search only with min_s and max_s constraints.
///
/// This is used for off-route re-entry where the bus might be at a completely
/// different part of the route than where it left off.
///
/// The `min_s_cm` parameter constrains the search to only segments that
/// project to positions >= min_s_cm. This prevents backward snaps and
/// reduces the risk of snapping too far forward and skipping stops.
///
/// The `max_s_cm` parameter constrains the search to only segments that
/// project to positions <= max_s_cm. This prevents forward snaps that skip
/// too many stops when the route has loops or crossing segments.
pub fn find_best_segment_grid_only_with_min_max_s(
    gps_x: DistCm,
    gps_y: DistCm,
    gps_heading: HeadCdeg,
    gps_speed: SpeedCms,
    route_data: &RouteData,
    is_first_fix: bool,
    min_s_cm: DistCm,
    max_s_cm: DistCm,
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
            let _ = route_data.grid.visit_cell(nx as u32, ny as u32, |idx: u16| {
                if let Some(seg) = route_data.get_node(idx as usize) {
                    // Skip segments that project to positions before min_s_cm
                    if seg.cum_dist_cm < min_s_cm {
                        return;
                    }
                    // Skip segments that project to positions after max_s_cm
                    if seg.cum_dist_cm > max_s_cm {
                        return;
                    }

                    let d2 = segment_score(gps_x, gps_y, &seg);

                    // Update best_any tracker
                    if d2 < best_any_dist2 {
                        best_any_dist2 = d2;
                        best_any_idx = idx as usize;
                    }

                    // Update best_eligible tracker if heading matches
                    if heading_eligible(gps_heading, gps_speed, seg.heading_cdeg, is_first_fix)
                        && d2 < best_eligible_dist2
                    {
                        best_eligible_dist2 = d2;
                        best_eligible_idx = idx as usize;
                        eligible_found = true;
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

/// Find best segment using grid search only with min_s constraint (backward compatibility wrapper).
///
/// This function is kept for backward compatibility and internally calls
/// `find_best_segment_grid_only_with_min_max_s` with a very large max_s constraint.
pub fn find_best_segment_grid_only_with_min_s(
    gps_x: DistCm,
    gps_y: DistCm,
    gps_heading: HeadCdeg,
    gps_speed: SpeedCms,
    route_data: &RouteData,
    is_first_fix: bool,
    min_s_cm: DistCm,
) -> (usize, i64) {
    // Use a very large max_s (effectively no upper bound) for backward compatibility
    find_best_segment_grid_only_with_min_max_s(
        gps_x,
        gps_y,
        gps_heading,
        gps_speed,
        route_data,
        is_first_fix,
        min_s_cm,
        DistCm::MAX, // No upper bound
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::binfile::{BusError, RouteData};
    use shared::{RouteNode, SpatialGrid, Stop};

    /// Create minimal test route data with specified segments.
    /// Returns loaded RouteData ready for testing.
    fn create_test_route_data(
        segments: &[(i32, i32, i16, i32)],
    ) -> Result<RouteData<'static>, BusError> {
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
    fn test_find_best_segment_restricted_window_early_exit() {
        // Create route with 20 segments
        let segments: Vec<(i32, i32, i16, i32)> = (0..20).map(|i| (i * 1000, 0, 0, 20_000)).collect();
        let segments_refs: &[(i32, i32, i16, i32)] = &segments;
        let route_data = create_test_route_data(segments_refs).unwrap();

        // GPS near segment 10, within SIGMA_GPS_CM (2000 cm)
        // Position at x=11500 to ensure segment 10 is uniquely closest
        // Segment 9: [9000, 11000], dist to x=11500 = 500
        // Segment 10: [10000, 12000], dist to x=11500 = 0
        let gps_x = 11_500;
        let gps_y = 0;

        // last_idx = 10, window = [8, 12], GPS at segment 10
        let (idx, dist2) = find_best_segment_restricted(
            gps_x, gps_y, 0, 500, &route_data, 10, false,
        );

        // Should find segment 10 within early exit threshold
        assert_eq!(idx, 10);
        assert!(dist2 < 4_000_000); // MAX_DIST2_EARLY_EXIT
    }

    #[test]
    fn test_find_best_segment_restricted_grid_fallback() {
        // Create route with 20 segments
        let segments: Vec<(i32, i32, i16, i32)> = (0..20).map(|i| (i * 1000, 0, 0, 20_000)).collect();
        let segments_refs: &[(i32, i32, i16, i32)] = &segments;
        let route_data = create_test_route_data(segments_refs).unwrap();

        // GPS far from last_idx, requires grid search
        // Use GPS position that is unambiguously closest to segment 15
        // Segment 14: starts at 14000, extends to 16000 (dx=2000)
        // Segment 15: starts at 15000, extends to 17000 (dx=2000)
        // Position at x=16500 is in segment 15 only (500 cm from start, 500 cm from end)
        let gps_x = 16_500;
        let gps_y = 0;

        // last_idx = 5, window = [3, 7], GPS at segment 15
        let (idx, _dist2) = find_best_segment_restricted(
            gps_x, gps_y, 0, 500, &route_data, 5, false,
        );

        // Should find segment 15 via grid search
        assert_eq!(idx, 15);
    }

    #[test]
    fn test_find_best_segment_grid_only() {
        let route_data = create_test_route_data(&[
            (0, 0, 0, 20_000),
            (2000, 0, 0, 20_000),
            (4000, 0, 0, 20_000),
        ])
        .unwrap();

        // GPS in middle of segment 1 (from 2000 to 4000)
        let gps_x = 3000;
        let gps_y = 0;

        let (idx, _dist2) = find_best_segment_grid_only(gps_x, gps_y, 0, 500, &route_data, false);

        assert_eq!(idx, 1);
    }

    #[test]
    fn test_find_best_segment_grid_only_outside_bounds() {
        let route_data = create_test_route_data(&[(0, 0, 0, 20_000)]).unwrap();

        // GPS outside grid bounds
        let (idx, dist2) = find_best_segment_grid_only(-50_000, -50_000, 0, 500, &route_data, false);

        // Should return fallback
        assert_eq!(idx, 0);
        assert_eq!(dist2, i64::MAX);
    }

    #[test]
    fn test_find_best_segment_grid_only_with_min_s_constraint() {
        // Create segments with increasing cum_dist
        // Each segment is 1000m long, so cumulative distances are:
        // seg0: 0, seg1: 1000, seg2: 2000, seg3: 3000, ...
        let segments: Vec<(i32, i32, i16, i32)> =
            (0..10).map(|i| (i * 1000, 0, 0, 1000 * 1000)).collect();
        let segments_refs: &[(i32, i32, i16, i32)] = &segments;
        let route_data = create_test_route_data(segments_refs).unwrap();

        // GPS at position after segment 2 (x=3000, which is in segment 3)
        let gps_x = 3000;
        let gps_y = 0;

        // Set min_s_cm to 250,000 cm (2.5 km) - should skip segments 0-2
        let (idx, _dist2) = find_best_segment_grid_only_with_min_s(
            gps_x, gps_y, 0, 500, &route_data, false, 250_000,
        );

        // Should skip to segment 3 or later (cum_dist >= 2,500,000 cm)
        assert!(idx >= 3);
    }

    #[test]
    fn test_find_best_segment_grid_only_with_min_s_no_filter() {
        let route_data = create_test_route_data(&[
            (0, 0, 0, 1000 * 1000), // 1000m segment
            (1000, 0, 0, 1000 * 1000), // 1000m segment
        ])
        .unwrap();

        // GPS at position of segment 1 (x=1000)
        let gps_x = 1000;
        let gps_y = 0;

        // min_s_cm = 0, no filtering
        let (idx, _dist2) =
            find_best_segment_grid_only_with_min_s(gps_x, gps_y, 0, 500, &route_data, false, 0);

        assert_eq!(idx, 1);
    }

    #[test]
    fn test_sentinel_heading_always_eligible() {
        let route_data = create_test_route_data(&[(0, 0, 18000, 20_000)]).unwrap();

        // Sentinel heading should always be eligible
        let (idx, _dist2) =
            find_best_segment_grid_only(0, 0, i16::MIN, 500, &route_data, false);

        assert_eq!(idx, 0);
    }

    #[test]
    fn test_first_fix_relaxed_heading() {
        let route_data = create_test_route_data(&[(0, 0, 18000, 20_000)]).unwrap();

        // First fix mode: relaxed 180° threshold
        let (idx, _dist2) = find_best_segment_grid_only(0, 0, 0, 500, &route_data, true);

        // Should match despite 180° heading difference
        assert_eq!(idx, 0);
    }

    #[test]
    fn test_zero_speed_heading_gate_disabled() {
        let route_data = create_test_route_data(&[(0, 0, 9001, 20_000)]).unwrap();

        // Zero speed: heading gate disabled
        let (idx, _dist2) = find_best_segment_grid_only(0, 0, 0, 0, &route_data, false);

        assert_eq!(idx, 0);
    }
}

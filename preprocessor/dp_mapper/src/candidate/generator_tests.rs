#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::grid::build_grid;

    fn make_simple_route() -> (Vec<shared::RouteNode>, SpatialGrid) {
        let nodes = vec![
            shared::RouteNode { len2_cm2: 100000000, heading_cdeg: 0, _pad: 0, x_cm: 0, y_cm: 0, cum_dist_cm: 0, dx_cm: 10000, dy_cm: 0, seg_len_cm: 10000 },
            shared::RouteNode { len2_cm2: 0, heading_cdeg: 0, _pad: 0, x_cm: 10000, y_cm: 0, cum_dist_cm: 10000, dx_cm: 0, dy_cm: 0, seg_len_cm: 0 },
        ];
        let grid = build_grid(&nodes, 10000);
        (nodes, grid)
    }

    #[test]
    fn test_projection_at_segment_start() {
        let (nodes, grid) = make_simple_route();

        let result = generate_candidates((0, 0), &nodes, &grid, 5);
        assert!(!result.is_empty());

        let best = &result[0];
        assert_eq!(best.seg_idx, 0);
        assert_eq!(best.t, 0.0);
        assert_eq!(best.progress_cm, 0);
        assert_eq!(best.dist_sq_cm2, 0);
    }

    #[test]
    fn test_projection_at_segment_end() {
        let (nodes, grid) = make_simple_route();

        let result = generate_candidates((10000, 0), &nodes, &grid, 5);
        assert!(!result.is_empty());

        let best = &result[0];
        assert_eq!(best.seg_idx, 0);
        assert_eq!(best.t, 1.0);
        assert_eq!(best.progress_cm, 10000);
        assert_eq!(best.dist_sq_cm2, 0);
    }

    #[test]
    fn test_projection_at_segment_mid() {
        let (nodes, grid) = make_simple_route();

        let result = generate_candidates((5000, 0), &nodes, &grid, 5);
        assert!(!result.is_empty());

        let best = &result[0];
        assert_eq!(best.seg_idx, 0);
        assert_eq!(best.t, 0.5);
        assert_eq!(best.progress_cm, 5000);
        assert_eq!(best.dist_sq_cm2, 0);
    }

    #[test]
    fn test_snap_candidate_generation() {
        let (nodes, grid) = make_simple_route();

        // Previous layer had max progress 5000
        let result = generate_candidates_with_snap((50000, 0), &nodes, &grid, 5, 5000);
        assert!(!result.is_empty(), "should have at least snap candidate");

        // Find the snap candidate (it should have the penalty distance)
        let snap = result.iter().find(|c| c.dist_sq_cm2 >= 1_000_000_000_000)
            .expect("should have snap candidate with penalty distance");

        assert_eq!(snap.t, 0.0, "snap should be at segment start");
        assert_eq!(snap.seg_idx, 0, "snap should be on segment 0");
        assert_eq!(snap.progress_cm, 0, "snap should be at progress 0");
    }

    #[test]
    fn test_snap_reachability() {
        let nodes = vec![
            shared::RouteNode { len2_cm2: 100000000, heading_cdeg: 0, _pad: 0, x_cm: 0, y_cm: 0, cum_dist_cm: 0, dx_cm: 5000, dy_cm: 0, seg_len_cm: 5000 },
            shared::RouteNode { len2_cm2: 100000000, heading_cdeg: 0, _pad: 0, x_cm: 5000, y_cm: 0, cum_dist_cm: 5000, dx_cm: 5000, dy_cm: 0, seg_len_cm: 5000 },
            shared::RouteNode { len2_cm2: 100000000, heading_cdeg: 0, _pad: 0, x_cm: 10000, y_cm: 0, cum_dist_cm: 10000, dx_cm: 5000, dy_cm: 0, seg_len_cm: 5000 },
            shared::RouteNode { len2_cm2: 0, heading_cdeg: 0, _pad: 0, x_cm: 15000, y_cm: 0, cum_dist_cm: 15000, dx_cm: 0, dy_cm: 0, seg_len_cm: 0 },
        ];
        let grid = build_grid(&nodes, 10000);

        // Previous max progress = 7500
        // Segment 0: 0+5000=5000 < 7500
        // Segment 1: 5000+5000=10000 >= 7500 ✓
        let result = generate_candidates_with_snap((0, 0), &nodes, &grid, 5, 7500);
        assert!(!result.is_empty());

        let snap = result.last().unwrap();
        // Snap should be on segment 1 (first segment whose end is >= 7500)
        assert_eq!(snap.seg_idx, 1);
        assert_eq!(snap.progress_cm, 5000);
    }
}

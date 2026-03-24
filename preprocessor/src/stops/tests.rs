// Unit tests for stop sequence validation

#[cfg(test)]
mod tests {
    use crate::stops::validation::validate_stop_sequence;
    use shared::{RouteNode, SpatialGrid};

    fn make_test_nodes(coords: &[(i64, i64)]) -> Vec<RouteNode> {
        let mut nodes = Vec::new();
        let mut cum_dist = 0i32;

        for i in 0..coords.len() {
            let (dx, dy, len2, seg_len, heading) = if i < coords.len() - 1 {
                let curr = coords[i];
                let next = coords[i + 1];
                let dx = next.0 - curr.0;
                let dy = next.1 - curr.1;
                let len2 = dx * dx + dy * dy;
                let seg_len = (len2 as f64).sqrt() as i32;
                let heading = (dy as f64).atan2(dx as f64).to_degrees() as i16 * 100;
                (dx, dy, len2, seg_len, heading)
            } else {
                (0, 0, 0, 0, 0)
            };

            nodes.push(RouteNode {
                len2_cm2: len2,
                heading_cdeg: heading,
                _pad: 0,
                x_cm: coords[i].0 as i32,
                y_cm: coords[i].1 as i32,
                cum_dist_cm: cum_dist,
                dx_cm: dx as i32,
                dy_cm: dy as i32,
                seg_len_cm: seg_len,
            });

            cum_dist += seg_len;
        }
        nodes
    }

    fn make_test_grid(nodes: &[RouteNode]) -> SpatialGrid {
        // Simple grid for testing - all segments in first cell
        let segment_indices: Vec<usize> = (0..nodes.len().saturating_sub(1)).collect();
        SpatialGrid {
            cells: vec![segment_indices],
            grid_size_cm: 10000,
            cols: 1,
            rows: 1,
            x0_cm: 0,
            y0_cm: 0,
        }
    }

    #[test]
    fn test_validate_monotonic_sequence() {
        // Simple collinear stops in order
        let stops = vec![(500, 0), (1500, 0), (2500, 0)];
        let nodes = make_test_nodes(&[(0, 0), (1000, 0), (2000, 0), (3000, 0)]);
        let grid = make_test_grid(&nodes);

        let result = validate_stop_sequence(&stops, &nodes, &grid);

        // All stops should project with increasing progress
        assert!(result.reversal_info.is_none(), "Expected no reversal but got: {:?}", result.reversal_info);
        assert_eq!(result.progress_values.len(), 3);
        // Progress values should be monotonically increasing
        assert!(result.progress_values[0] < result.progress_values[1]);
        assert!(result.progress_values[1] < result.progress_values[2]);
    }

    #[test]
    fn test_single_stop() {
        let stops = vec![(1000, 0)];
        let nodes = make_test_nodes(&[(0, 0), (2000, 0)]);
        let grid = make_test_grid(&nodes);

        let result = validate_stop_sequence(&stops, &nodes, &grid);

        assert!(result.reversal_info.is_none());
        assert_eq!(result.progress_values.len(), 1);
    }

    #[test]
    fn test_empty_stops() {
        let stops: Vec<(i64, i64)> = vec![];
        let nodes = make_test_nodes(&[(0, 0)]);
        let grid = make_test_grid(&nodes);

        let result = validate_stop_sequence(&stops, &nodes, &grid);

        assert!(result.reversal_info.is_none());
        assert_eq!(result.progress_values.len(), 0);
    }

    #[test]
    fn test_path_constraint_enforcement() {
        // Test that min_segment_idx constraint is enforced
        // Route with 3 segments: (0,0) -> (1000,0) -> (2000,0) -> (3000,0)
        // Stop 1 at (1000, 0) - should match segment 0 or 1
        // Stop 2 at (1500, 0) - must match segment >= Stop 1's segment
        let stops = vec![(1000, 0), (1500, 0)];
        let nodes = make_test_nodes(&[(0, 0), (1000, 0), (2000, 0), (3000, 0)]);
        let grid = make_test_grid(&nodes);

        let result = validate_stop_sequence(&stops, &nodes, &grid);

        // Both stops should be on the route with increasing progress
        assert!(result.reversal_info.is_none(), "Expected no reversal but got: {:?}", result.reversal_info);
        assert_eq!(result.progress_values.len(), 2);
        assert!(result.progress_values[0] < result.progress_values[1]);
    }

    #[test]
    fn test_close_stop_corridor_adjustment() {
        // Stops 79m apart (tpF805 Stop #2/#3 case)
        let progress_values = vec![127_689, 135_621]; // d = 7,932 cm

        // First, get standard corridors
        let mut stops = super::super::project_stops_validated(
            &progress_values,
            &crate::input::StopsInput { stops: vec![] } // Dummy input
        );

        // Apply close stop preprocessing
        super::super::preprocess_close_stop_corridors(&mut stops);

        // Verify 55%/10%/35% ratio
        // Stop 1 corridor_end = 127,689 + 0.35*7,932 = 130,465.5 -> 130,465 (integer division)
        // Stop 2 corridor_start = 135,621 - 0.55*7,932 = 131,258.5 -> 131,259 (integer division rounds down)
        // Gap = 794 cm (due to rounding)

        assert_eq!(stops[0].corridor_end_cm, 130_465);
        assert_eq!(stops[1].corridor_start_cm, 131_259);

        // Verify gap between corridors (may be 793 or 794 due to rounding)
        let gap = stops[1].corridor_start_cm - stops[0].corridor_end_cm;
        assert!(gap == 793 || gap == 794, "Expected gap between 793-794, got {}", gap);
    }

    #[test]
    fn test_no_adjustment_at_threshold() {
        // Stops exactly 120m apart - should NOT be adjusted
        let progress_values = vec![100_000, 112_000]; // d = 12,000 cm

        let mut stops = super::super::project_stops_validated(
            &progress_values,
            &crate::input::StopsInput { stops: vec![] }
        );

        let stops_before = stops.clone();
        super::super::preprocess_close_stop_corridors(&mut stops);

        // Corridors should be unchanged (threshold uses <, not <=)
        assert_eq!(stops[0].corridor_end_cm, stops_before[0].corridor_end_cm);
        assert_eq!(stops[1].corridor_start_cm, stops_before[1].corridor_start_cm);
    }

    #[test]
    fn test_no_adjustment_far_apart() {
        // Stops 200m apart - standard corridors apply
        let progress_values = vec![100_000, 120_000]; // d = 20,000 cm

        let mut stops = super::super::project_stops_validated(
            &progress_values,
            &crate::input::StopsInput { stops: vec![] }
        );

        let stops_before = stops.clone();
        super::super::preprocess_close_stop_corridors(&mut stops);

        // Should be unchanged
        assert_eq!(stops[0].corridor_end_cm, stops_before[0].corridor_end_cm);
        assert_eq!(stops[1].corridor_start_cm, stops_before[1].corridor_start_cm);
    }

    #[test]
    fn test_three_consecutive_close_stops() {
        // Three stops: A→B=80m, B→C=90m
        // B's corridor should be adjusted on both sides
        let progress_values = vec![100_000, 108_000, 117_000];

        let mut stops = super::super::project_stops_validated(
            &progress_values,
            &crate::input::StopsInput { stops: vec![] }
        );

        super::super::preprocess_close_stop_corridors(&mut stops);

        // Verify B's corridor boundaries
        // B.corridor_start from A→B: 108,000 - 0.55*8,000 = 103,600
        // B.corridor_end from B→C: 108,000 + 0.35*9,000 = 111,150
        assert_eq!(stops[1].corridor_start_cm, 103_600);
        assert_eq!(stops[1].corridor_end_cm, 111_150);

        // Verify B is still valid (start < progress < end)
        assert!(stops[1].corridor_start_cm < stops[1].progress_cm);
        assert!(stops[1].corridor_end_cm > stops[1].progress_cm);
    }
}

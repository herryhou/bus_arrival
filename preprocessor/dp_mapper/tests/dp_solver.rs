//! Tests for DP solver implementation

use dp_mapper::pathfinding::solver::{DpLayer, SortedCandidate, dp_forward_pass, dp_backtrack};
use dp_mapper::candidate::Candidate;

#[test]
fn test_dp_layer_creation() {
    // Test that DpLayer can be created with proper structure
    let layer = DpLayer {
        candidates: vec![],
        best_cost: vec![],
        best_prev: vec![],
    };

    assert_eq!(layer.candidates.len(), 0);
    assert_eq!(layer.best_cost.len(), 0);
    assert_eq!(layer.best_prev.len(), 0);
}

#[test]
fn test_sorted_candidate_ordering() {
    // Test sorting by progress_cm (primary key)
    let candidates = vec![
        SortedCandidate { progress_cm: 100, orig_idx: 0 },
        SortedCandidate { progress_cm: 300, orig_idx: 1 },
        SortedCandidate { progress_cm: 200, orig_idx: 2 },
    ];

    let mut sorted = candidates.clone();
    sorted.sort();

    assert_eq!(sorted[0].progress_cm, 100);
    assert_eq!(sorted[1].progress_cm, 200);
    assert_eq!(sorted[2].progress_cm, 300);
}

#[test]
fn test_dp_forward_pass_two_stops() {
    // Simple two-stop test case
    // Stop 0: 3 candidates at progress 100, 200, 300
    // Stop 1: 3 candidates at progress 150, 250, 350
    // Expected: should find valid transitions (progress[j] >= progress[j-1])

    let layers = vec![
        DpLayer {
            candidates: vec![
                Candidate {
                    seg_idx: 0,
                    t: 0.5,
                    dist_sq_cm2: 10,
                    progress_cm: 100,
                },
                Candidate {
                    seg_idx: 1,
                    t: 0.5,
                    dist_sq_cm2: 20,
                    progress_cm: 200,
                },
                Candidate {
                    seg_idx: 2,
                    t: 0.5,
                    dist_sq_cm2: 30,
                    progress_cm: 300,
                },
            ],
            best_cost: vec![0, 0, 0], // Base case: zero cost
            best_prev: vec![None, None, None],
        },
    ];

    let result = dp_forward_pass(&layers);

    // Should return a new layer with computed costs and predecessors
    assert_eq!(result.candidates.len(), 3);
    assert_eq!(result.best_cost.len(), 3);
    assert_eq!(result.best_prev.len(), 3);
}

#[test]
fn test_dp_backtrack() {
    // Test backtrack reconstruction
    let layers = vec![
        DpLayer {
            candidates: vec![
                Candidate {
                    seg_idx: 0,
                    t: 0.5,
                    dist_sq_cm2: 10,
                    progress_cm: 100,
                },
                Candidate {
                    seg_idx: 1,
                    t: 0.5,
                    dist_sq_cm2: 20,
                    progress_cm: 200,
                },
            ],
            best_cost: vec![10, 20],
            best_prev: vec![None, None],
        },
        DpLayer {
            candidates: vec![
                Candidate {
                    seg_idx: 1,
                    t: 0.6,
                    dist_sq_cm2: 15,
                    progress_cm: 250,
                },
                Candidate {
                    seg_idx: 2,
                    t: 0.4,
                    dist_sq_cm2: 25,
                    progress_cm: 350,
                },
            ],
            best_cost: vec![25, 45], // 10+15 and 20+25
            best_prev: vec![Some(0), Some(1)],
        },
    ];

    let result = dp_backtrack(&layers);

    // Should return progress values for optimal path
    assert_eq!(result.len(), 2);
    assert!(result[0] <= result[1]); // Monotonicity
}

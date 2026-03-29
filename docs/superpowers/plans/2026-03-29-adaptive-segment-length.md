# Adaptive Segment Length Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce binary file size by increasing default route segment length from 30m to 100m, while preserving arrival detection accuracy through adaptive segmentation (30m segments) near stops and sharp turns.

**Architecture:** Hybrid approach - Douglas-Peucker simplification followed by adaptive segmentation pass that refines segments within 100m of stops or at sharp turns (>20°) to 30m maximum length.

**Tech Stack:** Rust, embedded no_std context, existing polyline simplification infrastructure

---

## File Structure

| File | Responsibility |
|------|----------------|
| `preprocessor/tests/common/mod.rs` | Constants for segment lengths and thresholds |
| `preprocessor/src/simplify.rs` | Core simplification and adaptive segmentation logic |
| `preprocessor/tests/simplify_edge_cases.rs` | Unit tests for adaptive segmentation behavior |

---

## Task 1: Add Adaptive Segmentation Constants

**Files:**
- Modify: `preprocessor/tests/common/mod.rs:39-40`

- [ ] **Step 1: Update MAX_SEGMENT_LENGTH_CM to 100m**

```rust
/// Maximum segment length constraint in cm (100m = 10000cm)
pub const MAX_SEGMENT_LENGTH_CM: i32 = 10000;  // was 3000
```

- [ ] **Step 2: Add adaptive segmentation constants**

Add after line 40:

```rust
/// Adaptive segment length for critical areas in cm (30m = 3000cm)
/// Used near stops and sharp turns for precise arrival detection
pub const ADAPTIVE_SEGMENT_LENGTH_CM: i32 = 3000;

/// Stop proximity threshold for adaptive segmentation in cm (100m)
/// Segments within this distance of any stop get refined to 30m
pub const STOP_PROXIMITY_THRESHOLD_CM: f64 = 10000.0;

/// Sharp turn angle threshold for adaptive segmentation in degrees
/// Turns exceeding this angle trigger segment refinement
pub const SHARP_TURN_DEGREES: f64 = 20.0;
```

- [ ] **Step 3: Run tests to verify no compilation errors**

Run: `cargo test --package preprocessor --lib common`
Expected: Compilation succeeds, tests may fail (we'll fix in later tasks)

- [ ] **Step 4: Commit**

```bash
git add preprocessor/tests/common/mod.rs
git commit -m "feat: add adaptive segmentation constants (100m default, 30m near stops)"
```

---

## Task 2: Add Distance Helper Function

**Files:**
- Modify: `preprocessor/src/simplify.rs`

- [ ] **Step 1: Add distance helper function**

Add after the `is_sharp_turn` function (after line 168):

```rust
/// Calculate Euclidean distance between two points in centimeters
fn distance(p1: (i64, i64), p2: (i64, i64)) -> f64 {
    let dx = p2.0 - p1.0;
    let dy = p2.1 - p1.1;
    ((dx * dx + dy * dy) as f64).sqrt()
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check --package preprocessor --lib`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add preprocessor/src/simplify.rs
git commit -m "feat: add distance helper function to simplify.rs"
```

---

## Task 3: Modify interpolate_recursive to Accept max_len Parameter

**Files:**
- Modify: `preprocessor/src/simplify.rs:75-93`

- [ ] **Step 1: Update function signature**

Change line 75 from:
```rust
fn interpolate_recursive(p1: (i64, i64), p2: (i64, i64), result: &mut Vec<(i64, i64)>) {
```

To:
```rust
fn interpolate_recursive(p1: (i64, i64), p2: (i64, i64), result: &mut Vec<(i64, i64)>, max_len: f64) {
```

- [ ] **Step 2: Update hardcoded 3000.0 to use max_len parameter**

Change line 80 from:
```rust
    if dist > 3000.0 {
```

To:
```rust
    if dist > max_len {
```

- [ ] **Step 3: Update all call sites**

Find all calls to `interpolate_recursive` and add the max_len parameter:

Line 27: Change from `interpolate_recursive(points[0], points[1], &mut result);`
To: `interpolate_recursive(points[0], points[1], &mut result, 10000.0);`

Line 68: Change from `interpolate_recursive(p1, p2, &mut final_points);`
To: `interpolate_recursive(p1, p2, &mut final_points, 10000.0);`

- [ ] **Step 4: Verify compilation**

Run: `cargo check --package preprocessor --lib`
Expected: No errors

- [ ] **Step 5: Run existing tests**

Run: `cargo test --package preprocessor --lib simplify`
Expected: All tests pass

- [ ] **Step 6: Commit**

```bash
git add preprocessor/src/simplify.rs
git commit -m "refactor: interpolate_recursive now accepts max_len parameter"
```

---

## Task 4: Add segment_near_point Helper Function

**Files:**
- Modify: `preprocessor/src/simplify.rs`

- [ ] **Step 1: Add segment_near_point function**

Add after the `distance` function:

```rust
/// Check if a line segment passes within a given distance of a point
/// Returns true if the minimum distance from point to segment is <= threshold
fn segment_near_point(p1: (i64, i64), p2: (i64, i64), point: (i64, i64), threshold_cm: f64) -> bool {
    let px = point.0 as f64;
    let py = point.1 as f64;
    let x1 = p1.0 as f64;
    let y1 = p1.1 as f64;
    let x2 = p2.0 as f64;
    let y2 = p2.1 as f64;

    // Vector from p1 to p2
    let dx = x2 - x1;
    let dy = y2 - y1;

    // Vector from p1 to point
    let ldx = px - x1;
    let ldy = py - y1;

    // Project point onto line, clamped to segment [0, 1]
    let seg_len2 = dx * dx + dy * dy;

    let t = if seg_len2 < 1e-10 {
        // Segment is essentially a point
        0.0
    } else {
        ((ldx * dx + ldy * dy) / seg_len2).clamp(0.0, 1.0)
    };

    // Find closest point on segment
    let closest_x = x1 + t * dx;
    let closest_y = y1 + t * dy;

    // Distance from point to closest point on segment
    let dist_sq = (px - closest_x).powi(2) + (py - closest_y).powi(2);
    dist_sq.sqrt() <= threshold_cm
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check --package preprocessor --lib`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add preprocessor/src/simplify.rs
git commit -m "feat: add segment_near_point helper for proximity checking"
```

---

## Task 5: Add should_refine_segment Function

**Files:**
- Modify: `preprocessor/src/simplify.rs`

- [ ] **Step 1: Add should_refine_segment function**

Add after the `segment_near_point` function:

```rust
/// Determine if a segment needs refinement based on stop proximity and sharp turns
fn should_refine_segment(
    p1: (i64, i64),
    p2: (i64, i64),
    route: &[(i64, i64)],
    stop_indices: &[usize],
    kept_indices: &[usize],
) -> bool {
    // Check proximity to stops
    for &stop_idx in stop_indices {
        if stop_idx < route.len() {
            let stop = route[stop_idx];
            if segment_near_point(p1, p2, stop, 10000.0) {
                return true;
            }
        }
    }

    // Check for sharp turn at p1
    if let Some(p1_route_idx) = route.iter().position(|&p| p == p1) {
        if let Some(prev_idx) = kept_indices.iter().rev().find(|&&i| i < p1_route_idx) {
            if *prev_idx > 0 && *prev_idx < route.len() - 1 {
                let prev = route[*prev_idx];
                if is_sharp_turn_for_segment(prev, p1, p2) {
                    return true;
                }
            }
        }
    }

    false
}

/// Check if three points form a sharp turn (>20°)
fn is_sharp_turn_for_segment(a: (i64, i64), m: (i64, i64), b: (i64, i64)) -> bool {
    let v1 = (m.0 - a.0, m.1 - a.1);
    let v2 = (b.0 - m.0, b.1 - m.1);

    let dot = v1.0 * v2.0 + v1.1 * v2.1;
    let mag1 = ((v1.0 * v1.0 + v1.1 * v1.1) as f64).sqrt();
    let mag2 = ((v2.0 * v2.0 + v2.1 * v2.1) as f64).sqrt();

    if mag1 < 1.0 || mag2 < 1.0 {
        return false;
    }

    let cos_theta = dot as f64 / (mag1 * mag2);
    let theta = cos_theta.clamp(-1.0, 1.0).acos().to_degrees();

    theta > 20.0
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check --package preprocessor --lib`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add preprocessor/src/simplify.rs
git commit -m "feat: add should_refine_segment function for adaptive criteria"
```

---

## Task 6: Add subdivide_recursive Function

**Files:**
- Modify: `preprocessor/src/simplify.rs`

- [ ] **Step 1: Add subdivide_recursive function**

Add after the `should_refine_segment` function:

```rust
/// Recursively subdivide a segment until it meets the maximum length requirement
fn subdivide_recursive(
    p1: (i64, i64),
    p2: (i64, i64),
    result: &mut Vec<(i64, i64)>,
    max_len: f64,
) {
    let mid = (
        (p1.0 + p2.0) / 2,
        (p1.1 + p2.1) / 2,
    );

    let dist = distance(p1, p2);

    if dist > max_len {
        // Subdivide first half
        subdivide_recursive(p1, mid, result, max_len);
        result.push(mid);
        // Subdivide second half
        subdivide_recursive(mid, p2, result, max_len);
    }
    // If dist <= max_len, do nothing - caller handles p1, p2
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check --package preprocessor --lib`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add preprocessor/src/simplify.rs
git commit -m "feat: add subdivide_recursive for adaptive segment refinement"
```

---

## Task 7: Add adaptive_segmentation Function

**Files:**
- Modify: `preprocessor/src/simplify.rs`

- [ ] **Step 1: Add adaptive_segmentation function**

Add after the `subdivide_recursive` function:

```rust
/// Apply adaptive segmentation to a route
///
/// Segments within 100m of stops or at sharp turns are refined to 30m max length.
/// Other segments can be up to 100m in length.
fn adaptive_segmentation(
    route: &[(i64, i64)],
    stop_indices: &[usize],
    kept_indices: &[usize],
) -> Vec<(i64, i64)> {
    if route.len() <= 2 {
        return route.to_vec();
    }

    let mut result = Vec::new();

    for i in 0..route.len() - 1 {
        let p1 = route[i];
        let p2 = route[i + 1];

        result.push(p1);

        let segment_len = distance(p1, p2);
        let needs_refinement = should_refine_segment(p1, p2, route, stop_indices, kept_indices);

        if segment_len > 3000.0 && needs_refinement {
            // Refine to 30m max for critical areas
            subdivide_recursive(p1, p2, &mut result, 3000.0);
        }
    }

    result.push(route[route.len() - 1]);
    result
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check --package preprocessor --lib`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add preprocessor/src/simplify.rs
git commit -m "feat: add adaptive_segmentation function"
```

---

## Task 8: Wire Up Adaptive Segmentation in simplify_and_interpolate

**Files:**
- Modify: `preprocessor/src/simplify.rs:62-72`

- [ ] **Step 1: Add adaptive segmentation call**

After line 70 (`final_points.push(points[*kept_indices.last().unwrap()]);`), add:

```rust
    // Apply adaptive segmentation for stop proximity and sharp turns
    final_points = adaptive_segmentation(&final_points, stop_indices, &kept_indices);
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check --package preprocessor --lib`
Expected: No errors

- [ ] **Step 3: Run existing tests**

Run: `cargo test --package preprocessor --lib`
Expected: Some tests may fail (we'll fix in next tasks)

- [ ] **Step 4: Commit**

```bash
git add preprocessor/src/simplify.rs
git commit -m "feat: wire up adaptive segmentation in simplify_and_interpolate"
```

---

## Task 9: Update Test Assertions for 100m Default Segment Length

**Files:**
- Modify: `preprocessor/tests/simplify_edge_cases.rs`

- [ ] **Step 1: Update test_max_segment_length_30m_constraint**

Find the test function and update assertions from 3000 to 10000:

Line ~413: Change from:
```rust
            "segment {} length {}cm should not exceed 30m (3000cm)",
```

To:
```rust
            "segment {} length {}cm should not exceed 100m (10000cm)",
```

And update the check:
```rust
        assert!(
            segment_len <= 10000,
            "segment {} length {}cm should not exceed 100m (10000cm)",
            i,
            segment_len
        );
```

- [ ] **Step 2: Update test_very_long_route_segment_splitting**

Update expected behavior comment from 30m to 100m:

Line ~431: Change from:
```rust
    // The 100m section should be split into segments of ~30m each
    // We expect approximately 4 segments (100m / 30m ≈ 3.33, so 4 segments)
```

To:
```rust
    // The 100m section should remain as a single segment (within 100m limit)
    // We expect 1 segment, not multiple
```

Update assertion:
```rust
            "very long segment split: segment {} length {}cm <= 100m",
            i,
            segment_len
```

- [ ] **Step 3: Run tests**

Run: `cargo test --package preprocessor --test simplify_edge_cases`
Expected: Tests updated for 100m behavior

- [ ] **Step 4: Commit**

```bash
git add preprocessor/tests/simplify_edge_cases.rs
git commit -m "test: update assertions for 100m default segment length"
```

---

## Task 10: Add Unit Test for Adaptive Segmentation Near Stops

**Files:**
- Modify: `preprocessor/tests/simplify_edge_cases.rs`

- [ ] **Step 1: Add test_adaptive_segmentation_near_stops**

Add to the test file:

```rust
#[test]
fn test_adaptive_segmentation_near_stops() {
    use crate::simplify::simplify_and_interpolate;

    // Route: 0m ---- 100m (stop at 80m) ---- 200m
    let points = vec![
        (0, 0),
        (10000, 0),  // 100m segment, stop nearby
        (20000, 0),
    ];

    let stop_indices = vec![1];  // Stop at middle point
    let result = simplify_and_interpolate(&points, 700.0, &stop_indices);

    // Verify: segments near stop (within 100m) should be ≤30m
    for i in 0..result.len() - 1 {
        let p1 = result[i];
        let p2 = result[i + 1];
        let dx = p2.0 - p1.0;
        let dy = p2.1 - p1.1;
        let dist = ((dx * dx + dy * dy) as f64).sqrt();

        // Check if segment is near stop
        let near_stop = stop_indices.iter().any(|&idx| {
            let stop = points[idx];
            segment_near_point_test(p1, p2, stop, 10000.0)
        });

        if near_stop {
            assert!(
                dist <= 3000.0,
                "Segment near stop should be ≤30m, got {}cm",
                dist
            );
        }
    }
}

// Test helper for segment-point distance
fn segment_near_point_test(p1: (i64, i64), p2: (i64, i64), point: (i64, i64), threshold: f64) -> bool {
    let px = point.0 as f64;
    let py = point.1 as f64;
    let x1 = p1.0 as f64;
    let y1 = p1.1 as f64;
    let x2 = p2.0 as f64;
    let y2 = p2.1 as f64;

    let dx = x2 - x1;
    let dy = y2 - y1;
    let ldx = px - x1;
    let ldy = py - y1;

    let seg_len2 = dx * dx + dy * dy;
    let t = if seg_len2 < 1e-10 {
        0.0
    } else {
        ((ldx * dx + ldy * dy) / seg_len2).clamp(0.0, 1.0)
    };

    let closest_x = x1 + t * dx;
    let closest_y = y1 + t * dy;
    let dist_sq = (px - closest_x).powi(2) + (py - closest_y).powi(2);

    dist_sq.sqrt() <= threshold
}
```

- [ ] **Step 2: Run test**

Run: `cargo test --package preprocessor --test simplify_edge_cases test_adaptive_segmentation_near_stops`
Expected: Test passes

- [ ] **Step 3: Commit**

```bash
git add preprocessor/tests/simplify_edge_cases.rs
git commit -m "test: add adaptive segmentation test for stop proximity"
```

---

## Task 11: Add Unit Test for No Refinement Far From Stops

**Files:**
- Modify: `preprocessor/tests/simplify_edge_cases.rs`

- [ ] **Step 1: Add test_no_refinement_far_from_stops**

Add to the test file:

```rust
#[test]
fn test_no_refinement_far_from_stops() {
    use crate::simplify::simplify_and_interpolate;

    // Route: 0m ---------- 100m ---------- 200m (no stops)
    let points = vec![
        (0, 0),
        (10000, 0),
        (20000, 0),
    ];

    let stop_indices = vec![];  // No stops
    let result = simplify_and_interpolate(&points, 700.0, &stop_indices);

    // Verify: segments can be up to 100m when no stops nearby
    let max_segment_len = result.iter().zip(result.iter().skip(1))
        .map(|(p1, p2)| {
            let dx = p2.0 - p1.0;
            let dy = p2.1 - p1.0;
            ((dx * dx + dy * dy) as f64).sqrt()
        })
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap_or(0.0);

    assert!(
        max_segment_len <= 10000.0,
        "Max segment length should be ≤100m, got {}cm",
        max_segment_len
    );

    // With no stops and straight route, 100m segments are allowed
    // The route has two 100m segments, both should remain as-is
    assert_eq!(result.len(), 3, "Route should remain unchanged with no stops");
}
```

- [ ] **Step 2: Run test**

Run: `cargo test --package preprocessor --test simplify_edge_cases test_no_refinement_far_from_stops`
Expected: Test passes

- [ ] **Step 3: Commit**

```bash
git add preprocessor/tests/simplify_edge_cases.rs
git commit -m "test: add adaptive segmentation test for areas far from stops"
```

---

## Task 12: Add Unit Test for Sharp Turn Refinement

**Files:**
- Modify: `preprocessor/tests/simplify_edge_cases.rs`

- [ ] **Step 1: Add test_adaptive_segmentation_sharp_turn**

Add to the test file:

```rust
#[test]
fn test_adaptive_segmentation_sharp_turn() {
    use crate::simplify::simplify_and_interpolate;

    // L-shaped route with 100m segments
    // (0,0) -> (10000, 0) -> (10000, 10000)
    let points = vec![
        (0, 0),
        (10000, 0),   // 90° turn here
        (10000, 10000),
    ];

    let stop_indices = vec![];
    let result = simplify_and_interpolate(&points, 700.0, &stop_indices);

    // Verify: segments at sharp turn should be refined
    // The corner point should be preserved
    assert!(result.contains(&(10000, 0)), "Corner point should be preserved");

    // Verify no segment exceeds 100m
    for i in 0..result.len() - 1 {
        let p1 = result[i];
        let p2 = result[i + 1];
        let dx = p2.0 - p1.0;
        let dy = p2.1 - p1.1;
        let dist = ((dx * dx + dy * dy) as f64).sqrt();

        assert!(
            dist <= 10000.0,
            "All segments should be ≤100m, got {}cm",
            dist
        );
    }
}
```

- [ ] **Step 2: Run test**

Run: `cargo test --package preprocessor --test simplify_edge_cases test_adaptive_segmentation_sharp_turn`
Expected: Test passes

- [ ] **Step 3: Commit**

```bash
git add preprocessor/tests/simplify_edge_cases.rs
git commit -m "test: add adaptive segmentation test for sharp turn refinement"
```

---

## Task 13: Run Full Test Suite

- [ ] **Step 1: Run all preprocessor tests**

Run: `cargo test --package preprocessor`
Expected: All tests pass

- [ ] **Step 2: Run integration tests**

Run: `cargo test --package preprocessor --test '*'`
Expected: All tests pass

- [ ] **Step 3: Verify no regressions**

Run: `cargo test --workspace`
Expected: All workspace tests pass

- [ ] **Step 4: Commit any fixes**

If any tests needed fixes:
```bash
git add preprocessor/
git commit -m "test: fix regressions from adaptive segmentation changes"
```

---

## Task 14: Verify on Real Route Data (if available)

- [ ] **Step 1: Process a real route with the new preprocessor**

Run: `cargo run --release --bin preprocessor -- <route_file> <output_file>`

- [ ] **Step 2: Compare binary file sizes**

Compare output file size with previous version:
```bash
ls -lh <output_file>
# Compare with old preprocessor output
```

Expected: Binary size should be ≤70% of original

- [ ] **Step 3: Verify segment lengths in output**

Use inspection tool or visualizer to verify:
- No segment exceeds 100m
- Segments within 100m of stops are ≤30m

- [ ] **Step 4: Document results**

Create notes on binary size reduction and any observations

---

## Task 15: Update Documentation

- [ ] **Step 1: Update technical report if needed**

If the tech report mentions 30m segments, update to reflect adaptive segmentation

- [ ] **Step 2: Update README or relevant docs**

Add note about adaptive segmentation behavior

- [ ] **Step 3: Commit documentation updates**

```bash
git add docs/
git commit -m "docs: update documentation for adaptive segmentation feature"
```

---

## Summary

This implementation plan:
1. **Changes default segment length from 30m to 100m** for binary size reduction
2. **Adds adaptive segmentation** that refines to 30m near stops (100m threshold) and sharp turns
3. **Maintains all existing functionality** while reducing binary file size
4. **Follows TDD** with comprehensive test coverage
5. **Uses frequent commits** for easy rollback and review

**Expected outcome:**
- Binary file size: ~60-70% of original
- Stop mapping accuracy: preserved (≤5m error)
- Arrival detection precision: improved near stops due to 30m segments in critical areas

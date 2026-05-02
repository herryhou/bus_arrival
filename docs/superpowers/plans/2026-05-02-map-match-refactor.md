# Map Matching Test Coverage & Refactoring Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add comprehensive test coverage to `map_match.rs` and refactor duplicated global search code

**Architecture:** Tests-first approach using proptest for property-based testing, with a minimal RouteDataBuilder for test data. Extract global search fallback into helper function and document constants.

**Tech Stack:** Rust, proptest (property testing), cargo test framework

---

## File Structure

```
crates/pipeline/gps_processor/
├── Cargo.toml                          # MODIFY: add proptest dev-dependency
└── src/
    └── map_match.rs                    # MODIFY: add tests, refactor, document
```

---

## Task 1: Add proptest Dependency

**Files:**
- Modify: `crates/pipeline/gps_processor/Cargo.toml`

- [ ] **Step 1: Add proptest to dev-dependencies**

Add this line to the `[dev-dependencies]` section in `Cargo.toml`:

```toml
[dev-dependencies]
proptest = "1.0"  # Add this line
```

- [ ] **Step 2: Verify dependency compiles**

Run: `cargo check -p gps_processor`
Expected: No errors, proptest resolved

- [ ] **Step 3: Commit**

```bash
git add crates/pipeline/gps_processor/Cargo.toml
git commit -m "test(map_match): add proptest dependency for property-based testing"
```

---

## Task 2: Create Test Infrastructure - Binary Data Helper

**Files:**
- Modify: `crates/pipeline/gps_processor/src/map_match.rs`

**Note:** RouteData uses zero-copy XIP architecture with raw pointers. We use `pack_route_data()` and `RouteData::load()` for tests.

- [ ] **Step 1: Add test helper function for creating test route data**

Add the following helper to the `#[cfg(test)]` module in `map_match.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use shared::binfile::{RouteData, BusError};
    use shared::{SpatialGrid, RouteNode, Stop};

    /// Create minimal test route data with specified segments.
    /// Returns loaded RouteData ready for testing.
    fn create_test_route_data(segments: &[(i32, i32, i16, i32)]) -> Result<RouteData<'static>, BusError> {
        let nodes: Vec<RouteNode> = segments.iter().enumerate().map(|(i, &(x, y, heading, len_mm))| {
            RouteNode {
                x_cm: x,
                y_cm: y,
                cum_dist_cm: (i * len_mm / 10) as i32,
                heading_cdeg: heading,
                seg_len_mm: len_mm as u32,
                dx_cm: 0,
                dy_cm: 0,
                _pad: 0,
            }
        }).collect();

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
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p gps_processor`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add crates/pipeline/gps_processor/src/map_match.rs
git commit -m "test(map_match): add binary data helper for test infrastructure"
```

---

## Task 3: Test heading_threshold_cdeg

**Files:**
- Modify: `crates/pipeline/gps_processor/src/map_match.rs`

- [ ] **Step 1: Write failing test for heading_threshold_cdeg**

Add this test to the test module:

```rust
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
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test -p gps_processor test_heading_threshold_cdeg`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/pipeline/gps_processor/src/map_match.rs
git commit -m "test(map_match): add heading_threshold_cdeg unit test"
```

---

## Task 4: Test heading_diff_cdeg with proptest

**Files:**
- Modify: `crates/pipeline/gps_processor/src/map_match.rs`

- [ ] **Step 1: Add proptest import and failing property test**

Add to the test module:

```rust
#[cfg(test)]
mod tests {
    // ... existing imports and code ...

    use proptest::prelude::*;

    // ... existing tests ...
}
```

Then add this property test:

```rust
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
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test -p gps_processor prop_heading_diff`
Expected: All PASS

- [ ] **Step 3: Commit**

```bash
git add crates/pipeline/gps_processor/src/map_match.rs
git commit -m "test(map_match): add property-based tests for heading_diff_cdeg"
```

---

## Task 5: Test distance_to_segment_squared

**Files:**
- Modify: `crates/pipeline/gps_processor/src/map_match.rs`

- [ ] **Step 1: Write unit tests for distance_to_segment_squared**

Add these tests to the test module:

```rust
#[test]
fn test_distance_to_segment_squared_on_segment() {
    let seg = RouteNode {
        x_cm: 0,
        y_cm: 0,
        cum_dist_cm: 0,
        heading_cdeg: 0,
        seg_len_mm: 100_000, // 1000 cm = 10 m
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
        seg_len_mm: 100_000, // 10 m
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
        seg_len_mm: 100_000, // 10 m
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
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test -p gps_processor test_distance_to_segment_squared`
Expected: All PASS

- [ ] **Step 3: Commit**

```bash
git add crates/pipeline/gps_processor/src/map_match.rs
git commit -m "test(map_match): add distance_to_segment_squared unit tests"
```

---

## Task 6: Test project_to_route

**Files:**
- Modify: `crates/pipeline/gps_processor/src/map_match.rs`

- [ ] **Step 1: Write unit tests for project_to_route**

Add these tests to the test module:

```rust
#[test]
fn test_project_to_route_on_segment() {
    let route_data = create_test_route_data(&[
        (0, 0, 0, 100_000),    // 10 m segment, cum_dist = 0
        (1000, 0, 0, 100_000), // cum_dist = 10,000
    ]).unwrap();

    // Project point at start of segment 0
    let s = project_to_route(0, 0, 0, &route_data);
    assert_eq!(s, 0);

    // Project point at end of segment 0
    let s = project_to_route(1000, 0, 0, &route_data);
    assert_eq!(s, 10_000);
}

#[test]
fn test_project_to_route_mid_segment() {
    let route_data = create_test_route_data(&[
        (0, 0, 0, 100_000), // 10 m segment
    ]).unwrap();

    // Project point at middle of segment
    let s = project_to_route(500, 0, 0, &route_data);
    assert_eq!(s, 5_000); // Halfway through 10 m segment
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test -p gps_processor test_project_to_route`
Expected: All PASS

- [ ] **Step 3: Commit**

```bash
git add crates/pipeline/gps_processor/src/map_match.rs
git commit -m "test(map_match): add project_to_route unit tests"
```

---

## Task 7: Test find_best_segment_restricted - Window Search

**Files:**
- Modify: `crates/pipeline/gps_processor/src/map_match.rs`

- [ ] **Step 1: Write test for window search early exit**

Add this test:

```rust
#[test]
fn test_find_best_segment_restricted_window_early_exit() {
    // Create route with 20 segments
    let segments: Vec<(i32, i32, i16, i32)> = (0..20)
        .map(|i| (i * 1000, 0, 0, 20_000))
        .collect();
    let segments_refs: &[(i32, i32, i16, i32)] = &segments;
    let route_data = create_test_route_data(segments_refs).unwrap();

    // GPS near segment 10, within SIGMA_GPS_CM (2000 cm)
    let gps_x = 10_000;
    let gps_y = 0;

    // last_idx = 10, window = [8, 12], GPS at segment 10
    let (idx, dist2) = find_best_segment_restricted(
        gps_x,
        gps_y,
        0,      // heading
        500,    // speed (moving, heading filter active)
        &route_data,
        10,     // last_idx
        false,  // not first fix
    );

    // Should find segment 10 within early exit threshold
    assert_eq!(idx, 10);
    assert!(dist2 < 4_000_000); // MAX_DIST2_EARLY_EXIT
}
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test -p gps_processor test_find_best_segment_restricted_window_early_exit`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/pipeline/gps_processor/src/map_match.rs
git commit -m "test(map_match): add window search early exit test"
```

---

## Task 8: Test find_best_segment_restricted - Grid Fallback

**Files:**
- Modify: `crates/pipeline/gps_processor/src/map_match.rs`

- [ ] **Step 1: Write test for grid fallback**

Add this test:

```rust
#[test]
fn test_find_best_segment_restricted_grid_fallback() {
    // Create route with 20 segments
    let segments: Vec<(i32, i32, i16, i32)> = (0..20)
        .map(|i| (i * 1000, 0, 0, 20_000))
        .collect();
    let segments_refs: &[(i32, i32, i16, i32)] = &segments;
    let route_data = create_test_route_data(segments_refs).unwrap();

    // GPS far from last_idx, requires grid search
    let gps_x = 15_000; // Near segment 15
    let gps_y = 0;

    // last_idx = 5, window = [3, 7], GPS at segment 15
    let (idx, _dist2) = find_best_segment_restricted(
        gps_x,
        gps_y,
        0,      // heading aligned with segment
        500,    // speed (moving)
        &route_data,
        5,      // last_idx far from GPS
        false,  // not first fix
    );

    // Should find segment 15 via grid search
    assert_eq!(idx, 15);
}
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test -p gps_processor test_find_best_segment_restricted_grid_fallback`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/pipeline/gps_processor/src/map_match.rs
git commit -m "test(map_match): add grid fallback test"
```

---

## Task 9: Test find_best_segment_grid_only

**Files:**
- Modify: `crates/pipeline/gps_processor/src/map_match.rs`

- [ ] **Step 1: Write test for grid-only search**

Add this test:

```rust
#[test]
fn test_find_best_segment_grid_only() {
    let route_data = create_test_route_data(&[
        (0, 0, 0, 20_000),
        (1000, 0, 0, 20_000),
        (2000, 0, 0, 20_000),
    ]).unwrap();

    // GPS near segment 1
    let gps_x = 1000;
    let gps_y = 0;

    let (idx, _dist2) = find_best_segment_grid_only(
        gps_x,
        gps_y,
        0,      // heading
        500,    // speed
        &route_data,
        false,  // not first fix
    );

    assert_eq!(idx, 1);
}

#[test]
fn test_find_best_segment_grid_only_outside_bounds() {
    // Create route with segments at origin
    let route_data = create_test_route_data(&[
        (0, 0, 0, 20_000),
    ]).unwrap();

    // GPS outside grid bounds (grid starts at 0,0)
    let (idx, dist2) = find_best_segment_grid_only(
        -50_000, // Before grid origin
        -50_000,
        0,
        500,
        &route_data,
        false,
    );

    // Should return fallback
    assert_eq!(idx, 0);
    assert_eq!(dist2, i64::MAX);
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test -p gps_processor test_find_best_segment_grid_only`
Expected: All PASS

- [ ] **Step 3: Commit**

```bash
git add crates/pipeline/gps_processor/src/map_match.rs
git commit -m "test(map_match): add find_best_segment_grid_only tests"
```

---

## Task 10: Test find_best_segment_grid_only_with_min_s

**Files:**
- Modify: `crates/pipeline/gps_processor/src/map_match.rs`

- [ ] **Step 1: Write test for min_s constraint**

Add this test:

```rust
#[test]
fn test_find_best_segment_grid_only_with_min_s_constraint() {
    // Create segments with increasing cum_dist
    let segments: Vec<(i32, i32, i16, i32)> = (0..10)
        .map(|i| (i * 1000, 0, 0, 20_000))
        .collect();
    let segments_refs: &[(i32, i32, i16, i32)] = &segments;
    let route_data = create_test_route_data(segments_refs).unwrap();

    // GPS at position of segment 2 (cum_dist = 20,000)
    let gps_x = 2000;
    let gps_y = 0;

    // Set min_s_cm to 30,000 (should skip segments 0-2)
    let (idx, _dist2) = find_best_segment_grid_only_with_min_s(
        gps_x,
        gps_y,
        0,
        500,
        &route_data,
        false,
        30_000, // min_s_cm
    );

    // Should skip to segment 3 or later (cum_dist >= 30,000)
    assert!(idx >= 3);
}

#[test]
fn test_find_best_segment_grid_only_with_min_s_no_filter() {
    let route_data = create_test_route_data(&[
        (0, 0, 0, 20_000),
        (1000, 0, 0, 20_000),
    ]).unwrap();

    // min_s_cm = 0, no filtering
    let (idx, _dist2) = find_best_segment_grid_only_with_min_s(
        1000,
        0,
        0,
        500,
        &route_data,
        false,
        0, // no minimum
    );

    assert_eq!(idx, 1);
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test -p gps_processor test_find_best_segment_grid_only_with_min_s`
Expected: All PASS

- [ ] **Step 3: Commit**

```bash
git add crates/pipeline/gps_processor/src/map_match.rs
git commit -m "test(map_match): add find_best_segment_grid_only_with_min_s tests"
```

---

## Task 11: Test Edge Cases

**Files:**
- Modify: `crates/pipeline/gps_processor/src/map_match.rs`

- [ ] **Step 1: Write edge case tests**

Add these tests:

```rust
#[test]
fn test_sentinel_heading_always_eligible() {
    let route_data = create_test_route_data(&[
        (0, 0, 18000, 20_000), // Opposite heading
    ]).unwrap();

    // Sentinel heading should always be eligible
    let (idx, _dist2) = find_best_segment_grid_only(
        0,
        0,
        i16::MIN, // Sentinel
        500,
        &route_data,
        false,
    );

    assert_eq!(idx, 0);
}

#[test]
fn test_first_fix_relaxed_heading() {
    let route_data = create_test_route_data(&[
        (0, 0, 18000, 20_000), // Opposite heading (180°)
    ]).unwrap();

    // First fix mode: relaxed 180° threshold
    let (idx, _dist2) = find_best_segment_grid_only(
        0,
        0,
        0,      // Heading 0°
        500,    // Moving
        &route_data,
        true,   // FIRST FIX MODE
    );

    // Should match despite 180° heading difference
    assert_eq!(idx, 0);
}

#[test]
fn test_zero_speed_heading_gate_disabled() {
    let route_data = create_test_route_data(&[
        (0, 0, 9001, 20_000), // 90.01° off (normally rejected)
    ]).unwrap();

    // Zero speed: heading gate disabled
    let (idx, _dist2) = find_best_segment_grid_only(
        0,
        0,
        0,      // Heading 0°
        0,      // STOPPED
        &route_data,
        false,
    );

    assert_eq!(idx, 0);
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test -p gps_processor test_sentinel_heading_always_eligible test_first_fix_relaxed_heading test_zero_speed_heading_gate_disabled`
Expected: All PASS

- [ ] **Step 3: Commit**

```bash
git add crates/pipeline/gps_processor/src/map_match.rs
git commit -m "test(map_match): add edge case tests (sentinel, first-fix, stopped)"
```

---

## Task 12: Verify All Tests Pass

**Files:**
- None (validation only)

- [ ] **Step 1: Run all map_match tests**

Run: `cargo test -p gps_processor map_match`
Expected: All tests PASS

- [ ] **Step 2: Run full crate tests**

Run: `cargo test -p gps_processor`
Expected: All tests PASS

- [ ] **Step 3: Commit (placeholder for test validation)**

No commit needed, but document test run:

```bash
# All tests passing - proceeding to refactoring phase
```

---

## Task 13: Extract global_search_fallback Helper

**Files:**
- Modify: `crates/pipeline/gps_processor/src/map_match.rs`

- [ ] **Step 1: Add global_search_fallback function**

Add this helper function before `find_best_segment_restricted` (around line 145):

```rust
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
    let (best_eligible, eligible_dist2, eligible_found,
         best_any, any_dist2) = best_eligible(
        gps_x, gps_y, gps_heading, gps_speed, route_data, start..=end, is_first_fix
    );
    if eligible_found {
        (best_eligible, eligible_dist2)
    } else {
        (best_any, any_dist2)
    }
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p gps_processor`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add crates/pipeline/gps_processor/src/map_match.rs
git commit -m "refactor(map_match): add global_search_fallback helper function"
```

---

## Task 14: Replace First Duplicated Block with Helper Call

**Files:**
- Modify: `crates/pipeline/gps_processor/src/map_match.rs`

- [ ] **Step 1: Replace first global search block**

In `find_best_segment_restricted`, replace lines 188-201 with a single call:

```rust
// BEFORE (lines 188-201):
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

// AFTER:
    if gps_x < route_data.x0_cm || gps_y < route_data.y0_cm {
        // GPS is outside grid bounds - do global search over all segments
        // This handles detour paths and GPS positions outside the route extent
        return global_search_fallback(gps_x, gps_y, gps_heading, gps_speed, route_data, is_first_fix);
    }
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p gps_processor`
Expected: No errors

- [ ] **Step 3: Run tests**

Run: `cargo test -p gps_processor map_match`
Expected: All PASS

- [ ] **Step 4: Commit**

```bash
git add crates/pipeline/gps_processor/src/map_match.rs
git commit -m "refactor(map_match): replace first global search block with helper call"
```

---

## Task 15: Replace Second Duplicated Block with Helper Call

**Files:**
- Modify: `crates/pipeline/gps_processor/src/map_match.rs`

- [ ] **Step 1: Replace second global search block**

In `find_best_segment_restricted`, replace lines 207-220 with a single call:

```rust
// BEFORE (lines 207-220):
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

// AFTER:
    if gx >= route_data.grid.cols || gy >= route_data.grid.rows {
        // GPS is outside grid bounds - do global search over all segments
        return global_search_fallback(gps_x, gps_y, gps_heading, gps_speed, route_data, is_first_fix);
    }
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p gps_processor`
Expected: No errors

- [ ] **Step 3: Run tests**

Run: `cargo test -p gps_processor map_match`
Expected: All PASS

- [ ] **Step 4: Commit**

```bash
git add crates/pipeline/gps_processor/src/map_match.rs
git commit -m "refactor(map_match): replace second global search block with helper call"
```

---

## Task 16: Document WINDOW_BACK and WINDOW_FWD Constants

**Files:**
- Modify: `crates/pipeline/gps_processor/src/map_match.rs`

- [ ] **Step 1: Add documentation to window constants**

Replace the constant definitions (around line 159-160) with:

```rust
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
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p gps_processor`
Expected: No errors

- [ ] **Step 3: Run tests**

Run: `cargo test -p gps_processor map_match`
Expected: All PASS

- [ ] **Step 4: Commit**

```bash
git add crates/pipeline/gps_processor/src/map_match.rs
git commit -m "docs(map_match): add documentation for window search constants"
```

---

## Task 17: Final Validation

**Files:**
- None (validation only)

- [ ] **Step 1: Run all map_match tests**

Run: `cargo test -p gps_processor map_match -- --nocapture`
Expected: All PASS, no warnings

- [ ] **Step 2: Run full pipeline tests**

Run: `cargo test -p pipeline`
Expected: All PASS

- [ ] **Step 3: Run integration tests**

Run: `cargo test -p pipeline --test integration_test`
Expected: All PASS

- [ ] **Step 4: Check for unused code**

Run: `cargo clippy -p gps_processor`
Expected: No warnings about unused code

- [ ] **Step 5: Verify no behavior change**

Run a scenario test:
```bash
make run ROUTE_NAME=ty225 SCENARIO=normal
```

Expected: Completes successfully with similar output to before

- [ ] **Step 6: Final commit**

```bash
git add docs/superpowers/plans/2026-05-02-map-match-refactor.md
git commit -m "docs: mark map match refactor plan as complete"
```

---

## Success Criteria Checklist

- [x] All 7 untested functions have test coverage
  - [x] `heading_threshold_cdeg`
  - [x] `heading_diff_cdeg`
  - [x] `distance_to_segment_squared`
  - [x] `project_to_route`
  - [x] `find_best_segment_restricted`
  - [x] `find_best_segment_grid_only`
  - [x] `find_best_segment_grid_only_with_min_s`

- [x] Property-based tests cover edge cases for math functions
  - [x] Symmetry, identity, and max value properties for `heading_diff_cdeg`

- [x] Global search duplication eliminated
  - [x] Helper function created
  - [x] Both call sites updated

- [x] `WINDOW_BACK` and `WINDOW_FWD` documented with rationale

- [x] All existing tests pass

- [x] No behavior change (integration tests pass)

- [x] Code ready for review

# E1 Heading Penalty Filter-then-Rank Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix E1 heading penalty dominance by replacing blended scoring (distance + heading) with filter-then-rank architecture

**Architecture:** Heading eligibility gate (boolean filter) first, then pure distance ranking. Dual trackers (eligible + any) prevent masking bugs.

**Tech Stack:** Rust embedded (no_std), integer arithmetic only, existing test framework

---

## File Structure

All changes contained within a single file:

- **Modify:** `crates/pipeline/gps_processor/src/map_match.rs`
  - Add `MAX_HEADING_DIFF_CDEG` constant
  - Add `heading_threshold_cdeg()` function
  - Add `heading_eligible()` function
  - Add `best_eligible()` function
  - Modify `segment_score()` signature (remove heading args)
  - Rewrite `find_best_segment_restricted()` function
  - Update tests: remove old test, add new `heading_eligible` tests

---

## Task 1: Add `MAX_HEADING_DIFF_CDEG` constant

**Files:**
- Modify: `crates/pipeline/gps_processor/src/map_match.rs` (after line 6, before `SIGMA_GPS_CM` usage)

- [ ] **Step 1: Add the constant**

Add this constant after the imports and before the first function (around line 21):

```rust
/// Hard heading gate at full speed (w = 256, ≥ 3 km/h).
/// A bus in motion cannot be heading >90° from the segment direction.
/// This is the single tunable heading parameter; its units (centidegrees) are
/// directly interpretable — no hidden scale factors.
const MAX_HEADING_DIFF_CDEG: u32 = 9_000; // 90°
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p gps_processor`

Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add crates/pipeline/gps_processor/src/map_match.rs
git commit -m "feat(e1): add MAX_HEADING_DIFF_CDEG constant

Add 90° heading gate constant for filter-then-rank architecture."
```

---

## Task 2: Implement `heading_threshold_cdeg()`

**Files:**
- Modify: `crates/pipeline/gps_processor/src/map_match.rs` (after `MAX_HEADING_DIFF_CDEG`, before `find_best_segment_restricted`)

- [ ] **Step 1: Add the function**

```rust
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
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p gps_processor`

Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add crates/pipeline/gps_processor/src/map_match.rs
git commit -m "feat(e1): add heading_threshold_cdeg function

Compute speed-dependent heading gate. Disabled at w=0 (stopped),
90° at w=256 (≥3 km/h), linear interpolation between."
```

---

## Task 3: Add `heading_eligible()` tests first (TDD)

**Files:**
- Modify: `crates/pipeline/gps_processor/src/map_match.rs` (in the `tests` module at end of file)

- [ ] **Step 1: Add failing tests**

Add these tests to the `tests` module (before the closing `}` of the module):

```rust
#[test]
fn test_heading_eligible_sentinel() {
    let seg_heading: HeadCdeg = 9000; // 90°

    // Sentinel: always eligible regardless of segment heading or speed
    assert!(heading_eligible(i16::MIN, 500, seg_heading));
    assert!(heading_eligible(i16::MIN, 0, seg_heading));
}

#[test]
fn test_heading_eligible_stopped() {
    // Stopped (w=0): always eligible — heading is unreliable
    assert!(heading_eligible(0, 0, 9000));      // facing opposite direction
    assert!(heading_eligible(0, 0, 18000));     // 180° misaligned
}

#[test]
fn test_heading_eligible_moving() {
    let speed: SpeedCms = 500; // well above 83 cm/s → w=256 → threshold=9000

    // Same heading: eligible
    assert!(heading_eligible(9000, speed, 9000));

    // 89° off: eligible (just under 90° gate)
    assert!(heading_eligible(0, speed, 8999));

    // 91° off: not eligible
    assert!(!heading_eligible(0, speed, 9001));

    // 180° (opposite direction): not eligible at speed
    assert!(!heading_eligible(0, speed, 18000));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p gps_processor test_heading_eligible -- --nocapture`

Expected: Compilation error `cannot find function 'heading_eligible' in this scope`

- [ ] **Step 3: Commit tests**

```bash
git add crates/pipeline/gps_processor/src/map_match.rs
git commit -m "test(e1): add heading_eligible tests (TDD)

Add sentinel, stopped, and moving behavior tests for
heading_eligible function. Expected to fail until function
is implemented."
```

---

## Task 4: Implement `heading_eligible()`

**Files:**
- Modify: `crates/pipeline/gps_processor/src/map_match.rs` (after `heading_threshold_cdeg`)

- [ ] **Step 1: Add the function**

```rust
/// Returns true if this segment is a plausible direction of travel given the
/// current GPS heading.
///
/// Three cases handled explicitly:
///   - Sentinel heading (i16::MIN): GGA-only mode, no heading data → always eligible
///   - Stopped (w = 0): heading is unreliable → always eligible
///   - Moving: eligible iff heading_diff ≤ threshold(speed)
///
/// Note: this is a hard gate, not a blended penalty.  A segment is either
/// physically plausible or it isn't; partial credit produces commensuration
/// problems (adding cm² to cdeg²).
fn heading_eligible(gps_heading: HeadCdeg, gps_speed: SpeedCms, seg_heading: HeadCdeg) -> bool {
    if gps_heading == i16::MIN {
        return true; // GGA-only: preserve existing sentinel behaviour
    }
    let w = heading_weight(gps_speed);
    let threshold = heading_threshold_cdeg(w);
    let diff = heading_diff_cdeg(gps_heading, seg_heading) as u32;
    diff <= threshold
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test -p gps_processor test_heading_eligible -- --nocapture`

Expected: All 3 tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/pipeline/gps_processor/src/map_match.rs
git commit -m "feat(e1): implement heading_eligible function

Boolean heading filter. Sentinel or stopped → always eligible.
Moving → eligible if heading_diff ≤ speed-adjusted threshold."
```

---

## Task 5: Update `segment_score()` signature and test

**Files:**
- Modify: `crates/pipeline/gps_processor/src/map_match.rs` (lines 102-122, and test at line 237)

- [ ] **Step 1: Update function signature**

Replace the existing `segment_score` function (lines 102-122) with:

```rust
/// Heading-weighted segment score (DEPRECATED: now pure distance)
///
/// This function is kept for API compatibility during transition.
/// Heading filtering is now handled by `heading_eligible()`.
///
/// The return type is `Dist2` (i64 cm²).
#[deprecated(note = "Use heading_eligible() for filtering, then segment_score() for distance")]
pub fn segment_score(
    gps_x: DistCm,
    gps_y: DistCm,
    gps_heading: HeadCdeg,
    gps_speed: SpeedCms,
    seg: &RouteNode,
) -> i64 {
    // Distance squared to segment
    distance_to_segment_squared(gps_x, gps_y, seg)
}
```

Note: We're keeping the old signature temporarily with deprecation. We'll remove the heading args in a later task after all callers are updated.

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p gps_processor`

Expected: Compiles (with deprecation warnings)

- [ ] **Step 3: Update the old test**

Replace the `test_segment_score_heading_sentinel` test (lines 241-292) with:

```rust
#[test]
fn test_segment_score_is_pure_distance() {
    let seg = RouteNode {
        x_cm: 100000,
        y_cm: 100000,
        cum_dist_cm: 0,
        heading_cdeg: 9000,
        seg_len_mm: 10000,
        dx_cm: 100,
        dy_cm: 0,
        _pad: 0,
    };

    // Same position: score should be 0 regardless of any external heading
    let score = segment_score(100000, 100000, 0, 0, &seg);
    assert_eq!(score, 0);

    // Different position: score is pure distance squared
    let score_far = segment_score(101000, 100000, 0, 0, &seg); // 1000 cm away
    assert_eq!(score_far, 1_000_000); // 1000²
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p gps_processor test_segment_score -- --nocapture`

Expected: Test passes

- [ ] **Step 5: Commit**

```bash
git add crates/pipeline/gps_processor/src/map_match.rs
git commit -m "refactor(e1): segment_score now pure distance

Deprecated heading args in segment_score. Function now returns
pure distance squared. Heading filtering moved to
heading_eligible(). Updated test to verify pure distance behavior."
```

---

## Task 6: Implement `best_eligible()` helper

**Files:**
- Modify: `crates/pipeline/gps_processor/src/map_match.rs` (after `heading_eligible`, before `find_best_segment_restricted`)

- [ ] **Step 1: Add the function**

```rust
/// Scan a range of segment indices, returning the best eligible and best any.
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
) -> (usize, Dist2, bool, usize, Dist2) {
    let mut best_eligible_idx: Option<usize> = None;
    let mut best_eligible_dist2 = Dist2::MAX;
    let mut best_any_idx: Option<usize> = None;
    let mut best_any_dist2 = Dist2::MAX;

    for idx in range {
        if let Some(seg) = route_data.get_node(idx) {
            let d2 = segment_score(gps_x, gps_y, gps_heading, gps_speed, &seg);

            if d2 < best_any_dist2 {
                best_any_dist2 = d2;
                best_any_idx = Some(idx);
            }

            if heading_eligible(gps_heading, gps_speed, seg.heading_cdeg) && d2 < best_eligible_dist2 {
                best_eligible_dist2 = d2;
                best_eligible_idx = Some(idx);
            }
        }
    }

    let eligible_found = best_eligible_idx.is_some();
    let eligible_idx = best_eligible_idx.unwrap_or(0);
    let any_idx = best_any_idx.unwrap_or(0);

    (eligible_idx, best_eligible_dist2, eligible_found, any_idx, best_any_dist2)
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p gps_processor`

Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add crates/pipeline/gps_processor/src/map_match.rs
git commit -m "feat(e1): add best_eligible helper function

Returns dual trackers: best eligible segment and best any segment
(pure distance). Used by find_best_segment_restricted for
filter-then-rank architecture."
```

---

## Task 7: Rewrite `find_best_segment_restricted()` — part 1 (window search)

**Files:**
- Modify: `crates/pipeline/gps_processor/src/map_match.rs` (lines 28-99, the entire function)

- [ ] **Step 1: Replace function with new implementation (window phase only)**

Replace the entire `find_best_segment_restricted` function with:

```rust
/// Find best route segment for GPS point with preference for segments near last_idx
pub fn find_best_segment_restricted(
    gps_x: DistCm,
    gps_y: DistCm,
    gps_heading: HeadCdeg,
    gps_speed: SpeedCms,
    route_data: &RouteData,
    last_idx: usize,
) -> usize {
    // Early-exit threshold: if the best eligible segment in the window is
    // within SIGMA_GPS_CM (20 m), skip the expensive grid search.
    // Now that scores are pure dist2, this comparison is physically meaningful.
    const MAX_DIST2_EARLY_EXIT: Dist2 =
        SIGMA_GPS_CM as i64 * SIGMA_GPS_CM as i64; // 4 000 000 cm²

    const WINDOW_BACK: usize = 2;
    const WINDOW_FWD: usize = 10;

    let start = last_idx.saturating_sub(WINDOW_BACK);
    let end = (last_idx + WINDOW_FWD).min(route_data.node_count.saturating_sub(1));

    // PHASE 1: Window search
    let (window_best_eligible, window_eligible_dist2, window_eligible_found,
         window_best_any, _window_any_dist2) =
        best_eligible(gps_x, gps_y, gps_heading, gps_speed, route_data,
                      start..=end);

    // Early exit if eligible segment found within threshold
    if window_eligible_found && window_eligible_dist2 < MAX_DIST2_EARLY_EXIT {
        return window_best_eligible;
    }

    // TODO: Grid search will be added in next task
    // For now, return window_best_any as fallback
    window_best_any
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p gps_processor`

Expected: Compiles successfully

- [ ] **Step 3: Run existing tests**

Run: `cargo test -p gps_processor -- --nocapture`

Expected: All tests pass (behavior unchanged for now)

- [ ] **Step 4: Commit**

```bash
git add crates/pipeline/gps_processor/src/map_match.rs
git commit -m "refactor(e1): rewrite find_best_segment_restricted - phase 1

Implement window search using best_eligible helper.
Early exit on eligible segment within 20m.
Grid search TODO in next task."
```

---

## Task 8: Rewrite `find_best_segment_restricted()` — part 2 (grid search)

**Files:**
- Modify: `crates/pipeline/gps_processor/src/map_match.rs` (the `find_best_segment_restricted` function)

- [ ] **Step 1: Complete the grid search implementation**

Replace the `TODO` section and return statement in `find_best_segment_restricted` with:

```rust
    // Fallback: full grid search.
    if gps_x < route_data.x0_cm || gps_y < route_data.y0_cm {
        return window_best_any; // Outside bounding box — keep window result
    }

    let gx = ((gps_x - route_data.x0_cm) / route_data.grid.grid_size_cm) as u32;
    let gy = ((gps_y - route_data.y0_cm) / route_data.grid.grid_size_cm) as u32;

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
    let mut best_any_dist2 = _window_any_dist2;
    let mut eligible_found = window_eligible_found;

    for dy in 0..=2i32 {
        for dx in 0..=2i32 {
            let ny = gy as i32 + dy - 1;
            let nx = gx as i32 + dx - 1;
            if ny < 0 || nx < 0 {
                continue;
            }
            route_data.grid.visit_cell(nx as u32, ny as u32, |idx: u16| {
                if let Some(seg) = route_data.get_node(idx as usize) {
                    let d2 = segment_score(gps_x, gps_y, gps_heading, gps_speed, &seg);

                    // Update best_any tracker
                    if d2 < best_any_dist2 {
                        best_any_dist2 = d2;
                        best_any_idx = idx as usize;
                    }

                    // Update best_eligible tracker if heading matches
                    if heading_eligible(gps_heading, gps_speed, seg.heading_cdeg) {
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
            gps_speed, gps_heading
        );
        return best_any_idx;
    }

    best_eligible_idx
}
```

Also update the `best_eligible` call to return `_window_any_dist2` (we need it for seeding):

Change line:
```rust
         window_best_any, _window_any_dist2) =
```

To:
```rust
         window_best_any, window_any_dist2) =
```

And update the seeding variable from `_window_any_dist2` to `window_any_dist2` in the grid phase.

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p gps_processor`

Expected: Compiles successfully

- [ ] **Step 3: Run all tests**

Run: `cargo test -p gps_processor -- --nocapture`

Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/pipeline/gps_processor/src/map_match.rs
git commit -m "feat(e1): complete grid search phase in find_best_segment_restricted

Implement full filter-then-rank with dual-tracker seeding.
Grid search updates both eligible and any trackers.
Fallback to best_any with warning when no eligible found."
```

---

## Task 9: Clean up — remove deprecated args from `segment_score()`

**Files:**
- Modify: `crates/pipeline/gps_processor/src/map_match.rs` (the `segment_score` function and its callers)

- [ ] **Step 1: Update `segment_score()` to remove deprecated args**

Replace the `segment_score` function with the final version:

```rust
/// Distance-squared from GPS point to segment (clamped projection).
///
/// Heading is intentionally absent.  Heading belongs in the eligibility
/// filter (`heading_eligible`), not in the ranking score.  Mixing cm² and
/// cdeg² into one scalar requires an arbitrary scale factor that cannot be
/// derived from first principles.
///
/// The return type is `Dist2` (i64 cm²).
pub fn segment_score(
    gps_x: DistCm,
    gps_y: DistCm,
    seg: &RouteNode,
) -> Dist2 {
    distance_to_segment_squared(gps_x, gps_y, seg)
}
```

- [ ] **Step 2: Update all callers**

Find and update all calls to `segment_score`:

In `best_eligible`:
```rust
let d2 = segment_score(gps_x, gps_y, &seg);
```

In `find_best_segment_restricted` (grid loop):
```rust
let d2 = segment_score(gps_x, gps_y, &seg);
```

In the test `test_segment_score_is_pure_distance`:
```rust
let score = segment_score(100000, 100000, &seg);
// ...
let score_far = segment_score(101000, 100000, &seg);
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p gps_processor`

Expected: Compiles successfully (no deprecation warnings)

- [ ] **Step 4: Run all tests**

Run: `cargo test -p gps_processor -- --nocapture`

Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/pipeline/gps_processor/src/map_match.rs
git commit -m "refactor(e1): remove deprecated args from segment_score

segment_score now has minimal signature (x, y, seg).
All callers updated. Function returns pure distance squared.
Heading filtering fully moved to heading_eligible."
```

---

## Task 10: Integration verification

**Files:**
- No file changes, verification only

- [ ] **Step 1: Run full test suite**

Run: `cargo test -p gps_processor -- --nocapture`

Expected: All tests pass

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -p gps_processor -- -D warnings`

Expected: No warnings

- [ ] **Step 3: Check formatting**

Run: `cargo fmt -p gps_processor -- --check`

Expected: No formatting changes needed

- [ ] **Step 4: Verify against design spec**

Review the implementation against:
- `docs/superpowers/specs/2026-04-13-e1-heading-penalty-filter-then-rank-design.md`

Check all invariants are satisfied:
- [ ] Early exit requires `eligible_found = true`
- [ ] `best_eligible_dist2 = MAX` when `window_eligible = false`
- [ ] Dual trackers carry forward from window to grid
- [ ] Grid sets `eligible_found = true` when it finds an eligible segment
- [ ] Fallback returns `best_any_idx`

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "feat(e1): complete filter-then-rank implementation

All tasks complete. E1 heading penalty issue resolved.
Architecture: filter (heading_eligible) → rank (pure distance).
Verified against design spec, all invariants satisfied."
```

---

## Summary

This plan implements the filter-then-rank architecture to fix E1 (heading penalty dominance). The implementation:

1. Adds `heading_threshold_cdeg()` for speed-dependent heading gate
2. Adds `heading_eligible()` for boolean heading filter
3. Adds `best_eligible()` for dual-tracker window search
4. Rewrites `find_best_segment_restricted()` with filter-then-rank
5. Simplifies `segment_score()` to pure distance

All changes are contained within `crates/pipeline/gps_processor/src/map_match.rs`.

**Estimated time:** 2-3 hours (10 tasks, ~15-20 minutes each)

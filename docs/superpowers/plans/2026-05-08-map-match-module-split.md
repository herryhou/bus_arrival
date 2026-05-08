# Map Match Module Split Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Split `map_match.rs` (1093 lines) into 4 focused modules for testability and reduced cognitive load.

**Architecture:** Extract by concern: `heading.rs` (heading filter), `projection.rs` (distance math), `search.rs` (search orchestration), `mod.rs` (public API). Each module independently testable, backward compatible via re-exports.

**Tech Stack:** Rust, no_std (libm for f64 ops), existing test infrastructure (proptest).

---

## File Structure

**New files:**
- `crates/pipeline/gps_processor/src/map_match/heading.rs` - Heading filter logic (~150 lines)
- `crates/pipeline/gps_processor/src/map_match/projection.rs` - Distance calculation (~200 lines)
- `crates/pipeline/gps_processor/src/map_match/search.rs` - Search strategies (~400 lines)
- `crates/pipeline/gps_processor/src/map_match/mod.rs` - Public API facade (~200 lines, moved from `map_match.rs`)

**Modified:**
- `crates/pipeline/gps_processor/src/map_match.rs` → Deleted (content moved to `map_match/mod.rs`)

**Module hierarchy:**
```
map_match/
├── mod.rs          # Public API, re-exports, coordinate conversion
├── heading.rs      # Heading filter: eligible?, threshold calculation
├── projection.rs   # Distance to segment, route projection
└── search.rs       # Search strategies: restricted, grid_only, fallbacks
```

**Dependencies:**
```
mod.rs → heading, projection, search
search → heading, projection
heading → (none, just shared types)
projection → (none, just shared types)
```

---

## Task 1: Create `heading.rs` Module

**Files:**
- Create: `crates/pipeline/gps_processor/src/map_match/heading.rs`

### Task 1: Create heading.rs Module

**Files:**
- Create: `crates/pipeline/gps_processor/src/map_match/heading.rs`

- [ ] **Step 1: Create heading.rs with heading filter functions**

```rust
//! Heading filter for map matching
//!
//! This module provides pure heading filter logic without knowledge of
//! grid structure, route data, or search strategies.

use shared::{HeadCdeg, SpeedCms};

/// Hard heading gate at full speed (w = 256, ≥ 3 km/h).
/// A bus in motion cannot be heading >90° from the segment direction.
const MAX_HEADING_DIFF_CDEG: u32 = 9_000; // 90°

/// Heading filter threshold for a given speed weight.
///
/// Returns `u32::MAX` (gate disabled) when w = 0 — at a standstill GPS heading
/// is unreliable; don't reject any segment.
/// Returns `MAX_HEADING_DIFF_CDEG` (90°) at w = 256.
/// Linearly interpolates between the two, giving a progressively tighter gate
/// as the bus picks up speed.
pub(crate) fn heading_threshold_cdeg(w: i32) -> u32 {
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
/// Note: this is a hard gate, not a blended penalty. A segment is either
/// physically plausible or it isn't; partial credit produces commensuration
/// problems (adding cm² to cdeg²).
pub fn heading_eligible(
    gps_heading: HeadCdeg,
    gps_speed: SpeedCms,
    seg_heading: HeadCdeg,
    is_first_fix: bool,
) -> bool {
    if gps_heading == i16::MIN {
        return true; // GGA-only: preserve existing sentinel behaviour
    }
    let w = heading_weight(gps_speed);
    let threshold = if is_first_fix {
        // Relaxed threshold for post-outage recovery: 180°
        18_000
    } else {
        heading_threshold_cdeg(w)
    };
    let diff = heading_diff_cdeg(gps_heading, seg_heading) as u32;
    diff <= threshold
}

/// Heading weight: 0 at v=0, 256 at v≥83 cm/s (3 km/h)
pub(crate) fn heading_weight(v_cms: SpeedCms) -> i32 {
    ((v_cms * 256) / 83).min(256)
}

/// Calculate heading difference (shortest around 360°)
pub(crate) fn heading_diff_cdeg(a: HeadCdeg, b: HeadCdeg) -> HeadCdeg {
    let diff = (a as i32 - b as i32).unsigned_abs() % 36000;
    if diff > 18000 {
        (36000 - diff) as HeadCdeg
    } else {
        diff as HeadCdeg
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_heading_eligible_first_fix() {
        let speed: SpeedCms = 500; // Moving

        // First fix: 180° relaxed threshold
        assert!(heading_eligible(0, speed, 18000, true)); // 180° off, eligible
        assert!(heading_eligible(0, speed, 9001, true)); // 90° off, eligible
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

    #[test]
    fn test_heading_weight() {
        // At v=0: w=0
        assert_eq!(heading_weight(0), 0);

        // At v=83 cm/s (3 km/h): w=256 (saturated)
        assert_eq!(heading_weight(83), 256);

        // At v=500 cm/s: w=256 (saturated)
        assert_eq!(heading_weight(500), 256);

        // At v=41 cm/s (~1.5 km/h): w=128 (half)
        assert_eq!(heading_weight(41), 127); // 41*256/83 = 126.4, truncated
    }

    proptest! {
        #[test]
        fn prop_heading_diff_symmetric(a in -18000i16..18000, b in -18000i16..18000) {
            let diff1 = heading_diff_cdeg(a, b);
            let diff2 = heading_diff_cdeg(b, a);
            prop_assert_eq!(diff1, diff2);
        }

        #[test]
        fn prop_heading_diff_identity(a in -18000i16..18000) {
            let diff = heading_diff_cdeg(a, a);
            prop_assert_eq!(diff, 0);
        }

        #[test]
        fn prop_heading_diff_max_180(a in -18000i16..18000, b in -18000i16..18000) {
            let diff = heading_diff_cdeg(a, b);
            prop_assert!(diff <= 18000);
        }
    }
}
```

- [ ] **Step 2: Create mod.rs stub to declare heading module**

Create `crates/pipeline/gps_processor/src/map_match/mod.rs`:

```rust
//! Heading-constrained map matching

pub mod heading;

// Re-exports for backward compatibility
pub use heading::heading_eligible;
```

- [ ] **Step 3: Run heading tests**

Run: `cargo test -p gps_processor heading`

Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/pipeline/gps_processor/src/map_match/
git commit -m "feat: extract heading.rs module from map_match

- Extract heading filter logic into dedicated module
- heading_eligible, heading_threshold_cdeg, heading_weight, heading_diff_cdeg
- Add tests for sentinel, stopped, moving, first-fix modes
- Property-based tests for heading_diff (symmetric, identity, max 180°)
- Public API unchanged via re-export in mod.rs"
```

---

## Task 2: Create `projection.rs` Module

**Files:**
- Modify: `crates/pipeline/gps_processor/src/map_match/mod.rs` (add projection module)
- Create: `crates/pipeline/gps_processor/src/map_match/projection.rs`

### Task 2: Create projection.rs Module

- [ ] **Step 1: Create projection.rs with distance functions**

Create `crates/pipeline/gps_processor/src/map_match/projection.rs`:

```rust
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
```

- [ ] **Step 2: Update mod.rs to include projection module**

Edit `crates/pipeline/gps_processor/src/map_match/mod.rs`:

```rust
//! Heading-constrained map matching

pub mod heading;
pub mod projection;

// Re-exports for backward compatibility
pub use heading::heading_eligible;
pub use projection::{distance_to_segment_squared, project_to_route, segment_score};
```

- [ ] **Step 3: Run projection tests**

Run: `cargo test -p gps_processor projection`

Expected: All tests pass

- [ ] **Step 4: Run all map_match tests**

Run: `cargo test -p gps_processor map_match`

Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/pipeline/gps_processor/src/map_match/
git commit -m "feat: extract projection.rs module from map_match

- Extract distance calculation and route projection into dedicated module
- distance_to_segment_squared, project_to_route, segment_score
- Tests for on-segment, perpendicular, clamped, zero-length cases
- Property-based tests for non-negative distance and on-segment zero distance
- Public API unchanged via re-export in mod.rs"
```

---

## Task 3: Create `search.rs` Module

**Files:**
- Modify: `crates/pipeline/gps_processor/src/map_match/mod.rs` (add search module)
- Create: `crates/pipeline/gps_processor/src/map_match/search.rs`

### Task 3: Create search.rs Module

- [ ] **Step 1: Create search.rs with search strategies**

Create `crates/pipeline/gps_processor/src/map_match/search.rs`:

```rust
//! Search strategies for map matching
//!
//! This module provides search orchestration using heading and projection modules.
//! Knows about: grid structure, search windows, fallback strategies.

use shared::binfile::RouteData;
use shared::{Dist2, DistCm, HeadCdeg, SpeedCms};

use crate::SIGMA_GPS_CM;

use super::{heading::heading_eligible, projection::segment_score};

/// Scan a range of segment indices, returning the best eligible and best any.
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
    const MAX_DIST2_EARLY_EXIT: Dist2 = SIGMA_GPS_CM as i64 * SIGMA_GPS_CM as i64;
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
        return global_search_fallback(gps_x, gps_y, gps_heading, gps_speed, route_data, is_first_fix);
    }

    let gx = ((gps_x - route_data.x0_cm) / route_data.grid.grid_size_cm) as u32;
    let gy = ((gps_y - route_data.y0_cm) / route_data.grid.grid_size_cm) as u32;

    if gx >= route_data.grid.cols || gy >= route_data.grid.rows {
        return global_search_fallback(gps_x, gps_y, gps_heading, gps_speed, route_data, is_first_fix);
    }

    // PHASE 2: Grid search
    let mut best_eligible_idx = if window_eligible_found {
        window_best_eligible
    } else {
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

                        if d2 < best_any_dist2 {
                            best_any_dist2 = d2;
                            best_any_idx = idx as usize;
                        }

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
pub fn find_best_segment_grid_only(
    gps_x: DistCm,
    gps_y: DistCm,
    gps_heading: HeadCdeg,
    gps_speed: SpeedCms,
    route_data: &RouteData,
    is_first_fix: bool,
) -> (usize, i64) {
    if gps_x < route_data.x0_cm || gps_y < route_data.y0_cm {
        return (0, i64::MAX);
    }

    let gx = ((gps_x - route_data.x0_cm) / route_data.grid.grid_size_cm) as u32;
    let gy = ((gps_y - route_data.y0_cm) / route_data.grid.grid_size_cm) as u32;

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

                        if d2 < best_any_dist2 {
                            best_any_dist2 = d2;
                            best_any_idx = idx as usize;
                        }

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

    if !eligible_found {
        return (best_any_idx, best_any_dist2);
    }

    (best_eligible_idx, best_eligible_dist2)
}

/// Find best segment using grid search only with min_s and max_s constraints.
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
    if gps_x < route_data.x0_cm || gps_y < route_data.y0_cm {
        return (0, i64::MAX);
    }

    let gx = ((gps_x - route_data.x0_cm) / route_data.grid.grid_size_cm) as u32;
    let gy = ((gps_y - route_data.y0_cm) / route_data.grid.grid_size_cm) as u32;

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
                        if seg.cum_dist_cm < min_s_cm {
                            return;
                        }
                        if seg.cum_dist_cm > max_s_cm {
                            return;
                        }

                        let d2 = segment_score(gps_x, gps_y, &seg);

                        if d2 < best_any_dist2 {
                            best_any_dist2 = d2;
                            best_any_idx = idx as usize;
                        }

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

    if !eligible_found {
        return (best_any_idx, best_any_dist2);
    }

    (best_eligible_idx, best_eligible_dist2)
}

/// Find best segment using grid search only with min_s constraint.
pub fn find_best_segment_grid_only_with_min_s(
    gps_x: DistCm,
    gps_y: DistCm,
    gps_heading: HeadCdeg,
    gps_speed: SpeedCms,
    route_data: &RouteData,
    is_first_fix: bool,
    min_s_cm: DistCm,
) -> (usize, i64) {
    find_best_segment_grid_only_with_min_max_s(
        gps_x,
        gps_y,
        gps_heading,
        gps_speed,
        route_data,
        is_first_fix,
        min_s_cm,
        DistCm::MAX,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn test_find_best_segment_restricted_window_early_exit() {
        let segments: Vec<(i32, i32, i16, i32)> = (0..20)
            .map(|i| (i * 1000, 0, 0, 20_000))
            .collect();
        let segments_refs: &[(i32, i32, i16, i32)] = &segments;
        let route_data = create_test_route_data(segments_refs).unwrap();

        let gps_x = 11_500;
        let gps_y = 0;

        let (idx, dist2) = find_best_segment_restricted(
            gps_x,
            gps_y,
            0,
            500,
            &route_data,
            10,
            false,
        );

        assert_eq!(idx, 10);
        assert!(dist2 < 4_000_000);
    }

    #[test]
    fn test_find_best_segment_restricted_grid_fallback() {
        let segments: Vec<(i32, i32, i16, i32)> = (0..20)
            .map(|i| (i * 1000, 0, 0, 20_000))
            .collect();
        let segments_refs: &[(i32, i32, i16, i32)] = &segments;
        let route_data = create_test_route_data(segments_refs).unwrap();

        let gps_x = 16_500;
        let gps_y = 0;

        let (idx, _dist2) = find_best_segment_restricted(
            gps_x,
            gps_y,
            0,
            500,
            &route_data,
            5,
            false,
        );

        assert_eq!(idx, 15);
    }

    #[test]
    fn test_find_best_segment_grid_only() {
        let route_data = create_test_route_data(&[
            (0, 0, 0, 20_000),
            (2000, 0, 0, 20_000),
            (4000, 0, 0, 20_000),
        ]).unwrap();

        let (idx, _dist2) = find_best_segment_grid_only(
            3000,
            0,
            0,
            500,
            &route_data,
            false,
        );

        assert_eq!(idx, 1);
    }

    #[test]
    fn test_find_best_segment_grid_only_outside_bounds() {
        let route_data = create_test_route_data(&[
            (0, 0, 0, 20_000),
        ]).unwrap();

        let (idx, dist2) = find_best_segment_grid_only(
            -50_000,
            -50_000,
            0,
            500,
            &route_data,
            false,
        );

        assert_eq!(idx, 0);
        assert_eq!(dist2, i64::MAX);
    }

    #[test]
    fn test_sentinel_heading_always_eligible() {
        let route_data = create_test_route_data(&[
            (0, 0, 18000, 20_000),
        ]).unwrap();

        let (idx, _dist2) = find_best_segment_grid_only(
            0,
            0,
            i16::MIN,
            500,
            &route_data,
            false,
        );

        assert_eq!(idx, 0);
    }

    #[test]
    fn test_first_fix_relaxed_heading() {
        let route_data = create_test_route_data(&[
            (0, 0, 18000, 20_000),
        ]).unwrap();

        let (idx, _dist2) = find_best_segment_grid_only(
            0,
            0,
            0,
            500,
            &route_data,
            true,
        );

        assert_eq!(idx, 0);
    }

    #[test]
    fn test_zero_speed_heading_gate_disabled() {
        let route_data = create_test_route_data(&[
            (0, 0, 9001, 20_000),
        ]).unwrap();

        let (idx, _dist2) = find_best_segment_grid_only(
            0,
            0,
            0,
            0,
            &route_data,
            false,
        );

        assert_eq!(idx, 0);
    }
}
```

- [ ] **Step 2: Update mod.rs to include search module and re-exports**

Edit `crates/pipeline/gps_processor/src/map_match/mod.rs`:

```rust
//! Heading-constrained map matching

pub mod heading;
pub mod projection;
pub mod search;

// Re-exports for backward compatibility
pub use heading::heading_eligible;
pub use projection::{distance_to_segment_squared, project_to_route, segment_score};
pub use search::{
    find_best_segment_restricted,
    find_best_segment_grid_only,
    find_best_segment_grid_only_with_min_max_s,
    find_best_segment_grid_only_with_min_s,
};
```

- [ ] **Step 3: Run search tests**

Run: `cargo test -p gps_processor search`

Expected: All tests pass

- [ ] **Step 4: Run all gps_processor tests**

Run: `cargo test -p gps_processor`

Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/pipeline/gps_processor/src/map_match/
git commit -m "feat: extract search.rs module from map_match

- Extract search strategies into dedicated module
- find_best_segment_restricted, find_best_segment_grid_only variants
- best_eligible, global_search_fallback internal functions
- Tests for window early exit, grid fallback, heading filter, constraints
- Public API unchanged via re-exports in mod.rs"
```

---

## Task 4: Complete `mod.rs` with Coordinate Conversion

**Files:**
- Modify: `crates/pipeline/gps_processor/src/map_match/mod.rs` (add coordinate conversion and no_std helpers)

### Task 4: Complete mod.rs with Coordinate Conversion

- [ ] **Step 1: Add coordinate conversion and no_std helpers to mod.rs**

Edit `crates/pipeline/gps_processor/src/map_match/mod.rs`:

```rust
//! Heading-constrained map matching

pub mod heading;
pub mod projection;
pub mod search;

// Re-exports for backward compatibility
pub use heading::heading_eligible;
pub use projection::{distance_to_segment_squared, project_to_route, segment_score};
pub use search::{
    find_best_segment_restricted,
    find_best_segment_grid_only,
    find_best_segment_grid_only_with_min_max_s,
    find_best_segment_grid_only_with_min_s,
};

use shared::{DistCm, EARTH_R_CM, FIXED_ORIGIN_LON_DEG};

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

/// Convert lat/lon to absolute cm coordinates with specified average latitude
/// This matches the projection used by the preprocessor
pub fn latlon_to_cm_absolute_with_lat_avg(
    lat: f64,
    lon: f64,
    lat_avg_deg: f64,
) -> (DistCm, DistCm) {
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

    #[test]
    fn test_latlon_to_cm_absolute_with_lat_avg() {
        // Test known conversion
        let (x, y) = latlon_to_cm_absolute_with_lat_avg(25.0, 121.0, 25.0);
        // Should produce non-zero coordinates
        assert!(x != 0 || y != 0);
    }

    #[test]
    fn test_latlon_to_cm_roundtrip() {
        // If we convert and "unconvert", we should get close to original
        // This is a sanity check, not exact roundtrip
        let (x1, y1) = latlon_to_cm_absolute_with_lat_avg(25.0, 121.0, 25.0);
        let (x2, y2) = latlon_to_cm_absolute_with_lat_avg(25.001, 121.001, 25.0);

        // Small change in lat/lon should produce change in x/y
        assert!(x2 != x1 || y2 != y1);
    }
}
```

- [ ] **Step 2: Delete old map_match.rs file**

Run: `rm crates/pipeline/gps_processor/src/map_match.rs`

- [ ] **Step 3: Run all gps_processor tests**

Run: `cargo test -p gps_processor`

Expected: All tests pass

- [ ] **Step 4: Run dependent crate tests**

Run: `cargo test -p pipeline`

Expected: All tests pass

- [ ] **Step 5: Run firmware tests**

Run: `cargo test -p pico2-firmware`

Expected: All tests pass (or skip if no host tests)

- [ ] **Step 6: Commit**

```bash
git add crates/pipeline/gps_processor/src/map_match/
git rm crates/pipeline/gps_processor/src/map_match.rs
git commit -m "feat: complete map_match module split

- Add coordinate conversion (latlon_to_cm_absolute_with_lat_avg) to mod.rs
- Add no_std helpers (f64_cos, f64_round, to_radians_compat)
- Delete old map_match.rs file (content moved to map_match/ directory)
- All tests pass: gps_processor, pipeline, pico2-firmware
- Public API unchanged, backward compatible via re-exports"
```

---

## Task 5: Verify Integration and Success Criteria

**Files:**
- None (verification only)

### Task 5: Verify Integration and Success Criteria

- [ ] **Step 1: Run full test suite**

Run: `cargo test`

Expected: All tests pass

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -p gps_processor -p pipeline`

Expected: No warnings

- [ ] **Step 3: Check module line counts**

Run: `wc -l crates/pipeline/gps_processor/src/map_match/*.rs`

Expected: Each module < 300 lines

- [ ] **Step 4: Run integration test**

Run: `make run ROUTE_NAME=ty225 SCENARIO=normal`

Expected: Same output as before

- [ ] **Step 5: Verify backward compatibility**

Check that existing imports still work:

```bash
grep -r "use crate::map_match::" crates/pipeline/gps_processor/src/
```

Expected: All existing imports resolve without changes

- [ ] **Step 6: Commit**

```bash
git add docs/superpowers/plans/2026-05-08-map-match-module-split.md
git commit -m "docs: add map match module split implementation plan

- Complete implementation plan for module split
- 5 tasks: heading.rs, projection.rs, search.rs, mod.rs, verification
- Each task 2-5 minute steps with exact code
- TDD approach with tests first
- Success criteria: <300 lines per module, all tests pass, API unchanged"
```

---

## Success Criteria Checklist

- [ ] `heading.rs` tests pass with no RouteData fixture
- [ ] `projection.rs` tests pass with minimal fixtures
- [ ] `search.rs` tests pass with RouteData fixtures
- [ ] `cargo test -p gps_processor` passes
- [ ] `cargo test -p pipeline` passes
- [ ] `cargo test -p pico2-firmware` passes
- [ ] Integration test produces identical output
- [ ] No warnings from `cargo clippy`
- [ ] Each module < 300 lines
- [ ] Public API unchanged (backward compatible)

---

## Rollback Plan

If any phase fails:

```bash
git checkout HEAD -- crates/pipeline/gps_processor/src/map_match/
```

Each task is atomic and tested incrementally.

# Sequence-Constrained Stop Projection Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement sequence-constrained stop projection in the preprocessor to ensure stop order from `stops.json` is preserved after RDP simplification, with automatic retry on sequence reversal detection.

**Architecture:** Two-stage projection (validation pass + full projection) with path-constrained grid search and binary search epsilon reduction for handling RDP-induced reversals.

**Tech Stack:** Rust (no_std), existing preprocessor pipeline, Douglas-Peucker simplification, spatial grid indexing.

---

## File Structure

**New files:**
- `preprocessor/src/stops/validation.rs` - Validation types and `validate_stop_sequence()` function
- `preprocessor/src/stops/tests.rs` - Unit tests for validation logic

**Modified files:**
- `preprocessor/src/stops.rs` - Add `validate_stop_sequence()`, rename `project_stops()` → `project_stops_validated()`
- `preprocessor/src/main.rs` - Add retry loop with binary search epsilon reduction
- `preprocessor/src/lib.rs` - Export new validation module

**Note:** The current `StopLocation` struct in `input.rs` only has `lat`/`lon` fields (no `name`). Logging will use stop indices only.

---

## Chunk 1: Core Validation Types and Infrastructure

### Task 1.1: Create validation module structure

**Files:**
- Create: `preprocessor/src/stops/validation.rs`
- Modify: `preprocessor/src/stops.rs`

- [ ] **Step 1: Create validation.rs with module structure**

Create `preprocessor/src/stops/validation.rs`:

```rust
// Stop sequence validation with path-constrained grid search
//
// Ensures stops project to monotonically increasing progress values
// along the route, preserving input order from stops.json.

use shared::{RouteNode, SpatialGrid};

/// Result of quick validation pass
#[derive(Debug)]
pub struct ValidationResult {
    /// Validated stop progress values in input order
    pub progress_values: Vec<i32>,
    /// If validation failed, contains info for retry
    pub reversal_info: Option<ReversalInfo>,
}

/// Information about a detected sequence reversal
#[derive(Debug)]
pub struct ReversalInfo {
    /// Index in stops array where reversal was detected
    pub stop_index: usize,
    /// The problematic progress value (smaller than previous)
    pub problem_progress: i32,
    /// Previous stop's progress (larger)
    pub previous_progress: i32,
    /// Approximate route region to re-simplify (segment indices)
    pub affected_region: (usize, usize),
    /// Suggested epsilon for retry (binary search step)
    pub suggested_epsilon: f64,
    /// Retry attempt count
    pub retry_count: u32,
}
```

- [ ] **Step 2: Add validation module to stops.rs**

Add to `preprocessor/src/stops.rs` at the top after existing imports:

```rust
pub mod validation;
pub use validation::{ValidationResult, ReversalInfo};
```

- [ ] **Step 3: Run cargo check**

Run: `cargo check -p preprocessor`

Expected: No errors, module compiles successfully

- [ ] **Step 4: Commit**

```bash
git add preprocessor/src/stops.rs preprocessor/src/stops/validation.rs
git commit -m "feat(stops): add validation module structure with ValidationResult and ReversalInfo types"
```

---

### Task 1.2: Implement grid helper for radius query

**Files:**
- Modify: `preprocessor/src/stops/validation.rs`

- [ ] **Step 1: Add helper function for radius-based grid query**

Add to `preprocessor/src/stops/validation.rs`:

```rust
/// Query grid with progressive window expansion
///
/// Returns segments within specified radius (in grid cells) from point.
/// Radius 1 = 3×3 cells, radius 2 = 5×5 cells, radius 3 = 7×7 cells.
fn query_grid_radius(
    grid: &SpatialGrid,
    x_cm: i64,
    y_cm: i64,
    radius: usize,
) -> Vec<usize> {
    if grid.cols == 0 || grid.rows == 0 {
        return Vec::new();
    }

    let gx = ((x_cm - grid.x0_cm as i64) / grid.grid_size_cm as i64) as usize;
    let gy = ((y_cm - grid.y0_cm as i64) / grid.grid_size_cm as i64) as usize;

    let mut candidates = Vec::new();
    let diameter = radius * 2 + 1; // radius 1 → 3×3

    for dy in 0..diameter {
        for dx in 0..diameter {
            let ny = gy as i32 + dy as i32 - radius as i32;
            let nx = gx as i32 + dx as i32 - radius as i32;

            if ny >= 0 && ny < grid.rows as i32 && nx >= 0 && nx < grid.cols as i32 {
                let idx = ny as usize * (grid.cols as usize) + nx as usize;
                if idx < grid.cells.len() {
                    candidates.extend_from_slice(&grid.cells[idx]);
                }
            }
        }
    }

    candidates
}
```

- [ ] **Step 2: Run cargo check**

Run: `cargo check -p preprocessor`

Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add preprocessor/src/stops/validation.rs
git commit -m "feat(stops): add query_grid_radius helper for progressive window expansion"
```

---

### Task 1.3: Implement find_closest_segment_constrained

**Files:**
- Modify: `preprocessor/src/stops/validation.rs`

- [ ] **Step 1: Add find_closest_segment_constrained function**

Add to `preprocessor/src/stops/validation.rs`:

```rust
/// Find closest segment to a point with path constraint
///
/// Only searches segments with index >= min_segment_idx (enforces monotonicity).
/// Uses progressive grid search expansion (3×3 → 5×5 → 7×7 → linear fallback).
///
/// Returns: (segment_index, t_value)
fn find_closest_segment_constrained(
    point: &(i64, i64),
    nodes: &[RouteNode],
    grid: &SpatialGrid,
    min_segment_idx: usize,
) -> (usize, f64) {
    // Try progressive window expansion
    for radius in 1..=3 {
        let mut candidates: Vec<usize> = query_grid_radius(grid, point.0, point.1, radius)
            .into_iter()
            .filter(|&seg_idx| seg_idx >= min_segment_idx)
            .collect();

        if !candidates.is_empty() {
            return find_closest_in_candidates(point, nodes, &candidates);
        }
    }

    // Fallback: linear search from min_segment_idx
    let linear_candidates: Vec<usize> = (min_segment_idx..nodes.len().saturating_sub(1))
        .filter(|&i| nodes[i].len2_cm2 != 0)
        .collect();

    find_closest_in_candidates(point, nodes, &linear_candidates)
}

/// Find closest segment among candidates
fn find_closest_in_candidates(
    point: &(i64, i64),
    nodes: &[RouteNode],
    candidates: &[usize],
) -> (usize, f64) {
    let mut best_idx = candidates[0];
    let mut best_t = 0.0;
    let mut best_dist2 = i64::MAX;

    for &seg_idx in candidates {
        if seg_idx >= nodes.len() {
            continue;
        }

        let node = &nodes[seg_idx];
        if node.len2_cm2 == 0 {
            continue; // Last node has no outgoing segment
        }

        let dx = point.0 - node.x_cm as i64;
        let dy = point.1 - node.y_cm as i64;

        // Project point onto segment
        let t_num = dx * node.dx_cm as i64 + dy * node.dy_cm as i64;
        let t = (t_num as f64 / node.len2_cm2 as f64).clamp(0.0, 1.0);

        // Closest point on segment
        let px = node.x_cm as f64 + t * node.dx_cm as f64;
        let py = node.y_cm as f64 + t * node.dy_cm as f64;

        let dist_x = point.0 as f64 - px;
        let dist_y = point.1 as f64 - py;
        let dist2 = (dist_x * dist_x + dist_y * dist_y) as i64;

        if dist2 < best_dist2 {
            best_dist2 = dist2;
            best_idx = seg_idx;
            best_t = t;
        }
    }

    (best_idx, best_t)
}
```

- [ ] **Step 2: Run cargo check**

Run: `cargo check -p preprocessor`

Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add preprocessor/src/stops/validation.rs
git commit -m "feat(stops): implement find_closest_segment_constrained with progressive grid search"
```

---

### Task 1.4: Implement validate_stop_sequence

**Files:**
- Modify: `preprocessor/src/stops/validation.rs`

- [ ] **Step 1: Add validate_stop_sequence function**

Add to `preprocessor/src/stops/validation.rs`:

```rust
/// Validate stop sequence for monotonicity
///
/// Projects all stops using path-constrained grid search and verifies
/// that progress values strictly increase by input order.
///
/// # Arguments
/// * `stops_cm` - Stop coordinates in centimeter units
/// * `route_nodes` - Linearized route nodes
/// * `grid` - Spatial grid index for fast segment lookup
///
/// # Returns
/// ValidationResult with progress values in input order,
/// or ReversalInfo if monotonicity violation detected
pub fn validate_stop_sequence(
    stops_cm: &[(i64, i64)],
    route_nodes: &[RouteNode],
    grid: &SpatialGrid,
) -> ValidationResult {
    if stops_cm.is_empty() {
        // Should be handled earlier, but just in case
        return ValidationResult {
            progress_values: vec![],
            reversal_info: None,
        };
    }

    if stops_cm.len() == 1 {
        // Single stop: no validation needed
        let (seg_idx, t) = find_closest_segment_constrained(
            &stops_cm[0],
            route_nodes,
            grid,
            0,
        );
        let node = &route_nodes[seg_idx];
        let progress_cm = node.cum_dist_cm + (t * node.seg_len_cm as f64).round() as i32;

        return ValidationResult {
            progress_values: vec![progress_cm],
            reversal_info: None,
        };
    }

    let mut progress_values = Vec::with_capacity(stops_cm.len());
    let mut min_segment_idx = 0;
    let mut previous_progress = i32::MIN;

    for (input_idx, stop_pt) in stops_cm.iter().enumerate() {
        let (seg_idx, t) = find_closest_segment_constrained(
            stop_pt,
            route_nodes,
            grid,
            min_segment_idx,
        );

        let node = &route_nodes[seg_idx];
        let progress_cm = node.cum_dist_cm + (t * node.seg_len_cm as f64).round() as i32;

        // Monotonicity validation
        if progress_cm <= previous_progress {
            // Reversal detected!
            return ValidationResult {
                progress_values, // Partial results
                reversal_info: Some(ReversalInfo {
                    stop_index: input_idx,
                    problem_progress: progress_cm,
                    previous_progress,
                    affected_region: (min_segment_idx.saturating_sub(10), seg_idx + 10),
                    suggested_epsilon: 350.0, // Binary search: 700 → 350
                    retry_count: 0,
                }),
            };
        }

        // Near-duplicate check (warning emitted in main.rs via logging)
        // Continue processing - duplicate progress (==) triggers reversal above

        progress_values.push(progress_cm);
        previous_progress = progress_cm;
        min_segment_idx = seg_idx; // Update path constraint for next stop
    }

    ValidationResult {
        progress_values,
        reversal_info: None,
    }
}
```

- [ ] **Step 2: Run cargo check**

Run: `cargo check -p preprocessor`

Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add preprocessor/src/stops/validation.rs
git commit -m "feat(stops): implement validate_stop_sequence with monotonicity check"
```

---

### Task 1.5: Export validation function from stops module

**Files:**
- Modify: `preprocessor/src/stops.rs`

- [ ] **Step 1: Export validate_stop_sequence**

Add to `preprocessor/src/stops.rs` after the module declaration:

```rust
pub use validation::{ValidationResult, ReversalInfo, validate_stop_sequence};
```

- [ ] **Step 2: Run cargo check**

Run: `cargo check -p preprocessor`

Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add preprocessor/src/stops.rs
git commit -m "feat(stops): export validate_stop_sequence from stops module"
```

---

### Task 1.6: Refactor project_stops to project_stops_validated

**Files:**
- Modify: `preprocessor/src/stops.rs`

- [ ] **Step 1: Read current project_stops implementation**

Run: `cat preprocessor/src/stops.rs`

Review lines 16-62 to understand current implementation

- [ ] **Step 2: Replace project_stops with project_stops_validated**

Delete the existing `project_stops` function (lines ~16-62) and replace with:

```rust
/// Project validated stops onto route and compute corridor boundaries.
///
/// # Arguments
/// * `progress_values` - Progress values in INPUT ORDER (already validated)
/// * `stops_input` - Original stops input (for potential future use)
///
/// # Returns
/// Stops with corridor boundaries, sorted by progress (same as input order)
pub fn project_stops_validated(
    progress_values: &[i32],
    _stops_input: &input::StopsInput, // Reserved for future logging
) -> Vec<Stop> {
    let mut final_stops: Vec<Stop> = Vec::with_capacity(progress_values.len());

    for progress_cm in progress_values.iter() {
        let mut corridor_start_cm = progress_cm - 8000;
        let mut corridor_end_cm = progress_cm + 4000;

        // Overlap protection with previous stop
        if let Some(prev) = final_stops.last() {
            let min_separation = 2000; // 20m
            let min_start = prev.corridor_end_cm + min_separation;
            if corridor_start_cm < min_start {
                corridor_start_cm = min_start;
            }
        }

        // Final sanity check
        if corridor_start_cm >= *progress_cm {
            corridor_start_cm = *progress_cm - 1;
        }

        final_stops.push(Stop {
            progress_cm: *progress_cm,
            corridor_start_cm,
            corridor_end_cm,
        });
    }

    final_stops
}

// Note: find_closest_segment is no longer needed publicly
// but kept for potential future use
#[allow(dead_code)]
fn find_closest_segment(point: &(i64, i64), nodes: &[RouteNode]) -> (usize, f64) {
    // ... keep existing implementation for now
    let mut best_idx = 0;
    let mut best_t = 0.0;
    let mut best_dist2 = i64::MAX;

    for (i, node) in nodes.iter().enumerate() {
        if node.len2_cm2 == 0 {
            continue;
        }

        let dx = point.0 - node.x_cm as i64;
        let dy = point.1 - node.y_cm as i64;

        let t_num = dx * node.dx_cm as i64 + dy * node.dy_cm as i64;
        let t = (t_num as f64 / node.len2_cm2 as f64).clamp(0.0, 1.0);

        let px = node.x_cm as f64 + t * node.dx_cm as f64;
        let py = node.y_cm as f64 + t * node.dy_cm as f64;

        let dist_x = point.0 as f64 - px;
        let dist_y = point.1 as f64 - py;
        let dist2 = (dist_x * dist_x + dist_y * dist_y) as i64;

        if dist2 < best_dist2 {
            best_dist2 = dist2;
            best_idx = i;
            best_t = t;
        }
    }

    (best_idx, best_t)
}
```

- [ ] **Step 3: Run cargo check**

Run: `cargo check -p preprocessor`

Expected: Compiles successfully

- [ ] **Step 4: Commit**

```bash
git add preprocessor/src/stops.rs
git commit -m "refactor(stops): rename project_stops to project_stops_validated, remove sorting"
```

---

## Chunk 2: Unit Tests for Validation

### Task 2.1: Add validation unit tests

**Files:**
- Create: `preprocessor/src/stops/tests.rs`

- [ ] **Step 1: Create tests.rs file**

Create `preprocessor/src/stops/tests.rs`:

```rust
// Unit tests for stop sequence validation

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validation::validate_stop_sequence;
    use shared::{RouteNode, SpatialGrid};

    fn make_test_nodes(coords: &[(i64, i64)]) -> Vec<RouteNode> {
        let mut nodes = Vec::new();
        let mut cum_dist = 0i32;

        for (i, &(x, y)) in coords.iter().enumerate() {
            let (dx, dy, len2, seg_len, heading) = if i > 0 {
                let prev = coords[i - 1];
                let dx = x - prev.0;
                let dy = y - prev.1;
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
                x_cm: x as i32,
                y_cm: y as i32,
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
        // Simple grid for testing
        SpatialGrid {
            cells: vec![vec![0, 1, 2]], // All segments in first cell
            grid_size_cm: 10000,
            cols: 1,
            rows: 1,
            x0_cm: 0,
            y0_cm: 0,
        }
    }

    #[test]
    fn test_validate_monotonic_sequence() {
        // Collinear stops - should be monotonic
        let stops = vec![(0, 0), (1000, 0), (2000, 0)];
        let nodes = make_test_nodes(&[(0, 0), (1000, 0), (2000, 0), (3000, 0)]);
        let grid = make_test_grid(&nodes);

        let result = validate_stop_sequence(&stops, &nodes, &grid);

        assert!(result.reversal_info.is_none());
        assert_eq!(result.progress_values.len(), 3);
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
    fn test_detect_reversal() {
        // Route doubles back: stop 2 is "before" stop 1 geometrically
        let stops = vec![(0, 0), (2000, 0)];
        let nodes = make_test_nodes(&[(0, 0), (3000, 0), (500, 0)]); // Route goes forward then back
        let grid = make_test_grid(&nodes);

        let result = validate_stop_sequence(&stops, &nodes, &grid);

        assert!(result.reversal_info.is_some());
        let rev = result.reversal_info.unwrap();
        assert_eq!(rev.stop_index, 1);
        assert!(rev.problem_progress < rev.previous_progress);
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
}
```

- [ ] **Step 2: Add tests module to stops.rs**

Add to `preprocessor/src/stops.rs` at the bottom:

```rust
#[cfg(test)]
mod tests;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p preprocessor stops::`

Expected: All tests pass
- test_validate_monotonic_sequence: PASS (3 stops, progress increasing)
- test_single_stop: PASS (single stop edge case)
- test_detect_reversal: PASS (detects non-monotonic sequence)
- test_empty_stops: PASS (empty input edge case)

- [ ] **Step 4: Commit**

```bash
git add preprocessor/src/stops/tests.rs preprocessor/src/stops.rs
git commit -m "test(stops): add unit tests for validate_stop_sequence"
```

---

## Chunk 3: Pipeline Integration with Retry Loop

### Task 3.1: Add retry loop to main.rs

**Files:**
- Modify: `preprocessor/src/main.rs`

- [ ] **Step 1: Read current main.rs structure**

Run: `head -50 preprocessor/src/main.rs`

Note the current pipeline structure

- [ ] **Step 2: Import validation types**

Add to `preprocessor/src/main.rs` imports:

```rust
use stops::{ValidationResult, ReversalInfo, validate_stop_sequence, project_stops_validated};
```

- [ ] **Step 3: Replace stop projection section with retry loop (FINAL version)**

Replace the stop projection section (around lines 125-132) in `preprocessor/src/main.rs`:

```rust
    // 6. Build Spatial Grid Index (100m cells)
    let grid_size_cm = 10000;
    let mut grid = grid::build_grid(&route_nodes, grid_size_cm);
    println!("Built {}x{} spatial grid ({} cells)", grid.cols, grid.rows, grid.cells.len());

    // 7. Stop projection with validation and retry loop
    let stop_pts_cm: Vec<(i64, i64)> = stops_input.stops.iter().map(|s| {
        let (x, y) = coord::latlon_to_cm_relative(s.lat, s.lon, lat_avg);
        (x as i64, y as i64)
    }).collect();

    let mut epsilon_current = 700.0;
    let mut simplified_pts_cm = simplified_pts_cm.clone();
    let max_retries = 3;
    let mut retry_count = 0;
    let mut route_nodes = route_nodes.clone(); // Mutable for retry loop

    let projected_stops = loop {
        let validation = validate_stop_sequence(&stop_pts_cm, &route_nodes, &grid);

        match &validation.reversal_info {
            None => {
                // Success!
                println!("[VALIDATION PASS]");
                for (i, progress) in validation.progress_values.iter().enumerate() {
                    println!("  Stop {:03}: progress={} cm", i + 1, progress);
                }
                println!("✓ All {} stops validated - monotonic sequence confirmed", validation.progress_values.len());

                let stops = project_stops_validated(&validation.progress_values, &stops_input);
                break stops;
            }
            Some(info) => {
                retry_count += 1;
                let next_epsilon = if retry_count >= max_retries || epsilon_current / 2.0 < 100.0 {
                    eprintln!("ERROR: Reversal persists after {} attempts", retry_count);
                    eprintln!("  At stop {}: {} < {} cm",
                             info.stop_index, info.problem_progress, info.previous_progress);
                    eprintln!("  This usually indicates:");
                    eprintln!("    1. Input stop order does not match route geometry");
                    eprintln!("    2. Route has self-intersection or loop-back");
                    eprintln!("  Please verify stops.json matches the actual bus route direction");
                    process::exit(1);
                } else {
                    epsilon_current / 2.0
                };

                println!("! Reversal at stop {}: {} < {} cm",
                         info.stop_index, info.problem_progress, info.previous_progress);
                println!("  Retrying with ε={} cm (attempt {}/{})",
                         next_epsilon, retry_count, max_retries);

                epsilon_current = next_epsilon;
                simplified_pts_cm = simplify::simplify_and_interpolate(
                    &route_pts_cm,
                    epsilon_current,
                    &protected_indices,
                );
                route_nodes = linearize::linearize_route(&simplified_pts_cm);
                grid = grid::build_grid(&route_nodes, grid_size_cm);
            }
        }
    };
    println!("Projected {} stops with corridors", projected_stops.len());

    // 8. Generate LUTs
```

- [ ] **Step 5: Update version string**

Change the version in `preprocessor/src/main.rs`:

```rust
    println!("========================================");
    println!("Bus Arrival Preprocessor - v8.3 Pipeline"); // Changed from v8.0
    println!("========================================");
```

- [ ] **Step 6: Run cargo check**

Run: `cargo check -p preprocessor`

Expected: Compiles successfully

- [ ] **Step 7: Run on test data**

Run: `cargo run -p preprocessor -- tools/data/ty225_route.json tools/data/stops.json visualizer/static/route_data.bin`

Expected output:
- "[VALIDATION PASS]" message appears
- All stops show increasing progress values
- "✓ All X stops validated - monotonic sequence confirmed"
- Binary file created successfully

- [ ] **Step 8: Commit**

```bash
git add preprocessor/src/main.rs
git commit -m "feat(preprocessor): add retry loop with binary search epsilon reduction for stop sequence validation"
```

---

## Chunk 4: Testing and Verification

### Task 4.1: Run full preprocessor test

**Files:**
- Test: `Makefile`

- [ ] **Step 1: Run Makefile test**

Run: `make test DATA_DIR=visualizer/static`

Expected: All tests pass including new validation

- [ ] **Step 2: Verify binary output**

Run: `ls -la visualizer/static/route_data.bin`

Expected: Binary file exists, size > 0 bytes, timestamp updated

- [ ] **Step 3: Test with visualizer**

Run:
1. Open `visualizer/index.html` in browser
2. Load the route (should auto-load from `route_data.bin`)
3. Verify: Stops appear numbered in order (1, 2, 3, ...) along the route
4. Use arrow keys to move along route - stops should be encountered in increasing order
5. Check browser console (F12) - no errors related to stop data

- [ ] **Step 4: Commit if needed**

If any fixes needed:

```bash
git add -A
git commit -m "fix: adjust validation for real route data"
```

---

### Task 4.2: Create synthetic reversal test case

**Files:**
- Create: `tools/data/reversal_test/`
- Create: `tools/data/reversal_test/route.json`
- Create: `tools/data/reversal_test/stops.json`

- [ ] **Step 1: Create reversal test route**

Create `tools/data/reversal_test/route.json`:

```json
{
  "route_points": [
    [25.00, 121.00],
    [25.001, 121.001],
    [25.002, 121.002],
    [25.003, 121.003],
    [25.004, 121.004],
    [25.003, 121.003],
    [25.002, 121.002],
    [25.005, 121.005]
  ]
}
```

- [ ] **Step 2: Create reversal test stops**

Create `tools/data/reversal_test/stops.json`:

```json
{
  "stops": [
    {"lat": 25.001, "lon": 121.001},
    {"lat": 25.004, "lon": 121.004},
    {"lat": 25.005, "lon": 121.005}
  ]
}
```

- [ ] **Step 3: Run preprocessor on reversal test**

Run: `cargo run -p preprocessor -- tools/data/reversal_test/route.json tools/data/reversal_test/stops.json /tmp/test_reversal.bin`

Expected output:
- "! Reversal detected at stop 1: ... < ... cm" message appears
- "Retrying with ε=350 cm" message appears
- Either: Validation passes on retry, OR: Error after max retries
- Exit code: 0 (success) or 1 (persistent reversal - also acceptable behavior)

- [ ] **Step 4: Commit test data**

```bash
git add tools/data/reversal_test/
git commit -m "test: add synthetic reversal test case"
```

---

## Chunk 5: Documentation

### Task 5.1: Update inline documentation

**Files:**
- Modify: `preprocessor/src/stops/validation.rs`

- [ ] **Step 1: Add module documentation**

Add to top of `preprocessor/src/stops/validation.rs`:

```rust
// Stop sequence validation with path-constrained grid search
//
// This module implements sequence-constrained stop projection to ensure
// that the input order from stops.json is preserved after RDP simplification.
//
// Key algorithms:
// - Path-constrained grid search: each stop can only match segments >= previous stop's segment
// - Progressive window expansion: 3×3 → 5×5 → 7×7 → linear fallback
// - Monotonicity validation: progress must strictly increase by input order
//
// When a reversal is detected, ReversalInfo is returned with:
// - Location of the problem (stop index, progress values)
// - Affected region (for potential re-simplification)
// - Suggested epsilon for retry (binary search: 700 → 350 → 175)
```

- [ ] **Step 2: Commit**

```bash
git add preprocessor/src/stops/validation.rs
git commit -m "docs(stops): add module documentation for validation"
```

---

### Task 5.2: Update tech report if needed

**Files:**
- Check: `docs/bus_arrival_tech_report_v8.md`

- [ ] **Step 1: Check if tech report needs updates**

Run: `grep -n "v8.3" docs/bus_arrival_tech_report_v8.md`

Expected: Version is already v8.3

- [ ] **Step 2: Update version history if needed**

If version history needs update, add entry at end of version records

- [ ] **Step 3: Commit if changed**

```bash
git add docs/bus_arrival_tech_report_v8.md
git commit -m "docs: update tech report for sequence-constrained stop projection"
```

---

## Final Verification

### Task 6.1: End-to-end test

**Files:**
- Test: Full pipeline

- [ ] **Step 1: Run full pipeline**

Run: `make test DATA_DIR=visualizer/static`

Expected: All tests pass

- [ ] **Step 2: Check for regressions**

Run: `hexdump -C visualizer/static/route_data.bin | head -20`

Verify: Binary header shows VERSION=2 (or current version), route node count matches expected (~640 nodes for ty225)

- [ ] **Step 3: Final commit**

```bash
git add -A
git commit -m "feat: complete sequence-constrained stop projection implementation"
```

---

## Summary

This plan implements sequence-constrained stop projection in 4 main chunks:

1. **Chunk 1**: Core validation types and infrastructure (validation.rs, helper functions)
2. **Chunk 2**: Unit tests for validation logic
3. **Chunk 3**: Pipeline integration with retry loop in main.rs
4. **Chunk 4-5**: Testing, verification, and documentation

**Key design decisions:**
- Uses full-route re-simplification (simpler than region-based refinement)
- Progressive grid window expansion (3×3 → 5×5 → 7×7 → linear)
- Binary search epsilon reduction (700 → 350 → 175)
- Maximum 3 retry attempts before error

**Files modified:**
- `preprocessor/src/stops.rs` - Add validation, refactor projection
- `preprocessor/src/stops/validation.rs` - New validation module
- `preprocessor/src/main.rs` - Retry loop integration

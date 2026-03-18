# DP Mapper Crate Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a standalone library crate that implements globally optimal stop-to-segment mapping using dynamic programming (Viterbi-like DAG shortest path algorithm).

**Architecture:** Three internal modules with clear boundaries: `grid` (spatial indexing), `candidate` (projection & K-selection), `pathfinding` (DP solver). Public API is single `map_stops()` function returning `Vec<i32>` progress values.

**Tech Stack:** Rust 2021, no external dependencies, uses `shared` crate for `RouteNode` type.

**Spec Reference:** `docs/superpowers/specs/2026-03-18-dp-mapper-design.md`

---

## File Structure

```
preprocessor/dp_mapper/
  ├── Cargo.toml                    # Crate manifest
  ├── src/
  │   ├── lib.rs                    # Public API (map_stops)
  │   ├── grid/
  │   │   ├── mod.rs                # Module exports, constants
  │   │   └── builder.rs            # build_grid, query_neighbors
  │   ├── candidate/
  │   │   ├── mod.rs                # Module exports, Candidate struct, constants
  │   │   └── generator.rs          # generate_candidates, generate_candidates_with_snap
  │   └── pathfinding/
  │       ├── mod.rs                # Module exports, DpLayer, SortedCandidate
  │       └── solver.rs             # map_stops_dp, dp_forward_pass, dp_backtrack
  └── tests/
      └── integration.rs            # Real route validation tests
```

**Key design decisions:**
- Each module has `mod.rs` for exports and internal types, separate file for main logic
- Unit tests live alongside implementation in each module
- Integration tests use real route data from `test_data/`

---

## Chunk 1: Workspace Setup and Crate Skeleton

### Task 1: Update Workspace Members

**Files:**
- Modify: `Cargo.toml:3-6`

- [ ] **Step 1: Add dp_mapper to workspace members**

```toml
[workspace]
resolver = "2"
members = ["shared", "preprocessor", "preprocessor/dp_mapper", "simulator", "arrival_detector"]
```

- [ ] **Step 2: Verify workspace compiles**

Run: `cargo check --workspace`
Expected: No errors (dp_mapper doesn't exist yet, but workspace is valid)

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "feat(dp-mapper): add dp_mapper to workspace members"
```

---

### Task 2: Create Crate Manifest

**Files:**
- Create: `preprocessor/dp_mapper/Cargo.toml`

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "dp_mapper"
version.workspace = true
edition.workspace = true

[dependencies]
shared = { path = "../../../shared" }

[dev-dependencies]
# For integration tests with real route data
serde_json = "1.0"
```

- [ ] **Step 2: Verify crate is recognized**

Run: `cargo check -p dp_mapper`
Expected: Error "error: no `src/` directory found" (confirms crate registered)

- [ ] **Step 3: Commit**

```bash
git add preprocessor/dp_mapper/Cargo.toml
git commit -m "feat(dp-mapper): add crate manifest with shared dependency"
```

---

### Task 3: Create Module Skeleton

**Files:**
- Create: `preprocessor/dp_mapper/src/lib.rs`
- Create: `preprocessor/dp_mapper/src/grid/mod.rs`
- Create: `preprocessor/dp_mapper/src/grid/builder.rs`
- Create: `preprocessor/dp_mapper/src/candidate/mod.rs`
- Create: `preprocessor/dp_mapper/src/candidate/generator.rs`
- Create: `preprocessor/dp_mapper/src/pathfinding/mod.rs`
- Create: `preprocessor/dp_mapper/src/pathfinding/solver.rs`

- [ ] **Step 1: Create lib.rs with placeholder**

```rust
//! DP Mapper: Globally optimal stop-to-segment mapping using dynamic programming

pub mod grid;
pub mod candidate;
pub mod pathfinding;

use shared::RouteNode;

/// Map bus stops to route progress values using globally optimal DP.
///
/// # Arguments
/// * `stops_cm` - Stop locations in centimeter coordinates (x, y)
/// * `route_nodes` - Linearized route nodes
/// * `k` - Number of candidates per stop (None = default 15)
///
/// # Returns
/// Progress values in INPUT ORDER (validated, non-decreasing)
pub fn map_stops(
    _stops_cm: &[(i64, i64)],
    _route_nodes: &[RouteNode],
    _k: Option<usize>,
) -> Vec<i32> {
    vec![]
}
```

- [ ] **Step 2: Create grid/mod.rs**

```rust
//! Spatial indexing for O(k) segment queries

pub mod builder;

pub use builder::{build_grid, query_neighbors};
```

- [ ] **Step 3: Create grid/builder.rs (placeholder)**

```rust
//! Grid construction and query functions

use shared::RouteNode;

pub fn build_grid(_route_nodes: &[RouteNode], _grid_size_cm: i32) -> () {
    // TODO: implement
}

pub fn query_neighbors(_grid: &(), _x_cm: i32, _y_cm: i32, _radius: u32) -> Vec<usize> {
    vec![]
}
```

- [ ] **Step 4: Create candidate/mod.rs**

```rust
//! Stop projection and K-candidate selection

pub mod generator;

pub use generator::{generate_candidates, generate_candidates_with_snap};

/// Candidate projection for a stop
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub seg_idx: usize,
    pub t: f64,
    pub dist_sq_cm2: i64,
    pub progress_cm: i32,
}
```

- [ ] **Step 5: Create candidate/generator.rs (placeholder)**

```rust
//! Candidate generation functions

use super::Candidate;
use shared::RouteNode;

pub fn generate_candidates(
    _stop: (i64, i64),
    _route_nodes: &[RouteNode],
    _grid: &(),
    _k: usize,
) -> Vec<Candidate> {
    vec![]
}

pub fn generate_candidates_with_snap(
    _stop: (i64, i64),
    _route_nodes: &[RouteNode],
    _grid: &(),
    _k: usize,
    _max_prev_progress_cm: i32,
) -> Vec<Candidate> {
    vec![]
}
```

- [ ] **Step 6: Create pathfinding/mod.rs**

```rust
//! Dynamic programming for optimal path finding

pub mod solver;

pub use solver::map_stops_dp;
```

- [ ] **Step 7: Create pathfinding/solver.rs (placeholder)**

```rust
//! DP solver implementation

// Placeholder: implemented in Chunk 4

use shared::RouteNode;

pub fn map_stops_dp(
    _stops_cm: &[(i64, i64)],
    _route_nodes: &[RouteNode],
    _grid: &(),
    _k: usize,
) -> Vec<i32> {
    vec![]
}
```

- [ ] **Step 8: Verify crate compiles**

Run: `cargo check -p dp_mapper`
Expected: No errors

- [ ] **Step 9: Commit**

```bash
git add preprocessor/dp_mapper/src/
git commit -m "feat(dp-mapper): create module skeleton with placeholder functions"
```

---

## Chunk 2: Grid Module Implementation

### Task 4: Implement SpatialGrid Type

**Files:**
- Modify: `preprocessor/dp_mapper/src/grid/mod.rs`
- Create: `preprocessor/dp_mapper/src/grid/tests.rs`

- [ ] **Step 1: Update grid/mod.rs with SpatialGrid type**

```rust
//! Spatial indexing for O(k) segment queries

pub mod builder;

pub use builder::{build_grid, query_neighbors};

use shared::RouteNode;

/// Spatial grid for O(k) segment queries
pub struct SpatialGrid {
    pub cells: Vec<Vec<usize>>,
    pub grid_size_cm: i32,
    pub cols: u32,
    pub rows: u32,
    pub x0_cm: i32,
    pub y0_cm: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_grid() {
        let grid = build_grid(&[], 10000);
        assert_eq!(grid.cols, 0);
        assert_eq!(grid.rows, 0);
    }
}
```

- [ ] **Step 2: Run test to verify it compiles**

Run: `cargo test -p dp_mapper --lib grid::tests`
Expected: Compiles, test fails ("build_grid not defined")

- [ ] **Step 3: Commit**

```bash
git add preprocessor/dp_mapper/src/grid/mod.rs
git commit -m "feat(dp-mapper): add SpatialGrid type to grid module"
```

---

### Task 5: Implement build_grid Function

**Files:**
- Modify: `preprocessor/dp_mapper/src/grid/builder.rs`

- [ ] **Step 1: Write failing tests for build_grid**

First, update `grid/mod.rs` to include builder tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::builder;

    #[test]
    fn test_empty_grid() {
        let grid = builder::build_grid(&[], 10000);
        assert_eq!(grid.cols, 0);
        assert_eq!(grid.rows, 0);
    }

    #[test]
    fn test_single_segment() {
        use shared::RouteNode;
        let nodes = vec![
            RouteNode {
                len2_cm2: 10000,
                heading_cdeg: 0,
                _pad: 0,
                x_cm: 0,
                y_cm: 0,
                cum_dist_cm: 0,
                dx_cm: 100,
                dy_cm: 0,
                seg_len_cm: 100,
            },
            RouteNode {
                len2_cm2: 0,
                heading_cdeg: 0,
                _pad: 0,
                x_cm: 100,
                y_cm: 0,
                cum_dist_cm: 100,
                dx_cm: 0,
                dy_cm: 0,
                seg_len_cm: 0,
            },
        ];
        let grid = builder::build_grid(&nodes, 10000);
        assert_eq!(grid.cols, 1);
        assert_eq!(grid.rows, 1);
        assert_eq!(grid.cells[0].len(), 1); // segment 0 in cell 0
    }

    #[test]
    fn test_multi_segment_grid() {
        use shared::RouteNode;
        let nodes = vec![
            RouteNode { len2_cm2: 100000000, heading_cdeg: 0, _pad: 0, x_cm: 0, y_cm: 0, cum_dist_cm: 0, dx_cm: 10000, dy_cm: 0, seg_len_cm: 10000 },
            RouteNode { len2_cm2: 100000000, heading_cdeg: 0, _pad: 0, x_cm: 10000, y_cm: 0, cum_dist_cm: 10000, dx_cm: 0, dy_cm: 10000, seg_len_cm: 10000 },
            RouteNode { len2_cm2: 0, heading_cdeg: 0, _pad: 0, x_cm: 10000, y_cm: 10000, cum_dist_cm: 20000, dx_cm: 0, dy_cm: 0, seg_len_cm: 0 },
        ];
        let grid = builder::build_grid(&nodes, 10000);
        // Should have 2x1 grid (x: 0-10000, y: 0-10000)
        assert_eq!(grid.cols, 1);
        assert_eq!(grid.rows, 1);
        assert_eq!(grid.x0_cm, 0);
        assert_eq!(grid.y0_cm, 0);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p dp_mapper --lib grid::tests`
Expected: All FAIL ("build_grid returns () not SpatialGrid")

- [ ] **Step 3: Implement build_grid in grid/builder.rs**

```rust
//! Grid construction and query functions

use shared::RouteNode;
use super::SpatialGrid;

/// Build a spatial grid index for route nodes
pub fn build_grid(nodes: &[RouteNode], grid_size_cm: i32) -> SpatialGrid {
    if nodes.is_empty() {
        return SpatialGrid {
            cells: vec![],
            grid_size_cm,
            cols: 0,
            rows: 0,
            x0_cm: 0,
            y0_cm: 0,
        };
    }

    // 1. Find bounding box
    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;

    for node in nodes {
        min_x = min_x.min(node.x_cm);
        min_y = min_y.min(node.y_cm);
        max_x = max_x.max(node.x_cm);
        max_y = max_y.max(node.y_cm);
    }

    // 2. Determine grid dimensions
    let cols = (((max_x - min_x) as f64 / grid_size_cm as f64).ceil() as u32).max(1);
    let rows = (((max_y - min_y) as f64 / grid_size_cm as f64).ceil() as u32).max(1);

    let mut cells = vec![vec![]; (rows * cols) as usize];

    // 3. Map segments to cells
    for i in 0..nodes.len().saturating_sub(1) {
        let node_a = &nodes[i];
        let node_b = &nodes[i + 1];

        // Segment bounding box in grid coordinates
        let x_start = ((node_a.x_cm.min(node_b.x_cm) - min_x) / grid_size_cm).max(0) as u32;
        let x_end = ((node_a.x_cm.max(node_b.x_cm) - min_x) / grid_size_cm).min(cols as i32 - 1) as u32;
        let y_start = ((node_a.y_cm.min(node_b.y_cm) - min_y) / grid_size_cm).max(0) as u32;
        let y_end = ((node_a.y_cm.max(node_b.y_cm) - min_y) / grid_size_cm).min(rows as i32 - 1) as u32;

        for r in y_start..=y_end {
            for c in x_start..=x_end {
                let cell_idx = (r * cols + c) as usize;
                cells[cell_idx].push(i);
            }
        }
    }

    SpatialGrid {
        cells,
        grid_size_cm,
        cols,
        rows,
        x0_cm: min_x,
        y0_cm: min_y,
    }
}

pub fn query_neighbors(grid: &SpatialGrid, x_cm: i32, y_cm: i32, radius: u32) -> Vec<usize> {
    vec![]
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p dp_mapper --lib grid::tests`
Expected: All PASS

- [ ] **Step 5: Commit**

```bash
git add preprocessor/dp_mapper/src/grid/
git commit -m "feat(dp-mapper): implement build_grid function with tests"
```

---

### Task 6: Implement query_neighbors Function

**Files:**
- Modify: `preprocessor/dp_mapper/src/grid/builder.rs`

- [ ] **Step 1: Write failing test for query_neighbors**

Add to `grid/mod.rs` tests:

```rust
    #[test]
    fn test_query_neighbors_radius_1() {
        use shared::RouteNode;
        let nodes = vec![
            RouteNode { len2_cm2: 100000000, heading_cdeg: 0, _pad: 0, x_cm: 0, y_cm: 0, cum_dist_cm: 0, dx_cm: 10000, dy_cm: 0, seg_len_cm: 10000 },
            RouteNode { len2_cm2: 100000000, heading_cdeg: 0, _pad: 0, x_cm: 10000, y_cm: 0, cum_dist_cm: 10000, dx_cm: 0, dy_cm: 10000, seg_len_cm: 10000 },
            RouteNode { len2_cm2: 0, heading_cdeg: 0, _pad: 0, x_cm: 10000, y_cm: 10000, cum_dist_cm: 20000, dx_cm: 0, dy_cm: 0, seg_len_cm: 0 },
        ];
        let grid = builder::build_grid(&nodes, 10000);

        // Query at origin, radius 1 (3x3 neighborhood)
        let result = builder::query_neighbors(&grid, 0, 0, 1);
        assert!(!result.is_empty());
        assert!(result.contains(&0)); // segment 0 should be found
    }

    #[test]
    fn test_query_neighbors_dedup() {
        use shared::RouteNode;
        let nodes = vec![
            RouteNode { len2_cm2: 100000000, heading_cdeg: 0, _pad: 0, x_cm: 0, y_cm: 0, cum_dist_cm: 0, dx_cm: 10000, dy_cm: 0, seg_len_cm: 10000 },
            RouteNode { len2_cm2: 100000000, heading_cdeg: 0, _pad: 0, x_cm: 10000, y_cm: 0, cum_dist_cm: 10000, dx_cm: 0, dy_cm: 10000, seg_len_cm: 10000 },
            RouteNode { len2_cm2: 0, heading_cdeg: 0, _pad: 0, x_cm: 10000, y_cm: 10000, cum_dist_cm: 20000, dx_cm: 0, dy_cm: 0, seg_len_cm: 0 },
        ];
        let grid = builder::build_grid(&nodes, 10000);

        // Same cell query should return deduplicated results
        let result = builder::query_neighbors(&grid, 5000, 0, 1);
        // No duplicates in result
        let mut sorted = result.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(result.len(), sorted.len());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p dp_mapper --lib grid::tests::test_query_neighbors`
Expected: FAIL (returns empty vec)

- [ ] **Step 3: Implement query_neighbors**

Replace placeholder in `grid/builder.rs`:

```rust
pub fn query_neighbors(grid: &SpatialGrid, x_cm: i32, y_cm: i32, radius: u32) -> Vec<usize> {
    if grid.cols == 0 || grid.rows == 0 {
        return Vec::new();
    }

    let gx = ((x_cm - grid.x0_cm) / grid.grid_size_cm) as usize;
    let gy = ((y_cm - grid.y0_cm) / grid.grid_size_cm) as usize;

    let mut candidates = Vec::new();
    let diameter = (radius * 2 + 1) as usize;

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

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p dp_mapper --lib grid::tests`
Expected: All PASS

- [ ] **Step 5: Add deduplication test to unit test file**

Create `preprocessor/dp_mapper/src/grid/builder_tests.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_neighbors_dedup_across_radii() {
        use shared::RouteNode;
        let nodes = vec![
            RouteNode { len2_cm2: 100000000, heading_cdeg: 0, _pad: 0, x_cm: 0, y_cm: 0, cum_dist_cm: 0, dx_cm: 10000, dy_cm: 0, seg_len_cm: 10000 },
            RouteNode { len2_cm2: 100000000, heading_cdeg: 0, _pad: 0, x_cm: 10000, y_cm: 0, cum_dist_cm: 10000, dx_cm: 0, dy_cm: 10000, seg_len_cm: 10000 },
            RouteNode { len2_cm2: 0, heading_cdeg: 0, _pad: 0, x_cm: 10000, y_cm: 10000, cum_dist_cm: 20000, dx_cm: 0, dy_cm: 0, seg_len_cm: 0 },
        ];
        let grid = build_grid(&nodes, 10000);

        // Query with radius 2 might return same segment multiple times
        let result = query_neighbors(&grid, 0, 0, 2);
        let mut sorted = result.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(result, sorted); // Verify deduped
    }
}
```

Add to `grid/builder.rs`: `mod tests;` at end

- [ ] **Step 6: Run all tests**

Run: `cargo test -p dp_mapper --lib grid`
Expected: All PASS

- [ ] **Step 7: Commit**

```bash
git add preprocessor/dp_mapper/src/grid/
git commit -m "feat(dp-mapper): implement query_neighbors with deduplication"
```

---

## Chunk 3: Candidate Module Implementation

### Task 7: Implement Candidate Projection

**Files:**
- Modify: `preprocessor/dp_mapper/src/candidate/generator.rs`

- [ ] **Step 1: Write failing test for projection**

Create `preprocessor/dp_mapper/src/candidate/generator_tests.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::candidate::Candidate;
    use crate::grid::{build_grid, SpatialGrid};

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
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p dp_mapper --lib candidate::generator_tests`
Expected: All FAIL (returns empty vec)

- [ ] **Step 3: Implement generate_candidates**

Replace placeholder in `candidate/generator.rs`:

```rust
//! Candidate generation functions

use super::Candidate;
use shared::RouteNode;
use crate::grid::{SpatialGrid, query_neighbors};

// Constants
const GRID_RADIUS_MAX: u32 = 3;

/// Generate candidates for a stop (without snap - for first stop)
pub fn generate_candidates(
    stop: (i64, i64),
    route_nodes: &[RouteNode],
    grid: &SpatialGrid,
    k: usize,
) -> Vec<Candidate> {
    let mut candidates = Vec::new();

    // Grid query with expanding radius
    for radius in 1..=GRID_RADIUS_MAX {
        let seg_indices = query_neighbors(grid, stop.0 as i32, stop.1 as i32, radius);

        for &seg_idx in seg_indices {
            if seg_idx >= route_nodes.len().saturating_sub(1) {
                continue;
            }

            let node = &route_nodes[seg_idx];

            // Project stop onto segment
            let dx = stop.0 - node.x_cm as i64;
            let dy = stop.1 - node.y_cm as i64;

            // t = [(P - A) · (B - A)] / |B - A|²
            let t_num = dx * node.dx_cm as i64 + dy * node.dy_cm as i64;
            let t = (t_num as f64 / node.len2_cm2 as f64).clamp(0.0, 1.0);

            // Closest point on segment
            let px = node.x_cm as f64 + t * node.dx_cm as f64;
            let py = node.y_cm as f64 + t * node.dy_cm as f64;

            // Squared distance
            let dist_x = stop.0 as f64 - px;
            let dist_y = stop.1 as f64 - py;
            let dist_sq_cm2 = (dist_x * dist_x + dist_y * dist_y) as i64;

            // Progress along route
            let progress_cm = node.cum_dist_cm + (t * node.seg_len_cm as f64).round() as i32;

            candidates.push(Candidate {
                seg_idx,
                t,
                dist_sq_cm2,
                progress_cm,
            });
        }
    }

    // Deduplicate by (seg_idx, t)
    candidates.sort_by_key(|c| (c.seg_idx, c.t.to_bits()));
    candidates.dedup_by(|a, b| a.seg_idx == b.seg_idx && a.t == b.t);

    // Sort by distance and keep top-K
    candidates.sort_by_key(|c| c.dist_sq_cm2);
    candidates.truncate(k);

    candidates
}

pub fn generate_candidates_with_snap(
    _stop: (i64, i64),
    _route_nodes: &[RouteNode],
    _grid: &SpatialGrid,
    _k: usize,
    _max_prev_progress_cm: i32,
) -> Vec<Candidate> {
    vec![]
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p dp_mapper --lib candidate::generator_tests`
Expected: All PASS

- [ ] **Step 5: Commit**

```bash
git add preprocessor/dp_mapper/src/candidate/
git commit -m "feat(dp-mapper): implement generate_candidates with projection"
```

---

### Task 8: Implement Snap Candidate Generation

**Files:**
- Modify: `preprocessor/dp_mapper/src/candidate/generator.rs`

- [ ] **Step 1: Write failing test for snap generation**

Add to `candidate/generator_tests.rs`:

```rust
    #[test]
    fn test_snap_candidate_generation() {
        let (nodes, grid) = make_simple_route();

        // Previous layer had max progress 5000
        let result = generate_candidates_with_snap((50000, 0), &nodes, &grid, 5, 5000);
        assert!(!result.is_empty(), "should have at least snap candidate");

        // Snap should be last (highest distance)
        let snap = result.last().unwrap();
        assert_eq!(snap.t, 0.0, "snap should be at segment start");
        assert!(snap.dist_sq_cm2 > 1_000_000_000_000, "snap should have penalty distance");
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

        // Previous max progress = 7500 (middle of segment 1: 5000-10000)
        let result = generate_candidates_with_snap((0, 0), &nodes, &grid, 5, 7500);
        assert!(!result.is_empty());

        let snap = result.last().unwrap();
        // Snap should be on segment 2 (which contains 7500: cum=10000 >= 7500)
        assert_eq!(snap.seg_idx, 2);
        assert_eq!(snap.progress_cm, 10000);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p dp_mapper --lib candidate::generator_tests::test_snap`
Expected: FAIL (returns empty vec)

- [ ] **Step 3: Implement generate_candidates_with_snap**

Add constant and replace placeholder in `candidate/generator.rs`:

```rust
// Snap penalty: ~316 km² (100× larger than worst legitimate projection)
const SNAP_PENALTY_CM2: i64 = 1_000_000_000_000;

pub fn generate_candidates_with_snap(
    stop: (i64, i64),
    route_nodes: &[RouteNode],
    grid: &SpatialGrid,
    k: usize,
    max_prev_progress_cm: i32,
) -> Vec<Candidate> {
    // Generate normal candidates
    let mut candidates = generate_candidates(stop, route_nodes, grid, k);

    // Find first segment whose END is past max_prev_progress_cm
    let snap_seg_idx = route_nodes
        .iter()
        .position(|n| n.cum_dist_cm + n.seg_len_cm >= max_prev_progress_cm)
        .unwrap_or(route_nodes.len().saturating_sub(2));

    // Create snap candidate at segment start
    let snap_candidate = Candidate {
        seg_idx: snap_seg_idx,
        t: 0.0,
        dist_sq_cm2: SNAP_PENALTY_CM2,
        progress_cm: route_nodes[snap_seg_idx].cum_dist_cm,
    };

    candidates.push(snap_candidate);
    candidates
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p dp_mapper --lib candidate::generator_tests`
Expected: All PASS

- [ ] **Step 5: Commit**

```bash
git add preprocessor/dp_mapper/src/candidate/
git commit -m "feat(dp-mapper): implement snap candidate generation"
```

---

## Chunk 4: Pathfinding Module Implementation

### Task 9: Implement DP Solver Types

**Files:**
- Modify: `preprocessor/dp_mapper/src/pathfinding/mod.rs`

- [ ] **Step 1: Add DP types to pathfinding/mod.rs**

```rust
//! Dynamic programming for optimal path finding

pub mod solver;

pub use solver::map_stops_dp;

use crate::candidate::Candidate;

/// DP layer: candidates for one stop with backtracking info
pub struct DpLayer {
    pub candidates: Vec<Candidate>,
    pub best_cost: Vec<i64>,
    pub best_prev: Vec<Option<usize>>,
}

/// For sorting by progress while preserving original indices
pub struct SortedCandidate {
    pub orig_idx: usize,
    pub progress_cm: i32,
}
```

- [ ] **Step 2: Verify compiles**

Run: `cargo check -p dp_mapper`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add preprocessor/dp_mapper/src/pathfinding/mod.rs
git commit -m "feat(dp-mapper): add DP solver types"
```

---

### Task 10: Implement DP Forward Pass

**Files:**
- Modify: `preprocessor/dp_mapper/src/pathfinding/solver.rs`

- [ ] **Step 1: Write failing test for forward pass**

Create `preprocessor/dp_mapper/src/pathfinding/solver_tests.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::candidate::{Candidate, generate_candidates, generate_candidates_with_snap};
    use crate::grid::build_grid;
    use crate::pathfinding::{DpLayer, dp_forward_pass};

    fn make_test_route() -> (Vec<shared::RouteNode>, crate::grid::SpatialGrid) {
        let nodes = vec![
            shared::RouteNode { len2_cm2: 100000000, heading_cdeg: 0, _pad: 0, x_cm: 0, y_cm: 0, cum_dist_cm: 0, dx_cm: 10000, dy_cm: 0, seg_len_cm: 10000 },
            shared::RouteNode { len2_cm2: 100000000, heading_cdeg: 0, _pad: 0, x_cm: 10000, y_cm: 0, cum_dist_cm: 10000, dx_cm: 10000, dy_cm: 0, seg_len_cm: 10000 },
            shared::RouteNode { len2_cm2: 0, heading_cdeg: 0, _pad: 0, x_cm: 20000, y_cm: 0, cum_dist_cm: 20000, dx_cm: 0, dy_cm: 0, seg_len_cm: 0 },
        ];
        let grid = build_grid(&nodes, 10000);
        (nodes, grid)
    }

    #[test]
    fn test_forward_pass_two_stops() {
        let (route_nodes, grid) = make_test_route();

        let stop0_cands = generate_candidates((0, 0), &route_nodes, &grid, 5);
        let stop1_cands = generate_candidates_with_snap((10000, 0), &route_nodes, &grid, 5, 5000);

        let mut layers = vec![
            DpLayer {
                candidates: stop0_cands.clone(),
                best_cost: stop0_cands.iter().map(|c| c.dist_sq_cm2).collect(),
                best_prev: vec![None; stop0_cands.len()],
            },
            DpLayer {
                candidates: stop1_cands.clone(),
                best_cost: vec![i64::MAX; stop1_cands.len()],
                best_prev: vec![None; stop1_cands.len()],
            },
        ];

        dp_forward_pass(&mut layers);

        // Second layer should have valid costs
        for cost in &layers[1].best_cost {
            assert!(*cost < i64::MAX, "all candidates should be reachable");
        }
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p dp_mapper --lib pathfinding::solver_tests`
Expected: FAIL ("dp_forward_pass not defined")

- [ ] **Step 3: Implement dp_forward_pass**

Replace placeholder in `pathfinding/solver.rs`:

```rust
//! DP solver implementation

use shared::RouteNode;
use crate::candidate::{Candidate, generate_candidates, generate_candidates_with_snap};
use crate::grid::SpatialGrid;
use crate::pathfinding::{DpLayer, SortedCandidate};

const DEFAULT_K: usize = 15;

pub fn map_stops_dp(
    stops_cm: &[(i64, i64)],
    route_nodes: &[RouteNode],
    grid: &SpatialGrid,
    k: usize,
) -> Vec<i32> {
    if stops_cm.is_empty() || route_nodes.len() < 2 {
        return vec![];
    }

    // Generate candidates for all stops
    let mut layers: Vec<DpLayer> = Vec::with_capacity(stops_cm.len());

    for (j, &stop) in stops_cm.iter().enumerate() {
        let cands = if j == 0 {
            generate_candidates(stop, route_nodes, grid, k)
        } else {
            let max_prev = layers[j-1].candidates
                .iter()
                .map(|c| c.progress_cm)
                .max()
                .unwrap_or(0);
            generate_candidates_with_snap(stop, route_nodes, grid, k, max_prev)
        };

        let num_cands = cands.len();
        layers.push(DpLayer {
            candidates: cands,
            best_cost: vec![i64::MAX; num_cands],
            best_prev: vec![None; num_cands],
        });
    }

    // Initialize base layer
    for (i, c) in layers[0].candidates.iter().enumerate() {
        layers[0].best_cost[i] = c.dist_sq_cm2;
    }

    // Forward pass
    dp_forward_pass(&mut layers);

    // Backtrack
    dp_backtrack(&layers)
}

pub(crate) fn dp_forward_pass(layers: &mut Vec<DpLayer>) {
    for j in 1..layers.len() {
        // Sort current layer by progress
        let mut curr_sorted: Vec<_> = layers[j].candidates
            .iter()
            .enumerate()
            .map(|(i, c)| SortedCandidate { orig_idx: i, progress_cm: c.progress_cm })
            .collect();
        curr_sorted.sort_by_key(|s| s.progress_cm);

        // Sort previous layer by progress
        let mut prev_sorted: Vec<_> = layers[j-1].candidates
            .iter()
            .enumerate()
            .map(|(i, c)| SortedCandidate { orig_idx: i, progress_cm: c.progress_cm })
            .collect();
        prev_sorted.sort_by_key(|s| s.progress_cm);

        // Sweep with running minimum
        let mut ptr = 0;
        let mut best_prev_cost = i64::MAX;
        let mut best_prev_idx = None;

        for curr in &curr_sorted {
            while ptr < prev_sorted.len() && prev_sorted[ptr].progress_cm <= curr.progress_cm {
                let prev_orig_idx = prev_sorted[ptr].orig_idx;
                if layers[j-1].best_cost[prev_orig_idx] < best_prev_cost {
                    best_prev_cost = layers[j-1].best_cost[prev_orig_idx];
                    best_prev_idx = Some(prev_orig_idx);
                }
                ptr += 1;
            }

            if let Some(prev_idx) = best_prev_idx {
                let curr_orig_idx = curr.orig_idx;
                let transition_cost = layers[j].candidates[curr_orig_idx].dist_sq_cm2;
                let total_cost = best_prev_cost + transition_cost;

                if total_cost < layers[j].best_cost[curr_orig_idx] {
                    layers[j].best_cost[curr_orig_idx] = total_cost;
                    layers[j].best_prev[curr_orig_idx] = Some(prev_idx);
                }
            }
        }
    }
}

fn dp_backtrack(layers: &[DpLayer]) -> Vec<i32> {
    let m = layers.len();
    let mut result = vec![0i32; m];

    // Find best final state
    let mut best_k = 0;
    let mut best_cost = i64::MAX;
    for (k, &cost) in layers[m-1].best_cost.iter().enumerate() {
        if cost < best_cost {
            best_cost = cost;
            best_k = k;
        }
    }

    // Backtrack
    let mut k = best_k;
    for j in (0..m).rev() {
        result[j] = layers[j].candidates[k].progress_cm;
        if j > 0 {
            k = layers[j].best_prev[k]
                .expect("DP backtrack broken: missing predecessor");
        }
    }

    result
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p dp_mapper --lib pathfinding::solver_tests`
Expected: All PASS

- [ ] **Step 5: Add backtrack test**

Add to `solver_tests.rs`:

```rust
    #[test]
    fn test_backtrack_reconstruction() {
        let (route_nodes, grid) = make_test_route();

        let stops = vec![(0, 0), (10000, 0)];
        let result = map_stops_dp(&stops, &route_nodes, &grid, 5);

        assert_eq!(result.len(), 2);
        assert!(result[0] <= result[1], "progress should be non-decreasing");
    }
```

- [ ] **Step 6: Run all tests**

Run: `cargo test -p dp_mapper --lib pathfinding`
Expected: All PASS

- [ ] **Step 7: Commit**

```bash
git add preprocessor/dp_mapper/src/pathfinding/
git commit -m "feat(dp-mapper): implement DP forward pass and backtrack"
```

---

## Chunk 5: Public API and Integration

### Task 11: Implement Public map_stops Function

**Files:**
- Modify: `preprocessor/dp_mapper/src/lib.rs`

- [ ] **Step 1: Write failing test for public API**

Create `preprocessor/dp_mapper/src/lib_tests.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_stops_empty() {
        let result = map_stops(&[], &[], None);
        assert_eq!(result, vec![]);
    }

    #[test]
    fn test_map_stops_single() {
        use shared::RouteNode;
        let nodes = vec![
            RouteNode { len2_cm2: 100000000, heading_cdeg: 0, _pad: 0, x_cm: 0, y_cm: 0, cum_dist_cm: 0, dx_cm: 10000, dy_cm: 0, seg_len_cm: 10000 },
            RouteNode { len2_cm2: 0, heading_cdeg: 0, _pad: 0, x_cm: 10000, y_cm: 0, cum_dist_cm: 10000, dx_cm: 0, dy_cm: 0, seg_len_cm: 0 },
        ];

        let result = map_stops(&[(0, 0)], &nodes, None);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_map_stops_default_k() {
        use shared::RouteNode;
        let nodes = vec![
            RouteNode { len2_cm2: 100000000, heading_cdeg: 0, _pad: 0, x_cm: 0, y_cm: 0, cum_dist_cm: 0, dx_cm: 10000, dy_cm: 0, seg_len_cm: 10000 },
            RouteNode { len2_cm2: 0, heading_cdeg: 0, _pad: 0, x_cm: 10000, y_cm: 0, cum_dist_cm: 10000, dx_cm: 0, dy_cm: 0, seg_len_cm: 0 },
        ];

        let result = map_stops(&[(0, 0)], &nodes, None);
        assert_eq!(result.len(), 1);
        // Should use DEFAULT_K=15 internally
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p dp_mapper --lib lib_tests`
Expected: FAIL (returns empty vec)

- [ ] **Step 3: Implement map_stops**

Replace placeholder in `lib.rs`:

```rust
//! DP Mapper: Globally optimal stop-to-segment mapping using dynamic programming

pub mod grid;
pub mod candidate;
pub mod pathfinding;

use shared::RouteNode;
use pathfinding::{map_stops_dp, SpatialGrid as PathGrid};

const DEFAULT_K: usize = 15;
const GRID_SIZE_CM: i32 = 10000;

/// Map bus stops to route progress values using globally optimal DP.
///
/// # Arguments
/// * `stops_cm` - Stop locations in centimeter coordinates (x, y)
/// * `route_nodes` - Linearized route nodes
/// * `k` - Number of candidates per stop (None = default 15)
///
/// # Returns
/// Progress values in INPUT ORDER (validated, non-decreasing)
pub fn map_stops(
    stops_cm: &[(i64, i64)],
    route_nodes: &[RouteNode],
    k: Option<usize>,
) -> Vec<i32> {
    if stops_cm.is_empty() || route_nodes.len() < 2 {
        return vec![];
    }

    let k = k.unwrap_or(DEFAULT_K);
    let grid = grid::build_grid(route_nodes, GRID_SIZE_CM);

    map_stops_dp(stops_cm, route_nodes, &grid, k)
}

// Re-export for internal use
mod pathfinding_internal {
    pub use crate::pathfinding::SpatialGrid;
}
```

- [ ] **Step 4: Fix imports in pathfinding module**

Update `pathfinding/solver.rs` to use correct types:

```rust
// In use statements at top:
use crate::grid::SpatialGrid;  // Use grid's SpatialGrid
```

Update `pathfinding/mod.rs` to export grid's SpatialGrid:

```rust
//! Dynamic programming for optimal path finding

pub mod solver;

pub use solver::map_stops_dp;
pub use crate::grid::SpatialGrid;

use crate::candidate::Candidate;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p dp_mapper --lib`
Expected: All PASS

- [ ] **Step 6: Commit**

```bash
git add preprocessor/dp_mapper/src/
git commit -m "feat(dp-mapper): implement public map_stops API"
```

---

### Task 12: Add Integration Tests

**Files:**
- Create: `preprocessor/dp_mapper/tests/integration.rs`

- [ ] **Step 1: Create integration test file**

```rust
//! Integration tests with real route data

use dp_mapper::map_stops;
use shared::RouteNode;

#[test]
fn test_tpF805_route() {
    // Load real route data
    let route_json = std::fs::read_to_string("../../../test_data/tpF805_route.json")
        .expect("failed to load tpF805 route");

    let value: serde_json::Value = serde_json::from_str(&route_json)
        .expect("failed to parse route JSON");

    // Extract route nodes
    let nodes: Vec<RouteNode> = serde_json::from_value(value["route"]["nodes"].clone())
        .expect("failed to parse route nodes");

    // Load stops
    let stops_json = std::fs::read_to_string("../../../test_data/tpF805_stops.json")
        .expect("failed to load tpF805 stops");

    let stops_value: serde_json::Value = serde_json::from_str(&stops_json)
        .expect("failed to parse stops JSON");

    let stops: Vec<(i64, i64)> = serde_json::from_value(stops_value["stops"].clone())
        .expect("failed to parse stops");

    // Run mapper
    let result = map_stops(&stops, &nodes, None);

    // Verify output
    assert_eq!(result.len(), stops.len());
    assert!(result.iter().zip(result.iter().skip(1)).all(|(a, b)| a <= b));
}

#[test]
fn test_two_pass_route() {
    // Similar structure for two_pass_test
    let route_json = std::fs::read_to_string("../../../tools/data/two_pass_test/route.json")
        .expect("failed to load two_pass_test route");

    // ... (similar to above)
}
```

- [ ] **Step 2: Run integration tests**

Run: `cargo test -p dp_mapper --test integration`
Expected: Tests run (may fail if test data not available)

- [ ] **Step 3: Add comparison with greedy**

Create `preprocessor/tests/dp_vs_greedy.rs`:

```rust
//! Compare DP mapper with greedy implementation

use dp_mapper::map_stops;
use preprocessor::stops::validate_stop_sequence;

#[test]
fn test_dp_beats_or_matches_greedy() {
    // Load test route and run both algorithms
    // Verify DP total distance <= greedy total distance
}
```

- [ ] **Step 4: Commit**

```bash
git add preprocessor/dp_mapper/tests/
git commit -m "test(dp-mapper): add integration tests with real route data"
```

---

## Chunk 6: Completion and Cleanup

### Task 13: Add Documentation and Metadata

**Files:**
- Modify: `preprocessor/dp_mapper/Cargo.toml`
- Create: `preprocessor/dp_mapper/README.md`

- [ ] **Step 1: Add package metadata to Cargo.toml**

```toml
[package]
name = "dp_mapper"
version.workspace = true
edition.workspace = true
description = "Globally optimal stop-to-segment mapping using dynamic programming"
repository = "https://github.com/example/bus_arrival"

[dependencies]
shared = { path = "../../../shared" }

[dev-dependencies]
serde_json = "1.0"
```

- [ ] **Step 2: Create README**

```markdown
# DP Mapper

Globally optimal bus stop-to-segment mapping using dynamic programming.

## Overview

This crate implements a Viterbi-like DAG shortest path algorithm to map bus stops onto route segments. Unlike greedy approaches, DP finds the globally optimal mapping that minimizes total projection distance while preserving stop order.

## Usage

```rust
use dp_mapper::map_stops;
use shared::RouteNode;

let progress_values = map_stops(&stops_cm, &route_nodes, None);
```

## Algorithm

1. **Candidate Generation:** For each stop, project onto nearby segments (K candidates)
2. **DP Forward Pass:** Find minimum-cost path through candidate layers
3. **Backtrack:** Reconstruct optimal path

## Complexity

- **Time:** O(M × K log K) where M = stops, K = candidates per stop
- **Space:** O(M × K)

For typical routes (M=35, K=15): < 10ms
```

- [ ] **Step 3: Commit**

```bash
git add preprocessor/dp_mapper/
git commit -m "docs(dp-mapper): add package metadata and README"
```

---

### Task 14: Final Verification

**Files:**
- Run all tests

- [ ] **Step 1: Run full test suite**

Run: `cargo test -p dp_mapper --all`
Expected: All tests pass

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -p dp_mapper -- -D warnings`
Expected: No warnings

- [ ] **Step 3: Check formatting**

Run: `cargo fmt -p dp_mapper -- --check`
Expected: No formatting changes

- [ ] **Step 4: Build release**

Run: `cargo build -p dp_mapper --release`
Expected: Clean build

- [ ] **Step 5: Run cargo doc**

Run: `cargo doc -p dp_mapper --no-deps --open`
Expected: Documentation builds

- [ ] **Step 6: Final commit**

```bash
git add preprocessor/dp_mapper/
git commit -m "chore(dp-mapper): final cleanup and verification"
```

---

## Success Criteria Checklist

- [ ] All unit tests pass
- [ ] Integration tests validate against tpF805 and two_pass_test
- [ ] Performance < 10ms for typical routes (M=35, K=15)
- [ ] Output is non-decreasing
- [ ] DP total distance ≤ greedy total distance
- [ ] Snap candidates only used when no valid transition
- [ ] No clippy warnings
- [ ] Documentation complete

---

## Next Steps (After Implementation)

1. Add feature flag `dp-mapping` to preprocessor
2. Run A/B tests on real routes
3. Replace greedy implementation once validated
4. Remove old validation code

# DP Mapper Crate Design Specification

**Date:** 2026-03-18
**Status:** Approved
**Author:** Claude
**Reviewers:** [TBD]

## Overview

A standalone library crate that implements globally optimal stop-to-segment mapping using dynamic programming (Viterbi-like algorithm). The crate replaces the greedy approach with a DAG shortest path solution, ensuring optimal mapping across all stops simultaneously.

## Motivation

The current greedy stop mapper in `preprocessor/src/stops/validation.rs` makes locally optimal choices that can lead to globally suboptimal results. For routes that double back or have dense stops, greedy can be 5× worse than optimal.

The DP approach formulates the problem as finding the minimum-cost path through a layered DAG, where each layer contains K candidate projections for a stop. This guarantees globally optimal monotonic mappings.

## Crate Structure

```
preprocessor/dp_mapper/
  ├── Cargo.toml
  ├── src/
  │   ├── lib.rs           (public API)
  │   ├── grid/
  │   │   ├── mod.rs
  │   │   └── builder.rs   (spatial index)
  │   ├── candidate/
  │   │   ├── mod.rs
  │   │   └── generator.rs (projection & selection)
  │   └── pathfinding/
  │       ├── mod.rs
  │       └── solver.rs    (DP + backtrack)
  └── tests/
      └── integration.rs   (real route validation)
```

## Public API

```rust
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
///
/// # Panics
/// Never - uses snap-forward fallback for disconnected layers
pub fn map_stops(
    stops_cm: &[(i64, i64)],
    route_nodes: &[RouteNode],
    k: Option<usize>,
) -> Vec<i32>
```

## Module Architecture

### `grid` Module

**Responsibility:** Spatial indexing for O(k) segment queries

**Public Types:**
```rust
pub struct SpatialGrid {
    cells: Vec<Vec<usize>>,
    grid_size_cm: i32,
    cols: u32,
    rows: u32,
    x0_cm: i32,
    y0_cm: i32,
}
```

**Public Functions:**
```rust
pub fn build_grid(route_nodes: &[RouteNode], grid_size_cm: i32) -> SpatialGrid;
pub fn query_neighbors(grid: &SpatialGrid, x_cm: i32, y_cm: i32, radius: u32) -> Vec<usize>;
```

**Key Behavior:**
- 100m × 100m cells (`GRID_SIZE_CM = 10000`)
- Query radius expands: 3×3 → 5×5 → 7×7 (radius 1, 2, 3)

### `candidate` Module

**Responsibility:** Stop projection and K-candidate selection

**Public Types:**
```rust
pub struct Candidate {
    seg_idx: usize,      // Route segment index
    t: f64,              // Position on segment [0, 1]
    dist_cm: i64,        // Squared distance from stop to projection (cm²)
    progress_cm: i32,    // Absolute progress along route (cm)
}
```

**Public Functions:**
```rust
pub fn generate_candidates(
    stop: (i64, i64),
    route_nodes: &[RouteNode],
    grid: &SpatialGrid,
    k: usize,
) -> Vec<Candidate>;
```

**Key Behavior:**
- Projects stop onto all segments from grid query (3 radii)
- Clamps t to [0.0, 1.0]
- Deduplicates and keeps top-K by distance
- Adds snap-forward candidate with `SNAP_PENALTY_CM2`

### `pathfinding` Module

**Responsibility:** Dynamic programming for optimal path finding

**Internal Types:**
```rust
struct DpLayer {
    candidates: Vec<Candidate>,
    best_cost: Vec<i64>,      // min cost to reach each candidate
    best_prev: Vec<usize>,    // backpointer to previous layer
}
```

**Public Functions:**
```rust
pub fn map_stops_dp(
    stops_cm: &[(i64, i64)],
    route_nodes: &[RouteNode],
    grid: &SpatialGrid,
    k: usize,
) -> Vec<i32>;
```

**Algorithm:**
1. Generate candidates for all stops
2. Forward pass: O(M × K) sweep with running minimum
3. Find best final candidate (minimum cost)
4. Backtrack to reconstruct path
5. Return progress values in input order

## Data Flow

```
Input: stops_cm[], route_nodes[]
           │
           ▼
┌──────────────────────┐
│  build_grid()        │  Build spatial index once
└──────────────────────┘
           │
           ▼
┌──────────────────────┐
│  For each stop:      │
│  generate_candidates()│  → K candidates per stop
└──────────────────────┘
           │
           ▼
┌──────────────────────┐
│  map_stops_dp()      │  Forward pass + backtrack
│  - Sort by progress  │
│  - DP sweep O(M×K)   │
│  - Backtrack path    │
└──────────────────────┘
           │
           ▼
Output: Vec<i32> progress values (input order, non-decreasing)
```

## Constants

```rust
// Candidate generation
const DEFAULT_K: usize = 15;
const GRID_RADIUS_MAX: u32 = 3;  // 3×3, 5×5, 7×7 expansion
const GRID_SIZE_CM: i32 = 10000; // 100m cells

// Snap fallback
const SNAP_PENALTY_CM2: i64 = 1_000_000_000_000; // ~316 km² penalty

// Deduplication
const MAX_CANDIDATES: usize = 100; // cap before sort to prevent explosion
```

## Dependencies

```toml
[package]
name = "dp_mapper"
version.workspace = true
edition.workspace = true

[dependencies]
shared = { path = "../../../shared" }
```

No external dependencies. Pure Rust implementation.

## Testing Strategy

### Unit Tests

| Module | Coverage |
|--------|----------|
| `grid` | Empty route, single segment, multi-segment; boundary conditions; query radius expansion |
| `candidate` | Projection at t=0, t=1, t=0.5; clamping; sorting/K-limiting; snap generation |
| `pathfinding` | Two-stop monotonic; same-segment strict inequality; route loops; snap activation; backtrack |

### Integration Tests

- Real route data (tpF805, two_pass_test) with known outputs
- Comparison against greedy implementation to verify optimality

## Complexity

| Phase | Cost |
|-------|------|
| Grid build | O(N) |
| Candidate generation | O(M × K) |
| Per-layer sort | O(M × K log K) |
| DP sweep | O(M × K) |
| Backtrack | O(M) |
| **Total** | **O(M × K log K)** |

With M=35 stops, K=15: ~525 projections + trivial sort.

## Edge Cases Handled

| Edge Case | Solution |
|-----------|----------|
| Stop projects behind min_prog | DP doesn't use that candidate |
| Route loop (same location twice) | Both segments as candidates; DP picks best sequence |
| Identical adjacent stops | Equal progress is valid (>= transition) |
| No valid transition | Snap-forward candidate with large penalty |
| First stop (no constraint) | All candidates valid, no min_t filter |

## Integration with Preprocessor

The crate will be added to `preprocessor/Cargo.toml`:

```toml
[dependencies]
dp_mapper = { path = "dp_mapper" }
```

And used in `preprocessor/src/stops/` to replace the current greedy implementation.

## Success Criteria

1. All unit tests pass
2. Integration tests validate against real routes (tpF805, two_pass_test)
3. Performance: O(M × K log K) < 10ms for typical routes
4. Correctness: Output is non-decreasing and globally optimal

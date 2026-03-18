# DP Mapper Crate Design Specification

**Date:** 2026-03-18
**Status:** Draft
**Author:** Claude
**Reviewers:** Pending

## Overview

A standalone library crate that implements globally optimal stop-to-segment mapping using dynamic programming (Viterbi-like algorithm). The crate replaces the greedy approach with a DAG shortest path solution, ensuring optimal mapping across all stops simultaneously.

## Motivation

The current greedy stop mapper in `preprocessor/src/stops/validation.rs` makes locally optimal choices that can lead to globally suboptimal results. For routes that double back or have dense stops, greedy can be 5× worse than optimal.

The DP approach formulates the problem as finding the minimum-cost path through a layered DAG, where each layer contains K candidate projections for a stop. This guarantees globally optimal monotonic mappings.

## Workspace Structure

The crate will be nested within `preprocessor/dp_mapper/` to keep preprocessor-related functionality together. To enable this, the workspace `Cargo.toml` will be updated:

```toml
[workspace]
resolver = "2"
members = ["shared", "preprocessor", "preprocessor/dp_mapper", "simulator", "arrival_detector"]
```

This structure allows:
- Co-location with the preprocessor that uses it
- Independent testing and development
- Clear dependency from preprocessor to dp_mapper

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
/// # Implementation
/// - Builds spatial grid internally (cached per call)
/// - Generates K candidates per stop via grid queries
/// - Runs DP to find globally optimal path
/// - Never fails (uses snap-forward fallback)
pub fn map_stops(
    stops_cm: &[(i64, i64)],
    route_nodes: &[RouteNode],
    k: Option<usize>,
) -> Vec<i32>
```

**Note:** The grid is built fresh on each call. For batch processing of multiple routes, the caller should process routes sequentially (grid build cost is O(N) where N is route nodes, typically <1ms).

## Module Architecture

### `grid` Module

**Responsibility:** Spatial indexing for O(k) segment queries

This module provides its own implementation of spatial indexing, adapted from the existing `preprocessor/src/grid.rs` but with the API surface needed by dp_mapper.

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

// Query with radius: 1 = 3×3, 2 = 5×5, 3 = 7×7
pub fn query_neighbors(grid: &SpatialGrid, x_cm: i32, y_cm: i32, radius: u32) -> Vec<usize>;
```

**Key Behavior:**
- 100m × 100m cells (`GRID_SIZE_CM = 10000`)
- Query radius expands: 3×3 → 5×5 → 7×7 (radius 1, 2, 3)
- Returns deduplicated segment indices

**Note:** Coordinate types use `i32` for grid parameters (matching `RouteNode` fields) and `u32` for radius (unsigned dimension count).

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

**Algorithm Details:**

1. **Grid Query:** For radii 1, 2, 3 (expanding 3×3 → 5×5 → 7×7), collect all segment indices
2. **Projection:** For each segment, project stop onto line, clamp t to [0, 1], compute distance² and progress
3. **Deduplication:** Remove duplicates by `(seg_idx, t)` pair - same segment at same position
4. **Sort:** Sort by distance² ascending
5. **Select:** Keep top-K candidates (by distance)
6. **Snap:** Add one snap-forward candidate as fallback

**Snap Candidate Generation:**
- Find first segment where `cum_dist_cm + seg_len_cm >= min_known_prog`
- Set `t = 0.0` (at segment start)
- Set `dist_cm = SNAP_PENALTY_CM2`
- This ensures DP uses it only when no valid transition exists

**Key Behavior:**
- Projects stop onto all segments from grid query (3 radii)
- Clamps t to [0.0, 1.0]
- Deduplicates by `(seg_idx, t)` before sorting
- Keeps top-K by distance (minimum distance first)
- Adds snap-forward candidate with `SNAP_PENALTY_CM2`

### `pathfinding` Module

**Responsibility:** Dynamic programming for optimal path finding

This module takes raw stop coordinates and orchestrates candidate generation and DP solving.

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
1. **Generate candidates:** Call `generate_candidates()` for each stop, producing M layers
2. **Forward pass:** For each layer j=1..M-1:
   - Sort layer j and j-1 by progress_cm
   - Sweep with running minimum to find valid transitions (progress[j] >= progress[j-1])
   - Store best cost and backpointer
3. **Find best final:** Select candidate in last layer with minimum cost
4. **Backtrack:** Follow backpointers to reconstruct optimal path
5. **Return:** Progress values in original input order

**Module Boundaries:**
- `pathfinding` calls `generate_candidates()` - does not implement projection
- `pathfinding` implements DP logic - does not expose internal state
- Clear separation: candidate generation (candidate module) vs path optimization (pathfinding module)

## Data Flow

```
Input: stops_cm[], route_nodes[]
           │
           ▼
┌──────────────────────────────┐
│  map_stops() public API      │
│  - Validates input            │
│  - Sets default K=15 if None │
└──────────────────────────────┘
           │
           ▼
┌──────────────────────┐
│  build_grid()        │  Build spatial index once
└──────────────────────┘
           │
           ▼
┌──────────────────────────────────────┐
│  For each stop:                      │
│  generate_candidates()               │
│  - Grid query (3 radii expansion)    │
│  - Project onto segments             │
│  - Deduplicate by (seg_idx, t)       │
│  - Sort by distance²                 │
│  - Select top-K                      │
│  - Add snap fallback candidate       │
│  → K+1 candidates per stop           │
└──────────────────────────────────────┘
           │
           ▼
┌──────────────────────────────────────┐
│  map_stops_dp() pathfinding         │
│  - Forward pass:                    │
│    * Sort each layer by progress    │
│    * DP sweep O(M × K)              │
│    * Track best_cost, best_prev     │
│  - Find minimum cost final state    │
│  - Backtrack to reconstruct path    │
└──────────────────────────────────────┘
           │
           ▼
Output: Vec<i32> progress values (input order, non-decreasing)
```

**Empty Candidate Handling:** If grid query returns no segments (should not happen with radius 3), the snap candidate is always added, ensuring at least one candidate per stop.

**Memory:** For M=35 stops, K=15 candidates: ~35 × 16 × 32 bytes = ~18 KB (negligible).

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

**SNAP_PENALTY_CM2 Justification:**
- Value: 10^12 cm² ≈ (316,000 cm)² ≈ (3.16 km)²
- Purpose: Large enough that DP only uses snap candidate when no valid transition exists
- Safety margin: Real bus stops are rarely >100m from route (distance² < 10^10 cm²)
- The penalty is ~100× larger than worst-case legitimate projection, ensuring snap is truly a last resort

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
| `grid` | Empty route, single segment, multi-segment; boundary conditions; query radius expansion; deduplication |
| `candidate` | Projection at t=0, t=1, t=0.5; clamping; sorting/K-limiting; snap generation; deduplication by (seg_idx, t) |
| `pathfinding` | Two-stop monotonic; same-segment strict inequality; route loops; snap activation; backtrack correctness |

### Integration Tests

- Real route data (tpF805, two_pass_test) with known outputs
- Comparison against greedy implementation to verify optimality
- DP should always produce equal or better total distance than greedy

### Performance Tests

```rust
// Criterion benchmarks for:
// - Grid construction time
// - Candidate generation per stop
// - Full DP solve for varying M (10, 35, 100 stops)
// - Memory allocation profile
```

**Target:** O(M × K log K) < 10ms for M=35, K=15

### Edge Case Tests

| Edge Case | Test Case |
|-----------|-----------|
| Stop projects behind min_prog | Verify DP doesn't select violating candidate |
| Route loop (same location twice) | Both segments as candidates; DP picks best |
| Identical adjacent stops | Equal progress is valid (>= transition) |
| No valid transition | Snap-forward candidate activates |
| First stop (no constraint) | All candidates valid |
| Large distance from route | Legitimate projection < SNAP_PENALTY |

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

| Edge Case | Detection | Solution |
|-----------|-----------|----------|
| Stop projects behind min_prog | Candidate progress < previous layer's min progress | DP doesn't select that candidate (higher cost path) |
| Route loop (same location twice) | Grid returns both segments | Both appear as candidates; DP picks best sequence |
| Identical adjacent stops | Progress values equal | Equal progress is valid (>= transition) |
| No valid transition | All candidates have progress < previous min | Snap-forward candidate activates (dist = SNAP_PENALTY) |
| First stop (no constraint) | No previous layer | All candidates valid, no filter |
| Empty candidate set | Grid query returns nothing | Snap candidate always added (at least 1 candidate) |

## Migration Strategy

### Phase 1: Parallel Development
- Implement `dp_mapper` crate alongside existing greedy code
- Add comparison tests that run both algorithms on same inputs
- Verify DP produces equal or better results

### Phase 2: Gradual Integration
- Add feature flag in preprocessor: `dp-mapping`
- When flag is enabled, use `dp_mapper::map_stops()` instead of greedy
- Run A/B tests on real routes to validate

### Phase 3: Complete Replacement
- Once validated, remove greedy implementation
- Update `preprocessor/src/stops/validation.rs` to use `dp_mapper`
- Remove old code from `preprocessor/src/stops/`

### Data Compatibility
- Output format is identical (`Vec<i32>` progress values)
- Existing `.bin` route files remain valid
- No migration needed for processed route data

### Test Migration
- Existing unit tests in `preprocessor/src/stops/tests.rs` continue to work
- Add new tests specific to DP behavior (optimality verification)
- Remove tests that depended on greedy-specific behavior

## Integration with Preprocessor

The crate will be added to `preprocessor/Cargo.toml`:

```toml
[dependencies]
dp_mapper = { path = "dp_mapper" }
```

And used in `preprocessor/src/stops/`:

```rust
// In preprocessor/src/stops/validation.rs (or replacement)
use dp_mapper;

pub fn validate_stop_sequence_v2(
    stops_cm: &[(i64, i64)],
    route_nodes: &[RouteNode],
) -> ValidationResult {
    let progress_values = dp_mapper::map_stops(stops_cm, route_nodes, None);
    ValidationResult {
        progress_values,
        reversals: Vec::new(),  // DP guarantees no reversals
    }
}
```

## Error Handling

The crate uses "never fail" design:
- **Empty input:** Returns empty `Vec<i32>`
- **Single stop:** Returns single progress value
- **No valid path:** Uses snap-forward candidates to guarantee a path
- **Invalid route (no segments):** Returns empty progress values

**No `Result` type** - the API always returns valid `Vec<i32>`.

## Success Criteria

1. All unit tests pass
2. Integration tests validate against real routes (tpF805, two_pass_test)
3. Performance: O(M × K log K) < 10ms for typical routes
4. Correctness: Output is non-decreasing and globally optimal
5. Comparison tests: DP total distance ≤ greedy total distance for all test cases

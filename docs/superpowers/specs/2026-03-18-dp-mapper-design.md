# DP Mapper Crate Design Specification

**Date:** 2026-03-18
**Status:** Approved
**Author:** Claude
**Reviewers:** Spec Reviewer (Agent)

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
    seg_idx: usize,         // Route segment index
    t: f64,                 // Position on segment [0, 1]
    dist_sq_cm2: i64,       // Squared distance from stop to projection (cm²)
    progress_cm: i32,       // Absolute progress along route (cm)
}
```

**Public Functions:**
```rust
// For first stop (no previous layer), snap is not needed
pub fn generate_candidates(
    stop: (i64, i64),
    route_nodes: &[RouteNode],
    grid: &SpatialGrid,
    k: usize,
) -> Vec<Candidate>;

// For subsequent stops, snap uses previous layer's maximum progress
pub fn generate_candidates_with_snap(
    stop: (i64, i64),
    route_nodes: &[RouteNode],
    grid: &SpatialGrid,
    k: usize,
    max_prev_progress_cm: i32,  // Maximum progress from previous layer
) -> Vec<Candidate>;
```

**Algorithm Details:**

1. **Grid Query:** For radii 1, 2, 3 (expanding 3×3 → 5×5 → 7×7), collect all segment indices
2. **Projection:** For each segment, project stop onto line, clamp t to [0, 1], compute squared distance and progress
3. **Deduplication:** Remove duplicates by `(seg_idx, t)` pair - same segment at same position
4. **Sort:** Sort by squared distance ascending
5. **Select:** Keep top-K candidates (by distance)
6. **Snap:** Add one snap-forward candidate as fallback (for `generate_candidates_with_snap`)

**Snap Candidate Generation Algorithm:**

```rust
// Snap candidate generation for stop j (j > 0):
// Input: max_prev_progress_cm from previous layer's candidates
//
// Purpose: Guarantee at least one candidate reachable from ALL previous candidates.
// For DP sweep transition prev.progress <= curr.progress to fire, snap must satisfy:
//   snap.progress >= max(prev.progress) for all prev in previous layer

// 1. Find first segment whose END is past max_prev_progress_cm:
let snap_seg_idx = route_nodes
    .iter()
    .position(|n| n.cum_dist_cm + n.seg_len_cm >= max_prev_progress_cm)
    .unwrap_or(route_nodes.len().saturating_sub(2));  // Default to last valid segment

// 2. Create snap candidate at segment start:
let snap_candidate = Candidate {
    seg_idx: snap_seg_idx,
    t: 0.0,                                     // At segment start
    dist_sq_cm2: SNAP_PENALTY_CM2,              // Large penalty (squared)
    progress_cm: route_nodes[snap_seg_idx].cum_dist_cm,
};
```

**Key points:**
- Using `max_prev_progress_cm` ensures snap is reachable from **every** previous candidate
- Condition `cum_dist_cm + seg_len_cm >= max_prev_progress_cm` finds segment **containing** the threshold
- Fallback `len().saturating_sub(2)` handles edge cases (returns last valid segment index)

**Key Behavior:**
- Projects stop onto all segments from grid query (3 radii)
- Clamps t to [0.0, 1.0]
- Deduplicates by `(seg_idx, t)` before sorting
- Keeps top-K by squared distance (minimum distance first)
- Adds snap-forward candidate with `SNAP_PENALTY_CM2` (only for non-first stops)

### `pathfinding` Module

**Responsibility:** Dynamic programming for optimal path finding

This module takes raw stop coordinates and orchestrates candidate generation and DP solving.

**Internal Types:**
```rust
struct DpLayer {
    candidates: Vec<Candidate>,
    best_cost: Vec<i64>,                // min cost to reach each candidate
    best_prev: Vec<Option<usize>>,      // backpointer to previous layer
}

// For sorting by progress while preserving original indices
struct SortedCandidate {
    orig_idx: usize,
    progress_cm: i32,
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

**DP Transition Rule:**

The DP sweep enforces the transition constraint directly using `<=` comparison:

```rust
// In the forward pass sweep:
while ptr < prev_sorted.len()
    && prev_sorted[ptr].progress_cm <= curr.progress_cm {
    // Include this previous candidate as valid predecessor
    ...
}
```

**Canonical transition rule:** A transition from candidate `a` (stop j-1) to candidate `b` (stop j) is valid iff `a.progress_cm <= b.progress_cm`. This allows:
- Different segments: `b.seg_idx > a.seg_idx`
- Same segment, advanced position: `b.seg_idx == a.seg_idx && b.t >= a.t` (equal progress allowed)
- Route loops: same location visited twice, different visits

**Note:** Equal progress (`>=`) is explicitly allowed to handle identical adjacent stops and route loops.

**DP Forward Pass Algorithm:**

```rust
// For each layer j from 1 to M-1:
// 1. Generate candidates for stop j (with snap)
// 2. Sort both layers by progress_cm for efficient sweep
// 3. Sweep to find minimum cost transitions

fn dp_forward_pass(
    layers: &mut Vec<DpLayer>,
) {
    for j in 1..layers.len() {
        // Sort current layer by progress, tracking original indices
        let curr_sorted: Vec<_> = layers[j].candidates
            .iter()
            .enumerate()
            .map(|(i, c)| SortedCandidate { orig_idx: i, progress_cm: c.progress_cm })
            .collect();
        curr_sorted.sort_by_key(|s| s.progress_cm);

        // Sort previous layer by progress
        let prev_sorted: Vec<_> = layers[j-1].candidates
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
            // Advance pointer: include all prev candidates with progress <= curr.progress
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
```

**Backtrack Algorithm:**

```rust
fn dp_backtrack(layers: &[DpLayer]) -> Vec<i32> {
    let m = layers.len();
    let mut result = vec![0i32; m];

    // Find best final state (minimum cost in last layer)
    let mut best_k = 0;
    let mut best_cost = i64::MAX;
    for (k, &cost) in layers[m-1].best_cost.iter().enumerate() {
        if cost < best_cost {
            best_cost = cost;
            best_k = k;
        }
    }

    // Backtrack to reconstruct path
    let mut k = best_k;
    for j in (0..m).rev() {
        result[j] = layers[j].candidates[k].progress_cm;
        if j > 0 {
            k = layers[j].best_prev[k]
                .expect("DP backtrack broken: missing predecessor for non-base layer");
        }
    }

    result
}
```

**Module Boundaries:**
- `pathfinding` calls `generate_candidates()` and `generate_candidates_with_snap()` - does not implement projection
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
│  Stop 0: generate_candidates()      │
│  - Grid query (3 radii expansion)    │
│  - Project onto segments             │
│  - Deduplicate by (seg_idx, t)       │
│  - Sort by squared distance          │
│  - Select top-K                      │
│  - NO snap (first stop)              │
│  → K candidates                      │
└──────────────────────────────────────┘
           │
           ▼
┌──────────────────────────────────────┐
│  Stop j (j > 0):                     │
│  generate_candidates_with_snap()     │
│  - Grid query (3 radii expansion)    │
│  - Project onto segments             │
│  - Deduplicate by (seg_idx, t)       │
│  - Sort by squared distance          │
│  - Select top-K                      │
│  - Add snap fallback candidate       │
│  → K+1 candidates                    │
└──────────────────────────────────────┘
           │
           ▼
┌──────────────────────────────────────┐
│  map_stops_dp() pathfinding         │
│  - Forward pass (DP sweep):          │
│    * Sort each layer by progress    │
│    * For each candidate in layer j:  │
│      - Find valid transitions from j-1│
│      - Track best_cost, best_prev   │
│  - Find minimum cost final state    │
│  - Backtrack to reconstruct path    │
└──────────────────────────────────────┘
           │
           ▼
Output: Vec<i32> progress values (input order, non-decreasing)
```

**Empty Candidate Handling:** If grid query returns no segments (should not happen with radius 3), the snap candidate is always added for j > 0, ensuring at least one candidate per non-first stop. For the first stop, an empty result is an error condition.

**Memory:** For M=35 stops, K=15 candidates: ~35 × 16 × (32 + 16) bytes = ~27 KB (negligible).

## Constants

```rust
// Candidate generation
const DEFAULT_K: usize = 15;
const GRID_RADIUS_MAX: u32 = 3;  // 3×3, 5×5, 7×7 expansion
const GRID_SIZE_CM: i32 = 10000; // 100m cells

// Snap fallback (squared distance units)
const SNAP_PENALTY_CM2: i64 = 1_000_000_000_000; // ~316 km² penalty

// Deduplication
const MAX_CANDIDATES: usize = 100; // cap before sort to prevent explosion
```

**SNAP_PENALTY_CM2 Justification:**
- Value: 10^12 cm² ≈ (316,000 cm)² ≈ (3.16 km)²
- Purpose: Large enough that DP only uses snap candidate when no valid transition exists
- Safety margin: Real bus stops are rarely >100m from route (distance² < 10^10 cm²)
- The penalty is ~100× larger than worst-case legitimate projection, ensuring snap is truly a last resort
- All distance costs use squared units (cm²) for consistency

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
| `pathfinding` | Two-stop monotonic; same-segment equality allowed; route loops; snap activation; backtrack correctness |

### Integration Tests

- Real route data (tpF805, two_pass_test) with known outputs
- Comparison against greedy implementation to verify optimality
- **Expected results:**
  - Simple routes: DP produces identical or similar results to greedy
  - Complex routes (loops, dense stops): DP produces lower total distance
  - All routes: DP progress values are non-decreasing (greedy may have reversals)

### Performance Tests

```rust
// Criterion benchmarks for:
// - Grid construction time (baseline)
// - Candidate generation per stop (with and without snap)
// - Full DP solve for varying M: M=10 (small), M=35 (typical), M=100 (large)
// - Comparison benchmark: greedy vs DP for same inputs
// - Memory allocation profile
```

**Performance Targets:**
- **Typical route:** M=35 stops, K=15 candidates < 10ms total
- **Large route:** M=100 stops, K=15 candidates < 30ms total
- **Small route:** M=10 stops, K=15 candidates < 5ms total

**Definition of "typical":** Based on Taipei bus routes, most routes have 20-50 stops. M=35 is the median value.

### Edge Case Tests

| Edge Case | Test Case |
|-----------|-----------|
| Stop projects behind min_progress | Verify DP doesn't select violating candidate |
| Route loop (same location twice) | Both segments as candidates; DP picks best |
| Identical adjacent stops | Equal progress is valid (<= transition) |
| No valid transition | Snap-forward candidate activates |
| First stop (no constraint) | All candidates valid, no snap |
| Large distance from route | Legitimate projection < SNAP_PENALTY |
| Snap candidate selection | Verify snap progress >= max_prev_progress |
| Snap reachability | Verify snap reachable from ALL previous candidates |
| Backtrack at j=0 | Verify no unwrap_or panic at base layer |

## Complexity

| Phase | Cost |
|-------|------|
| Grid build | O(N) where N = route nodes |
| Candidate generation | O(M × K) projections |
| Per-layer sort | O(M × K log K) |
| DP sweep | O(M × K) with running minimum |
| Backtrack | O(M) |
| **Total** | **O(M × K log K)** |

With M=35 stops, K=15: ~525 projections + trivial sort.

## Edge Cases Handled

| Edge Case | Detection | Solution |
|-----------|-----------|----------|
| Stop projects behind min_progress | Candidate progress < previous layer's min progress | DP doesn't select that candidate (higher cost path) |
| Route loop (same location twice) | Grid returns both segments | Both appear as candidates; DP picks best sequence |
| Identical adjacent stops | Progress values equal | Equal progress is valid (<= transition) |
| No valid transition | All candidates have progress < previous max_progress | Snap-forward candidate activates (dist = SNAP_PENALTY) |
| First stop (no constraint) | No previous layer | All candidates valid, no snap |
| Empty candidate set (first stop) | Grid query returns nothing | Error: return empty progress values |
| Empty candidate set (j > 0) | Grid query returns nothing | Snap candidate always added (at least 1 candidate) |
| Snap unreachable (old bug) | Snap anchored at min_prev_progress | Fixed: anchor at max_prev_progress ensures reachability |

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
- **First stop with no candidates:** Returns empty progress values (error condition)

**No `Result` type** - the API always returns valid `Vec<i32>`. Callers should check that the result length equals the input length to detect errors.

## Success Criteria

1. All unit tests pass
2. Integration tests validate against real routes (tpF805, two_pass_test)
3. Performance: < 10ms for typical routes (M=35, K=15)
4. Correctness: Output is non-decreasing and globally optimal
5. Comparison tests: DP total distance ≤ greedy total distance for all test cases
6. Snap activation: Snap candidates only used when no valid transition exists
7. Snap reachability: Snap candidate always reachable from all previous candidates

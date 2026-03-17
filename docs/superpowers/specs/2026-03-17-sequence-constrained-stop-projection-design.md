# Sequence-Constrained Stop Projection Design

**Date:** 2026-03-17
**Status:** Draft
**Author:** Claude
**Related:** Tech Report v8.3, Section 17 (離線預處理流程)

## Problem Statement

The current preprocessor projects stops onto the route and then sorts by progress, which loses the input order from `stops.json`. This can cause issues when RDP simplification creates geometry that causes stop sequence reversal.

The updated tech report (Section 17) specifies:
- **Step 6 (moved earlier)**: Build Spatial Grid Index BEFORE stop projection
- **Step 7 (new)**: Sequence-constrained stop projection with:
  1. Grid-assisted O(k) local segment search
  2. Path constraint: search starts from previous stop's matched node
  3. Monotonicity validation: ensure $s_{stop,j} > s_{stop,j-1}$ in INPUT order
  4. Automatic RDP tolerance reduction if reversal detected

## Design Overview

**Approach:** Two-Stage Projection with Monotonicity Validation

1. **Quick validation pass**: Project all stops using path-constrained grid search
2. **Monotonicity check**: Verify progress strictly increases by input order
3. **On reversal**: Re-simplify affected region with reduced epsilon (binary search)
4. **Full processing**: After validation, compute corridors with overlap protection

```
New Pipeline (with retry loop):
┌─────────────────────────────────────────────────────────────┐
│ 1-3. Parse, coordinate conversion, protection identification │
│ 4. Simplify route (may retry)                               │
│ 5. Linearize route                                           │
│ 6. Build Spatial Grid Index                                  │
│ 7a. Validation pass (path-constrained, grid-assisted)        │
│     → If non-monotonic: re-simplify, retry                  │
│ 7b. Full projection with corridors                           │
│ 8-9. Generate LUTs, pack binary                              │
└─────────────────────────────────────────────────────────────┘
```

## Data Structures

### In `stops.rs`:

```rust
/// Result of quick validation pass
pub struct ValidationResult {
    /// Validated stop progress values in input order
    pub progress_values: Vec<i32>,
    /// If validation failed, contains info for retry
    pub reversal_info: Option<ReversalInfo>,
}

/// Information about a detected sequence reversal
pub struct ReversalInfo {
    /// Index in stops array where reversal was detected
    pub stop_index: usize,
    /// The problematic progress value (smaller than previous)
    pub problem_progress: i32,
    /// Previous stop's progress (larger)
    pub previous_progress: i32,
    /// Approximate route region to re-simplify (node indices)
    pub affected_region: (usize, usize),
    /// Suggested epsilon for retry (binary search step)
    pub suggested_epsilon: f64,
}
```

### Modified APIs

```rust
// NEW: Quick validation pass with path-constrained search
pub fn validate_stop_sequence(
    stops_cm: &[(i64, i64)],
    route_nodes: &[RouteNode],
    grid: &SpatialGrid,
) -> ValidationResult

// MODIFIED: Now assumes validated input (progress in input order)
pub fn project_stops_validated(
    progress_values: &[i32],
) -> Vec<Stop>

// NEW: Find closest segment with path constraint + progressive window
fn find_closest_segment_constrained(
    point: &(i64, i64),
    nodes: &[RouteNode],
    grid: &SpatialGrid,
    min_node_idx: usize,
) -> (usize, f64)
```

### In `simplify.rs`:

```rust
// NEW: Re-simplify a specific region with lower tolerance
pub fn refine_region(
    full_route: &[(i64, i64)],
    simplified: &[(i64, i64)],
    region: (usize, usize),
    epsilon_cm: f64,
    protected_indices: &[usize],
) -> Vec<(i64, i64)>

fn find_closest_in_full(
    full_route: &[(i64, i64)],
    target: (i64, i64),
) -> usize
```

## Algorithm Details

### Path-Constrained Grid Search

For each stop in input order:

1. Start with `min_node_idx = 0` (or previous stop's segment)
2. Perform grid search around stop's position:
   - Try 3×3 cell window
   - Filter to segments with index ≥ `min_node_idx`
   - If no valid candidates, expand to 5×5
   - If still none, expand to 7×7
   - Fallback: linear search from `min_node_idx` to end
3. Find closest segment, compute progress
4. Validate: `progress > previous_progress`
5. Update `min_node_idx` for next stop

### Binary Search Epsilon Reduction

On reversal detection:
- Initial epsilon: 700 cm
- Binary search: 700 → 350 → 175 → 87.5
- Minimum threshold: 100 cm (below this = error)
- Region: estimate based on affected nodes

### Region Refinement

1. Map simplified region indices back to full route indices
2. Extract region with 10-point buffer
3. Identify protected points in region
4. Re-simplify with new epsilon
5. Stitch back into full simplified route

## Error Handling

| Case | Handling |
|------|----------|
| Empty stops array | Early error: "No stops provided" |
| Single stop | Skip validation, project directly |
| Duplicate progress | Treat as reversal (re-simplify) |
| Grid search fails | Fallback to linear search |
| Epsilon < 100cm | Error with diagnostic info |
| Near-duplicate (<100cm) | Warning, continue |

## Validation Logging

```
[VALIDATION PASS]
Validating stop sequence...
  Stop 001 (忠義路): progress=12,340 cm (node 42)
  Stop 002 (中山路): progress=45,678 cm (node 187)
  ...
✓ All 23 stops validated - monotonic sequence confirmed

[OR ON REVERSAL]
! Reversal detected at stop 15: progress=876,543 < previous_progress=901,234 cm
  Affected region: nodes 450-520
  Re-simplifying with ε=350 cm
```

## Implementation Phases

1. **Phase 1**: Core validation logic (`stops.rs`)
2. **Phase 2**: Simplification refinement (`simplify.rs`)
3. **Phase 3**: Pipeline integration (`main.rs`)
4. **Phase 4**: Testing and validation
5. **Phase 5**: Documentation

## Testing Strategy

- Unit tests for `validate_stop_sequence()`
- Unit tests for path constraint enforcement
- Unit tests for grid search fallback
- Integration test with synthetic reversal case
- Verification on real route data (ty225)

## Impact Assessment

| Component | Changes |
|-----------|---------|
| `stops.rs` | Add validation, modify projection API |
| `simplify.rs` | Add region refinement |
| `main.rs` | Add retry loop |
| `shared/` | No changes |
| Binary format | No changes (v8.3) |

**Benefits:**
- ✅ Ensures stop sequence matches input order
- ✅ Grid-assisted O(k) performance
- ✅ Handles RDP-induced reversals
- ✅ Clear error diagnostics

**Trade-offs:**
- ⚠️ May require multiple preprocessing passes
- ⚠️ Added complexity in validation logic

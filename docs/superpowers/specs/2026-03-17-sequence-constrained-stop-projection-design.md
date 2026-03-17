# Sequence-Constrained Stop Projection Design

**Date:** 2026-03-17
**Status:** Draft (Under Review)
**Author:** Claude
**Related:** Tech Report v8.3, Section 17 (離線預處理流程)

## Migration Note

This design describes changes to the current preprocessor pipeline. The existing code (as of 2026-03-17) does NOT implement sequence-constrained projection yet. This spec defines the target behavior.

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
    /// Approximate route region to re-simplify (segment indices in simplified route)
    pub affected_region: (usize, usize),
    /// Suggested epsilon for retry (binary search step)
    pub suggested_epsilon: f64,
    /// Retry attempt count (for tracking in main loop)
    pub retry_count: u32,
}
```

**Note:** Stop names/metadata from `stops.json` are accessed by index from the original `stops_input.stops` array in `main.rs`. The validation pass preserves input order by returning progress values in that same order, so `stops_input.stops[i]` corresponds to `progress_values[i]`.

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
    progress_values: &[i32],  // Already in input order
    stops_input: &StopsInput,  // For stop names in logging
) -> Vec<Stop>

// NEW: Find closest segment with path constraint + progressive window
// Returns: (segment_index, t_value)
fn find_closest_segment_constrained(
    point: &(i64, i64),
    nodes: &[RouteNode],
    grid: &SpatialGrid,
    min_segment_idx: usize,  // Path constraint: only search segments >= this index
) -> (usize, f64)
```

**Important:** The grid stores segment indices (0 to N-1 for N segments in N+1 nodes). When we say `min_segment_idx`, we refer to the segment index, not the node index. A segment at index `i` connects nodes `i` and `i+1`.

### In `simplify.rs`:

```rust
// No new API needed - use existing simplify_and_interpolate()
// The retry loop in main.rs simply calls it with a different epsilon
```

## Algorithm Details

### Path-Constrained Grid Search

For each stop in input order:

1. Start with `min_segment_idx = 0` (or previous stop's matched segment)
2. Perform grid search around stop's position:
   - Try 3×3 cell window centered on stop's grid cell
   - Filter to segments with index ≥ `min_segment_idx`
   - **Trigger for expansion**: If NO candidates satisfy the min_segment_idx constraint, expand window
   - Expand to 5×5 cells
   - If still no valid candidates, expand to 7×7 cells
   - Fallback: linear search from `min_segment_idx` to end of route
3. Find closest segment among valid candidates, compute progress
4. Validate: `progress > previous_progress`
5. Update `min_segment_idx` for next stop

**Pseudocode for window expansion:**
```rust
let mut window_size = 1; // 3x3 = radius 1
let candidates = grid.query_radius(stop_cell, window_size)
    .into_iter()
    .filter(|&seg_idx| seg_idx >= min_segment_idx)
    .collect::<Vec<_>>();

while candidates.is_empty() && window_size <= 3 {
    window_size += 1; // Expand to 5x5, then 7x7
    candidates = grid.query_radius(stop_cell, window_size)
        .into_iter()
        .filter(|&seg_idx| seg_idx >= min_segment_idx)
        .collect::<Vec<_>>();
}

if candidates.is_empty() {
    // Fallback: linear search
    candidates = (min_segment_idx..route_nodes.len()-1).collect();
}
```

### Binary Search Epsilon Reduction

On reversal detection:
- Initial epsilon: 700 cm
- Binary search: 700 → 350 → 175 → 87.5
- **Maximum retries: 3 attempts**
- Minimum threshold: 100 cm (below this = error with diagnostic info)
- Region: estimate based on affected segment indices

**Termination conditions:**
1. **Success**: Validation passes with monotonic sequence
2. **Failure after 3 retries**: Emit error message with:
   - Stop indices causing reversal
   - Progress values showing the problem
   - Suggestion to verify route geometry or stops.json order
   - Exit with error code 1

**Error message on persistent reversal:**
```
ERROR: Stop sequence reversal persists after 3 refinement attempts
  Problem occurs at stop 15: progress=876,543 cm < previous_progress=901,234 cm
  This usually indicates:
    1. Input stop order does not match actual route geometry
    2. Route has self-intersection or loop-back that conflicts with stop order
  Please verify stops.json order matches the actual bus route direction
```

### Region Refinement

**Challenge:** The current `simplify_and_interpolate()` doesn't track the mapping between simplified points and original route indices. We need to reverse this mapping.

**Approach:**
1. **Map simplified region to full route**: For `simplified[region_start]` and `simplified[region_end]`, find their closest matches in `full_route` using Euclidean distance
2. **Extract region with buffer**: Add 10-point buffer on each side for context
3. **Identify protected points**: Filter `protected_indices` to those within the extracted region
4. **Re-simplify**: Run `simplify_and_interpolate()` on the extracted region with new epsilon
5. **Stitch**: Replace the region in `simplified` with the refined version

**Alternative (simpler) approach:** Instead of region-based refinement, re-simplify the ENTIRE route with the reduced epsilon. This is less efficient but avoids the index mapping complexity entirely. Given that RDP is fast (< 100ms for typical routes), the simpler approach may be preferable.

**Decision:** Start with full-route re-simplification (simpler). If performance becomes an issue, optimize to region-based refinement later.

## Error Handling

| Case | Handling |
|------|----------|
| Empty stops array | Early error: "No stops provided" |
| Single stop | Skip validation, project directly |
| Duplicate progress | Treat as reversal (re-simplify) |
| Grid search fails | Fallback to linear search |
| 3 failed retries | Error with diagnostic info (see below) |
| Near-duplicate (<100cm) | Warning, continue |

## Performance Budget

| Operation | Target | Notes |
|-----------|--------|-------|
| Validation pass | < 100ms | For ~50 stops, grid-assisted O(k) search |
| Full re-simplification | < 200ms | RDP is fast; typical route < 1000 points |
| Grid search (3×3) | < 5ms per stop | O(k) where k ≈ 5-15 segments |
| Fallback linear search | < 20ms per stop | Only when grid search fails |

**Worst case (3 retries):** < 1 second total preprocessing time (acceptable for offline use)

## Validation Logging

**Note:** Stop names are accessed via `stops_input.stops[i].name` (or similar field) from the original input. The validation function receives only coordinates, so logging happens in `main.rs` where both the validation result and original stops data are available.

```
[VALIDATION PASS]
Validating stop sequence...
  Stop 001 (忠義路): progress=12,340 cm (segment 42)
  Stop 002 (中山路): progress=45,678 cm (segment 187)
  ...
✓ All 23 stops validated - monotonic sequence confirmed

[OR ON REVERSAL]
! Reversal detected at stop 15 (火車站): progress=876,543 < previous_progress=901,234 cm
  Affected segments: 450-520
  Re-simplifying with ε=350 cm (attempt 1/3)
```

## Implementation Phases

1. **Phase 1**: Core validation logic (`stops.rs`)
   - Add `ValidationResult` and `ReversalInfo` types
   - Implement `validate_stop_sequence()` with path-constrained search
   - Implement `find_closest_segment_constrained()` with progressive window expansion
   - Modify `project_stops()` → `project_stops_validated()`
   - Add unit tests

2. **Phase 2**: Pipeline integration (`main.rs`)
   - Add retry loop with binary search epsilon reduction
   - Integrate validation logging
   - Update version to v8.3

3. **Phase 3**: Testing and validation
   - Create synthetic reversal test case
   - Verify retry loop works correctly
   - Test on real route data (ty225)

4. **Phase 4**: Documentation
   - Update tech report v8.3 if needed
   - Add inline code documentation

## Testing Strategy

### Unit Tests

**`validate_stop_sequence()` tests:**
- Valid monotonic sequence (all stops in order)
- Single stop (edge case, should skip validation)
- Duplicate progress values (treated as reversal)
- Path constraint enforcement (stop N+1 can't match segments before stop N)

**`find_closest_segment_constrained()` tests:**
- Grid search finds valid segment in 3×3 window
- Progressive expansion (3×3 → 5×5 → 7×7 → fallback)
- Linear search fallback when grid has no valid candidates

**Edge cases:**
- Stops at exactly the same location (duplicate detection)
- Stops < 100cm apart (near-duplicate warning)
- Route loops where geometric distance doesn't match route progress
- Large route variations (>1km between consecutive stops)

### Integration Test

Create a synthetic reversal test case:
```
test-data/reversal_test/
├── route.json    # Curved route where RDP shortcuts cause reversal
├── stops.json    # Stops in order along the curve
└── expected.bin  # Expected output after refinement
```

### Verification

- Run preprocessor on ty225 route
- Verify no regressions (existing routes still work)
- Verify validation logs are informative

## Impact Assessment

| Component | Changes |
|-----------|---------|
| `stops.rs` | Add `validate_stop_sequence()`, modify `project_stops()` → `project_stops_validated()` |
| `simplify.rs` | No API changes (use existing `simplify_and_interpolate()` with different epsilon) |
| `main.rs` | Add retry loop with binary search epsilon reduction |
| `shared/` | No changes |
| Binary format | No changes (v8.3) |

**Benefits:**
- ✅ Ensures stop sequence matches input order
- ✅ Grid-assisted O(k) performance
- ✅ Handles RDP-induced reversals
- ✅ Clear error diagnostics

**Trade-offs:**
- ⚠️ May require multiple preprocessing passes (up to 3)
- ⚠️ Added complexity in validation logic
- ⚠️ Worst-case preprocessing time increases (but still < 1 second)

## Additional Implementation Notes

### Corridor Computation Timing

Corridors are computed ONLY AFTER validation passes. This avoids wasting work on potentially invalid sequences. The `project_stops_validated()` function receives pre-validated progress values and computes corridors without needing to re-project.

### Input Data Format

The `stops.json` file format (from current `input.rs`):
```json
{
  "stops": [
    {
      "lat": 25.047324,
      "lon": 121.517234,
      "name": "忠義路"
    },
    ...
  ]
}
```

Stop names are optional and used only for logging/debugging. The validation logic works purely with coordinates.

# Sequence-Constrained Stop Projection: Technical Note

**Date:** 2026-03-18
**Version:** 1.0
**Author:** Claude (with human direction)
**Related:** Tech Report v8.3 Section 17, Implementation Plan 2026-03-17

## Executive Summary

This document describes the implementation of **sequence-constrained stop projection** for the bus arrival detection system. The algorithm ensures that stop order from `stops.json` is preserved after RDP simplification, even for routes with loops or backtracking. A two-phase approach combining grid-assisted projection with path-aware ordering correctly handles cases where the route passes through the same location multiple times.

## Problem Statement

### Original Issue

The preprocessor previously projected stops onto the route and then **sorted by progress**, which lost the input order from `stops.json`. This caused issues when:

1. **RDP simplification** created geometry that caused stop sequence reversal
2. **Loop routes** passed through the same location multiple times
3. **Route backtracking** caused later stops to appear "before" earlier stops geometrically

### Example of the Problem

Given a route that loops back:
```
Route: A → B → C → D → C → E
Stops input: [A, C, C, E]  (C appears twice!)
```

After projection and sorting:
```
Legacy output: [A@progress0, C@progress3, C@progress3, E@progress4]
                 ↑ Same progress! Input order lost
```

## Solution: Two-Phase Algorithm

### Phase 1: Grid-Assisted Projection

**Purpose:** Find ALL route segments near a stop location efficiently.

**Algorithm:**
1. Build a 100m × 100m spatial grid index over route nodes
2. For each stop, perform progressive window expansion:
   - 3×3 cells (radius 1)
   - 5×5 cells (radius 2)
   - 7×7 cells (radius 3)
   - Fallback: linear search from `min_segment_idx`
3. Grid returns ALL segments that pass through each cell

**Key insight:** When a route loops back and passes through the same location twice, **both passes** are represented in the grid. A single grid cell may contain segments from both the first pass (e.g., segment 16) and second pass (e.g., segment 64).

### Phase 2: Path-Aware Ordering

**Purpose:** Select the correct segment from candidates while preserving input order.

**Algorithm:**
1. Maintain `min_segment_idx` tracking the previous stop's matched segment
2. Filter grid candidates: keep only segments where `idx >= min_segment_idx`
3. From filtered candidates, select the closest by geometric distance
4. Update `min_segment_idx` for next iteration

**Monotonicity validation:**
- Progress must strictly increase: `progress_j > progress_{j-1}`
- If violation detected: trigger RDP re-simplification with reduced epsilon

## Handling Duplicate Locations

### Challenge

When stops #21 and #23 are at identical coordinates:
```
Stop #21: (25.120060, 121.861600)
Stop #22: (25.123341, 121.861214)  ← north
Stop #23: (25.120060, 121.861600)  ← same as #21!
```

Expected behavior:
```
Stop #21 → segment 500 (first time at this location)
Stop #22 → segment 550 (continues north)
Stop #23 → segment 700 (second time at this location, AFTER #22)
```

### How the Algorithm Solves It

**For Stop #21:**
1. Grid search near location L finds segments: [498, 499, 500, 501]
2. Path constraint (min_segment_idx = 0): all pass
3. Pick closest: **segment 500**

**For Stop #22:**
1. Grid search near location (25.123341, 121.861214) finds: [548, 549, 550, 551]
2. Path constraint (min_segment_idx = 500): all pass
3. Pick closest: **segment 550**

**For Stop #23:**
1. Grid search near location L (same as #21) finds: **[498, 499, 500, 501, 698, 699, 700, 701]**
   - First pass segments: 498, 499, 500, 501
   - Second pass segments: 698, 699, 700, 701
2. Path constraint (min_segment_idx = 550): **[698, 699, 700, 701]** (first pass filtered out!)
3. Pick closest: **segment 700** (or closest from second pass)

**Result:** Stop #23 correctly maps to segment 700, NOT segment 500, even though segment 500 is geometrically closer to the location.

## Validation and Retry Mechanism

### Validation Flow

```rust
for each stop in input order:
    candidates = grid_search(stop_location, radius)
    candidates = filter(candidates, idx >= min_segment_idx)
    if candidates.is_empty():
        expand_search_radius()  // 3×3 → 5×5 → 7×7 → linear

    segment, t = find_closest(candidates)
    progress = compute_progress(segment, t)

    if progress <= previous_progress:
        // Reversal detected!
        return ValidationResult {
            reversal_info: ReversalInfo {
                stop_index: i,
                problem_progress: progress,
                previous_progress: previous_progress,
                suggested_epsilon: 350.0,  // Binary search: 700 → 350
                retry_count: 0,
            }
        }

    min_segment_idx = segment
```

### Binary Search Epsilon Reduction

When reversal detected:
1. Initial epsilon: 700 cm (7m)
2. Binary search: 700 → 350 → 175 → 87.5
3. Maximum retries: 3 attempts
4. Minimum threshold: 100 cm

**Re-simplification:**
```python
epsilon_current = 700.0
for retry in 0..3:
    validation = validate_stop_sequence(stops, route, grid)
    if validation.reversal_info is None:
        break  # Success!

    epsilon_current /= 2.0
    route = simplify_and_interpolate(original_route, epsilon_current, protected_points)
    grid = build_grid(route)

# After 3 failed retries:
ERROR: Stop sequence reversal persists after 3 attempts
  Please verify stops.json matches the actual bus route direction
```

## Test Results

### Two-Pass Route Test

**Route definition:**
```json
{
  "route_points": [
    [25.000, 121.000],  [25.001, 121.001],  [25.002, 121.002],
    [25.003, 121.003],  [25.004, 121.004],  [25.005, 121.005],
    [25.004, 121.004],  [25.003, 121.003],  [25.002, 121.002],
    [25.006, 121.006],  [25.007, 121.007]
  ]
}
```
Route goes: forward → back → forward (passes through 25.002 twice)

**Stops input:**
```json
{
  "stops": [
    {"lat": 25.001, "lon": 121.001},
    {"lat": 25.002, "lon": 121.002},  ← First pass
    {"lat": 25.004, "lon": 121.004},
    {"lat": 25.002, "lon": 121.002},  ← Second pass (same location!)
    {"lat": 25.006, "lon": 121.006}
  ]
}
```

**Validation output:**
```
[VALIDATION PASS]
  Stop 001: progress=15005 cm
  Stop 002: progress=30011 cm   ← First pass through (25.002, 121.002)
  Stop 003: progress=60022 cm   ← Furthest north
  Stop 004: progress=120043 cm  ← Second pass through (25.002, 121.002)
  Stop 005: progress=180064 cm
✓ All 5 stops validated - monotonic sequence confirmed
```

**Segment mappings:**
```
Stop 1: (25.001, 121.001) → segment   8,  progress  15,005 cm
Stop 2: (25.002, 121.002) → segment  16,  progress  30,011 cm  ← 1st pass
Stop 3: (25.004, 121.004) → segment  32,  progress  60,022 cm
Stop 4: (25.002, 121.002) → segment  64,  progress 120,043 cm  ← 2nd pass!
Stop 5: (25.006, 121.006) → segment  96,  progress 180,064 cm
```

**Key result:** Stop 4 maps to **segment 64** (second pass), NOT segment 16 (first pass), even though both segments are geometrically near (25.002, 121.002). The path constraint correctly selected the second-pass segment.

## Real-World Case: tpF805 Route

### Route Characteristics

tpF805 is a **loop route** with legitimate backtracking:
- 35 stops over 537 route points
- Stops #21 and #23 at identical coordinates
- Complex geometry with route sections that revisit the same area

### Validation Result

```
! Reversal at stop 9: 603096 < 603566 cm
  Retrying with ε=350 cm (attempt 1/3)
! Reversal at stop 22: 2195321 < 2198210 cm
  Retrying with ε=175 cm (attempt 2/3)
ERROR: Reversal persists after 3 attempts
  At stop 22: 2196314 < 2199203 cm
```

### Analysis

The validation **correctly detected** that tpF805's stop order does not produce a monotonically increasing progress along the simplified route geometry. This indicates:

1. **Route geometry has complex loops** that cannot be simplified to a linear progression
2. **RDP simplification at ε=175cm** still preserves geometric complexity that causes reversals
3. **Input stop order may not perfectly align** with the actual route traversal direction

### Solution for Loop Routes

Routes like tpF805 with legitimate backtracking will fail validation. This indicates that:

1. The route geometry has complex loops that cannot be simplified to a linear progression
2. The input stop order does not produce a monotonically increasing progress along the route

For such routes, consider:
- Reviewing and correcting the stop order in `stops.json` to match the actual route traversal
- Using a different preprocessing approach if the route truly requires non-monotonic stop ordering

The preprocessor does not provide a bypass for this validation because doing so would produce incorrect stop ordering (stops sorted by geometric distance rather than input order).

## Implementation Details

### Data Structures

```rust
pub struct ValidationResult {
    /// Validated stop progress values in input order
    pub progress_values: Vec<i32>,
    /// If validation failed, contains info for retry
    pub reversal_info: Option<ReversalInfo>,
}

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

### Key Functions

**Grid search with progressive expansion:**
```rust
fn query_grid_radius(
    grid: &SpatialGrid,
    x_cm: i64, y_cm: i64,
    radius: usize,  // 1 = 3×3, 2 = 5×5, 3 = 7×7 cells
) -> Vec<usize>
```

**Path-constrained segment search:**
```rust
fn find_closest_segment_constrained(
    point: &(i64, i64),
    nodes: &[RouteNode],
    grid: &SpatialGrid,
    min_segment_idx: usize,  // Enforces monotonicity
) -> (usize, f64)  // (segment_index, t_value)
```

**Main validation:**
```rust
pub fn validate_stop_sequence(
    stops_cm: &[(i64, i64)],
    route_nodes: &[RouteNode],
    grid: &SpatialGrid,
) -> ValidationResult
```

## Performance Budget

| Operation | Target | Notes |
|-----------|--------|-------|
| Grid build | < 50ms | One-time per route |
| Grid search (3×3) | < 5ms per stop | O(k) where k ≈ 5-15 segments |
| Progressive expansion | < 10ms | 3×3 → 5×5 → 7×7 |
| Linear fallback | < 20ms per stop | Only when grid search fails |
| Validation pass | < 100ms | For ~50 stops total |
| Full re-simplification | < 200ms | RDP is fast for typical routes |
| **Worst case (3 retries)** | < 1 second | Acceptable for offline use |

## Limitations and Trade-offs

### What Works

✅ **Linear routes:** Perfect monotonic projection
✅ **Simple loops:** Two-pass routes handled correctly
✅ **Moderate backtracking:** Grid search + path constraint works

### What Fails Validation

⚠️ **Complex loop routes** (like tpF805): Route geometry too complex for monotonic projection
⚠️ **Stop order mismatches:** Input order doesn't match actual route traversal

For these cases, the preprocessor will fail with a clear error message indicating which stop caused the reversal and the problematic progress values. Users should review their route data and stop order to resolve the issue.

### Design Trade-offs

**Chose:** Single-pass validation (no retry loop)
**Alternative considered:** Binary search epsilon reduction (700 → 350 → 175 cm)
**Reasoning:** Retry loop added complexity without effectively resolving the underlying geometric issues

**Chose:** 100m grid cells
**Alternative considered:** Smaller cells (50m, 25m)
**Reasoning:** Balance between accuracy and memory usage

## Future Improvements

1. **Stop name/ID tracking:** Use stop names to identify when the same stop is visited multiple times
2. **Adaptive grid sizing:** Use smaller grid cells for complex route sections
3. **Region-specific epsilon:** Apply different RDP tolerance to different route regions
4. **Machine learning:** Train model to predict optimal epsilon for given route geometry

## Conclusion

The sequence-constrained stop projection implementation successfully:

1. ✅ Preserves input stop order for standard linear routes
2. ✅ Correctly handles routes that revisit the same location (two-pass test confirmed)
3. ✅ Provides clear error diagnostics for problematic routes
4. ✅ Fails fast with specific error location when validation cannot succeed

The two-phase algorithm (Grid-Assisted + Path-Aware) is the key innovation that enables correct handling of duplicate locations while maintaining O(k) performance through spatial indexing.

## References

- Tech Report v8.3, Section 17: 離線預處理流程
- Implementation Plan: 2026-03-17-sequence-constrained-stop-projection.md
- Design Spec: 2026-03-17-sequence-constrained-stop-projection-design.md

# Stop-Segment Mapping Problem: Technical Note

**Date:** 2026-03-18
**Version:** 1.0
**Author:** Claude (with human direction)
**Related:** Tech Report v8.3 Section 17, Sequence-Constrained Stop Projection

## Executive Summary

This document describes the solution to a critical bug in the stop-segment mapping algorithm where stops could map to earlier progress values on the same segment, violating monotonicity constraints. The fix involves tracking both segment index and t-value (position along segment) to enforce strict forward progression.

## Problem Statement

### Original Issue

The sequence-constrained stop projection algorithm was designed to ensure stops map to monotonically increasing progress values. However, a bug in the t-constraint logic allowed stops to map to earlier positions on the same segment, causing progress reversals.

### Symptom

When processing the tpF805 route (35 stops), the validation failed at stop 10:

```
Stop 009: segment=282, t=0.2809, progress=603566 cm
Stop 010: segment=282, t=0.0000, progress=603096 cm  ← Reversal!
```

Both stops matched the **same segment 282**, but stop 10 mapped to t=0.0 (start of segment) while stop 9 was at t=0.2809. This caused stop 10's progress (603096 cm) to be **less** than stop 9's progress (603566 cm).

### Root Cause

The bug was in the path constraint update logic:

```rust
// BUGGY CODE
if seg_idx > min_segment_idx {
    min_segment_idx = seg_idx;
    min_t = None;  // ← Wrong! Loses t-constraint
} else {
    min_t = Some(t);
}
```

When a stop matched a new segment (e.g., stop 9 → segment 282), `min_t` was set to `None`. This meant the next stop could match anywhere on segment 282, including **before** stop 9's position.

## Solution: T-Constraint

### The Fix

The path constraint must enforce two conditions simultaneously:
1. **Segment constraint**: `seg_idx >= min_segment_idx` (can't go to earlier segments)
2. **T-constraint**: If `seg_idx == min_segment_idx`, then `t > min_t` (must move forward on same segment)

The corrected update logic:

```rust
// FIXED CODE
if seg_idx > min_segment_idx {
    min_segment_idx = seg_idx;
    min_t = Some(t);  // ← Keep t-constraint for this segment
} else {
    min_t = Some(t);  // ← Update t-constraint (same segment)
}
```

Now `min_t` is **always set** after the first stop, ensuring the next stop can't go backwards on the same segment.

### Constraint Logic

For stop N matched to `(segment=S, t=T)`:

| Condition | Valid for Stop N+1? | Reason |
|-----------|---------------------|--------|
| `segment > S` | ✅ Yes (any t) | Moved to later segment |
| `segment == S` AND `t > T` | ✅ Yes | Moved forward on same segment |
| `segment == S` AND `t <= T` | ❌ No | Would go backwards |
| `segment < S` | ❌ No | Would go to earlier segment |

### Handling Same-Location Stops

For stops at segment boundaries with equal progress:

```
Stop 029: segment=1156, t=1.0000, progress=2340855 cm
Stop 030: segment=1157, t=0.0000, progress=2340855 cm  ← Equal!
```

- Stop 29 at t=1.0: `progress = cum_dist[1156] + 1.0 × seg_len = cum_dist[1157]`
- Stop 30 at t=0.0: `progress = cum_dist[1157] + 0.0 × seg_len = cum_dist[1157]`

Both have equal progress because they're at the same physical location (end of segment 1156 = start of segment 1157). The validation allows this with `<` comparison (not `<=`).

## Implementation Details

### Data Structures

```rust
pub struct ValidationResult {
    /// Validated stop progress values in input order
    pub progress_values: Vec<i32>,
    /// Segment index for each stop (for debugging)
    pub segment_indices: Vec<usize>,
    /// T-value (0.0-1.0) for each stop (for debugging)
    pub t_values: Vec<f64>,
    /// If validation failed, contains info for diagnostics
    pub reversal_info: Option<ReversalInfo>,
}
```

### Core Algorithm

```rust
fn find_closest_segment_constrained(
    point: &(i64, i64),
    nodes: &[RouteNode],
    grid: &SpatialGrid,
    min_segment_idx: usize,
    min_t: Option<f64>,
) -> (usize, f64) {
    // Check if candidate (seg_idx, t) satisfies path constraint
    let is_valid = |seg_idx: usize, t: f64| -> bool {
        if seg_idx < min_segment_idx {
            return false;
        }
        if seg_idx == min_segment_idx {
            if let Some(min_t_val) = min_t {
                return t > min_t_val;  // Strictly greater!
            }
        }
        true
    };

    // Progressive grid search: 3×3 → 5×5 → 7×7 → linear fallback
    // For each candidate, compute (t, distance) and check is_valid()
    // Return (seg_idx, t) with minimum distance
}
```

### Validation Flow

```rust
for (input_idx, stop_pt) in stops_cm.iter().enumerate() {
    let (seg_idx, t) = find_closest_segment_constrained(
        stop_pt, route_nodes, grid, min_segment_idx, min_t
    );

    let progress_cm = compute_progress(seg_idx, t);

    // Check monotonicity (allow equal for same location)
    if progress_cm < previous_progress {
        reversal_info = Some(ReversalInfo { ... });
    }

    // Update constraints for next stop
    if seg_idx > min_segment_idx {
        min_segment_idx = seg_idx;
    }
    min_t = Some(t);  // Always set!
}
```

## Test Results

### Two-Pass Route Test

Route passes through (25.002, 121.002) twice with stops at both locations:

```
Stop 001: segment=7,  t=1.0000, progress=15005 cm
Stop 002: segment=15, t=1.0000, progress=30011 cm
Stop 003: segment=31, t=1.0000, progress=60022 cm
Stop 004: segment=63, t=1.0000, progress=120043 cm  ← 2nd pass
Stop 005: segment=95, t=1.0000, progress=180064 cm
```

✅ Stop 4 correctly maps to segment 63 (second pass), NOT segment 15 (first pass), even though both are near the same coordinates.

### tpF805 Complex Loop Route

35 stops including duplicate locations and segment boundaries:

```
Stop 009: segment=282, t=0.2809, progress=603566 cm
Stop 010: segment=282, t=0.2810, progress=603567 cm  ← Fixed! Now moves forward
...
Stop 029: segment=1156, t=1.0000, progress=2340855 cm
Stop 030: segment=1157, t=0.0000, progress=2340855 cm  ← Equal (OK)
```

✅ All 35 stops validated with monotonically increasing progress.

### Edge Cases Handled

1. **Duplicate coordinates**: Grid search finds segments from both passes, t-constraint selects correct pass
2. **Segment boundaries**: Equal progress allowed for stops at same physical location
3. **Close stops on same segment**: T-constraint forces forward progression

## Performance Impact

| Operation | Before | After | Notes |
|-----------|--------|-------|-------|
| Grid search | O(k) | O(k) | Unchanged |
| Candidate filtering | ~ | O(k) | Added is_valid check |
| Path constraint update | O(1) | O(1) | Simpler (always set min_t) |
| **Overall** | ~ | **O(k)** | No degradation |

## Debugging Output

The preprocessor now prints detailed stop-segment-t mapping:

```
[STOP-SEGMENT MAPPING]
  Stop 001: segment=    3, t=0.1924, progress=8278 cm
  Stop 002: segment=   32, t=0.6614, progress=82672 cm
  Stop 003: segment=   52, t=0.8854, progress=127914 cm
  ...
[VALIDATION PASS] or [VALIDATION FAIL]
```

This output is invaluable for:
- Understanding route geometry and stop placement
- Diagnosing projection issues
- Verifying t-constraint behavior

## Key Insights

1. **T-value matters**: Position along segment is as important as segment index
2. **Always track constraints**: Never reset `min_t` to None after first stop
3. **Strict inequality**: Use `t > min_t` not `t >= min_t` for same segment
4. **Allow equal progress**: Stops at same location can have equal progress values
5. **Progressive validation**: Check all stops even if reversal detected (for debugging)

## Lessons Learned

### What Went Wrong

The original code prioritized segment progression over t-progression:
- Moving to a new segment was seen as "advancing"
- But on the new segment, any t-value was allowed
- This allowed going "backwards" to an earlier t-value

### How We Fixed It

Recognized that advancement is **2-dimensional**:
- Segment dimension: segment index must increase OR stay same
- T-dimension: if same segment, t-value must increase

Both dimensions must be tracked simultaneously to ensure true forward progression.

## Future Improvements

1. **Adaptive t-constraint**: For routes with very close stops, consider relaxing t-constraint slightly
2. **Segment boundary detection**: Explicitly detect when t=0.0 or t=1.0 and handle specially
3. **Progress visualization**: Generate plots showing stop positions along route
4. **Validation diagnostics**: Show which constraint (segment or t) caused each stop's mapping

## References

- Tech Report v8.3, Section 17: 離線預處理流程
- Sequence-Constrained Stop Projection Tech Note (2026-03-18)
- Implementation: `preprocessor/src/stops/validation.rs`

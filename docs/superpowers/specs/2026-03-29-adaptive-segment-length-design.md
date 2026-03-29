# Adaptive Segment Length Design

**Date:** 2026-03-29
**Author:** Claude Code
**Status:** Approved

## Overview

Reduce binary size by increasing default route segment length from 30m to 100m, while preserving accuracy near stops and sharp turns through adaptive segmentation.

## Motivation

- **Primary Goal:** Reduce binary file size for embedded device storage
- **Target Route Length:** < 50 km (urban bus routes, short commuter lines)
- **Approach:** Hybrid adaptive segmentation - longer segments on straight sections, shorter segments near critical areas

## Architecture

### Key Parameters

| Parameter | Value | Description |
|-----------|-------|-------------|
| `MAX_SEGMENT_LENGTH_CM` | 10000 cm (100m) | Default maximum segment length |
| `ADAPTIVE_SEGMENT_LENGTH_CM` | 3000 cm (30m) | Maximum segment length in critical areas |
| `STOP_PROXIMITY_THRESHOLD_CM` | 10000 cm (100m) | Distance threshold for stop proximity |
| `SHARP_TURN_DEGREES` | 20.0° | Turn angle threshold for refinement |

### Data Flow

```
Raw GPS Points → Douglas-Peucker → Simplified Route → Adaptive Segmentation → Final Route
                                                       ↓
                                              (Insert shorter segments
                                               near stops & turns)
```

## Component Changes

### 1. Constants (`preprocessor/tests/common/mod.rs`)

```rust
// Change from 30m to 100m
pub const MAX_SEGMENT_LENGTH_CM: i32 = 10000;  // was 3000

// New constants for adaptive segmentation
pub const ADAPTIVE_SEGMENT_LENGTH_CM: i32 = 3000;  // 30m for critical areas
pub const STOP_PROXIMITY_THRESHOLD_CM: f64 = 10000.0;  // 100m - covers entire pre-stop corridor
pub const SHARP_TURN_DEGREES: f64 = 20.0;
```

### 2. Core Logic (`simplify.rs`)

#### Modify `simplify_and_interpolate`

After building `final_points` (line 70), add:
```rust
final_points = adaptive_segmentation(&final_points, stop_indices, &kept_indices);
```

#### Modify `interpolate_recursive`

Change signature to accept `max_len` parameter:
```rust
fn interpolate_recursive(p1: (i64, i64), p2: (i64, i64), result: &mut Vec<(i64, i64)>, max_len: f64)
```

#### Add `adaptive_segmentation` function

```rust
fn adaptive_segmentation(
    route: &[(i64, i64)],
    stop_indices: &[usize],
    kept_indices: &[usize],
) -> Vec<(i64, i64)> {
    let mut result = Vec::new();

    for i in 0..route.len() - 1 {
        let p1 = route[i];
        let p2 = route[i + 1];

        result.push(p1);

        let segment_len = distance(p1, p2);
        let needs_refinement = should_refine_segment(p1, p2, route, stop_indices, kept_indices);

        if segment_len > ADAPTIVE_SEGMENT_LENGTH_CM as f64 && needs_refinement {
            subdivide_recursive(p1, p2, &mut result, ADAPTIVE_SEGMENT_LENGTH_CM as f64);
        }
    }

    result.push(route[route.len() - 1]);
    result
}
```

#### Add `should_refine_segment` function

A segment needs refinement if **ANY** of these are true:
1. Any point on the segment is within 100m of a stop (covers entire pre-stop corridor)
2. The segment forms a turn >20° with adjacent segments

## Algorithm Details

### Refinement Criteria

1. **Stop Proximity:** Check if segment passes within 100m of any stop (ensures entire pre-stop corridor uses 30m segments for precise arrival detection)
2. **Sharp Turn:** Check if the angle at segment start/end exceeds 20°

### Segmentation Strategy

- **Non-critical areas:** Allow segments up to 100m
- **Critical areas (within 100m of stops or sharp turns):** Subdivide to max 30m
- **Subdivision:** Recursive midpoint splitting until threshold met
- **Rationale:** 100m stop proximity ensures the entire pre-stop corridor uses refined segments for maximum arrival detection precision

## Error Handling & Edge Cases

| Case | Handling |
|------|----------|
| Empty route | Return empty vector |
| Single point | Return single point |
| Two points | Apply adaptive segmentation to the single segment |
| Segment exactly at threshold | No subdivision needed |
| Stop at segment endpoint | Don't double-refine (endpoint already refined) |
| Consecutive sharp turns | Refine each affected segment independently |

### Validation

After adaptive segmentation, verify:
1. No segment exceeds `MAX_SEGMENT_LENGTH_CM` (100m)
2. Segments within 100m of stops don't exceed `ADAPTIVE_SEGMENT_LENGTH_CM` (30m)
3. All original stop positions are preserved
4. Route start and end points remain unchanged

### Error Recovery

- If subdivision creates too many points (>10x original), log warning and continue
- If distance calculation fails (NaN), use midpoint as fallback
- If stop index is out of bounds, skip that stop check and log

## Testing Strategy

### Unit Tests

Add to `simplify_edge_cases.rs`:

1. `test_adaptive_segmentation_near_stops` - 100m segment with stop at 40m
2. `test_adaptive_segmentation_sharp_turn` - L-shaped route with 100m segments
3. `test_no_refinement_far_from_stops` - Long straight route, no stops
4. `test_max_segment_100m_enforced` - Verify no segment exceeds 100m
5. `test_critical_areas_max_30m` - Verify segments near stops are ≤30m

### Integration Tests

1. **Real route test:** Process actual bus route data
   - Measure binary size reduction (target: 60-70% of original)
   - Verify stop mapping accuracy
   - Confirm stop protection behavior

2. **Edge case routes:** Figure-8, U-turn, zigzag patterns

### Performance Validation

- Benchmark preprocessing time impact
- Verify binary file size reduction on real data

## Migration Plan

### Implementation Steps

1. **Phase 1:** Add constants to `common/mod.rs`
2. **Phase 2:** Implement adaptive segmentation functions in `simplify.rs`
3. **Phase 3:** Update `interpolate_recursive` to accept max_len parameter
4. **Phase 4:** Wire up adaptive segmentation in `simplify_and_interpolate`
5. **Phase 5:** Add new tests, update existing assertions
6. **Phase 6:** Verification on real route data

### Rollback Plan

- Keep old `MAX_SEGMENT_LENGTH_CM = 3000` as reference
- Feature flag adaptive segmentation via constant
- Git revert is straightforward (changes localized)

### Compatibility

- **Binary format:** No changes
- **Arrival detector:** No changes (input format unchanged)
- **Existing routes:** Must be reprocessed with new preprocessor

## Success Criteria

### Binary Size Reduction

| Metric | Target | Measurement |
|--------|--------|-------------|
| Binary file size | ≤ 70% of original | Compare file sizes |
| Segment count | ~65-70% of original | Count segments |

### Accuracy Preservation

| Metric | Target | Measurement |
|--------|--------|-------------|
| Stop mapping error | ≤ 5m | Existing validation tests |
| Stop protection | 100% preserved | All stops as route nodes |
| Sharp turn accuracy | No regression | Visual inspection |

### Performance

| Metric | Target | Measurement |
|--------|--------|-------------|
| Preprocessing time | ≤ 120% of original | Benchmark |
| Runtime performance | No change | Arrival detector unchanged |

### Test Coverage

- All existing tests pass
- New adaptive segmentation tests pass
- Real route data processes successfully

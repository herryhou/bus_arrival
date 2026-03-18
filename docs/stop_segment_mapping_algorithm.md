# Bus Route Stop-Segment Mapping Algorithm

**Problem:** Given a bus route (polyline) and an ordered list of stop locations, project each stop onto the route such that the stop order is preserved and progress values are monotonically non-decreasing.

**Status:** Final (v1.0)
**Date:** 2026-03-18

---

## 1. Problem Definition

### 1.1 Inputs

- **Route**: A polyline represented as `N` nodes `P[0], P[1], ..., P[N-1]` in planar coordinates (cm)
  - Each node `P[i]` has: `(x_cm, y_cm, cum_dist_cm, dx_cm, dy_cm, seg_len_cm)`
  - Segment `i` connects `P[i]` to `P[i+1]` for `i ∈ [0, N-2]`

- **Stops**: An ordered list of `M` stop locations `S[0], S[1], ..., S[M-1]` in planar coordinates (cm)
  - Input order must be preserved in output

- **Spatial Grid**: A 100m × 100m grid index mapping planar regions to segment indices

### 1.2 Outputs

For each stop `S[j]`, produce:
- `segment[j]`: Index of route segment `[0, N-2]`
- `t[j]`: Position along segment `[0.0, 1.0]` where 0.0 = start, 1.0 = end
- `progress[j]`: Cumulative distance from route start (cm)

### 1.3 Constraints

**C1 (Monotonicity):** `progress[j] >= progress[j-1]` for all `j > 0`

**C2 (Path Continuity):** Each stop must map to a point reachable by forward progression from the previous stop's mapped point

**C3 (Proximity):** Each stop should map to the closest point on the route satisfying C1 and C2

---

## 2. Key Observations

### 2.1 The Two-Dimensional Constraint

Stop mapping involves **two dimensions of advancement**:

| Dimension | State Space | Advancement Condition |
|-----------|-------------|----------------------|
| Segment | `{0, 1, ..., N-2}` | `segment[j] >= segment[j-1]` |
| Position | `[0.0, 1.0]` | If same segment: `t[j] > t[j-1]` |

**Critical Insight:** These dimensions are **coupled**. Advancement in either dimension satisfies the path continuity constraint.

### 2.2 Progress Computation

For stop `j` mapped to segment `i` at position `t`:

```
progress[j] = cum_dist[i] + t × seg_len[i]
```

Where `cum_dist[i]` is the accumulated distance to the start of segment `i`.

### 2.3 The Reversal Problem

When `segment[j] == segment[j-1]` and `t[j] <= t[j-1]`:

```
progress[j] = cum_dist[i] + t[j] × seg_len[i]
            <= cum_dist[i] + t[j-1] × seg_len[i]
            = progress[j-1]
```

This violates constraint C1 (monotonicity).

---

## 3. Algorithm

### 3.1 High-Level Structure

```
FOR each stop S[j] in input order:
    Find closest route point satisfying path constraint from previous stop
    Record (segment[j], t[j], progress[j])
    Update path constraint for next iteration
```

### 3.2 Formal Specification

**Data Structures:**
```
min_seg: int          // Minimum allowed segment index (initially 0)
min_t: float | null   // Minimum allowed t on segment min_seg (initially null)
```

**Main Loop:**
```
FOR j = 0 TO M-1:
    (seg, t) = FIND_CLOSEST_CONSTRAINED(S[j], route, grid, min_seg, min_t)
    progress[j] = route[seg].cum_dist + t × route[seg].seg_len

    // Check monotonicity (allow equal for same physical location)
    IF progress[j] < progress[j-1]:
        REPORT reversal at stop j

    // Update constraint for next stop
    min_seg = seg
    min_t = t  // Always set! Never null after first stop
```

### 3.3 Constrained Search Subroutine

**FIND_CLOSEST_CONSTRAINED(point, route, grid, min_seg, min_t):**

```
INPUT: point (x, y), route nodes, spatial grid, min_seg, min_t
OUTPUT: (segment, t) closest to point satisfying path constraint

best_seg = min_seg
best_t = 0.0
best_dist² = ∞
found = false

DEFINE VALID(seg_idx, t_val):
    IF seg_idx < min_seg: RETURN false
    IF seg_idx == min_seg AND min_t != null:
        RETURN t_val > min_t  // Strict inequality!
    RETURN true

// Phase 1: Grid search with progressive window expansion
FOR radius = 1 TO 3:
    candidates = QUERY_GRID(grid, point.x, point.y, radius)

    FOR each seg_idx IN candidates:
        IF seg_idx >= N OR route[seg_idx].seg_len == 0: CONTINUE

        // Compute projection
        (t_val, dist²) = PROJECT_POINT_TO_SEGMENT(point, route[seg_idx])

        IF VALID(seg_idx, t_val) AND dist² < best_dist²:
            best_seg = seg_idx
            best_t = t_val
            best_dist² = dist²
            found = true

    IF found: RETURN (best_seg, best_t)

// Phase 2: Linear fallback (for edge cases)
FOR seg_idx = min_seg TO N-2:
    IF route[seg_idx].seg_len == 0: CONTINUE

    (t_val, dist²) = PROJECT_POINT_TO_SEGMENT(point, route[seg_idx])

    IF VALID(seg_idx, t_val) AND dist² < best_dist²:
        best_seg = seg_idx
        best_t = t_val
        best_dist² = dist²
        found = true

IF found: RETURN (best_seg, best_t)
ELSE: RETURN (min_seg + 1, 0.0)  // Last resort (should not occur)
```

### 3.4 Point Projection

**PROJECT_POINT_TO_SEGMENT(point, segment):**

```
INPUT: point Q, segment from A to B
OUTPUT: (t, dist²) where t ∈ [0, 1]

// Vector math
AQ = Q - A
AB = B - A

// Project Q onto line AB (unclamped)
t_raw = (AQ · AB) / |AB|²

// Clamp to segment bounds
t = CLAMP(t_raw, 0.0, 1.0)

// Closest point on segment
P = A + t × AB

// Squared distance
dist² = |Q - P|²

RETURN (t, dist²)
```

---

## 4. Invariants and Proofs

### 4.1 Invariant (Maintained by Algorithm)

> After processing stop `j`, the constraint `(min_seg, min_t)` ensures that any subsequent stop `k > j` mapping to `(segment[k], t[k])` satisfies:
> - `segment[k] > min_seg` OR
> - `segment[k] == min_seg` AND `t[k] > min_t`

**Proof by Induction:**

*Base (j=0):* Initially `min_seg=0`, `min_t=null`. VALID() accepts any segment. ✓

*Inductive Step:* Assume invariant holds after stop `j-1` with constraint `(min_seg, min_t)`. For stop `j`:
- If `segment[j] > min_seg`: New constraint `(segment[j], t[j])` satisfies invariant
- If `segment[j] == min_seg`: Then `t[j] > min_t` (by VALID()). New constraint `(min_seg, t[j])` satisfies invariant ✓

### 4.2 Monotonicity Theorem

> **Theorem:** The algorithm produces monotonically non-decreasing progress values: `progress[j] >= progress[j-1]` for all `j > 0`.

**Proof:**

Consider stops `j-1` and `j` with mappings `(s₁, t₁)` and `(s₂, t₂)`:

*Case 1: `s₂ > s₁`*
```
progress[j] = cum_dist[s₂] + t₂ × len[s₂]
            >= cum_dist[s₂]  (since t₂ >= 0)
            > cum_dist[s₁] + len[s₁]  (since s₂ > s₁)
            >= cum_dist[s₁] + t₁ × len[s₁]
            = progress[j-1]
```

*Case 2: `s₂ == s₁`*
```
progress[j] = cum_dist[s₁] + t₂ × len[s₁]
progress[j-1] = cum_dist[s₁] + t₁ × len[s₁]

By invariant: t₂ > t₁
Thus: progress[j] > progress[j-1]
```

Both cases satisfy monotonicity. ✓

---

## 5. Complexity Analysis

### 5.1 Time Complexity

For each of `M` stops:

| Phase | Operation | Complexity |
|-------|-----------|------------|
| Grid search | 3×3, 5×5, 7×7 window expansion | O(k) where k = avg segments per cell |
| Linear fallback | Scan from `min_seg` to N | O(N) worst case |
| Overall per stop | O(k + N) worst case | |
| **Total** | **M × O(k + N)** | |

**Practical Performance:**
- Grid cells contain ~5-15 segments typically
- k << N, so grid search dominates
- Expected: O(M × k) = O(M) for constant k

### 5.2 Space Complexity

| Component | Space | Notes |
|-----------|-------|-------|
| Route nodes | O(N) | Input |
| Spatial grid | O(N) | Precomputed |
| Stop results | O(M) | Output |
| **Total** | **O(N + M)** | |

---

## 6. Edge Cases and Handling

### 6.1 Identical Coordinates

**Problem:** Stops `j` and `j+1` at identical coordinates.

**Handling:** Both map to same `(segment, t)`, giving equal progress. The comparison uses `<` not `<=`, so equal progress is allowed.

### 6.2 Segment Boundaries

**Problem:** Stop `j` at `(s, t=1.0)` (end of segment), stop `j+1` at `(s+1, t=0.0)` (start of next segment).

**Handling:** Both have `progress = cum_dist[s+1]`. Equal progress is allowed.

### 6.3 Route Loop (Same Location Visited Twice)

**Problem:** Route passes through location L twice. Stops #A and #B both at L, with #A < #B in input order.

**Handling:** Grid search finds segments from both passes in same cell. T-constraint ensures:
- Stop #A: Matches segment S₁ from first pass
- Stop #B: Must satisfy `t > t_A` on S₁, so skips to segment S₂ from second pass

### 6.4 No Valid Segment

**Problem:** Stop location is far from route; grid search finds no valid candidates.

**Handling:** Linear fallback scans all segments from `min_seg` onward. Last resort returns `(min_seg + 1, t=0.0)`.

---

## 7. Pseudocode Summary

```
FUNCTION MAP_STOPS_TO_ROUTE(route, stops, grid):
    N = length(route.nodes)
    M = length(stops)

    results = array of size M
    min_seg = 0
    min_t = null

    FOR j = 0 TO M-1:
        point = stops[j]

        // Find closest valid segment
        (seg, t) = FIND_CLOSEST_CONSTRAINED(point, route, grid, min_seg, min_t)
        progress = route[seg].cum_dist + t * route[seg].seg_len

        results[j] = {segment: seg, t: t, progress: progress}

        IF j > 0 AND progress < results[j-1].progress:
            SIGNAL reversal detected

        // Update constraint
        min_seg = seg
        min_t = t

    RETURN results

FUNCTION FIND_CLOSEST_CONSTRAINED(point, route, grid, min_seg, min_t):

    DEFINE VALID(seg_idx, t_val):
        IF seg_idx < min_seg: RETURN false
        IF seg_idx == min_seg AND min_t IS NOT null:
            RETURN t_val > min_t
        RETURN true

    best = {seg: min_seg, t: 0.0, dist²: ∞}
    found = false

    // Try grid search
    FOR radius = 1 TO 3:
        FOR each seg_idx IN grid.query(point, radius):
            (t_val, dist²) = project_to_segment(point, route[seg_idx])
            IF VALID(seg_idx, t_val) AND dist² < best.dist²:
                best = {seg: seg_idx, t: t_val, dist²: dist²}
                found = true
        IF found: RETURN (best.seg, best.t)

    // Try linear search
    FOR seg_idx = min_seg TO length(route) - 2:
        (t_val, dist²) = project_to_segment(point, route[seg_idx])
        IF VALID(seg_idx, t_val) AND dist² < best.dist²:
            best = {seg: seg_idx, t: t_val, dist²: dist²}
            found = true

    IF found: RETURN (best.seg, best.t)
    ELSE: RETURN (min_seg + 1, 0.0)  // Fallback
```

---

## 8. Implementation Notes

### 8.1 Coordinate System

- All computations use **centimeter units** for precision
- Latitude/longitude converted to relative (x, y) using average latitude
- Cumulative distances tracked as 32-bit integers (range: ~0 to 21 km)

### 8.2 Spatial Grid Index

- Grid cell size: 100m × 100m (10,000 cm × 10,000 cm)
- Each cell stores list of segment indices passing through it
- Query radius: 1 = 3×3 cells, 2 = 5×5 cells, 3 = 7×7 cells

### 8.3 Numerical Precision

- Dot products use 64-bit integers (i64) to avoid overflow
- Division to floating-point done last (t = num / denom)
- Progress rounded to nearest centimeter

### 8.4 Data Structure Sizes

```
RouteNode: 36 bytes
  - x_cm, y_cm: i32 (4 bytes each)
  - cum_dist_cm: i32 (4 bytes)
  - dx_cm, dy_cm: i32 (4 bytes each)
  - seg_len_cm: i32 (4 bytes)
  - len2_cm2: i64 (8 bytes)
  - heading_cdeg: i16 (2 bytes)
  - _pad: i16 (2 bytes)
```

---

## 9. Validation and Testing

### 9.1 Test Cases

| Test | Description | Expected |
|------|-------------|----------|
| Linear route | Stops on straight line | Strictly increasing progress |
| Two-pass route | Route through same point twice | Second stop uses later segment |
| Segment boundary | Stops at adjacent segment boundaries | Equal progress allowed |
| tpF805 | Real-world loop route | All 35 stops validated |
| Identical coords | Multiple stops at same location | All map to same (segment, t) |

### 9.2 Validation Criteria

```
PASS IF:
  - All stops mapped to valid segments
  - progress[j] >= progress[j-1] for all j
  - Input order preserved in output

FAIL IF:
  - Any progress[j] < progress[j-1]
  - Stop cannot map to any segment (degenerate route)
```

---

## 10. References

1. Tech Report v8.3, Section 17: 離線預處理流程
2. Sequence-Constrained Stop Projection Tech Note (2026-03-18)
3. Stop-Segment Mapping Problem Solution (this document)

---

**Appendix A: Notation Glossary**

- `N`: Number of route nodes
- `M`: Number of stops
- `P[i]`: Route node i
- `S[j]`: Stop j (input coordinates)
- `segment[j]`: Output segment index for stop j
- `t[j]`: Output t-value for stop j (position along segment)
- `progress[j]`: Output cumulative distance for stop j
- `cum_dist[i]`: Cumulative distance to start of segment i
- `seg_len[i]`: Length of segment i

**Appendix B: Version History**

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2026-03-18 | Initial specification with t-constraint fix |

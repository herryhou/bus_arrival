# Map Matching Specification

## Overview

Implements Filter-then-Rank architecture for segment selection:

1. **Filter**: Heading gate removes implausible directions
2. **Rank**: Distance-based scoring selects best segment

**Key invariant:** Early exit requires `eligible_found = true`.

## Inputs

| Parameter | Type | Range | Description |
|-----------|------|-------|-------------|
| gps_x | DistCm | ±214 km | GPS X coordinate (cm) |
| gps_y | DistCm | ±214 km | GPS Y coordinate (cm) |
| gps_heading | HeadCdeg | -18000..18000 | GPS heading (0.01°) |
| gps_speed | SpeedCms | 0..1667 | GPS speed (cm/s) |
| route_data | &RouteData | - | Route with grid index |
| last_idx | usize | 0..node_count-1 | Previous segment |
| is_first_fix | bool | - | Post-outage recovery flag |

## Outputs

| Parameter | Type | Range | Description |
|-----------|------|-------|-------------|
| seg_idx | usize | 0..node_count-1 | Best segment index |
| dist2 | Dist2 | 0..∞ | Distance² to segment (cm²) |

## Invariants (MUST)

- [ ] `MAX_HEADING_DIFF_CDEG = 9000` (90° heading gate)
- [ ] At standstill (w=0): heading gate disabled (return u32::MAX)
- [ ] `is_first_fix=true`: 180° relaxed threshold
- [ ] Early exit: only if `window_eligible_found = true` AND `dist2 < SIGMA_GPS_CM²`
- [ ] `best_eligible_dist2 = MAX` when `window_eligible = false` (prevent bug masking)
- [ ] Grid search ALWAYS sets `eligible_found = true` when finding eligible segment
- [ ] Fallback returns `best_any_idx` (explicit degradation)

## Formulas

**Heading weight:**
```rust
fn heading_weight(speed: SpeedCms) -> i32 {
    // 0..256 mapping: 0 cm/s → 0, ≥83 cm/s → 256
    ((speed * 256) / 83).min(256)
}
```

**Heading threshold:**
```rust
fn heading_threshold_cdeg(w: i32) -> u32 {
    if w == 0 { return u32::MAX; }
    let range = 36_000u32 - 9_000u32; // 27_000
    36_000 - range * w as u32 / 256
}
```

**Segment score (pure distance):**
```rust
fn segment_score(gps_x: DistCm, gps_y: DistCm, seg: &RouteNode) -> Dist2 {
    // Point-to-segment distance² (no sqrt)
    // Returns cm²
}
```

**Heading difference:**
```rust
fn heading_diff_cdeg(a: HeadCdeg, b: HeadCdeg) -> HeadCdeg {
    let diff = (a as i32 - b as i32).unsigned_abs() % 36000;
    if diff > 18000 {
        (36000 - diff) as HeadCdeg
    } else {
        diff as HeadCdeg
    }
}
```

## Algorithm Phases

**Phase 1: Window Search**
- Search `last_idx ± WINDOW_BACK/FWD` (default: -2/+10)
- Returns `window_eligible_found` flag
- Early exit if eligible segment found within SIGMA_GPS_CM (20 m)

**Phase 2: Grid Search** (if needed)
- 3×3 cell neighborhood around GPS position
- Carries over window best as seed
- Sets `eligible_found = true` when finding eligible segment

**Fallback:**
- If no eligible segment found in window or grid
- Return `best_any_idx` (pure distance selection)
- Explicit degradation (logged in firmware)

## Type Definitions

```rust
type DistCm = i32;      // Distance in centimeters
type SpeedCms = i32;    // Speed in cm/s
type HeadCdeg = i16;    // Heading in centidegrees (0.01°)
type Dist2 = i64;       // Distance squared in cm²
```

## Constants

```rust
const MAX_HEADING_DIFF_CDEG: u32 = 9_000;  // 90°
const SIGMA_GPS_CM: DistCm = 2000;         // 20 m
const MAX_DIST2_EARLY_EXIT: Dist2 = 4_000_000;  // SIGMA_GPS_CM²
const WINDOW_BACK: usize = 2;
const WINDOW_FWD: usize = 10;
```

## Version Notes

- v8.8 (E1 fix): Removed heading from scoring. Filter-then-Rank architecture.
  - OLD: `d² + λ·diff²·w_h` (broken: λ missing, units mismatched)
  - NEW: Filter by heading, then rank by distance only
- v8.7: RouteNode layout optimization (52→24 bytes)

## Related Files

- `crates/pipeline/gps_processor/src/map_match.rs` — Implementation
- `crates/pipeline/gps_processor/tests/bdd_localization.rs` — BDD tests

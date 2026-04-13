# E1 Fix: Filter-then-Rank Heading Architecture

**Date:** 2026-04-13
**Issue:** E1 — The Heading Penalty Has No Tunable λ — It Silently Overrides Distance
**Status:** Approved

---

## Problem Statement

The spec defines the segment score as:

$$\text{score}(i) = d^2 + \lambda \cdot \text{diff}^2 \cdot w_h$$

with λ = 1 cm²/cdeg² as a starting value. However, the actual code in `segment_score` computes:

```rust
((heading_diff as i64).pow(2) * w as i64) >> 8
```

**λ is absent** (effectively λ = 1, unitless). This creates a unit mismatch:

- `heading_diff` is in centidegrees (0.01°)
- A 90° difference = 9,000 cdeg
- Heading term = 9000² × 256 >> 8 = **324,000,000**
- Typical on-route d² for a bus 10–50 cm off the line = **100–25,000**
- **The heading term is 10,000× larger than the distance term**

This turns a soft penalty into a de facto hard exclusion. Near stops where GPS heading is unreliable (bus nearly stationary, heading jitter), the heading term dominates even with the low-speed weight ramp.

---

## Solution: Filter-then-Rank

Replace blended scoring (distance + heading) with a two-stage process:

1. **Filter:** Apply a hard heading gate based on speed-adjusted threshold
2. **Rank:** Select best segment by pure distance squared

### Rationale

Heading and distance are fundamentally different criteria. Mixing them requires an arbitrary scale factor (λ) that cannot be derived from first principles. A heading threshold in degrees is directly interpretable by any developer, versus λ or a normalization ratio that require understanding the full scoring formula to audit.

---

## Architecture

### Module Structure

All changes contained within `crates/pipeline/gps_processor/src/map_match.rs`:

**New Functions:**
- `heading_threshold_cdeg(w: i32) -> u32` — Compute speed-dependent heading gate
- `heading_eligible(gps_heading, gps_speed, seg_heading) -> bool` — Boolean heading filter
- `best_eligible(...) -> (usize, Dist2, bool, usize, Dist2)` — Helper returning (best_eligible_idx, best_eligible_dist2, eligible_found, best_any_idx, best_any_dist2)

**Modified Functions:**
- `segment_score(...)` — Signature shrinks from 5 args to 3; returns pure `Dist2` (cm²)
- `find_best_segment_restricted(...)` — Full rewrite to use filter-then-rank with dual trackers

**Constants:**
- `MAX_HEADING_DIFF_CDEG: u32 = 9_000` — 90° heading gate at full speed (module-level)
- `MAX_DIST2_EARLY_EXIT: Dist2` — 20m² early exit threshold (function-local const)

### Data Flow

```
GPS point (x, y, heading, speed)
         │
         ▼
┌─────────────────────────────────────────────────────────────┐
│ 1. Search window [last-2, last+10]                          │
│    best_eligible returns:                                   │
│    - (best_eligible_idx, best_eligible_dist2, eligible_found│
│       best_any_idx, best_any_dist2)                         │
└─────────────────────────────────────────────────────────────┘
         │
         ▼
   eligible_found?
    AND dist² < 20m²
         │
    ┌────┴────┐
    │ Yes     │ No
    ▼         ▼
  RETURN    Window → Grid seeding:
              ┌───────────────────────┬───────────────────────┐
              │ eligible_found = true  │ eligible_found = false │
              ▼                       ▼                       │
         best_eligible =        best_eligible_dist2 = MAX     │
         window_best_eligible   (no eligible winner yet)      │
         best_eligible_dist2 =                              │
         window_eligible_dist2                                │
                                                              │
         best_any = window_best_any  (both paths)            │
         best_any_dist2 = window_any_dist2                    │
              └───────────────────────┴───────────────────────┘
                              │
                              ▼
                      ┌────────────────────────────────┐
                      │ 2. Grid search (3×3)          │
                      │ For each segment:             │
                      │ - If eligible AND d² < best_  │
                      │   eligible_dist2: update      │
                      │   best_eligible AND set       │
                      │   eligible_found = true       │
                      │ - If d² < best_any_dist2:     │
                      │   update best_any             │
                      └────────────────────────────────┘
                               │
                               ▼
                        eligible_found?
                          ┌────┴────┐
                          │ Yes     │ No → log warning
                          ▼         ▼    return best_any_idx
                       RETURN best_eligible_idx
```

### Key Invariants

| Invariant | Why it matters |
|-----------|----------------|
| Early exit requires `eligible_found = true` | Prevents returning heading-ineligible segment from window (latent U-turn bug fix) |
| `best_eligible_dist2 = MAX` when `window_eligible = false` | Prevents heading-ineligible window result from masking eligible-but-farther grid segments |
| Dual trackers carry forward from window to grid | Grid can improve on either eligible or pure-distance result independently |
| Grid sets `eligible_found = true` when it finds an eligible segment | Critical when `window_eligible = false`; grid may discover the first eligible result |
| Fallback returns `best_any_idx` — closest by pure distance across window and grid combined, regardless of heading | Provenance (window vs grid) not preserved; fallback prioritizes proximity over heading when no eligible segment exists |

### Implementation Detail

When `window_eligible = false`, `best_eligible_idx` is semantically undefined but must be initialised to a concrete value:

```rust
// When no eligible segment exists in the window, best_eligible_idx is unused
// because best_eligible_dist2 = MAX guarantees any grid segment will
// overwrite it. Initialise to last_idx as a safe default — it's never
// returned without being overwritten, but the value prevents uninitialised
// memory and makes debugging clearer.
let best_eligible_idx = if window_eligible {
    window_best_eligible
} else {
    last_idx // Safe default; will be overwritten by first eligible grid segment
};
```

---

## Detailed Implementation

### 1. Constants and `heading_threshold_cdeg`

```rust
/// Hard heading gate at full speed (w = 256, ≥ 3 km/h).
/// A bus in motion cannot be heading >90° from the segment direction.
/// This is the single tunable heading parameter; its units (centidegrees) are
/// directly interpretable — no hidden scale factors.
const MAX_HEADING_DIFF_CDEG: u32 = 9_000; // 90°

/// Heading filter threshold for a given speed weight.
///
/// Returns `u32::MAX` (gate disabled) when w = 0 — at a standstill GPS heading
/// is unreliable; don't reject any segment.
/// Returns `MAX_HEADING_DIFF_CDEG` (90°) at w = 256.
/// Linearly interpolates between the two, giving a progressively tighter gate
/// as the bus picks up speed.
///
/// At w = 128 (≈1.5 km/h):  threshold ≈ 22 500 cdeg (225°, nearly open)
/// At w = 256 (≥3 km/h):    threshold =  9 000 cdeg (90°, meaningful gate)
fn heading_threshold_cdeg(w: i32) -> u32 {
    if w == 0 {
        return u32::MAX;
    }
    // threshold = 36000 - (36000 - MAX_HEADING_DIFF_CDEG) × w / 256
    let range = 36_000u32 - MAX_HEADING_DIFF_CDEG; // 27 000
    36_000 - range * w as u32 / 256
}
```

### 2. `heading_eligible`

```rust
/// Returns true if this segment is a plausible direction of travel given the
/// current GPS heading.
///
/// Three cases handled explicitly:
///   - Sentinel heading (i16::MIN): GGA-only mode, no heading data → always eligible
///   - Stopped (w = 0): heading is unreliable → always eligible
///   - Moving: eligible iff heading_diff ≤ threshold(speed)
///
/// Note: this is a hard gate, not a blended penalty.  A segment is either
/// physically plausible or it isn't; partial credit produces commensuration
/// problems (adding cm² to cdeg²).
fn heading_eligible(gps_heading: HeadCdeg, gps_speed: SpeedCms, seg_heading: HeadCdeg) -> bool {
    if gps_heading == i16::MIN {
        return true; // GGA-only: preserve existing sentinel behaviour
    }
    let w = heading_weight(gps_speed);
    let threshold = heading_threshold_cdeg(w);
    let diff = heading_diff_cdeg(gps_heading, seg_heading) as u32;
    diff <= threshold
}
```

### 3. `segment_score` — pure distance, heading removed

```rust
/// Distance-squared from GPS point to segment (clamped projection).
///
/// Heading is intentionally absent.  Heading belongs in the eligibility
/// filter (`heading_eligible`), not in the ranking score.  Mixing cm² and
/// cdeg² into one scalar requires an arbitrary scale factor that cannot be
/// derived from first principles.
///
/// The return type is `Dist2` (i64 cm²).
pub fn segment_score(
    gps_x: DistCm,
    gps_y: DistCm,
    seg: &RouteNode,
) -> Dist2 {
    distance_to_segment_squared(gps_x, gps_y, seg)
}
```

### 4. `best_eligible` helper

```rust
/// Scan a range of segment indices, returning the best eligible and best any.
///
/// Returns:
/// - (best_eligible_idx, best_eligible_dist2, eligible_found,
///    best_any_idx, best_any_dist2)
///
/// "Best" = minimum dist2. If no segment passes the heading filter,
/// best_eligible_dist2 = Dist2::MAX and eligible_found = false.
fn best_eligible(
    gps_x: DistCm,
    gps_y: DistCm,
    gps_heading: HeadCdeg,
    gps_speed: SpeedCms,
    route_data: &RouteData,
    range: impl Iterator<Item = usize>,
) -> (usize, Dist2, bool, usize, Dist2) {
    let mut best_eligible_idx: Option<usize> = None;
    let mut best_eligible_dist2 = Dist2::MAX;
    let mut best_any_idx: Option<usize> = None;
    let mut best_any_dist2 = Dist2::MAX;

    for idx in range {
        if let Some(seg) = route_data.get_node(idx) {
            let d2 = segment_score(gps_x, gps_y, &seg);

            if d2 < best_any_dist2 {
                best_any_dist2 = d2;
                best_any_idx = Some(idx);
            }

            if heading_eligible(gps_heading, gps_speed, seg.heading_cdeg) && d2 < best_eligible_dist2 {
                best_eligible_dist2 = d2;
                best_eligible_idx = Some(idx);
            }
        }
    }

    let eligible_found = best_eligible_idx.is_some();
    let eligible_idx = best_eligible_idx.unwrap_or(0);
    let any_idx = best_any_idx.unwrap_or(0);

    (eligible_idx, best_eligible_dist2, eligible_found, any_idx, best_any_dist2)
}
```

### 5. `find_best_segment_restricted` — full rewrite

```rust
pub fn find_best_segment_restricted(
    gps_x: DistCm,
    gps_y: DistCm,
    gps_heading: HeadCdeg,
    gps_speed: SpeedCms,
    route_data: &RouteData,
    last_idx: usize,
) -> usize {
    // Early-exit threshold: if the best eligible segment in the window is
    // within SIGMA_GPS_CM (20 m), skip the expensive grid search.
    // Now that scores are pure dist2, this comparison is physically meaningful.
    const MAX_DIST2_EARLY_EXIT: Dist2 =
        SIGMA_GPS_CM as i64 * SIGMA_GPS_CM as i64; // 4 000 000 cm²

    const WINDOW_BACK: usize = 2;
    const WINDOW_FWD: usize = 10;

    let start = last_idx.saturating_sub(WINDOW_BACK);
    let end = (last_idx + WINDOW_FWD).min(route_data.node_count.saturating_sub(1));

    let (window_best_eligible, window_eligible_dist2, window_eligible_found,
         window_best_any, window_any_dist2) =
        best_eligible(gps_x, gps_y, gps_heading, gps_speed, route_data,
                      start..=end);

    if window_eligible_found && window_eligible_dist2 < MAX_DIST2_EARLY_EXIT {
        return window_best_eligible;
    }

    // Fallback: full grid search.
    if gps_x < route_data.x0_cm || gps_y < route_data.y0_cm {
        return window_best_any; // Outside bounding box — keep window result
    }

    let gx = ((gps_x - route_data.x0_cm) / route_data.grid.grid_size_cm) as u32;
    let gy = ((gps_y - route_data.y0_cm) / route_data.grid.grid_size_cm) as u32;

    // Carry over the window winner as the seed — grid search only improves on it.
    let mut best_eligible_idx = if window_eligible_found {
        window_best_eligible
    } else {
        // Safe default; will be overwritten by first eligible grid segment
        last_idx
    };
    let mut best_eligible_dist2 = if window_eligible_found {
        window_eligible_dist2
    } else {
        Dist2::MAX
    };
    let mut best_any_idx = window_best_any;
    let mut best_any_dist2 = window_any_dist2;
    let mut eligible_found = window_eligible_found;

    for dy in 0..=2i32 {
        for dx in 0..=2i32 {
            let ny = gy as i32 + dy - 1;
            let nx = gx as i32 + dx - 1;
            if ny < 0 || nx < 0 {
                continue;
            }
            route_data.grid.visit_cell(nx as u32, ny as u32, |idx: u16| {
                if let Some(seg) = route_data.get_node(idx as usize) {
                    let d2 = segment_score(gps_x, gps_y, &seg);

                    // Update best_any tracker
                    if d2 < best_any_dist2 {
                        best_any_dist2 = d2;
                        best_any_idx = idx as usize;
                    }

                    // Update best_eligible tracker if heading matches
                    if heading_eligible(gps_heading, gps_speed, seg.heading_cdeg) {
                        if d2 < best_eligible_dist2 {
                            best_eligible_dist2 = d2;
                            best_eligible_idx = idx as usize;
                            eligible_found = true;
                        }
                    }
                }
            });
        }
    }

    // If no segment in window or grid passed the heading filter, fall back to
    // pure distance over the window. This is an explicit, logged degradation —
    // not a silent wrong answer.
    if !eligible_found {
        #[cfg(feature = "firmware")]
        defmt::warn!(
            "heading filter: no eligible segments at speed={} cdeg heading={}, \
             falling back to pure-distance selection",
            gps_speed, gps_heading
        );
        return best_any_idx;
    }

    best_eligible_idx
}
```

---

## Test Updates

The existing test `test_segment_score_heading_sentinel` needs to be split and migrated:

### New `heading_eligible` tests

```rust
#[test]
fn test_heading_eligible_sentinel() {
    let seg_heading: HeadCdeg = 9000; // 90°

    // Sentinel: always eligible regardless of segment heading or speed
    assert!(heading_eligible(i16::MIN, 500, seg_heading));
    assert!(heading_eligible(i16::MIN, 0, seg_heading));
}

#[test]
fn test_heading_eligible_stopped() {
    // Stopped (w=0): always eligible — heading is unreliable
    assert!(heading_eligible(0, 0, 9000));      // facing opposite direction
    assert!(heading_eligible(0, 0, 18000));     // 180° misaligned
}

#[test]
fn test_heading_eligible_moving() {
    let speed: SpeedCms = 500; // well above 83 cm/s → w=256 → threshold=9000

    // Same heading: eligible
    assert!(heading_eligible(9000, speed, 9000));

    // 89° off: eligible (just under 90° gate)
    assert!(heading_eligible(0, speed, 8999));

    // 91° off: not eligible
    assert!(!heading_eligible(0, speed, 9001));

    // 180° (opposite direction): not eligible at speed
    assert!(!heading_eligible(0, speed, 18000));
}
```

### Updated `segment_score` test

```rust
#[test]
fn test_segment_score_is_pure_distance() {
    let seg = RouteNode {
        x_cm: 100000,
        y_cm: 100000,
        cum_dist_cm: 0,
        heading_cdeg: 9000,
        seg_len_mm: 10000,
        dx_cm: 100,
        dy_cm: 0,
        _pad: 0,
    };

    // Same position: score should be 0 regardless of any external heading
    let score = segment_score(100000, 100000, &seg);
    assert_eq!(score, 0);

    // Different position: score is pure distance squared
    let score_far = segment_score(101000, 100000, &seg); // 1000 cm away
    assert_eq!(score_far, 1_000_000); // 1000²
}
```

---

## Benefits

1. **Eliminates unit mismatch:** No more adding cm² to cdeg²
2. **Interpretable parameters:** 90° heading threshold vs λ ≈ 0.000003
3. **Fixes latent bugs:** Early exit now requires eligible segment; dual-tracker prevents masking
4. **Clear degradation path:** No-eligible fallback is explicit and logged
5. **Simpler scoring:** `segment_score` returns pure distance, easier to reason about

---

## Files Changed

- `crates/pipeline/gps_processor/src/map_match.rs` — All implementation changes
- `crates/pipeline/gps_processor/src/map_match.rs` (tests) — Test updates

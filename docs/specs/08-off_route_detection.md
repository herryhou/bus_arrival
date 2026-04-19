# Off-Route Detection Specification (v8.9)

## Overview

Detects when GPS consistently doesn't fit route geometry (>50 m for 5+ ticks). Implements hysteresis (5 to confirm, 2 to clear) with position freezing.

## Purpose

Off-route detection handles sustained GPS drift where positions consistently don't fit route geometry. This feature detects:

- **Urban canyon multipath** causing GPS drift away from road
- **Physical deviations** (detour, depot, wrong route loaded)

### Limitations

Cannot detect "along-route drift" where GPS stays on the road but advances faster than the bus. This requires external ground truth and is not detectable with a single GPS sensor.

## Invariants (MUST)

- [ ] **Threshold:** `OFF_ROUTE_D2_THRESHOLD = 25,000,000 cm²` (50 m)
- [ ] **Confirm ticks:** 5 consecutive ticks (avoid false positives from multipath)
- [ ] **Clear ticks:** 2 consecutive good ticks (fast re-acquisition)
- [ ] **Position frozen immediately** when `suspect_ticks = 0`
- [ ] **Arrival suppressed** during off-route episodes
- [ ] **GPS jump recovery:** find nearest stop ahead of frozen position

## State Transitions

```
Normal → Suspect (1st tick: d² > 50 m²)
Suspect → Off-Route (5th consecutive tick)
Off-Route → Normal (2 consecutive good ticks, d² ≤ 50 m²)
```

### During Suspect State

- Position frozen immediately (`frozen_s_cm` set)
- Skip projection and filters to prevent `s_cm` advance
- Return `DrOutage` with frozen position
- Increment `off_route_suspect_ticks`

### During Off-Route State

- Return `OffRoute` result with last valid position
- Suppress arrivals (detection layer must check `ProcessResult`)
- Maintain frozen position

### Recovery to Normal

- After 2 consecutive good matches (`off_route_clear_ticks >= 2`)
- Reset `off_route_suspect_ticks = 0`
- Clear `frozen_s_cm = None`
- Resume normal processing

## GPS Jump Recovery

When returning from off-route with a large GPS jump (>50 m):

1. Check if GPS jumped more than 50m (5000 cm) from frozen position
2. Find closest stop ahead of frozen position with constraints:
   - Stop must be ahead of frozen position (`stop.progress_cm > frozen_s`)
   - Distance within 200m (`dist < OFF_ROUTE_D2_THRESHOLD`)
   - Velocity constraint: reachable given elapsed time
3. If valid stop found, jump to it and set `in_recovery = true`
4. If no valid stop, clear frozen state and continue normal processing

## Constants

```rust
/// Off-route distance threshold (cm²) — 50m² = 25,000,000 cm²
const OFF_ROUTE_D2_THRESHOLD: i64 = 25_000_000;

/// Ticks to confirm off-route (avoid false positives from multipath)
const OFF_ROUTE_CONFIRM_TICKS: u8 = 5;

/// Ticks to clear off-route (fast re-acquisition)
const OFF_ROUTE_CLEAR_TICKS: u8 = 2;

/// Jump recovery threshold (cm)
const JUMP_RECOVERY_THRESHOLD: i64 = 5000;
```

## State Fields

```rust
pub struct KalmanState {
    // ... other fields ...
    /// Consecutive ticks with poor GPS match (off-route suspect counter)
    pub off_route_suspect_ticks: u8,
    /// Consecutive ticks with good GPS match (off-route clear counter)
    pub off_route_clear_ticks: u8,
    /// Frozen position when off-route is first suspected (for immediate position freezing)
    pub frozen_s_cm: Option<DistCm>,
}
```

## ProcessResult Variants

```rust
pub enum ProcessResult {
    /// GPS is off-route — position frozen, awaiting re-acquisition
    OffRoute {
        last_valid_s: DistCm,
        last_valid_v: SpeedCms,
    },
    /// Dead-reckoning mode (used during suspect state)
    DrOutage {
        s_cm: DistCm,
        v_cms: SpeedCms,
    },
    // ... other variants ...
}
```

## Integration Points

### Detection Layer

The detection layer must check `ProcessResult` and suppress arrivals during off-route:

```rust
match process_result {
    ProcessResult::OffRoute { .. } => {
        // Suppress arrivals, maintain current FSM state
    }
    ProcessResult::Valid { signals, .. } => {
        // Normal arrival detection
    }
    // ... other cases ...
}
```

### Map Matching

Map matching must provide `match_d2` (squared distance) for off-route detection:

```rust
let (seg_idx, match_d2) = find_best_segment_restricted(
    gps_x, gps_y, gps.heading_cdeg, gps.speed_cms,
    route_data, state.last_seg_idx, use_relaxed_heading
);

// Check off-route BEFORE projection
if match_d2 > OFF_ROUTE_D2_THRESHOLD {
    // Handle off-route detection
}
```

## Testing Considerations

### Test Scenarios

1. **Normal operation:** GPS consistently < 50m from route
2. **Transient multipath:** Single tick > 50m (should not trigger)
3. **Sustained multipath:** 5+ consecutive ticks > 50m (should trigger)
4. **Fast recovery:** Return to < 50m for 2 ticks (should clear)
5. **GPS jump recovery:** Large position jump after off-route
6. **Stop index recovery:** Find correct stop after jump

### Edge Cases

- GPS outage during off-route (reset counters)
- First fix warm-up period (skip off-route detection)
- Large jumps without valid stops (continue normal processing)
- Velocity constraint violations during recovery

## Related Files

- `crates/pipeline/gps_processor/src/kalman.rs` — Implementation
- `crates/shared/src/lib.rs` — `KalmanState` with off-route fields
- `docs/superpowers/specs/2026-04-14-off-route-detection-design.md` — Original design document

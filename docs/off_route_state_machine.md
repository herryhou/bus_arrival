# Off-Route State Machine

## Overview

The off-route detection state machine detects when GPS consistently doesn't match route geometry (distance > 50m for 5+ seconds), indicating either:
- Urban canyon multipath causing GPS drift away from road
- Physical deviation (detour, depot, wrong route loaded)

**Limitation:** Cannot detect "along-route drift" where GPS stays on the road but advances faster than the bus. This requires external ground truth.

## States

### Normal
- GPS matches route within 50m
- Position updates normally via Kalman filter
- `off_route_suspect_ticks = 0`
- `off_route_clear_ticks = 0`
- `frozen_s_cm = None`
- `off_route_freeze_time = None`

### Suspect(N)
- GPS has been >50m from route for N consecutive ticks (1-4)
- Position is **frozen** at last known good location
- `off_route_suspect_ticks = N` (1-4)
- `off_route_clear_ticks = 0`
- `frozen_s_cm = Some(last_valid_s)`
- `off_route_freeze_time = Some(timestamp of first suspect tick)`
- Returns `ProcessResult::DrOutage` to prevent position advance

### OffRoute
- GPS has been >50m from route for 5+ consecutive ticks
- Position remains frozen, recovery required
- `off_route_suspect_ticks >= 5`
- `off_route_clear_ticks = 0`
- `frozen_s_cm = Some(last_valid_s)`
- `off_route_freeze_time = Some(timestamp of first suspect tick)`
- Returns `ProcessResult::OffRoute` to trigger re-acquisition recovery

## State Transitions

```
Normal → Suspect(1)
    Guard: match_d2 > 25,000,000 (50m threshold)
    Action: Freeze position immediately, record freeze_time

Suspect(N) → Suspect(N+1)
    Guard: match_d2 > 25,000,000 and N < 4
    Action: Keep position frozen

Suspect(4) → OffRoute
    Guard: match_d2 > 25,000,000 (5th bad tick)
    Action: Return OffRoute result, trigger recovery flag

Suspect(N) → Normal
    Guard: match_d2 ≤ 25,000,000 for 2 consecutive ticks
    Action: Clear suspect counter, unfreeze position, clear freeze_time

OffRoute → Normal
    Guard: match_d2 ≤ 25,000,000 (first good tick after OffRoute)
    Action: Trigger M12 recovery, clear freeze_time after recovery

Any → Suspect(1) (via GPS outage)
    Guard: GPS outage occurs
    Action: Reset all counters, clear freeze_time
```

## Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `OFF_ROUTE_D2_THRESHOLD` | 25,000,000 cm² | 50m distance threshold |
| `OFF_ROUTE_CONFIRM_TICKS` | 5 | Ticks to confirm off-route |
| `OFF_ROUTE_CLEAR_TICKS` | 2 | Ticks to clear off-route |

## Interactions with Other Modules

### Warmup (is_first_fix=true)
- All off-route transitions are **disabled** during warmup
- Prevents false positives during Kalman filter initialization
- `off_route_suspect_ticks` never increments during warmup

### GPS Outage (has_fix=false)
- Resets all off-route counters to 0
- Clears `off_route_freeze_time`
- Conservative: requires fresh off-route detection after outage

### Speed Constraint Filter
- Uses **frozen position** when in Suspect or OffRoute state
- Prevents position advance during off-route episode
- Formula: `current_s = frozen_s_cm.unwrap_or(s_cm)`

### Monotonicity Filter
- Uses **frozen position** when in Suspect or OffRoute state
- Prevents backward position jumps during off-route episode
- Formula: `current_s = frozen_s_cm.unwrap_or(s_cm)`

### Re-acquisition Recovery (M12)
- Triggered when returning from OffRoute to Normal
- Uses `freeze_time` to calculate elapsed time for velocity constraint
- M12 recovery scans all stops to find correct index
- Clears `needs_recovery_on_reacquisition` flag after completion

## ProcessResult Return Values

| State | ProcessResult | Position Used |
|-------|----------------|---------------|
| Normal | `Valid { signals, v_cms, seg_idx }` | Kalman-filtered |
| Suspect(N) | `DrOutage { s_cm, v_cms }` | Frozen position |
| OffRoute | `OffRoute { last_valid_s, last_valid_v, freeze_time }` | Frozen position |

## Key Implementation Details

### Immediate Position Freezing
Position is frozen **immediately** on first suspect tick (N=1), not when OffRoute is confirmed. This prevents position drift during the 5-tick confirmation period.

### Accurate Freeze Time (Bug 5 Fix)
`off_route_freeze_time` is set when position first freezes (tick 1), not when OffRoute is confirmed (tick 5). This ensures M12 recovery calculates accurate elapsed time for velocity constraint validation.

### Hysteresis Prevents Flapping
The 5-tick confirmation and 2-tick clear thresholds prevent false positives from transient multipath while allowing fast re-acquisition.

### No §4.5 Inline Recovery (Bug 2 Fix)
Previous implementation had conflicting recovery mechanisms. Now M12 handles all post-off-route recovery scenarios with clean input (raw GPS projection, not pre-snapped).

## Testing

### Unit Tests (`test_off_route_detection.rs`)
- `test_off_route_confirms_after_5_ticks` - Basic hysteresis
- `test_off_route_disabled_during_warmup` - Warmup guard
- `test_off_route_clears_after_2_good_ticks` - Clear hysteresis
- `test_off_route_hysteresis_partial_clear` - Partial clear scenario
- `test_off_route_counter_resets_on_outage` - Outage interaction

### Integration Tests (`test_off_route_integration.rs`)
- `test_off_route_freezes_position` - Basic position freezing
- `test_re_acquisition_runs_recovery` - Recovery infrastructure
- `test_full_off_route_cycle` - Complete off-route → recovery → normal cycle
- `test_off_route_freeze_time_set_once` - Bug 1 regression test
- `test_m12_recovery_works_without_section_4_5` - Bug 2 regression test

## Related Files

- Implementation: `crates/pipeline/gps_processor/src/kalman.rs`
- State machine: `crates/pico2-firmware/src/state.rs`
- Types: `crates/shared/src/lib.rs`
- Design: `docs/superpowers/specs/2026-04-25-off-route-refactoring-design.md`

# D3 + H1 Speed Constraint and Recovery Fix Design

**Date:** 2026-04-12
**Status:** Approved
**Related:** Code review findings in `docs/claude_review.md`

---

## Overview

**Goal:** Fix D3 (speed constraint) and H1 (recovery module) to achieve spec compliance.

**Scope:**
- D3: Change `V_MAX_CMS` from 3000 to 1667 cm/s (60 km/h city bus), `SIGMA_GPS_CM` from 5000 to 2000 cm
- H1: Wire `recovery::find_stop_index` into firmware state machine with GPS jump trigger conditions
- Update recovery module to use corrected speed constraint

**Files affected:**
- `crates/pipeline/gps_processor/src/kalman.rs` - Speed constraint constants
- `crates/pipeline/detection/src/recovery.rs` - Uses V_MAX_CMS
- `crates/pico2-firmware/src/state.rs` - Wire recovery into main loop

**Not affected:** D1 (F1/F3 signals) and D2 (monotonicity threshold) - deferred

---

## D3 - Speed Constraint Fix

### Current (Incorrect)

```rust
const V_MAX_CMS: SpeedCms = 3000;  // 108 km/h
const SIGMA_GPS_CM: DistCm = 5000;  // 50 m
// max_dist = 3000 * dt + 5000 = 8000 cm for dt=1
```

### Fixed (Spec-Compliant)

```rust
/// Maximum bus speed for city bus operations: 60 km/h = 1667 cm/s
/// Per spec Section 9.1: urban transit routes, not highway speeds
const V_MAX_CMS: SpeedCms = 1667;

/// GPS noise margin for urban canyon conditions: 20 m
/// Per spec Section 9.1: accommodates multipath errors
const SIGMA_GPS_CM: DistCm = 2000;
// max_dist = 1667 * dt + 2000 = 3667 cm for dt=1
```

### Changes Required

1. `kalman.rs`: Update `V_MAX_CMS` and `SIGMA_GPS_CM` constants with explanatory comments
2. `recovery.rs`: Update `V_MAX_CMS` from 3000 to 1667 with same rationale

### Behavior Impact

GPS samples with position changes > 36.67 m between 1-second ticks will be rejected (vs 80 m currently). This is intentional per spec for city bus scenario.

---

## H1 - Recovery Module Wiring

### Current State

`recovery::find_stop_index` exists in `crates/pipeline/detection/src/recovery.rs` but is never called in firmware.

### Integration Point

In `state.rs::process_gps()`, after valid GPS processing but before arrival detection:

```rust
// After Module ⑦ Kalman filter output
let (s_cm, v_cms) = match result {
    ProcessResult::Valid { s_cm, v_cms, seg_idx } => {
        // NEW: Check for GPS jump requiring recovery
        if should_trigger_recovery(s_cm, self.kalman.s_cm, self.kalman.last_seg_idx, seg_idx) {
            if let Some(recovered_idx) = find_stop_index(
                s_cm,
                v_cms,
                dt_since_last_fix,
                self.route_data.stops,
                self.last_known_stop_index,
            ) {
                self.last_known_stop_index = recovered_idx as u8;
                // Reset stop states for consistency
                reset_stop_states_after_recovery(&mut self.stop_states, recovered_idx);
            }
        }
        ...
    }
}
```

### Trigger Conditions (Per Spec Section 15.1)

1. **GPS jump > 200 m** between consecutive fixes
2. **Restart mismatch** - detected stop index vs stored value
3. **Sustained position/stop divergence** - stop ambiguity persists

### State Additions Required

```rust
pub struct State<'a> {
    // ... existing fields ...
    /// Last confirmed stop index for GPS jump recovery
    last_known_stop_index: u8,
    /// Last valid position for jump detection (cm)
    last_valid_s_cm: DistCm,
}
```

### Trigger Function

```rust
fn should_trigger_recovery(
    s_cm: DistCm,
    prev_s_cm: DistCm,
    prev_seg_idx: usize,
    new_seg_idx: usize,
) -> bool {
    // Condition 1: GPS jump > 200 m
    let jump_distance = (s_cm - prev_s_cm).abs() as u32;
    if jump_distance > 20000 {
        return true;
    }

    // Condition 2: Segment discontinuity (route jump)
    let seg_jump = if new_seg_idx > prev_seg_idx {
        new_seg_idx - prev_seg_idx
    } else {
        prev_seg_idx - new_seg_idx
    };
    if seg_jump > 10 {  // More than 10 segments = likely anomaly
        return true;
    }

    false
}
```

---

## Error Handling

### Recovery Failure Handling

- If `find_stop_index` returns `None` (no valid candidate): Keep `last_known_stop_index` unchanged, log warning
- If recovery succeeds but index is unexpected: Use recovered value, log for debugging
- If recovery module panics: Firmware should continue without recovery (graceful degradation)

### State Consistency After Recovery

```rust
fn reset_stop_states_after_recovery(
    stop_states: &mut heapless::Vec<StopState, 256>,
    recovered_idx: usize,
) {
    // Reset all stop states to Idle
    for state in stop_states.iter_mut() {
        state.reset();
    }
    // Optional: Mark recovered stop as Approaching if within corridor
}
```

### Edge Cases

- **Cold start** (no `last_known_stop_index`): Assume index 0, don't trigger recovery
- **All stops excluded** by velocity constraint: Recovery returns None, continue normally
- **Recovery index exceeds** route stop count: Invalid state, log and skip

---

## Testing

### Unit Tests for Recovery Trigger

1. `test_recovery_triggered_by_200m_jump` - GPS jump > 200m triggers recovery
2. `test_recovery_finds_correct_stop_after_jump` - Recovery finds nearest valid stop
3. `test_recovery_velocity_exclusion` - Stops beyond V_MAX * dt are excluded
4. `test_recovery_index_penalty` - Backward jumps penalized correctly
5. `test_no_recovery_for_small_gps_noise` - Small GPS jumps don't trigger recovery
6. `test_recovery_segment_jump` - Large segment discontinuity triggers recovery

### Unit Tests for Speed Constraint

1. `test_speed_constraint_rejects_37m_jump` - New 3667 cm limit enforced
2. `test_speed_constraint_allows_normal_movement` - Legitimate movement passes
3. `test_speed_constraint_dt_scaling` - Constraint scales with time delta

### Integration Tests

1. `test_full_recovery_flow` - End-to-end GPS jump → recovery → state reset
2. `test_recovery_with_corridor_entry` - Recovery followed by corridor detection

---

## Implementation Notes

### Order of Changes (Important)

1. First fix `V_MAX_CMS` and `SIGMA_GPS_CM` in `kalman.rs` (D3)
2. Then update `V_MAX_CMS` in `recovery.rs` to use the corrected value
3. Finally wire recovery into `state.rs` (H1)

This order ensures recovery uses the correct speed constraint from the start.

### Constants Centralization

Consider moving `V_MAX_CMS` to `shared` crate so both `kalman.rs` and `recovery.rs` use the same source. This prevents future divergence.

### Function Signature Alignment

The current `recovery::find_stop_index` signature:
```rust
pub fn find_stop_index(
    s_cm: DistCm,
    _v_filtered: SpeedCms,  // Reserved for future use
    dt_since_last_fix: u64,
    stops: &[Stop],
    last_index: u8,
) -> Option<usize>
```

Note: `v_filtered` is currently unused but reserved. We'll keep it for future enhancements.

---

## Spec Compliance

- D3: Fully aligned with spec Section 9.1 (speed constraint: D_max = 3667 cm)
- H1: Fully aligned with spec Section 15.1 (stop index recovery)
- Constants match spec Appendix (V_max = 1667 cm/s, σ_gps = 2000 cm)

---

## Summary

| Issue | Change | Impact |
|-------|--------|--------|
| D3 | V_MAX: 3000→1667, GPS_σ: 5000→2000 | Stricter GPS acceptance, spec compliant |
| H1 | Wire recovery module into main loop | Enables GPS jump recovery per spec |

**Deferred:** D1 (F1/F3 signal separation) and D2 (monotonicity threshold)

# Off-Route Bug Fixes Design

**Date:** 2026-04-27
**Status:** Design
**Related:** `docs/off-route_review_by_claude.md`

## Overview

This design addresses 11 issues identified in the off-route detection and recovery system:
- 3 Critical (C1-C3): Runtime bugs causing data corruption
- 5 Medium (M1-M5): Incorrect behavior under specific conditions
- 3 Low (L1-L3): Documentation errors (not in scope for this design)

**Fix Strategy:** Tiered approach - Critical fixes first, validated, then Medium fixes.

## Validation Status

All issues were validated against actual code before design:

| ID | Component | Validated | Severity |
|----|-----------|-----------|----------|
| C1 | `state.rs` Valid branch | ✅ | Critical |
| C2 | `reset_off_route_state` | ✅ | Critical |
| C3 | `find_stop_index` | ✅ | Critical |
| M1 | `kalman.rs` Suspect path | ✅ | Medium |
| M2 | `probability.rs` | ✅ | Medium |
| M3 | Re-entry snap | ✅ | Medium |
| M4 | `find_stop_index` | ✅ | Medium |
| M5 | `should_persist` | ✅ | Medium |

## Tier 1: Critical Fixes

### C2: `reset_off_route_state` must clear `frozen_s_cm`

**File:** `crates/pipeline/gps_processor/src/kalman.rs`, line 105-109

**Problem:** Function clears `off_route_suspect_ticks`, `off_route_clear_ticks`, and `off_route_freeze_time` but NOT `frozen_s_cm`. This causes:
- Spurious re-entry snaps after GPS outage during Suspect state
- `frozen_s_cm` persists indefinitely through long outages

**Fix:**
```rust
pub fn reset_off_route_state(state: &mut KalmanState) {
    state.off_route_suspect_ticks = 0;
    state.off_route_clear_ticks = 0;
    state.off_route_freeze_time = None;
    state.frozen_s_cm = None;  // ← ADD THIS
}
```

**Dependencies:** None. Fix first.

### C1: `off_route_freeze_time` cleared before recovery use

**File:** `crates/pipeline/gps_processor/src/kalman.rs`, line 92

**Problem:** `update_off_route_hysteresis` clears `off_route_freeze_time` at Normal transition (line 92). Later, `state.rs` line 291 tries to calculate `elapsed_seconds` using this field, but it's already `None`, falling back to `dt=1`. This causes the velocity exclusion window to collapse to 1667 cm, hard-excluding stops >16m away.

**Fix:** Remove line 92 from `kalman.rs`. Clear `off_route_freeze_time` in `state.rs` after both recovery paths complete:

```rust
// state.rs: After both recovery blocks (~line 320)
self.kalman.off_route_freeze_time = None;
```

**Dependencies:** Requires C2 fix first (freeze state must be consistent).

### C3: Recovery has no pre-freeze spatial anchor

**File:** `crates/pipeline/detection/src/recovery.rs`, `find_stop_index`

**Problem:** Recovery only uses `last_index` for backward exclusion. No use of frozen position (`frozen_s_cm`) or direction of travel before freeze. On routes with loops, parallel segments, or closely-spaced stops, recovery can select a stop cluster that is physically inconsistent with pre-freeze trajectory.

**Fix:** Store `FreezeContext` at freeze time, pass to recovery:

```rust
// New struct (shared crate or gps_processor)
pub struct FreezeContext {
    pub frozen_s_cm: DistCm,
    pub frozen_stop_idx: u8,
}

// Store at freeze time (kalman.rs line 68)
// Current stop index must be passed from state.rs (self.last_known_stop_index)
state.freeze_ctx = Some(FreezeContext {
    frozen_s_cm: state.s_cm,
    frozen_stop_idx: current_stop_idx,  // Passed from caller
});

// Pass to recovery (both call sites in state.rs)
pub fn find_stop_index(
    s_cm: DistCm,
    v_filtered: SpeedCms,
    dt_since_last_fix: u64,
    stops: &[Stop],
    last_index: u8,
    freeze_ctx: &Option<FreezeContext>,  // ← NEW
) -> Option<usize>
```

**Recovery scoring update:**
- Add spatial anchor penalty: `10000 * max(0, frozen_stop_idx - i)` for stops behind frozen position
- Only apply if `freeze_ctx.is_some()` and `s_cm < frozen_s_cm - 5000` (bus behind freeze point)

**Dependencies:** Benefits from C1 fix (correct `dt` makes spatial anchor more effective). Requires `freeze_ctx` field in `KalmanState`.

## Tier 2: Medium Fixes

### M1: `SuspectOffRoute` should be separate from `DrOutage`

**File:** `crates/pipeline/gps_processor/src/kalman.rs`, line 214-217

**Problem:** During Suspect (ticks 1-4), returns `DrOutage` with frozen position. This is indistinguishable from genuine GPS signal loss, causing `warmup_total_ticks` to increment and potentially unblocking detection with frozen position.

**Fix:**
```rust
// New variant
pub enum ProcessResult {
    Valid { ... },
    Rejected(&'static str),
    Outage,
    DrOutage { s_cm, v_cms },
    OffRoute { last_valid_s, last_valid_v, freeze_time },
    SuspectOffRoute { s_cm, v_cms },  // ← NEW
}

// kalman.rs line 214: Return SuspectOffRoute
OffRouteStatus::Suspect => {
    dr.last_gps_time = Some(gps.timestamp);
    return ProcessResult::SuspectOffRoute {
        s_cm: state.frozen_s_cm.unwrap_or(state.s_cm),
        v_cms: state.v_cms,
    };
}

// state.rs: Handle same as OffRoute
ProcessResult::SuspectOffRoute { .. } => {
    self.needs_recovery_on_reacquisition = true;
    return None;  // Suppress detection, don't increment warmup
}
```

### M2: `OffRoute` should fully suppress probability

**File:** `crates/pipeline/detection/src/probability.rs`

**Problem:** Both `OffRoute` and `DrOutage` neutralize only `p3` to 128. For `OffRoute`, the vehicle is not on the route at all; the entire probability computation is meaningless. `p1`, `p2`, `p4` are still computed and can push probability above threshold 191.

**Fix:**
```rust
// At start of probability calculation
if gps_status == GpsStatus::OffRoute {
    return 0;  // Entire probability invalid
}
```

**Dependencies:** Works with M1 fix (SuspectOffRoute also suppressed).

### M3: Re-entry should blend `v_cms`, not hard-assign

**File:** `crates/pipeline/gps_processor/src/kalman.rs`, line 237

**Problem:** First GPS after re-entry is typically worst-quality (HDOP spike, heading uncertainty). Hard-assigning `state.v_cms = gps.speed_cms` bypasses EMA smoothing used elsewhere.

**Fix:**
```rust
// Blend instead of hard assignment
state.v_cms = state.v_cms + 3 * (gps.speed_cms.max(0).min(V_MAX_CMS) - state.v_cms) / 10;
```

### M4: Use actual speed in recovery velocity constraint

**File:** `crates/pipeline/detection/src/recovery.rs`, line 56

**Problem:** `_v_filtered` parameter is "reserved for future use". Velocity exclusion uses worst-case `V_MAX_CMS * dt`, allowing physically impossible stop selection.

**Fix:**
```rust
// Replace line 56
let effective_v = if v_filtered > 0 { v_filtered as u64 } else { V_MAX_CMS as u64 };
let max_reachable = effective_v * dt_since_last_fix;
```

**Dependencies:** Works better after C1 fix (correct `dt`).

### M5: Gate persistence on off-route status

**File:** `crates/pico2-firmware/src/state.rs`, line 581

**Problem:** `should_persist` doesn't check off-route/Suspect status. Can write incorrect anchor to Flash during Suspect state.

**Fix:**
```rust
pub fn should_persist(&self, current_stop: u8) -> bool {
    if self.kalman.frozen_s_cm.is_some() { return false; }
    if self.kalman.off_route_suspect_ticks > 0 { return false; }
    // ... existing checks
}
```

## Implementation Order

**Tier 1 (Critical):**
1. C2: `reset_off_route_state` - add `frozen_s_cm = None`
2. C1: Preserve `off_route_freeze_time` - remove line 92, add clear in state.rs
3. C3: Add `FreezeContext` - new struct, field in `KalmanState`, update recovery

**Tier 2 (Medium):**
1. M1: Add `SuspectOffRoute` variant
2. M3: Blend `v_cms` on re-entry
3. M4: Use actual speed in recovery
4. M2: Full probability suppression
5. M5: Gate persistence

## Testing

Each fix requires integration test coverage:

**C2:** GPS outage during Suspect → verify no spurious snap on recovery
**C1:** Off-route episode with recovery → verify correct `dt` used
**C3:** Route with loop/detour → verify recovery selects spatially consistent stop
**M1:** Suspect state → verify detection suppressed, warmup not incremented
**M2-M5:** Corresponding integration scenarios

## Files Modified

- `crates/pipeline/gps_processor/src/kalman.rs`
- `crates/pipeline/detection/src/recovery.rs`
- `crates/pipeline/detection/src/probability.rs`
- `crates/pico2-firmware/src/state.rs`
- `crates/shared/src/types.rs` (potential `FreezeContext` location)

## Rollback Plan

Each fix is independently revertable. If issues arise:
- Revert the specific commit
- Add integration test to capture regression
- Re-apply fix with test coverage

# D4: Arriving → Idle Transition Fix

**Date:** 2026-04-12
**Status:** Design Approved
**Related:** Code review finding in `docs/claude_review.md`

---

## Overview

**Goal:** Add missing corridor exit transition from `Arriving` state back to `Idle` to prevent the FSM from getting stuck with `dwell_time_s` incrementing indefinitely when GPS drifts backward.

**Scope:**
- Add corridor exit check to `Arriving` state (matching `Approaching`)
- Reset `dwell_time_s` on corridor exit
- Preserve `announced` flag (one-time announcement rule)
- Add unit test for the new transition

**Files affected:**
- `crates/pipeline/detection/src/state_machine.rs` - Add corridor exit check in `Arriving` state

---

## Problem Statement

Per code review D4: The `Arriving` state has no exit path back to `Idle` when `s_cm < corridor_start_cm`. If GPS drifts backward while in `Arriving`, the FSM stays stuck with `dwell_time_s` incrementing forever.

**Current behavior:**
```
Arriving (s_cm < corridor_start_cm)
  → Stuck in Arriving forever
  → dwell_time_s keeps incrementing
  → Stop remains "active" indefinitely
```

**Expected behavior:**
```
Arriving (s_cm < corridor_start_cm)
  → Transition to Idle
  → Reset dwell_time_s to 0
  → Stop becomes inactive
```

---

## Solution Design

### State Machine Update

Add a corridor exit check to the `Arriving` state that mirrors the existing logic in `Approaching`:

```rust
FsmState::Arriving => {
    // Existing: Arrival transition
    if d_to_stop < 5000 && probability > THETA_ARRIVAL {
        self.fsm_state = FsmState::AtStop;
        self.dwell_time_s += 1;
        self.last_probability = probability;
        self.announced = true;
        return StopEvent::Arrived;
    }

    // Existing: Departure transition
    if d_to_stop > 4000 && s_cm > stop_progress {
        self.fsm_state = FsmState::Departed;
        self.last_probability = probability;
        return StopEvent::Departed;
    }

    // NEW: Corridor exit check
    if s_cm < corridor_start_cm {
        self.fsm_state = FsmState::Idle;
        self.dwell_time_s = 0;
        // announced flag NOT reset (preserves one-time rule)
        return StopEvent::None;
    }

    self.dwell_time_s += 1;
}
```

### State Transition Diagram

```
                    ┌─────────────────────┐
                    │      Arriving       │
                    └──────────┬──────────┘
                               │
         ┌─────────────────────┼─────────────────────┐
         │                     │                     │
    d<5000,            d>4000,            s<corridor
    prob>191            s>stop              start
         │                     │                     │
         ▼                     ▼                     ▼
    ┌─────────┐          ┌──────────┐         ┌─────────┐
    │ AtStop  │          │ Departed │         │  Idle   │
    └─────────┘          └──────────┘         └─────────┘
                                              (dwell=0)
```

### Key Design Decisions

**1. Reset `dwell_time_s` on corridor exit**
- Prevents inflated dwell values if corridor is re-entered later
- Matches `Approaching` behavior (consistency)
- Fresh dwell count for next corridor entry

**2. Preserve `announced` flag**
- User choice: stricter one-time announcement rule
- Prevents duplicate announcements if GPS noise causes exit/re-entry
- Stop cannot be announced again even after leaving corridor

**3. Return `StopEvent::None` on corridor exit**
- Corridor exit is not an arrival/departure event
- Matches `Approaching` behavior
- No external notification needed

---

## Testing

### Unit Test: `test_arriving_to_idle_on_corridor_exit`

```rust
#[test]
fn test_arriving_to_idle_on_corridor_exit() {
    let mut state = StopState::new(0);
    let stop_progress = 10000;
    let corridor_start_cm = 2000;

    // Enter corridor and reach Arriving state
    state.update(5000, 100, stop_progress, corridor_start_cm, 0);
    assert_eq!(state.fsm_state, FsmState::Approaching);

    state.update(6000, 100, stop_progress, corridor_start_cm, 100);
    assert_eq!(state.fsm_state, FsmState::Arriving);
    assert_eq!(state.dwell_time_s, 1);

    // GPS drifts backward past corridor start
    state.update(1000, 100, stop_progress, corridor_start_cm, 50);

    // Should transition to Idle and reset dwell_time
    assert_eq!(state.fsm_state, FsmState::Idle,
        "Arriving should transition to Idle on corridor exit");
    assert_eq!(state.dwell_time_s, 0,
        "dwell_time_s should be reset on corridor exit");
}
```

### Edge Cases Covered

1. **Normal corridor exit:** GPS drifts backward → `Idle`
2. **Re-entry after exit:** Can re-enter corridor but `announced` flag prevents re-announcement
3. **Dwell time reset:** Fresh dwell count on re-entry
4. **No event generated:** `StopEvent::None` returned (not arrival/departure)

---

## Verification

### Success Criteria

- [ ] `Arriving` state transitions to `Idle` when `s_cm < corridor_start_cm`
- [ ] `dwell_time_s` is reset to 0 on corridor exit
- [ ] `announced` flag is preserved (not reset)
- [ ] Unit test passes
- [ ] No regression in existing tests

### Expected Behavior Changes

**Before fix:**
- GPS drift backward in `Arriving` → stuck forever
- `dwell_time_s` increments indefinitely

**After fix:**
- GPS drift backward in `Arriving` → transition to `Idle`
- `dwell_time_s` resets to 0
- Stop becomes inactive (no longer in active stops list)

---

## Summary

| Change | Impact |
|--------|--------|
| Add corridor exit check to `Arriving` | Prevents FSM stuck state |
| Reset `dwell_time_s` on exit | Prevents inflated dwell values |
| Preserve `announced` flag | Maintains one-time announcement rule |

**Files to modify:** 1 file (`state_machine.rs`)
**New tests:** 1 unit test
**Breaking changes:** None

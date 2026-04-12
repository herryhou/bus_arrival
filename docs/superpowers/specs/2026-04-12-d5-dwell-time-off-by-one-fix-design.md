# D5: Dwell-Time Counter Off-by-One Fix

**Date:** 2026-04-12
**Status:** Design Approved
**Related:** Code review finding in `docs/claude_review.md`

---

## Overview

**Goal:** Fix dwell-time counter to start counting when `Approaching` state is entered, not on the tick after.

**Scope:**
- Remove `else` wrapper in `Approaching` branch that delays first dwell increment
- Update test assertions to verify correct behavior
- Ensure all existing tests still pass

**Files affected:**
- `crates/pipeline/detection/src/state_machine.rs` - Fix Approaching branch logic
- `crates/pipeline/detection/src/state_machine.rs` - Update test assertions

---

## Problem Statement

Per code review D5: The spec says `τ_dwell` starts counting from when `Approaching` is entered. In the current `update()` method, the `Idle` arm transitions the FSM to `Approaching` but does not increment `dwell_time_s`; the increment only fires on the next tick when already in `Approaching`.

**Current behavior:**
```
Tick 1: Idle → Approaching (transition only)
Tick 2: Approaching (still in corridor) → dwell_time_s = 1
Tick 3: Approaching (still in corridor) → dwell_time_s = 2
...
Tick T: dwell_time_s = T - 1 (off-by-one!)
```

**Expected behavior:**
```
Tick 1: Idle → Approaching → dwell_time_s = 1 (counting starts when entering)
Tick 2: Approaching → dwell_time_s = 2
...
Tick T: dwell_time_s = T
```

**Impact:** After 10 seconds in corridor, `dwell_time_s = 9` instead of `10`. For `T_ref = 10s`, the dwell feature computes `p4 = (9 × 255 / 10) = 229` instead of `255`, under-weighting the dwell feature in arrival probability.

---

## Solution Design

### Root Cause

The `Approaching` branch uses an `else` wrapper that excludes the corridor exit check from the dwell increment:

```rust
FsmState::Approaching => {
    if d_to_stop < 5000 {
        self.fsm_state = FsmState::Arriving;
    }
    // Can exit corridor back to Idle if we leave the corridor
    if s_cm < corridor_start_cm {
        self.fsm_state = FsmState::Idle;
        self.dwell_time_s = 0;
    } else {
        // Update dwell time when in corridor
        self.dwell_time_s += 1;
    }
}
```

The `else` means "only increment if we didn't exit corridor". But the corridor exit check has already returned, so the first Approaching tick (right after Idle transition) is excluded from the increment.

### Fix

Remove the `else` wrapper and always increment when in corridor:

```rust
FsmState::Approaching => {
    if d_to_stop < 5000 {
        self.fsm_state = FsmState::Arriving;
    }
    
    // Can exit corridor back to Idle if we leave the corridor
    if s_cm < corridor_start_cm {
        self.fsm_state = FsmState::Idle;
        self.dwell_time_s = 0;
    }
    
    // Update dwell time when in corridor (including first tick after transition)
    if s_cm >= corridor_start_cm {
        self.dwell_time_s += 1;
    }
}
```

Now the dwell increment happens on every tick where `s_cm >= corridor_start_cm`, including the first tick after the Idle → Approaching transition.

### Design Rationale

**Why this approach:**
1. **Separation of concerns:** Idle branch handles transitions only. Dwell counting belongs in the state that cares about dwell time.
2. **Consistency:** All dwell time counting happens within the corridor-active states.
3. **Minimal change:** Single logic fix, no new state or flags needed.

**Why not increment in Idle branch:**
- Adding dwell logic to Idle would mix concerns (transition vs. timing)
- Idle doesn't "know" about corridors or dwell time
- Keeps transition logic clean and focused

---

## State Transition Diagram

```
Tick 1: Idle → Approaching
         s_cm >= corridor_start_cm
         dwell_time_s = 1 ✓ (FIXED)

Tick 2: Approaching (still in corridor)
         dwell_time_s = 2

Tick 3: Approaching (still in corridor)
         dwell_time_s = 3

...

Exit corridor: Approaching → Idle
         s_cm < corridor_start_cm
         dwell_time_s = 0 (reset)
```

---

## Testing

### Update Existing Test

**Test:** `test_idle_to_approaching_on_corridor_entry` (line 305)

**Current assertion (line 320):**
```rust
assert_eq!(state.dwell_time_s, 0); // No increment on transition tick
```

**Update to:**
```rust
assert_eq!(state.dwell_time_s, 1, "dwell_time_s should be 1 on corridor entry");
```

### Test Coverage

The fix affects the following test scenarios:

1. **Normal corridor entry:** dwell_time_s increments immediately on transition
2. **Multiple ticks in corridor:** Each tick increments dwell_time_s
3. **Corridor exit:** dwell_time_s resets to 0
4. **Re-entry:** Second entry immediately starts counting from 1

All existing tests should pass with updated expectations.

---

## Verification

### Success Criteria

- [ ] `Approaching` branch always increments `dwell_time_s` when in corridor
- [ ] First tick after Idle → Approaching transition sets `dwell_time_s = 1`
- [ ] Test assertion updated to verify correct behavior
- [ ] All existing tests pass
- [ ] No regression in arrival detection behavior

### Expected Behavior Changes

**Before fix:**
- Enter corridor → `dwell_time_s = 0` (no increment on transition tick)
- After 10 seconds in corridor → `dwell_time_s = 9`
- For `T_ref = 10s`: `p4 = 229` (under-weighted)

**After fix:**
- Enter corridor → `dwell_time_s = 1` (counts from entry)
- After 10 seconds in corridor → `dwell_time_s = 10`
- For `T_ref = 10s`: `p4 = 255` (correct per spec)

---

## Summary

| Change | Impact |
|--------|--------|
| Remove `else` wrapper in Approaching | Dwell counts from corridor entry, not next tick |
| Update test assertion | Verify correct behavior |
| Minimal code change | Single logic fix, no new state |

**Files to modify:** 1 file (`state_machine.rs`)
**Tests to update:** 1 test assertion
**Lines changed:** ~5

The fix ensures dwell-time counting starts when the `Approaching` state is entered, matching spec Section 13.2's intent for the `τ_dwell` parameter.

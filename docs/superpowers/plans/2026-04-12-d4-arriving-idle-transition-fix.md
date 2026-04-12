# D4 Arriving → Idle Transition Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add missing corridor exit transition from `Arriving` state back to `Idle` to prevent FSM stuck state.

**Architecture:** Add a single corridor exit check (`s_cm < corridor_start_cm`) to the `Arriving` state match arm that transitions to `Idle` and resets `dwell_time_s`, matching existing behavior in `Approaching` state.

**Tech Stack:** Rust, no_std embedded firmware, existing state machine in `crates/pipeline/detection/src/state_machine.rs`

---

## File Structure

**Single file modification:**
- `crates/pipeline/detection/src/state_machine.rs` - Add corridor exit check to `Arriving` state and unit test

No new files. This is a focused bug fix to the existing FSM.

---

## Task 1: Write failing test for Arriving → Idle transition

**Files:**
- Modify: `crates/pipeline/detection/src/state_machine.rs` (add test in `tests` module)

- [ ] **Step 1: Add the failing test to the tests module**

Location: After line 441 (after `test_should_announce_requires_active_state`)

```rust
#[test]
fn test_arriving_to_idle_on_corridor_exit() {
    // D4 fix: Arriving state should transition to Idle when exiting corridor
    // This prevents FSM from getting stuck with dwell_time_s incrementing forever
    let mut state = StopState::new(0);
    let stop_progress = 10000;
    let corridor_start_cm = 2000;

    // Start: Enter corridor (Idle -> Approaching)
    state.update(5000, 100, stop_progress, corridor_start_cm, 0);
    assert_eq!(state.fsm_state, FsmState::Approaching);

    // Move to Arriving state (Approaching -> Arriving)
    state.update(6000, 100, stop_progress, corridor_start_cm, 100);
    assert_eq!(state.fsm_state, FsmState::Arriving);
    assert_eq!(state.dwell_time_s, 1, "dwell_time should be 1 after first Arriving tick");

    // GPS drifts backward past corridor start
    state.update(1000, 100, stop_progress, corridor_start_cm, 50);

    // Should transition to Idle and reset dwell_time
    assert_eq!(state.fsm_state, FsmState::Idle,
        "Arriving should transition to Idle when s_cm < corridor_start_cm");
    assert_eq!(state.dwell_time_s, 0,
        "dwell_time_s should be reset to 0 on corridor exit");
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p detection test_arriving_to_idle_on_corridor_exit`

Expected output:
```
test test_arriving_to_idle_on_corridor_exit ... FAILED

assertion `left == right`: Arriving should transition to Idle when s_cm < corridor_start_cm
  left: `Arriving`
 right: `Idle
```

- [ ] **Step 3: Commit the failing test**

```bash
git add crates/pipeline/detection/src/state_machine.rs
git commit -m "test(fsm): add failing test for Arriving to Idle corridor exit transition

D4: Arriving state has no exit path back to Idle when s_cm < corridor_start_cm.
Test verifies that GPS drift backward causes proper transition to Idle with
dwell_time_s reset."
```

---

## Task 2: Implement Arriving → Idle corridor exit check

**Files:**
- Modify: `crates/pipeline/detection/src/state_machine.rs` (update `Arriving` match arm)

- [ ] **Step 1: Add the corridor exit check to Arriving state**

Location: Inside the `FsmState::Arriving` match arm, after the departure check (after line 103)

Find this code:
```rust
FsmState::Arriving => {
    if d_to_stop < 5000 && probability > crate::probability::THETA_ARRIVAL {
        self.fsm_state = FsmState::AtStop;
        self.dwell_time_s += 1;
        self.last_probability = probability;
        self.announced = true;
        return StopEvent::Arrived;
    }
    if d_to_stop > 4000 && s_cm > stop_progress {
        self.fsm_state = FsmState::Departed;
        self.last_probability = probability;
        return StopEvent::Departed;
    }
    self.dwell_time_s += 1;
}
```

Replace with:
```rust
FsmState::Arriving => {
    if d_to_stop < 5000 && probability > crate::probability::THETA_ARRIVAL {
        self.fsm_state = FsmState::AtStop;
        self.dwell_time_s += 1;
        self.last_probability = probability;
        self.announced = true;
        return StopEvent::Arrived;
    }
    if d_to_stop > 4000 && s_cm > stop_progress {
        self.fsm_state = FsmState::Departed;
        self.last_probability = probability;
        return StopEvent::Departed;
    }
    // D4 fix: Corridor exit check (same logic as Approaching state)
    if s_cm < corridor_start_cm {
        self.fsm_state = FsmState::Idle;
        self.dwell_time_s = 0;
        // Note: announced flag is NOT reset (preserves one-time announcement rule)
        return StopEvent::None;
    }
    self.dwell_time_s += 1;
}
```

- [ ] **Step 2: Run the test to verify it passes**

Run: `cargo test -p detection test_arriving_to_idle_on_corridor_exit`

Expected output:
```
test test_arriving_to_idle_on_corridor_exit ... ok
```

- [ ] **Step 3: Run all state_machine tests to ensure no regression**

Run: `cargo test -p detection state_machine`

Expected: All tests pass

- [ ] **Step 4: Commit the fix**

```bash
git add crates/pipeline/detection/src/state_machine.rs
git commit -m "fix(fsm): add Arriving to Idle corridor exit transition (D4)

Per spec and code review D4: Arriving state has no exit path back to
Idle when s_cm < corridor_start_cm, causing FSM to get stuck with
dwell_time_s incrementing indefinitely.

Fix: Add corridor exit check matching Approaching state behavior:
- Transition to Idle when s_cm < corridor_start_cm
- Reset dwell_time_s to 0
- Preserve announced flag (one-time announcement rule)"
```

---

## Task 3: Verify full workspace tests pass

- [ ] **Step 1: Run all workspace tests**

Run: `cargo test --workspace`

Expected: All tests pass

- [ ] **Step 2: Run clippy on detection crate**

Run: `cargo clippy -p detection`

Expected: No warnings related to our changes

- [ ] **Step 3: Final verification commit**

```bash
git add -A
git commit -m "test(d4): verify all tests pass after Arriving-Idle fix

- All workspace tests passing
- No clippy warnings
- D4 fix complete: Arriving state now properly exits to Idle on
  corridor exit (s_cm < corridor_start_cm)"
```

---

## Summary

**Total tasks:** 3
**Files modified:** 1 (`state_machine.rs`)
**New tests:** 1
**Lines changed:** ~10 (corridor exit check + comment)

The fix adds a single corridor exit check to the `Arriving` state that:
1. Transitions to `Idle` when `s_cm < corridor_start_cm`
2. Resets `dwell_time_s` to 0 (prevents inflated dwell values)
3. Preserves `announced` flag (maintains one-time announcement rule)

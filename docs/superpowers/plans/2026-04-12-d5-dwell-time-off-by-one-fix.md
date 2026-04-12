# D5 Dwell-Time Counter Off-by-One Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix dwell-time counter to start counting from corridor entry, not the tick after transition.

**Architecture:** Remove `else` wrapper in `Approaching` branch so `dwell_time_s` increments on first tick in corridor (including transition tick from Idle).

**Tech Stack:** Rust, no_std embedded firmware, existing state machine in `crates/pipeline/detection/src/state_machine.rs`

---

## File Structure

**Single file modification:**
- `crates/pipeline/detection/src/state_machine.rs` - Fix Approaching branch dwell increment logic

No new files. This is a focused bug fix to existing FSM logic.

---

## Task 1: Write failing test for dwell-time counting from corridor entry

**Files:**
- Modify: `crates/pipeline/detection/src/state_machine.rs` (update test assertion)

- [ ] **Step 1: Update test assertion to expect correct behavior**

Find test `test_idle_to_approaching_on_corridor_entry` (around line 305-320).

**Find this assertion:**
```rust
// Enter corridor: should transition to Approaching
state.update(2000, 100, stop_progress, corridor_start_cm, 0);
assert_eq!(state.fsm_state, FsmState::Approaching);
assert_eq!(state.dwell_time_s, 0); // No increment on transition tick
```

**Replace with:**
```rust
// Enter corridor: should transition to Approaching
state.update(2000, 100, stop_progress, corridor_start_cm, 0);
assert_eq!(state.fsm_state, FsmState::Approaching);
assert_eq!(state.dwell_time_s, 1, "dwell_time_s should be 1 on corridor entry (D5 fix)");
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p detection test_idle_to_approaching_on_corridor_entry`

Expected output:
```
test test_idle_to_approaching_on_corridor_entry ... FAILED

assertion `left == right` failed
  left: `1`
 right: `0`
note: "D5 fix" message
```

- [ ] **Step 3: Commit the failing test**

```bash
git add crates/pipeline/detection/src/state_machine.rs
git commit -m "test(fsm): add failing test for corridor entry dwell counting (D5)

D5: Dwell-time counter should start counting when Approaching state
is entered (corridor entry), not on the tick after transition.

Test verifies that entering corridor sets dwell_time_s = 1, not 0."
```

---

## Task 2: Fix Approaching branch to count from corridor entry

**Files:**
- Modify: `crates/pipeline/detection/src/state_machine.rs` (update Approaching branch logic)

- [ ] **Step 1: Remove else wrapper in Approaching branch**

Find the `FsmState::Approaching` branch (around lines 79-91).

**Find this code:**
```rust
FsmState::Approaching => {
    if d_to_stop < 5000 {
        self.fsm_state = FsmState::Arriving;
    }
    // Can exit corridor back to Idle if we leave the corridor
    if s_cm < corridor_start_cm {
        self.fsm_state = FsmState::Idle;
        self.dwell_time_s = 0; // Reset dwell time when leaving corridor
    } else {
        // Update dwell time when in corridor
        self.dwell_time_s += 1;
    }
}
```

**Replace with:**
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

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test -p detection test_idle_to_approaching_on_corridor_entry`

Expected output:
```
test test_idle_to_approaching_on_corridor_entry ... ok
```

- [ ] **Step 3: Run all state_machine tests**

Run: `cargo test -p detection state_machine`

Expected: All tests pass

- [ ] **Step 4: Commit the fix**

```bash
git add crates/pipeline/detection/src/state_machine.rs
git commit -m "fix(fsm): count dwell_time_s from corridor entry, not next tick (D5)

D5 fix: Remove else wrapper in Approaching branch that delayed first
dwell increment. Now dwell_time_s starts counting when Approaching
state is entered (corridor entry), not on the tick after transition.

Before: Enter corridor → dwell_time_s=0, next tick → dwell_time_s=1
After:  Enter corridor → dwell_time_s=1 (immediate)

This ensures τ_dwell counts from when Approaching is entered, per spec.
After 10s in corridor: dwell_time_s=10 (not 9), so p4=255 (not 229)."
```

---

## Task 3: Update related test for consistency

**Files:**
- Modify: `crates/pipeline/detection/src/state_machine.rs` (update test assertions)

- [ ] **Step 1: Update test_dwell_time_only_counts_in_corridor assertions**

Find test `test_dwell_time_only_counts_in_corridor` (around line 347-376).

**Update the assertions to reflect new behavior:**

Find this section:
```rust
// Enter corridor: first tick transitions, no increment
state.update(5000, 100, stop_progress, corridor_start_cm, 0);
assert_eq!(state.fsm_state, FsmState::Approaching);
assert_eq!(state.dwell_time_s, 0);
```

Replace with:
```rust
// Enter corridor: first tick transitions AND increments dwell_time
state.update(5000, 100, stop_progress, corridor_start_cm, 0);
assert_eq!(state.fsm_state, FsmState::Approaching);
assert_eq!(state.dwell_time_s, 1, "First tick in corridor should count toward dwell");
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test -p detection test_dwell_time_only_counts_in_corridor`

Expected: PASS

- [ ] **Step 3: Commit the test update**

```bash
git add crates/pipeline/detection/src/state_machine.rs
git commit -m "test(fsm): update dwell_time test for D5 fix consistency

Update test_dwell_time_only_counts_in_corridor to reflect that
corridor entry now increments dwell_time_s immediately (not on
next tick). Ensures test assertions match fixed behavior."
```

---

## Task 4: Run full verification

- [ ] **Step 1: Run all state_machine tests**

Run: `cargo test -p detection state_machine`

Expected: All tests pass

- [ ] **Step 2: Run all detection crate tests**

Run: `cargo test -p detection`

Expected: All tests pass

- [ ] **Step 3: Run workspace tests**

Run: `cargo test --workspace`

Expected: All tests pass

- [ ] **Step 4: Run clippy**

Run: `cargo clippy -p detection`

Expected: No warnings related to our changes

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "test(d5): verify all tests pass after dwell-time counting fix

- All state_machine tests passing
- All detection tests passing
- All workspace tests passing
- No clippy warnings

D5 fix complete: Dwell-time counter now starts counting from corridor
entry, matching spec. After 10s in corridor: p4=255 (not 229)."
```

---

## Summary

**Total tasks:** 4
**Files modified:** 1 (`state_machine.rs`)
**Tests updated:** 2 test assertions
**Lines changed:** ~10

The fix removes the `else` wrapper in the `Approaching` branch so that `dwell_time_s` increments on every tick where `s_cm >= corridor_start_cm`, including the first tick after the Idle → Approaching transition. This ensures dwell-time counting starts when the `Approaching` state is entered, matching spec intent.

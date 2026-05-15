# Recovery Module Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix recovery module integration by replacing non-existent `detection::recovery::find_stop_index()` calls with the correct `crate::recovery::recover()` function.

**Architecture:** The recovery module exists at `src/recovery/` with a pure `recover()` function. Two call sites in `control/mod.rs` incorrectly reference `detection::recovery::find_stop_index()` which doesn't exist. Fix by using the existing `crate::recovery::RecoveryInput` struct and `recover()` function.

**Tech Stack:** Rust, no_std firmware, heapless Vec for stop collection.

---

## File Structure

**Files to modify:**
- `crates/pico2-firmware/src/control/mod.rs` — Fix two recovery call sites

**Files to reference:**
- `crates/pico2-firmware/src/recovery/mod.rs` — RecoveryInput definition
- `crates/pico2-firmware/src/recovery/search.rs` — recover() function implementation
- `crates/pico2-firmware/src/recovery_trigger.rs` — GPS jump detection

---

### Task 1: Write failing test for GPS jump recovery integration

**Files:**
- Create: `crates/pico2-firmware/src/control/recovery_integration_test.rs`

- [ ] **Step 1: Create test module file**

```rust
//! Integration tests for recovery module usage in control layer

#[cfg(test)]
mod recovery_integration_tests {
    use super::*;
    use crate::recovery::RecoveryInput;
    use heapless::Vec;
    use shared::Stop;

    #[test]
    fn test_recovery_input_gps_jump() {
        // Test that RecoveryInput can be constructed for GPS jump scenario
        let stops = Vec::from_slice(&[
            Stop { progress_cm: 1000, corridor_start_cm: 0, corridor_end_cm: 2000 },
            Stop { progress_cm: 5000, corridor_start_cm: 4000, corridor_end_cm: 6000 },
            Stop { progress_cm: 9000, corridor_start_cm: 8000, corridor_end_cm: 10000 },
        ]).unwrap();

        let input = RecoveryInput {
            s_cm: 5100,
            v_cms: 1000,
            dt_seconds: 5,
            stops,
            hint_idx: 1,
            frozen_s_cm: None,  // GPS jump: no frozen position
            search_window: 10,
        };

        // Should recover to stop 1
        let result = crate::recovery::recover(input);
        assert_eq!(result, Some(1));
    }

    #[test]
    fn test_recovery_input_reacquisition() {
        // Test that RecoveryInput can be constructed for re-acquisition scenario
        let stops = Vec::from_slice(&[
            Stop { progress_cm: 1000, corridor_start_cm: 0, corridor_end_cm: 2000 },
            Stop { progress_cm: 5000, corridor_start_cm: 4000, corridor_end_cm: 6000 },
        ]).unwrap();

        let input = RecoveryInput {
            s_cm: 4800,
            v_cms: 1000,
            dt_seconds: 10,
            stops,
            hint_idx: 1,
            frozen_s_cm: Some(5000),  // Re-acquisition: has frozen position
            search_window: 10,
        };

        // Should recover to stop 1 (within reach, spatial anchor penalty applied)
        let result = crate::recovery::recover(input);
        assert_eq!(result, Some(1));
    }
}
```

- [ ] **Step 2: Add test module to control/mod.rs**

Add at the end of `crates/pico2-firmware/src/control/mod.rs`:

```rust
#[cfg(test)]
mod recovery_integration_tests;
```

- [ ] **Step 3: Run test to verify it compiles and passes**

Run: `rtk cargo test -p pico2-firmware recovery_integration_tests`

Expected: COMPILE FAIL (test module created but not yet integrated into control flow)

- [ ] **Step 4: Commit**

```bash
rtk git add crates/pico2-firmware/src/control/recovery_integration_test.rs
rtk git add crates/pico2-firmware/src/control/mod.rs
rtk git commit -m "test: add recovery integration tests"
```

---

### Task 2: Fix GPS jump recovery call site (line 587)

**Files:**
- Modify: `crates/pico2-firmware/src/control/mod.rs:554-608`

- [ ] **Step 1: Replace detection::recovery::find_stop_index() call**

Find the GPS jump recovery block (around line 587) and replace:

```rust
// OLD CODE (remove this):
if let Some(recovered_idx) = detection::recovery::find_stop_index(
    s_raw,
    est_state.dr.filtered_v,
    dt_since_last_fix,
    &stops_vec,
    self.last_stop_index,
    &None,  // No freeze context in Normal mode (GPS jump recovery)
) {
```

Replace with:

```rust
// NEW CODE:
let recovery_input = crate::recovery::RecoveryInput {
    s_cm: s_raw,
    v_cms: est_state.dr.filtered_v,
    dt_seconds: dt_since_last_fix,
    stops: stops_vec,
    hint_idx: self.last_stop_index,
    frozen_s_cm: None,  // No frozen position in Normal mode
    search_window: 10,
};

if let Some(recovered_idx) = crate::recovery::recover(recovery_input) {
```

- [ ] **Step 2: Build to verify syntax**

Run: `rtk cargo build -p pico2-firmware`

Expected: SUCCESS (compiles without errors)

- [ ] **Step 3: Run integration tests**

Run: `rtk cargo test -p pico2-firmware recovery_integration_tests`

Expected: PASS

- [ ] **Step 4: Commit**

```bash
rtk git add crates/pico2-firmware/src/control/mod.rs
rtk git commit -m "fix: use crate::recovery::recover() for GPS jump recovery"
```

---

### Task 3: Fix re-acquisition recovery call site (line 654)

**Files:**
- Modify: `crates/pico2-firmware/src/control/mod.rs:637-671`

- [ ] **Step 1: Replace detection::recovery::find_stop_index() call**

Find the re-acquisition recovery block (around line 654) and replace:

```rust
// OLD CODE (remove this):
if let Some(recovered_idx) = detection::recovery::find_stop_index(
    est.s_cm,
    est_state.dr.filtered_v,
    elapsed_seconds,
    &stops_vec,
    self.last_stop_index,
    &None,  // No freeze context in Normal mode (re-acquisition recovery)
) {
```

Replace with:

```rust
// NEW CODE:
let recovery_input = crate::recovery::RecoveryInput {
    s_cm: est.s_cm,
    v_cms: est_state.dr.filtered_v,
    dt_seconds: elapsed_seconds,
    stops: stops_vec,
    hint_idx: self.last_stop_index,
    frozen_s_cm: None,  // No frozen position for re-acquisition
    search_window: 10,
};

if let Some(recovered_idx) = crate::recovery::recover(recovery_input) {
```

- [ ] **Step 2: Build to verify syntax**

Run: `rtk cargo build -p pico2-firmware`

Expected: SUCCESS

- [ ] **Step 3: Run all tests**

Run: `rtk cargo test -p pico2-firmware`

Expected: ALL PASS

- [ ] **Step 4: Commit**

```bash
rtk git add crates/pico2-firmware/src/control/mod.rs
rtk git commit -m "fix: use crate::recovery::recover() for re-acquisition recovery"
```

---

### Task 4: Verify no references to detection::recovery remain

**Files:**
- Modify: None (verification only)

- [ ] **Step 1: Search for remaining references**

Run: `rtk grep -r "detection::recovery" crates/pico2-firmware/src/`

Expected: NO RESULTS (all references removed)

- [ ] **Step 2: Verify firmware builds**

Run: `rtk cargo build -p pico2-firmware --features firmware`

Expected: SUCCESS

- [ ] **Step 3: Run clippy**

Run: `rtk cargo clippy -p pico2-firmware`

Expected: No warnings related to recovery

- [ ] **Step 4: Commit**

```bash
rtk git commit --allow-empty -m "chore: verify recovery module integration complete"
```

---

### Task 5: Add documentation comment explaining recovery usage

**Files:**
- Modify: `crates/pico2-firmware/src/control/mod.rs`

- [ ] **Step 1: Add comment before GPS jump recovery block**

Add before the GPS jump recovery section (around line 554):

```rust
// GPS Jump Recovery (H1)
// When GPS position jumps >200m in Normal mode, use recovery module
// to find correct stop index. RecoveryInput uses:
// - s_cm: current GPS position (jumped)
// - v_cms: filtered velocity from dead reckoning
// - dt_seconds: time since last valid GPS fix
// - frozen_s_cm: None (no position freeze in Normal mode)
```

- [ ] **Step 2: Add comment before re-acquisition recovery block**

Add before the re-acquisition recovery section (around line 637):

```rust
// Re-acquisition Recovery
// After returning from OffRoute without snap, use recovery module
// to find correct stop index based on elapsed time since freeze.
// RecoveryInput uses:
// - s_cm: Kalman-filtered position
// - v_cms: filtered velocity
// - dt_seconds: time since off_route_since
// - frozen_s_cm: None (freeze context cleared after off-route)
```

- [ ] **Step 3: Build and test**

Run: `rtk cargo test -p pico2-firmware`

Expected: PASS

- [ ] **Step 4: Final commit**

```bash
rtk git add crates/pico2-firmware/src/control/mod.rs
rtk git commit -m "docs: add recovery usage documentation"
```

---

## Self-Review Results

**1. Spec coverage:**
- ✅ GPS jump recovery (H1) covered — Task 2
- ✅ Re-acquisition recovery covered — Task 3
- ✅ All references to non-existent module removed — Task 4
- ✅ Tests verify integration — Task 1

**2. Placeholder scan:**
- ✅ No TBD/TODO in code steps
- ✅ All parameters explicitly specified
- ✅ Expected outputs provided for all steps

**3. Type consistency:**
- ✅ `RecoveryInput` struct fields match across all tasks
- ✅ Function signature consistent (`recover(RecoveryInput) -> Option<u8>`)
- ✅ Return type handling consistent (`if let Some(recovered_idx)`)

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-05-09-recovery-module-integration.md`. Two execution options:

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** — Execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?

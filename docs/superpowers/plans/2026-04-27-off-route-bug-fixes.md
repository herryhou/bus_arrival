# Off-Route Bug Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix 8 validated bugs in off-route detection and recovery (3 critical, 5 medium)

**Architecture:** Tiered fixes - Critical (C2→C1→C3) validated first, then Medium (M1→M3→M4→M2→M5)

**Tech Stack:** Embedded Rust (no_std), RP2350, Embassy-RP, heapless containers

---

## File Structure Map

```
crates/
├── shared/
│   └── src/types.rs                    # ADD: FreezeContext struct
├── pipeline/gps_processor/
│   └── src/kalman.rs                   # MODIFY: C2, C1, C3, M1, M3
├── pipeline/detection/
│   ├── src/recovery.rs                 # MODIFY: C3, M4
│   └── src/probability.rs              # MODIFY: M2
└── pico2-firmware/
    ├── src/state.rs                    # MODIFY: C1, C3, M1, M5
    └── tests/
        └── test_off_route_integration.rs  # ADD: Tests for C2, C1, C3, M1, M5
```

---

## Tier 1: Critical Fixes

### Task 1: Add FreezeContext to shared types

**Files:**
- Create: `crates/shared/src/types.rs` (add struct)
- Modify: `crates/shared/src/lib.rs` (export if needed)

- [ ] **Step 1: Add FreezeContext struct to shared types**

Read the existing types file to find the right location:

```bash
# Check if types.rs exists and what's in it
cat crates/shared/src/types.rs | head -50
```

Expected: File exists with KalmanState and other shared types.

- [ ] **Step 2: Add FreezeContext struct after KalmanState definition**

```rust
/// Context captured at position freeze time for recovery spatial anchoring
///
/// When GPS goes off-route, we capture the position and stop index.
/// Recovery uses this to avoid selecting stops that are spatially
/// inconsistent with the pre-freeze trajectory (e.g., on routes with loops).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FreezeContext {
    /// Position (cm along route) when freeze occurred
    pub frozen_s_cm: DistCm,
    /// Stop index when freeze occurred
    pub frozen_stop_idx: u8,
}
```

Add this after `KalmanState` struct definition, before `DrState`.

- [ ] **Step 3: Add freeze_ctx field to KalmanState**

```rust
pub struct KalmanState {
    // ... existing fields ...

    /// Off-route freeze context for spatial anchoring during recovery
    pub freeze_ctx: Option<FreezeContext>,
}
```

- [ ] **Step 4: Run cargo check to verify compilation**

```bash
cargo check --release -p shared
```

Expected: No errors.

- [ ] **Step 5: Commit**

```bash
git add crates/shared/src/types.rs
git commit -m "feat: add FreezeContext for off-route recovery spatial anchoring"
```

---

### Task 2: Fix C2 - reset_off_route_state must clear frozen_s_cm

**Files:**
- Modify: `crates/pipeline/gps_processor/src/kalman.rs:105-109`

- [ ] **Step 1: Write failing test for C2 bug**

Create test file `crates/pipeline/gps_processor/tests/test_c2_frozen_s_cm_reset.rs`:

```rust
//! Test C2: reset_off_route_state must clear frozen_s_cm
//!
//! Bug: reset_off_route_state cleared off_route_freeze_time but NOT frozen_s_cm,
//! causing spurious re-entry snaps after GPS outage during Suspect state.

use shared::{DistCm, KalmanState};
use gps_processor::kalman::reset_off_route_state;

#[test]
fn test_reset_off_route_state_clears_frozen_s_cm() {
    let mut state = KalmanState {
        s_cm: 10000,
        v_cms: 500,
        frozen_s_cm: Some(10000),
        off_route_suspect_ticks: 3,
        off_route_clear_ticks: 0,
        off_route_freeze_time: Some(12345),
        freeze_ctx: None,
        last_seg_idx: 0,
        dr_filter_gain: 10,
    };

    reset_off_route_state(&mut state);

    // All off-route fields should be cleared
    assert_eq!(state.off_route_suspect_ticks, 0, "suspect_ticks should be 0");
    assert_eq!(state.off_route_clear_ticks, 0, "clear_ticks should be 0");
    assert_eq!(state.off_route_freeze_time, None, "freeze_time should be None");
    assert_eq!(state.frozen_s_cm, None, "frozen_s_cm should be None (C2 fix)");
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p gps_processor test_c2_frozen_s_cm_reset -- --exact
```

Expected: FAIL - `frozen_s_cm` is `Some(10000)` instead of `None`

- [ ] **Step 3: Implement fix in kalman.rs**

Edit line 105-109:

```rust
pub fn reset_off_route_state(state: &mut KalmanState) {
    state.off_route_suspect_ticks = 0;
    state.off_route_clear_ticks = 0;
    state.off_route_freeze_time = None;
    state.frozen_s_cm = None;  // C2 fix: clear frozen position
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test -p gps_processor test_c2_frozen_s_cm_reset -- --exact
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/pipeline/gps_processor/src/kalman.rs
git add crates/pipeline/gps_processor/tests/test_c2_frozen_s_cm_reset.rs
git commit -m "fix(c2): reset_off_route_state now clears frozen_s_cm"
```

---

### Task 3: Fix C1 - Preserve off_route_freeze_time until after recovery

**Files:**
- Modify: `crates/pipeline/gps_processor/src/kalman.rs:92`
- Modify: `crates/pico2-firmware/src/state.rs:298,~320`

- [ ] **Step 1: Write failing test for C1 bug**

Create test file `crates/pico2-firmware/tests/test_c1_freeze_time_preserved.rs`:

```rust
//! Test C1: off_route_freeze_time should be preserved until after recovery
//!
//! Bug: update_off_route_hysteresis cleared off_route_freeze_time at Normal
//! transition, but state.rs needs it for recovery dt calculation. This caused
//! dt=1 fallback, collapsing velocity window to 1667 cm.

// Note: This is an integration test requiring full firmware state machine
// The test simulates an off-route episode and verifies correct dt usage

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Integration test - requires route data
    fn test_off_route_freeze_time_preserved_for_recovery() {
        // Simulate:
        // 1. GPS goes off-route (freeze_time set)
        // 2. GPS returns to route (Normal transition)
        // 3. Recovery runs with correct dt (not 1)
        //
        // Before fix: freeze_time cleared at step 2, recovery uses dt=1
        // After fix: freeze_time preserved until after step 3
    }
}
```

- [ ] **Step 2: Remove freeze_time clear in kalman.rs**

Edit line 92 in `update_off_route_hysteresis`:

```rust
// After 2 consecutive good matches, reset suspect counter and unfreeze
if state.off_route_clear_ticks >= OFF_ROUTE_CLEAR_TICKS {
    state.off_route_suspect_ticks = 0;
    state.frozen_s_cm = None;
    // REMOVED: state.off_route_freeze_time = None;  // C1: don't clear here
    OffRouteStatus::Normal
```

- [ ] **Step 3: Add freeze_time clear in state.rs after both recovery paths**

Read state.rs to find the correct location (after both H1 and needs_recovery blocks):

```bash
# Find where both recovery paths end
grep -n "reset_stop_states_after_recovery\|needs_recovery_on_reacquisition" \
    crates/pico2-firmware/src/state.rs
```

Expected: Lines ~207 and ~286-314

Add after line 314 (after needs_recovery block):

```rust
// Clear freeze time after both recovery paths complete
self.kalman.off_route_freeze_time = None;
```

- [ ] **Step 4: Run cargo check**

```bash
cargo check --release --target thumbv8m.main-none-eabi -p pico2-firmware
```

Expected: No errors.

- [ ] **Step 5: Commit**

```bash
git add crates/pipeline/gps_processor/src/kalman.rs
git add crates/pico2-firmware/src/state.rs
git add crates/pico2-firmware/tests/test_c1_freeze_time_preserved.rs
git commit -m "fix(c1): preserve off_route_freeze_time until after recovery completes"
```

---

### Task 4: Fix C3 - Add FreezeContext to recovery

**Files:**
- Modify: `crates/pipeline/gps_processor/src/kalman.rs:68` (store context)
- Modify: `crates/pipeline/detection/src/recovery.rs` (use context)
- Modify: `crates/pico2-firmware/src/state.rs` (pass context, call recovery)

- [ ] **Step 1: Update find_stop_index signature**

Edit `crates/pipeline/detection/src/recovery.rs`:

```rust
use shared::{DistCm, SpeedCm, Stop, FreezeContext};

/// Find correct stop after GPS jump
///
/// # Parameters
/// - `s_cm`: Current GPS position (cm)
/// - `v_filtered`: Filtered speed estimate (cm/s)
/// - `dt_since_last_fix`: Seconds elapsed since last valid GPS fix
/// - `stops`: Array of all stops on route
/// - `last_index`: Last known stop index before GPS anomaly
/// - `freeze_ctx`: Optional context from off-route freeze (C3 fix)
pub fn find_stop_index(
    s_cm: DistCm,
    v_filtered: SpeedCms,
    dt_since_last_fix: u64,
    stops: &[Stop],
    last_index: u8,
    freeze_ctx: &Option<FreezeContext>,  // C3: NEW PARAMETER
) -> Option<usize> {
    let mut best_idx: Option<usize> = None;
    let mut best_score = i32::MAX;

    // C3: Spatial anchor penalty - prefer stops at or after frozen position
    let spatial_anchor_penalty = if let Some(ctx) = freeze_ctx {
        // If bus is behind freeze point, heavily penalize stops behind frozen_stop_idx
        if s_cm < ctx.frozen_s_cm.saturating_sub(5000) {
            10000  // Large penalty for backward jumps
        } else {
            0
        }
    } else {
        0
    };

    for (i, stop) in stops.iter().enumerate() {
        let d = (s_cm - stop.progress_cm).abs();

        // Filter: within ±200m and >= last_index - 1
        if d >= GPS_JUMP_THRESHOLD || (i as u8) < last_index.saturating_sub(1) {
            continue;
        }

        let dist = (s_cm - stop.progress_cm).abs();
        let index_penalty = 5000 * (last_index as i32 - i as i32).max(0);

        // C3: Add spatial anchor penalty
        let index_penalty = index_penalty + spatial_anchor_penalty;

        // Velocity penalty: hard exclusion if reaching this stop requires
        // exceeding V_MAX_CMS given the elapsed time since last valid fix
        let dist_to_stop = if stop.progress_cm > s_cm {
            (stop.progress_cm - s_cm) as u64
        } else {
            0
        };
        let max_reachable = V_MAX_CMS as u64 * dt_since_last_fix;
        if dist_to_stop > max_reachable {
            continue;  // Hard exclusion
        }

        let score = dist.saturating_add(index_penalty);

        if score < best_score {
            best_score = score;
            best_idx = Some(i);
        }
    }

    best_idx
}
```

- [ ] **Step 2: Update both call sites in state.rs**

First call site (H1 jump recovery, ~line 197):

```rust
// BEFORE:
if let Some(recovered_idx) = detection::recovery::find_stop_index(
    s_cm,
    v_cms,
    dt_since_last_fix,
    &stops_vec,
    self.last_known_stop_index,
) {

// AFTER:
if let Some(recovered_idx) = detection::recovery::find_stop_index(
    s_cm,
    v_cms,
    dt_since_last_fix,
    &stops_vec,
    self.last_known_stop_index,
    &self.kalman.freeze_ctx,  // C3: pass freeze context
) {
```

Second call site (re-acquisition recovery, ~line 308):

```rust
// BEFORE:
if let Some(recovered_idx) = detection::recovery::find_stop_index(
    s_cm,
    v_cms,
    elapsed_seconds,
    &stops_vec,
    self.last_known_stop_index,
) {

// AFTER:
if let Some(recovered_idx) = detection::recovery::find_stop_index(
    s_cm,
    v_cms,
    elapsed_seconds,
    &stops_vec,
    self.last_known_stop_index,
    &self.kalman.freeze_ctx,  // C3: pass freeze context
) {
```

- [ ] **Step 3: Store FreezeContext in kalman.rs**

Edit `update_off_route_hysteresis` at line 66-70 to accept current_stop_idx:

```rust
// BEFORE:
pub fn update_off_route_hysteresis(
    state: &mut KalmanState,
    match_d2: i64,
    gps_timestamp: u64,
) -> OffRouteStatus {

// AFTER:
pub fn update_off_route_hysteresis(
    state: &mut KalmanState,
    match_d2: i64,
    gps_timestamp: u64,
    current_stop_idx: u8,  // C3: for freeze context
) -> OffRouteStatus {
```

Then at line 66-70:

```rust
if state.off_route_suspect_ticks == 0 {
    // First tick of off-route suspect: freeze position immediately
    state.frozen_s_cm = Some(state.s_cm);
    // Record freeze time at the same time position is frozen (Bug 5 fix)
    state.off_route_freeze_time = Some(gps_timestamp);
    // C3: Store freeze context for spatial anchoring
    state.freeze_ctx = Some(shared::FreezeContext {
        frozen_s_cm: state.s_cm,
        frozen_stop_idx: current_stop_idx,
    });
}
```

- [ ] **Step 4: Update all call sites of update_off_route_hysteresis**

Find all call sites:

```bash
grep -n "update_off_route_hysteresis" crates/pipeline/gps_processor/src/kalman.rs
```

Expected: Line ~197

Update the call:

```rust
// BEFORE:
let off_route_status = update_off_route_hysteresis(state, match_d2, gps.timestamp);

// AFTER:
let off_route_status = update_off_route_hysteresis(
    state,
    match_d2,
    gps.timestamp,
    0,  // Placeholder - Step 6-7 pass actual value from state.rs
);
```

- [ ] **Step 5: Pass actual stop index from state.rs**

In state.rs, the call to `process_gps_update` needs to pass current stop index. Read the relevant section:

```bash
grep -B5 -A5 "process_gps_update" crates/pico2-firmware/src/state.rs | head -30
```

We need to modify `process_gps_update` signature to accept current_stop_idx, or get it from RouteData.

For now, use `self.last_known_stop_index`:

```rust
// In state.rs, where process_gps_update is called (~line 157)
// BEFORE:
let result = gps_processor::kalman::process_gps_update(
    &mut self.kalman,
    &mut self.dr,
    &gps,
    &self.route_data,
    current_time,
    self.first_fix,
);

// AFTER: This requires updating process_gps_update signature
// See next step
```

- [ ] **Step 6: Update process_gps_update signature**

Edit `kalman.rs`:

```rust
// BEFORE:
pub fn process_gps_update(
    state: &mut KalmanState,
    dr: &mut DrState,
    gps: &GpsPoint,
    route_data: &RouteData,
    _current_time: u64,
    is_first_fix: bool,

// AFTER:
pub fn process_gps_update(
    state: &mut KalmanState,
    dr: &mut DrState,
    gps: &GpsPoint,
    route_data: &RouteData,
    _current_time: u64,
    is_first_fix: bool,
    current_stop_idx: u8,  // C3: for freeze context
```

Then update the call at line ~197:

```rust
let off_route_status = update_off_route_hysteresis(
    state,
    match_d2,
    gps.timestamp,
    current_stop_idx,  // C3: use passed value
);
```

- [ ] **Step 7: Update state.rs to pass current_stop_idx**

```rust
// In state.rs, update the call to process_gps_update
let result = gps_processor::kalman::process_gps_update(
    &mut self.kalman,
    &mut self.dr,
    &gps,
    &self.route_data,
    current_time,
    self.first_fix,
    self.last_known_stop_index,  // C3: pass current stop index
);
```

- [ ] **Step 8: Run cargo check**

```bash
cargo check --release --target thumbv8m.main-none-eabi -p pico2-firmware
```

Expected: No errors.

- [ ] **Step 9: Commit**

```bash
git add crates/shared/src/types.rs
git add crates/pipeline/detection/src/recovery.rs
git add crates/pipeline/gps_processor/src/kalman.rs
git add crates/pico2-firmware/src/state.rs
git commit -m "fix(c3): add FreezeContext for spatial anchoring in recovery"
```

---

### Task 5: Validate Tier 1 fixes with integration test

**Files:**
- Create: `crates/pico2-firmware/tests/test_tier1_integration.rs`

- [ ] **Step 1: Create integration test**

```rust
//! Integration test for Tier 1 fixes (C1, C2, C3)
//!
//! Tests off-route episode with recovery:
//! 1. GPS goes off-route (freeze triggered)
//! 2. GPS outage during Suspect (C2: frozen_s_cm cleared)
//! 3. GPS returns to route (C1: freeze_time preserved for dt)
//! 4. Recovery with spatial anchor (C3: freeze_ctx used)

#[cfg(test)]
mod tests {
    #[test]
    #[ignore] // Requires full route data setup
    fn test_off_route_recovery_with_freeze_context() {
        // TODO: Set up route data and simulate:
        // - Off-route episode
        // - GPS outage during suspect
        // - Re-entry with recovery
        // Verify: correct stop index, no spurious snaps
    }
}
```

- [ ] **Step 2: Run cargo test**

```bash
cargo test -p pico2-firmware test_tier1_integration -- --ignored
```

- [ ] **Step 3: Commit**

```bash
git add crates/pico2-firmware/tests/test_tier1_integration.rs
git commit -m "test: add tier 1 integration test scaffold"
```

---

## Tier 2: Medium Fixes

### Task 6: Fix M1 - Add SuspectOffRoute variant

**Files:**
- Modify: `crates/pipeline/gps_processor/src/kalman.rs` (ProcessResult enum)
- Modify: `crates/pico2-firmware/src/state.rs` (handle new variant)

- [ ] **Step 1: Add SuspectOffRoute variant to ProcessResult**

Edit `kalman.rs` ProcessResult enum (~line 127):

```rust
pub enum ProcessResult {
    Valid {
        signals: PositionSignals,
        v_cms: SpeedCms,
        seg_idx: usize,
    },
    Rejected(&'static str),
    Outage,
    DrOutage {
        s_cm: DistCm,
        v_cms: SpeedCms,
    },
    /// GPS is off-route — position frozen, awaiting re-acquisition
    OffRoute {
        last_valid_s: DistCm,
        last_valid_v: SpeedCms,
        freeze_time: u64,
    },
    /// GPS is suspect off-route — position frozen, awaiting confirmation
    /// M1: Separate from DrOutage to prevent warmup timeout exploitation
    SuspectOffRoute {
        s_cm: DistCm,
        v_cms: SpeedCms,
    },
}
```

- [ ] **Step 2: Return SuspectOffRoute in Suspect branch**

Edit `kalman.rs` at line 210-218:

```rust
OffRouteStatus::Suspect => {
    // During off-route suspicion, skip projection and filters to prevent s_cm advance
    // M1: Return SuspectOffRoute instead of DrOutage to distinguish from genuine GPS loss
    dr.last_gps_time = Some(gps.timestamp);
    return ProcessResult::SuspectOffRoute {
        s_cm: state.frozen_s_cm.unwrap_or(state.s_cm),
        v_cms: state.v_cms,
    };
}
```

- [ ] **Step 3: Handle SuspectOffRoute in state.rs**

Find the ProcessResult match in `state.rs` (~line 156) and add the new branch:

```rust
// In the match result block
ProcessResult::SuspectOffRoute { s_cm: _, v_cms: _ } => {
    // M1: Suspect off-route - suppress detection, don't increment warmup
    // Mark for recovery when we return to Normal
    self.needs_recovery_on_reacquisition = true;
    return None;
}
```

Place this after the `OffRoute` branch, before `DrOutage`.

- [ ] **Step 4: Run cargo check**

```bash
cargo check --release --target thumbv8m.main-none-eabi -p pico2-firmware
```

Expected: No errors.

- [ ] **Step 5: Commit**

```bash
git add crates/pipeline/gps_processor/src/kalman.rs
git add crates/pico2-firmware/src/state.rs
git commit -m "fix(m1): add SuspectOffRoute variant to distinguish from DrOutage"
```

---

### Task 7: Fix M3 - Blend v_cms on re-entry

**Files:**
- Modify: `crates/pipeline/gps_processor/src/kalman.rs:237`

- [ ] **Step 1: Edit re-entry snap to blend velocity**

Line 237 in `kalman.rs`:

```rust
// BEFORE:
state.v_cms = gps.speed_cms;

// AFTER:
// M3: Blend velocity instead of hard assignment
// First GPS after re-entry is worst-quality (HDOP spike, heading uncertainty)
// Use EMA blend (3/10 gain) same as normal updates
state.v_cms = state.v_cms + 3 * (gps.speed_cms.max(0).min(V_MAX_CMS) - state.v_cms) / 10;
```

- [ ] **Step 2: Run cargo check**

```bash
cargo check --release -p gps_processor
```

Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add crates/pipeline/gps_processor/src/kalman.rs
git commit -m "fix(m3): blend v_cms on re-entry instead of hard assignment"
```

---

### Task 8: Fix M4 - Use actual speed in recovery

**Files:**
- Modify: `crates/pipeline/detection/src/recovery.rs:56`

- [ ] **Step 1: Edit velocity constraint to use actual speed**

Line 56 in `recovery.rs`:

```rust
// BEFORE:
let max_reachable = V_MAX_CMS as u64 * dt_since_last_fix;

// AFTER:
// M4: Use actual filtered speed instead of worst-case V_MAX
// If we have a valid speed estimate, use it; otherwise fall back to V_MAX
let effective_v = if v_filtered > 0 {
    v_filtered as u64
} else {
    V_MAX_CMS as u64
};
let max_reachable = effective_v * dt_since_last_fix;
```

- [ ] **Step 2: Run cargo check**

```bash
cargo check --release -p detection
```

Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add crates/pipeline/detection/src/recovery.rs
git commit -m "fix(m4): use actual filtered speed in recovery velocity constraint"
```

---

### Task 9: Fix M2 - Full probability suppression for OffRoute

**Files:**
- Modify: `crates/pipeline/detection/src/probability.rs`

- [ ] **Step 1: Find probability calculation function**

```bash
grep -n "pub fn\|fn.*probability" crates/pipeline/detection/src/probability.rs | head -10
```

Expected: `pub fn calculate_arrival_probability` or similar

- [ ] **Step 2: Add early return for OffRoute status**

At the start of the probability function, add:

```rust
// M2: OffRoute means vehicle is not on route at all
// Entire probability computation is meaningless - fully suppress
if gps_status == GpsStatus::OffRoute {
    return 0;
}
```

Find where `GpsStatus` is defined and used. Add this check after the function signature, before any probability calculations.

- [ ] **Step 3: Run cargo check**

```bash
cargo check --release -p detection
```

Expected: No errors.

- [ ] **Step 4: Commit**

```bash
git add crates/pipeline/detection/src/probability.rs
git commit -m "fix(m2): fully suppress probability for OffRoute status"
```

---

### Task 10: Fix M5 - Gate persistence on off-route status

**Files:**
- Modify: `crates/pico2-firmware/src/state.rs:581`

- [ ] **Step 1: Edit should_persist to check off-route status**

Line 581 in `state.rs`:

```rust
// BEFORE:
pub fn should_persist(&self, current_stop: u8) -> bool {
    // Only persist when stop index actually changes
    if current_stop == self.last_persisted_stop {
        return false;
    }

    // Rate limit: no more than once per 60 seconds (60 ticks at 1Hz)
    if self.ticks_since_persist < 60 {
        return false;
    }

    true
}

// AFTER:
pub fn should_persist(&self, current_stop: u8) -> bool {
    // M5: Don't persist if we're off-route or suspect
    // This prevents writing incorrect anchors to Flash
    if self.kalman.frozen_s_cm.is_some() {
        return false;
    }
    if self.kalman.off_route_suspect_ticks > 0 {
        return false;
    }

    // Only persist when stop index actually changes
    if current_stop == self.last_persisted_stop {
        return false;
    }

    // Rate limit: no more than once per 60 seconds (60 ticks at 1Hz)
    if self.ticks_since_persist < 60 {
        return false;
    }

    true
}
```

- [ ] **Step 2: Run cargo check**

```bash
cargo check --release --target thumbv8m.main-none-eabi -p pico2-firmware
```

Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add crates/pico2-firmware/src/state.rs
git commit -m "fix(m5): gate persistence on off-route status"
```

---

### Task 11: Validate Tier 2 fixes with integration test

**Files:**
- Create: `crates/pico2-firmware/tests/test_tier2_integration.rs`

- [ ] **Step 1: Create integration test**

```rust
//! Integration test for Tier 2 fixes (M1-M5)
//!
//! Tests:
//! M1: Suspect state doesn't increment warmup timeout
//! M2: OffRoute fully suppresses probability
//! M3: v_cms is blended on re-entry
//! M4: Recovery uses actual speed
//! M5: Persistence gated on off-route status

#[cfg(test)]
mod tests {
    #[test]
    #[ignore] // Requires full state machine setup
    fn test_suspect_does_not_increment_warmup() {
        // M1: Verify SuspectOffRoute doesn't cause warmup timeout
    }

    #[test]
    #[ignore]
    fn test_offroute_suppresses_probability() {
        // M2: Verify probability is 0 during OffRoute
    }

    #[test]
    #[ignore]
    fn test_persistence_blocked_during_suspect() {
        // M5: Verify should_persist returns false during Suspect
    }
}
```

- [ ] **Step 2: Run cargo test**

```bash
cargo test -p pico2-firmware test_tier2_integration -- --ignored
```

- [ ] **Step 3: Commit**

```bash
git add crates/pico2-firmware/tests/test_tier2_integration.rs
git commit -m "test: add tier 2 integration test scaffold"
```

---

### Task 12: Run full test suite and verify

- [ ] **Step 1: Run all tests**

```bash
cargo test --release
```

Expected: All tests pass.

- [ ] **Step 2: Run firmware build**

```bash
cargo build --release --target thumbv8m.main-none-eabi -p pico2-firmware
```

Expected: Build succeeds.

- [ ] **Step 3: Check for warnings**

```bash
cargo clippy --release --target thumbv8m.main-none-eabi -p pico2-firmware
```

Expected: No new warnings introduced.

- [ ] **Step 4: Commit final cleanup if needed**

```bash
# If any cleanup needed
git add -A
git commit -m "chore: final cleanup after off-route bug fixes"
```

---

## Summary

This plan implements 8 bug fixes in dependency order:

**Tier 1 (Critical):**
- C2: `reset_off_route_state` clears `frozen_s_cm`
- C1: `off_route_freeze_time` preserved until after recovery
- C3: `FreezeContext` for spatial anchoring

**Tier 2 (Medium):**
- M1: `SuspectOffRoute` variant
- M3: Blend `v_cms` on re-entry
- M4: Use actual speed in recovery
- M2: Full probability suppression
- M5: Gate persistence on off-route

Each fix is independently commitable and revertable.

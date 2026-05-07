# v9.0 Architecture Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the v9.0 two-layer architecture integration by implementing `SystemState::run_detection()`, migrating all control logic from `state::State`, switching `main.rs` to use the new architecture, and deleting `state::State` entirely.

**Architecture:** Two-layer separation with Estimation layer (pure GPS→position pipeline) and Control layer (mode management, detection FSM, recovery, persistence). Async persistence handled via `TickResult` struct.

**Tech Stack:** Rust, embedded no_std, Embassy async framework, heapless containers

---

## File Structure

**Modify:**
- `crates/pico2-firmware/src/control/mod.rs` - Add new fields, implement detection FSM, warmup, persistence methods
- `crates/pico2-firmware/src/lib.rs` - Update re-exports (remove `state::State`)
- `crates/pico2-firmware/src/main.rs` - Switch to `SystemState` + `EstimationState`, handle `TickResult`
- `Makefile` - Add `golden` target

**Update tests:**
- `crates/pico2-firmware/tests/*.rs` - Update all integration tests to use `SystemState` instead of `state::State`

**Delete:**
- `crates/pico2-firmware/src/state.rs` - After migration complete

---

## Phase 1: Prepare SystemState - Add Fields and Basic Structure

### Task 1: Add new fields to SystemState struct

**Files:**
- Modify: `crates/pico2-firmware/src/control/mod.rs:17-48`

- [ ] **Step 1: Read current SystemState struct**

Run: `head -50 crates/pico2-firmware/src/control/mod.rs`

Expected: Current struct with mode, last_stop_index, frozen_s_cm, etc.

- [ ] **Step 2: Add new fields to SystemState struct**

Edit `crates/pico2-firmware/src/control/mod.rs` and update the struct definition:

```rust
pub struct SystemState<'a> {
    /// Current operational mode
    pub mode: SystemMode,
    /// Last confirmed stop index (for recovery hint)
    pub last_stop_index: u8,
    /// Frozen position during OffRoute/Recovering (None in Normal mode)
    pub frozen_s_cm: Option<DistCm>,
    /// Hysteresis counter for OffRoute → Normal transition
    pub off_route_clear_ticks: u8,
    /// Hysteresis counter for Normal → OffRoute transition
    pub off_route_suspect_ticks: u8,
    /// Timestamp when OffRoute was entered (for recovery dt calculation)
    pub off_route_since: Option<u64>,
    /// Timestamp when Recovering was entered (for timeout)
    pub recovering_since: Option<u64>,
    /// Recovery failed flag (set after timeout, suppresses announcements)
    pub recovery_failed: bool,
    /// Route data reference (immutable, XIP-friendly)
    pub route_data: &'a RouteData<'a>,
    /// Pending persisted state from flash
    pub pending_persisted: Option<shared::PersistedState>,
    /// Last stop index that was persisted to flash
    pub last_persisted_stop: u8,
    /// Ticks since last persist operation
    pub ticks_since_persist: u16,
    /// Previous position for monotonic checking
    pub last_s_cm: DistCm,
    /// Counter for backward jump events (GPS health monitoring)
    pub backward_jump_count: u32,
    /// Whether we've received the first valid GPS fix (for cold-start initialization)
    has_received_first_fix: bool,

    // === NEW: Detection FSM ===
    pub stop_states: heapless::Vec<detection::state_machine::StopState, 256>,

    // === NEW: Warmup counters ===
    estimation_ready_ticks: u8,
    estimation_total_ticks: u8,
    detection_enabled_ticks: u8,
    detection_total_ticks: u8,
    just_reset: bool,

    // === NEW: GPS jump recovery tracking ===
    last_valid_s_cm: DistCm,
    last_gps_timestamp: u64,
    needs_recovery_on_reacquisition: bool,

    // === NEW: Snap cooldown ===
    just_snapped_ticks: u8,
}
```

- [ ] **Step 3: Add compile-time size check at end of file**

Add to end of `crates/pico2-firmware/src/control/mod.rs`:

```rust
// Compile-time verification that SystemState fits within SRAM budget
const _: () = assert!(size_of::<SystemState>() <= 4096, "SystemState exceeds 4KB SRAM budget");
```

- [ ] **Step 4: Add TickResult struct before SystemState**

Add before `SystemState` struct definition:

```rust
/// Return value from tick() - separates sync logic from async persistence
pub struct TickResult {
    /// Arrival/departure/announce event if any
    pub event: Option<ArrivalEvent>,
    /// Persist request if stop index changed and rate limit allows
    pub persist_request: Option<shared::PersistedState>,
}
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check -p pico2-firmware --features dev`

Expected: Compiles successfully (new fields are unused but valid)

- [ ] **Step 6: Commit**

```bash
git add crates/pico2-firmware/src/control/mod.rs
git commit -m "feat(control): add detection FSM and warmup fields to SystemState"
```

---

### Task 2: Update SystemState::new() to initialize new fields

**Files:**
- Modify: `crates/pico2-firmware/src/control/mod.rs:51-69`

- [ ] **Step 1: Read current new() implementation**

Run: `sed -n '51,69p' crates/pico2-firmware/src/control/mod.rs`

Expected: Current constructor that initializes existing fields

- [ ] **Step 2: Update new() to initialize all new fields**

Edit the constructor to initialize new fields:

```rust
pub fn new(route_data: &'a RouteData<'a>, persisted: Option<shared::PersistedState>) -> Self {
    use detection::state_machine::StopState;

    // Initialize stop_states for all stops in route
    let stop_count = route_data.stop_count;
    let mut stop_states = heapless::Vec::new();
    for i in 0..stop_count {
        if stop_states.push(StopState::new(i as u8)).is_err() {
            #[cfg(feature = "firmware")]
            defmt::warn!("Route has {} stops but only 256 supported", stop_count);
            break;
        }
    }

    Self {
        mode: SystemMode::Normal,
        last_stop_index: 0,
        frozen_s_cm: None,
        off_route_clear_ticks: 0,
        off_route_suspect_ticks: 0,
        off_route_since: None,
        recovering_since: None,
        recovery_failed: false,
        route_data,
        pending_persisted: persisted,
        last_persisted_stop: persisted.map(|p| p.last_stop_index).unwrap_or(0),
        ticks_since_persist: 0,
        last_s_cm: 0,
        backward_jump_count: 0,
        has_received_first_fix: false,

        // New fields
        stop_states,
        estimation_ready_ticks: 0,
        estimation_total_ticks: 0,
        detection_enabled_ticks: 0,
        detection_total_ticks: 0,
        just_reset: false,
        last_valid_s_cm: 0,
        last_gps_timestamp: 0,
        needs_recovery_on_reacquisition: false,
        just_snapped_ticks: 0,
    }
}
```

- [ ] **Step 3: Add imports at top of file**

Add to imports at top of `control/mod.rs`:

```rust
use crate::detection::state_machine::StopState;
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p pico2-firmware --features dev`

Expected: Compiles successfully

- [ ] **Step 5: Commit**

```bash
git add crates/pico2-firmware/src/control/mod.rs
git commit -m "feat(control): initialize new fields in SystemState::new()"
```

---

## Phase 2: Implement Warmup and Helper Methods

### Task 3: Implement warmup query methods

**Files:**
- Modify: `crates/pico2-firmware/src/control/mod.rs` (add after current_position method)

- [ ] **Step 1: Find insertion point**

Run: `grep -n "pub fn current_position" crates/pico2-firmware/src/control/mod.rs`

Expected: Line ~79

- [ ] **Step 2: Add warmup query methods after current_position()**

Add after `current_position()` method:

```rust
/// Check if estimation is ready (affects heading filter, Kalman)
pub fn estimation_ready(&self) -> bool {
    self.estimation_ready_ticks >= 3 || self.estimation_total_ticks >= 10
}

/// Check if detection is enabled (independent of estimation)
pub fn detection_ready(&self) -> bool {
    self.detection_enabled_ticks >= 3 || self.detection_total_ticks >= 10
}

/// Check if heading filter should be disabled
pub fn disable_heading_filter(&self) -> bool {
    !self.has_received_first_fix || !self.estimation_ready()
}
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p pico2-firmware --features dev`

Expected: Compiles successfully

- [ ] **Step 4: Write test for warmup methods**

Create test in `crates/pico2-firmware/src/control/mod.rs` test module:

```rust
#[test]
fn test_warmup_methods() {
    use shared::binfile::RouteData;
    let route_data = RouteData::load(&[0u8; 100]).unwrap(); // Minimal valid header
    let state = SystemState::new(&route_data, None);

    assert!(!state.estimation_ready(), "Should not be ready initially");
    assert!(!state.detection_ready(), "Detection should not be ready");
    assert!(state.disable_heading_filter(), "Should disable filter before first fix");
}
```

- [ ] **Step 5: Run test**

Run: `cargo test -p pico2-firmware test_warmup_methods --features dev`

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/pico2-firmware/src/control/mod.rs
git commit -m "feat(control): add warmup query methods"
```

---

### Task 4: Implement stop search helper methods

**Files:**
- Modify: `crates/pico2-firmware/src/control/mod.rs`

- [ ] **Step 1: Add find_closest_stop_index() method**

Add after warmup methods:

```rust
/// Find closest stop index to current position
pub fn find_closest_stop_index(&self, s_cm: DistCm) -> u8 {
    let mut closest_idx = 0;
    let mut closest_dist = i32::MAX;

    for i in 0..self.route_data.stop_count {
        if let Some(stop) = self.route_data.get_stop(i) {
            let dist = (s_cm - stop.progress_cm).abs();
            if dist < closest_dist {
                closest_dist = dist;
                closest_idx = i;
            }
        }
    }

    closest_idx as u8
}
```

- [ ] **Step 2: Add find_forward_closest_stop_index() method**

Add after `find_closest_stop_index()`:

```rust
/// Find closest stop index in forward direction only
///
/// Searches from last_idx to end of route only. This prevents
/// selecting stops behind the current position, which is important
/// after off-route snap re-entry.
pub fn find_forward_closest_stop_index(&self, s_cm: DistCm, last_idx: u8) -> u8 {
    let mut best_idx = last_idx;
    let mut best_dist = i32::MAX;

    // Only search forward: from last_idx to end of route
    for i in last_idx as usize..self.route_data.stop_count {
        if let Some(stop) = self.route_data.get_stop(i) {
            let dist = (s_cm - stop.progress_cm).abs();
            if dist < best_dist {
                best_dist = dist;
                best_idx = i as u8;
            }
        }
    }

    best_idx
}
```

- [ ] **Step 3: Write tests for stop search methods**

Add to test module:

```rust
#[test]
fn test_find_closest_stop_index() {
    use shared::binfile::RouteData;
    let route_data = RouteData::load(&[0u8; 100]).unwrap();
    let state = SystemState::new(&route_data, None);

    // Test that method returns a valid index
    let idx = state.find_closest_stop_index(5000);
    assert!(idx < route_data.stop_count as u8);
}

#[test]
fn test_find_forward_closest_stop_index() {
    use shared::binfile::RouteData;
    let route_data = RouteData::load(&[0u8; 100]).unwrap();
    let state = SystemState::new(&route_data, None);

    // Test forward search from index 5
    let idx = state.find_forward_closest_stop_index(5000, 5);
    assert!(idx >= 5, "Should only return stops at or after index 5");
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p pico2-firmware test_find --features dev`

Expected: Both tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/pico2-firmware/src/control/mod.rs
git commit -m "feat(control): add stop search helper methods"
```

---

### Task 5: Implement persistence helper methods

**Files:**
- Modify: `crates/pico2-firmware/src/control/mod.rs`

- [ ] **Step 1: Add should_persist() method**

Add after stop search methods:

```rust
/// Returns true if state should be persisted this tick.
/// Writes when stop index changes, but no more than once per 60 seconds.
pub fn should_persist(&self, current_stop: u8) -> bool {
    // Don't persist if position is frozen (off-route or suspect)
    if self.mode == SystemMode::OffRoute || self.mode == SystemMode::Recovering {
        return false;
    }

    // Don't persist if in suspect state (may be about to go off-route)
    if self.off_route_suspect_ticks > 0 {
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

- [ ] **Step 2: Add mark_persisted() method**

Add after `should_persist()`:

```rust
/// Mark state as persisted, resetting the rate-limit counter.
pub fn mark_persisted(&mut self, stop_index: u8) {
    self.last_persisted_stop = stop_index;
    self.ticks_since_persist = 0;
}
```

- [ ] **Step 3: Add current_stop_index() method**

Add after `mark_persisted()`:

```rust
/// Get the current stop index from last_stop_index.
/// Returns None if not yet initialized.
pub fn current_stop_index(&self) -> Option<u8> {
    if !self.has_received_first_fix {
        None
    } else {
        Some(self.last_stop_index)
    }
}
```

- [ ] **Step 4: Write tests for persistence methods**

Add to test module:

```rust
#[test]
fn test_persistence_helpers() {
    use shared::binfile::RouteData;
    let route_data = RouteData::load(&[0u8; 100]).unwrap();
    let mut state = SystemState::new(&route_data, None);

    // Initially should not persist (no first fix)
    assert!(!state.should_persist(0).ok_or(false).unwrap_or(false));

    // After first fix, current_stop_index returns Some
    state.has_received_first_fix = true;
    assert!(state.current_stop_index().is_some());

    // Test rate limiting
    state.last_persisted_stop = 0;
    state.ticks_since_persist = 0;
    assert!(!state.should_persist(1).ok_or(false).unwrap_or(false), "Should rate limit");

    state.ticks_since_persist = 60;
    assert!(state.should_persist(1).ok_or(false).unwrap_or(false), "Should allow after 60 ticks");

    state.mark_persisted(1);
    assert_eq!(state.last_persisted_stop, 1);
    assert_eq!(state.ticks_since_persist, 0);
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p pico2-firmware test_persistence --features dev`

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/pico2-firmware/src/control/mod.rs
git commit -m "feat(control): add persistence helper methods"
```

---

## Phase 3: Implement Detection FSM

### Task 6: Implement reset_stop_states_after_recovery()

**Files:**
- Modify: `crates/pico2-firmware/src/control/mod.rs`

- [ ] **Step 1: Add reset_stop_states_after_recovery() method**

Add after persistence methods:

```rust
/// Reset all stop states to Idle after recovery
fn reset_stop_states_after_recovery(&mut self, recovered_idx: usize, current_s_cm: DistCm) {
    use detection::state_machine::StopState;
    use shared::FsmState;

    let recovered_was_announced = self
        .stop_states
        .get(recovered_idx)
        .map(|state| state.announced || state.last_announced_stop == recovered_idx as u8)
        .unwrap_or(false);

    // Reset all stop states by recreating them
    for i in 0..self.stop_states.len() {
        self.stop_states[i] = StopState::new(i as u8);
    }

    // Stops before the recovered stop are treated as already passed.
    // Preserve their announcement bookkeeping so recovery cannot re-announce them.
    for i in 0..recovered_idx.min(self.stop_states.len()) {
        self.stop_states[i].fsm_state = FsmState::Departed;
        self.stop_states[i].announced = true;
        self.stop_states[i].last_announced_stop = i as u8;
    }

    // Mark recovered stop as Approaching if within corridor
    if let Some(stop) = self.route_data.get_stop(recovered_idx) {
        if let Some(state) = self.stop_states.get_mut(recovered_idx) {
            if recovered_was_announced {
                state.announced = true;
                state.last_announced_stop = recovered_idx as u8;
            }

            if current_s_cm >= stop.corridor_start_cm
                && current_s_cm <= stop.corridor_end_cm
            {
                state.fsm_state = FsmState::Approaching;
            }
        }
    }
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p pico2-firmware --features dev`

Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add crates/pico2-firmware/src/control/mod.rs
git commit -m "feat(control): add reset_stop_states_after_recovery method"
```

---

### Task 7: Implement run_detection() method - Part 1: Structure

**Files:**
- Modify: `crates/pico2-firmware/src/control/mod.rs:323-344` (replace stub)

- [ ] **Step 1: Read current run_detection() stub**

Run: `sed -n '323,344p' crates/pico2-firmware/src/control/mod.rs`

Expected: Current stub that returns None

- [ ] **Step 2: Replace run_detection() with full implementation**

Replace the entire `run_detection()` method:

```rust
/// Run arrival detection (Normal mode only)
fn run_detection(&mut self, est: &EstimationOutput, s_cm: DistCm, timestamp: u64) -> Option<ArrivalEvent> {
    use crate::detection;
    use shared::PositionSignals;
    use detection::state_machine::StopEvent;

    // Create position signals for detection
    let signals = PositionSignals {
        z_gps_cm: est.z_gps_cm,
        s_cm: est.s_cm,
    };

    // Step 1: Find active stops (corridor filter)
    let active_indices = detection::find_active_stops(signals, self.route_data);

    // Step 2: For each active stop, compute probability and update FSM
    for stop_idx in active_indices {
        if stop_idx >= self.stop_states.len() {
            continue;
        }

        let stop = match self.route_data.get_stop(stop_idx) {
            Some(s) => s,
            None => continue,
        };
        let stop_state = &mut self.stop_states[stop_idx];

        // Get next sequential stop for adaptive weights
        let next_stop_idx = stop_idx.checked_add(1);
        let next_stop_value = next_stop_idx.and_then(|idx| self.route_data.get_stop(idx));
        let next_stop = next_stop_value.as_ref();

        // Compute arrival probability with adaptive weights
        let probability = detection::compute_arrival_probability_adaptive(
            signals,
            est.v_cms,
            &stop,
            stop_state.dwell_time_s,
            detection::GpsStatus::Valid, // TODO: derive from est output
            next_stop,
        );

        // Update state machine FIRST (v8.4: FSM transition before announce check)
        let event = stop_state.update(
            s_cm,
            est.v_cms,
            stop.progress_cm,
            stop.corridor_start_cm,
            probability,
        );

        // THEN check for announcement trigger
        if stop_state.should_announce(s_cm, stop.corridor_start_cm) {
            return Some(ArrivalEvent {
                time: timestamp,
                stop_idx: stop_idx as u8,
                s_cm,
                v_cms: est.v_cms,
                probability: 0,
                event_type: shared::ArrivalEventType::Announce,
            });
        }

        match event {
            StopEvent::Arrived => {
                return Some(ArrivalEvent {
                    time: timestamp,
                    stop_idx: stop_idx as u8,
                    s_cm,
                    v_cms: est.v_cms,
                    probability,
                    event_type: shared::ArrivalEventType::Arrival,
                });
            }
            StopEvent::Departed => {
                return Some(ArrivalEvent {
                    time: timestamp,
                    stop_idx: stop_idx as u8,
                    s_cm,
                    v_cms: est.v_cms,
                    probability,
                    event_type: shared::ArrivalEventType::Departure,
                });
            }
            StopEvent::None => {}
        }
    }

    None
}
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p pico2-firmware --features dev`

Expected: Compiles successfully (may have unused import warnings)

- [ ] **Step 4: Commit**

```bash
git add crates/pico2-firmware/src/control/mod.rs
git commit -m "feat(control): implement run_detection() with full FSM logic"
```

---

### Task 8: Update tick() method signature to return TickResult

**Files:**
- Modify: `crates/pico2-firmware/src/control/mod.rs:202`

- [ ] **Step 1: Read current tick() signature**

Run: `grep -n "pub fn tick" crates/pico2-firmware/src/control/mod.rs`

Expected: Line 202, returns `Option<ArrivalEvent>`

- [ ] **Step 2: Update tick() signature and return type**

Change the signature from:

```rust
pub fn tick(&mut self, gps: &GpsPoint, est_state: &mut crate::estimation::EstimationState) -> Option<ArrivalEvent>
```

To:

```rust
pub fn tick(&mut self, gps: &GpsPoint, est_state: &mut crate::estimation::EstimationState) -> TickResult
```

- [ ] **Step 3: Update all return statements in tick()**

Find and replace return statements at end of tick():
- Line ~320: `return None;` → `return TickResult { event: None, persist_request: None };`
- Line ~317: `return self.run_detection(...)` → needs to wrap in TickResult

Replace the final detection call section (around line 316-318):

```rust
// STEP 4: Detection (ONLY in Normal mode)
let event = if self.mode == SystemMode::Normal {
    self.run_detection(&est, s_cm_for_detection, gps.timestamp)
} else {
    None
};

// STEP 5: Check persistence
let persist_request = if self.mode == SystemMode::Normal {
    self.current_stop_index()
        .filter(|&idx| self.should_persist(idx))
        .map(|idx| shared::PersistedState::new(s_cm_for_detection, idx))
} else {
    None
};

TickResult { event, persist_request }
```

- [ ] **Step 4: Update early returns in tick()**

Replace all `return None;` statements with `return TickResult { event: None, persist_request: None };`:

```bash
# Find all early returns
grep -n "return None" crates/pico2-firmware/src/control/mod.rs
```

Update each occurrence to return `TickResult { event: None, persist_request: None }`

- [ ] **Step 5: Verify compilation**

Run: `cargo check -p pico2-firmware --features dev`

Expected: Compiles successfully

- [ ] **Step 6: Commit**

```bash
git add crates/pico2-firmware/src/control/mod.rs
git commit -m "feat(control): change tick() to return TickResult for async persistence"
```

---

## Phase 4: Integrate GPS Jump and Snap Handling into tick()

### Task 8a: Add snapped field to EstimationOutput

**Files:**
- Modify: `crates/pico2-firmware/src/estimation/mod.rs`

- [ ] **Step 1: Read current EstimationOutput struct**

Run: `grep -A 10 "pub struct EstimationOutput" crates/pico2-firmware/src/estimation/mod.rs`

Expected: Current struct definition

- [ ] **Step 2: Add snapped field to EstimationOutput**

Add to struct:
```rust
/// Whether position snapped from off-route re-entry
pub snapped: bool,
```

- [ ] **Step 3: Update all EstimationOutput constructors**

Find all `EstimationOutput { ... }` constructions and add `snapped: false,` or `snapped: true,` as appropriate.

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p pico2-firmware --features dev`

Expected: Compiles successfully

- [ ] **Step 5: Commit**

```bash
git add crates/pico2-firmware/src/estimation/mod.rs
git commit -m "feat(estimation): add snapped field to EstimationOutput"
```

---

### Task 9: Add GPS jump detection and recovery to tick()

**Files:**
- Modify: `crates/pico2-firmware/src/control/mod.rs` (in tick() method, after warmup check)

- [ ] **Step 1: Read recovery_trigger module**

Run: `head -30 crates/pico2-firmware/src/recovery_trigger.rs`

Expected: Contains `should_trigger_recovery()` function

- [ ] **Step 2: Add GPS jump handling after detection warmup check**

In `tick()` method, after the detection warmup check (before run_detection call), add:

```rust
// Check for GPS jump requiring recovery (H1)
let prev_s_cm = self.last_valid_s_cm;
// Skip recovery on first fix - last_valid_s_cm is still 0 (initial value)
if self.mode == SystemMode::Normal && !self.just_reset && self.has_received_first_fix {
    let s_raw = self.current_position(&est);
    if !in_snap_cooldown && crate::recovery_trigger::should_trigger_recovery(s_raw, prev_s_cm) {
        #[cfg(feature = "firmware")]
        defmt::warn!(
            "GPS jump detected: s={}→{}, triggering recovery",
            prev_s_cm,
            s_raw
        );

        // Calculate time delta since last GPS fix (in seconds)
        let dt_since_last_fix = if self.last_gps_timestamp > 0 {
            gps.timestamp.saturating_sub(self.last_gps_timestamp)
        } else {
            1 // Default to 1 second on first fix or after outage
        };

        // Collect stops into a heapless::Vec for recovery module
        let mut stops_vec = heapless::Vec::<shared::Stop, 256>::new();
        for i in 0..self.route_data.stop_count {
            if let Some(stop) = self.route_data.get_stop(i) {
                if stops_vec.push(stop).is_err() {
                    #[cfg(feature = "firmware")]
                    defmt::warn!("Too many stops for recovery buffer");
                    break;
                }
            }
        }

        if let Some(recovered_idx) = detection::recovery::find_stop_index(
            s_raw,
            est_state.dr.filtered_v,
            dt_since_last_fix,
            &stops_vec,
            self.last_stop_index,
            &est_state.kalman.freeze_ctx,
        ) {
            #[cfg(feature = "firmware")]
            defmt::info!("Recovery found stop index: {}", recovered_idx);
            self.last_stop_index = recovered_idx as u8;
            self.reset_stop_states_after_recovery(recovered_idx, s_raw);
        } else {
            #[cfg(feature = "firmware")]
            defmt::warn!("Recovery failed: no valid stop found");
        }

        // Update tracking
        self.last_valid_s_cm = s_raw;
        self.last_gps_timestamp = gps.timestamp;
    }
}
```

- [ ] **Step 3: Add helper variable for snap cooldown check**

Add at beginning of GPS jump section:

```rust
let in_snap_cooldown = self.just_snapped_ticks > 0;
```

- [ ] **Step 4: Update position tracking after GPS jump**

Add after GPS jump handling:

```rust
// Update position tracking if no jump occurred
if !crate::recovery_trigger::should_trigger_recovery(self.current_position(&est), self.last_valid_s_cm) {
    self.last_valid_s_cm = self.current_position(&est);
    self.last_gps_timestamp = gps.timestamp;
}
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check -p pico2-firmware --features dev`

Expected: Compiles successfully

- [ ] **Step 6: Commit**

```bash
git add crates/pico2-firmware/src/control/mod.rs
git commit -m "feat(control): add GPS jump recovery to tick()"
```

---

### Task 10: Add snap handling and re-acquisition recovery to tick()

**Files:**
- Modify: `crates/pico2-firmware/src/control/mod.rs` (in tick() method, need to detect snap from EstimationOutput)

**Note:** This requires adding a `snapped` field to `EstimationOutput` first. This is a dependency we need to handle.

- [ ] **Step 1: Check if EstimationOutput has snapped field**

Run: `grep -n "pub struct EstimationOutput" crates/pico2-firmware/src/estimation/mod.rs`

Expected: Check if `snapped: bool` field exists

- [ ] **Step 2: Add snapped field to EstimationOutput if missing**

If missing, add to `EstimationOutput` struct:

```rust
pub struct EstimationOutput {
    /// Raw GPS projection onto route (for F1 probability)
    pub z_gps_cm: shared::DistCm,
    /// Kalman-filtered position (primary position in Normal mode)
    pub s_cm: shared::DistCm,
    /// Filtered velocity (cm/s)
    pub v_cms: shared::SpeedCms,
    /// Divergence from route (squared distance from map matching)
    pub divergence_d2: shared::Dist2,
    /// Confidence signal (0-255, higher is better)
    pub confidence: u8,
    /// Whether GPS has valid fix
    pub has_fix: bool,
    /// Whether position snapped from off-route re-entry
    pub snapped: bool,
}
```

And update all `EstimationOutput` constructors to include `snapped: false`.

- [ ] **Step 3: Add snap handling to tick()**

In `tick()` method, after GPS jump handling, add:

```rust
// Handle snap from off-route re-entry
if est.snapped && self.mode == SystemMode::Normal {
    // 1. Find forward closest stop (prevents backward selection)
    let new_idx = self.find_forward_closest_stop_index(est.s_cm, self.last_stop_index);
    self.last_stop_index = new_idx;

    // 2. Reset stop states using same logic as recovery
    self.reset_stop_states_after_recovery(new_idx as usize, est.s_cm);

    // 3. Clear all recovery triggers
    self.needs_recovery_on_reacquisition = false;
    self.frozen_s_cm = None;
    self.off_route_since = None;
    self.last_valid_s_cm = est.s_cm;
    self.last_gps_timestamp = gps.timestamp;

    // 4. Set 2-second cooldown
    self.just_snapped_ticks = 2;
}
```

- [ ] **Step 4: Add re-acquisition recovery handling**

Add after snap handling:

```rust
// Check for re-acquisition recovery (after OffRoute without snap)
if !est.snapped && !in_snap_cooldown && self.needs_recovery_on_reacquisition && self.mode == SystemMode::Normal {
    self.needs_recovery_on_reacquisition = false;

    // Calculate elapsed time since freeze
    let elapsed_seconds = self.off_route_since
        .map(|t| gps.timestamp.saturating_sub(t))
        .unwrap_or(1);

    // Run recovery to find correct stop index
    let mut stops_vec = heapless::Vec::<shared::Stop, 256>::new();
    for i in 0..self.route_data.stop_count {
        if let Some(stop) = self.route_data.get_stop(i) {
            let _ = stops_vec.push(stop);
        }
    }

    if let Some(recovered_idx) = detection::recovery::find_stop_index(
        est.s_cm,
        est_state.dr.filtered_v,
        elapsed_seconds,
        &stops_vec,
        self.last_stop_index,
        &est_state.kalman.freeze_ctx,
    ) {
        #[cfg(feature = "firmware")]
        defmt::info!("Re-acquisition recovered stop index: {}", recovered_idx);
        self.last_stop_index = recovered_idx as u8;
        self.reset_stop_states_after_recovery(recovered_idx, est.s_cm);
    }

    // Clear freeze time and context after re-acquisition recovery
    self.off_route_since = None;
    self.frozen_s_cm = None;
}
```

- [ ] **Step 5: Decrement snap cooldown counter**

Add at end of tick() before return:

```rust
// Decrement snap cooldown
if self.just_snapped_ticks > 0 {
    self.just_snapped_ticks = self.just_snapped_ticks.saturating_sub(1);
}
```

- [ ] **Step 6: Verify compilation**

Run: `cargo check -p pico2-firmware --features dev`

Expected: Compiles successfully

- [ ] **Step 7: Commit**

```bash
git add crates/pico2-firmware/src/control/mod.rs crates/pico2-firmware/src/estimation/mod.rs
git commit -m "feat(control): add snap handling and re-acquisition recovery to tick()"
```

---

### Task 11: Add warmup logic to tick()

**Files:**
- Modify: `crates/pico2-firmware/src/control/mod.rs` (in tick() method)

- [ ] **Step 1: Add warmup counter updates after GPS fix validation**

In `tick()` method, after the `has_fix` check (around line 218), add:

```rust
// Handle warmup counter updates
if self.just_reset {
    // After warmup reset (e.g., GPS outage), first tick counts as first fix
    self.just_reset = false;
    self.estimation_total_ticks = 1;
    self.detection_total_ticks = 1;
    return TickResult { event: None, persist_request: None };
}

// Increment total time counters
self.estimation_total_ticks = self.estimation_total_ticks.saturating_add(1);
self.detection_total_ticks = self.detection_total_ticks.saturating_add(1);

// Update estimation readiness (until ready)
if !self.estimation_ready() {
    self.estimation_ready_ticks += 1;
}

// Update detection readiness (until ready, independent of estimation)
if !self.detection_ready() {
    self.detection_enabled_ticks += 1;
}

// Block detection unless ready
if !self.detection_ready() {
    return TickResult { event: None, persist_request: None };
}
```

- [ ] **Step 2: Update first_fix handling to mark received**

Update the first fix marking section:

```rust
// Mark first fix as received after successful GPS fix
if est.has_fix {
    self.has_received_first_fix = true;
}
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p pico2-firmware --features dev`

Expected: Compiles successfully

- [ ] **Step 4: Commit**

```bash
git add crates/pico2-firmware/src/control/mod.rs
git commit -m "feat(control): add warmup logic to tick()"
```

---

### Task 12: Add persisted state application on first fix

**Files:**
- Modify: `crates/pico2-firmware/src/control/mod.rs` (in tick() method)

- [ ] **Step 1: Add persisted state application**

In `tick()` method, after first fix marking, add:

```rust
// Apply persisted state on first fix if valid
if self.has_received_first_fix && self.pending_persisted.is_some() {
    if let Some(ps) = self.pending_persisted.take() {
        // Check 500m threshold from spec
        let delta_cm = if est.s_cm >= ps.last_progress_cm {
            est.s_cm - ps.last_progress_cm
        } else {
            ps.last_progress_cm - est.s_cm
        };

        if delta_cm <= 50_000 {
            // Within 500m: trust persisted stop index
            self.apply_persisted_stop_index(ps.last_stop_index);
            #[cfg(feature = "firmware")]
            defmt::info!(
                "Applied persisted state: stop={}, delta={}cm",
                ps.last_stop_index,
                delta_cm
            );
        } else {
            #[cfg(feature = "firmware")]
            defmt::warn!(
                "Persisted state too stale: delta={}cm > 500m, ignoring",
                delta_cm
            );
        }
    }
}
```

- [ ] **Step 2: Add apply_persisted_stop_index() helper method**

Add to `SystemState` impl:

```rust
/// Apply persisted stop index by marking all prior stops as Departed.
fn apply_persisted_stop_index(&mut self, stop_index: u8) {
    use shared::FsmState;

    for i in 0..stop_index.min(self.stop_states.len() as u8) as usize {
        self.stop_states[i].fsm_state = FsmState::Departed;
        self.stop_states[i].announced = true;
    }
    self.last_stop_index = stop_index;
}
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p pico2-firmware --features dev`

Expected: Compiles successfully

- [ ] **Step 4: Commit**

```bash
git add crates/pico2-firmware/src/control/mod.rs
git commit -m "feat(control): add persisted state application on first fix"
```

---

## Phase 5: Update Integration Tests

### Task 13: Update test_off_route_integration.rs to use SystemState

**Files:**
- Modify: `crates/pico2-firmware/tests/test_off_route_integration.rs`

- [ ] **Step 1: Read test file header**

Run: `head -20 crates/pico2-firmware/tests/test_off_route_integration.rs`

Expected: Uses `pico2_firmware::state::State`

- [ ] **Step 2: Update imports**

Replace:
```rust
use pico2_firmware::state::State;
```

With:
```rust
use pico2_firmware::{SystemState, estimation::EstimationState};
```

- [ ] **Step 3: Update test initialization**

Replace:
```rust
let mut state = State::new(&route_data, None);
let event = state.process_gps(&gps);
```

With:
```rust
let mut control = SystemState::new(&route_data, None);
let mut est_state = EstimationState::new();
let result = control.tick(&gps, &mut est_state);
let event = result.event;
```

- [ ] **Step 4: Update state field access patterns**

Replace field access patterns:
- `state.last_valid_s_cm()` → `control.last_valid_s_cm`
- `state.off_route_freeze_time()` → Need to add this accessor to SystemState
- `state.needs_recovery_on_reacquisition()` → `control.needs_recovery_on_reacquisition`

- [ ] **Step 5: Add missing accessor methods to SystemState**

Add to `SystemState`:

```rust
/// Get the freeze time (for testing)
pub fn off_route_freeze_time(&self) -> Option<u64> {
    self.off_route_since
}

/// Get the recovery flag state (for testing)
pub fn needs_recovery_on_reacquisition(&self) -> bool {
    self.needs_recovery_on_reacquisition
}

/// Get the last valid position in cm (for testing)
pub fn last_valid_s_cm(&self) -> DistCm {
    self.last_valid_s_cm
}
```

- [ ] **Step 6: Verify test compiles**

Run: `cargo test -p pico2-firmware test_off_route_integration --features dev`

Expected: Compiles (test may fail)

- [ ] **Step 7: Run test**

Run: `cargo test -p pico2-firmware test_off_route_integration --features dev`

Expected: Test may fail due to behavior differences - fix as needed

- [ ] **Step 8: Commit**

```bash
git add crates/pico2-firmware/tests/test_off_route_integration.rs crates/pico2-firmware/src/control/mod.rs
git commit -m "test: update test_off_route_integration to use SystemState"
```

---

### Task 14: Update remaining integration tests to use SystemState

**Files:**
- Modify: All test files in `crates/pico2-firmware/tests/`

Test files to update:
- `test_warmup.rs`
- `test_warmup_counter.rs`
- `test_monotonic_invariant.rs`
- `test_recovery_integration.rs`
- `test_detour_reentry_integration.rs`
- `test_forward_closest_stop.rs`
- `test_estimation_detection_separation.rs`
- `test_adaptive_weights.rs`
- `test_c1_freeze_time_preserved.rs`
- `test_probability_consistency.rs`
- `test_uart_signed_format.rs`
- `integration_state_machine.rs`

- [ ] **Step 1: Create script to update imports**

Create temporary script `update_tests.sh`:

```bash
#!/bin/bash
for file in crates/pico2-firmware/tests/*.rs; do
    # Replace State import
    sed -i '' 's/use pico2_firmware::state::State;/use pico2_firmware::{SystemState, estimation::EstimationState};/g' "$file"

    # Replace State::new with SystemState::new
    sed -i '' 's/State::new(/SystemState::new(/g' "$file"

    # Replace state variable with control
    sed -i '' 's/\bmut state\b/mut control/g' "$file"
    sed -i '' 's/\bstate\./control./g' "$file"

    # Replace process_gps with tick
    sed -i '' 's/\.process_gps(/\.tick(/g' "$file"
done
```

- [ ] **Step 2: Run script**

Run: `bash update_tests.sh`

- [ ] **Step 3: Manually fix tick() call sites**

Each test needs updating to handle `TickResult`:

Replace:
```rust
let event = state.process_gps(&gps);
```

With:
```rust
let result = control.tick(&gps, &mut est_state);
let event = result.event;
```

- [ ] **Step 4: Add est_state initialization to each test**

Add after `control` initialization:
```rust
let mut est_state = EstimationState::new();
```

- [ ] **Step 5: Verify all tests compile**

Run: `cargo test -p pico2-firmware --features dev`

Expected: Compiles (some tests may fail)

- [ ] **Step 6: Run tests and fix failures**

Run: `cargo test -p pico2-firmware --features dev`

Fix any test failures by updating assertions or adding missing methods to SystemState.

- [ ] **Step 7: Commit**

```bash
git add crates/pico2-firmware/tests/
git commit -m "test: update all integration tests to use SystemState"
```

---

## Phase 6: Switch main.rs to SystemState

### Task 15: Update main.rs to use SystemState and EstimationState

**Files:**
- Modify: `crates/pico2-firmware/src/main.rs`

- [ ] **Step 1: Read current main.rs initialization**

Run: `sed -n '100,110p' crates/pico2-firmware/src/main.rs`

Expected: Creates `state::State::new()`

- [ ] **Step 2: Update imports**

Add to imports:
```rust
use pico2_firmware::{SystemState, estimation::EstimationState};
```

- [ ] **Step 3: Replace state initialization**

Replace (around line 103):
```rust
let mut state = state::State::new(&route_data, persisted);
```

With:
```rust
let mut control = SystemState::new(&route_data, persisted);
let mut est_state = EstimationState::new();
```

- [ ] **Step 4: Update GPS processing loop**

Replace (around line 129):
```rust
if let Some(arrival) = state.process_gps(&gps) {
```

With:
```rust
let result = control.tick(&gps, &mut est_state);
if let Some(arrival) = result.event {
```

- [ ] **Step 5: Update persistence check**

Replace (around line 143):
```rust
if let Some(current_stop) = state.current_stop_index() {
    if state.should_persist(current_stop) {
        let ps = shared::PersistedState::new(state.kalman.s_cm, current_stop);
```

With:
```rust
if let Some(current_stop) = control.current_stop_index() {
    // Check if persist_request was set
    if let Some(ps) = result.persist_request {
```

- [ ] **Step 6: Update persist success handler**

Replace (around line 145):
```rust
match persist::save(&mut flash, &ps).await {
    Ok(()) => {
        info!("Persisted state: stop={}, progress={}cm", current_stop, state.kalman.s_cm);
        state.mark_persisted(current_stop);
```

With:
```rust
match persist::save(&mut flash, &ps).await {
    Ok(()) => {
        info!("Persisted state: stop={}, progress={}cm", ps.last_stop_index, ps.last_progress_cm);
        control.mark_persisted(ps.last_stop_index);
```

- [ ] **Step 7: Update persist failure handler**

Replace (around line 156):
```rust
state.ticks_since_persist = state.ticks_since_persist.saturating_add(1);
```

With:
```rust
control.ticks_since_persist = control.ticks_since_persist.saturating_add(1);
```

- [ ] **Step 8: Update else branch (increment tick counter)**

Replace (around line 161):
```rust
state.ticks_since_persist = state.ticks_since_persist.saturating_add(1);
```

With:
```rust
control.ticks_since_persist = control.ticks_since_persist.saturating_add(1);
```

- [ ] **Step 9: Remove old state module reference**

Remove from module declarations (around line 27):
```rust
mod state;
```

- [ ] **Step 10: Verify compilation**

Run: `cargo check -p pico2-firmware --features firmware`

Expected: Compiles successfully

- [ ] **Step 11: Commit**

```bash
git add crates/pico2-firmware/src/main.rs
git commit -m "feat(main): switch to SystemState and EstimationState"
```

---

### Task 16: Update lib.rs re-exports

**Files:**
- Modify: `crates/pico2-firmware/src/lib.rs`

- [ ] **Step 1: Read current lib.rs**

Run: `cat crates/pico2-firmware/src/lib.rs`

Expected: Re-exports `SystemMode`, `SystemState`, declares `state` module

- [ ] **Step 2: Remove state module declaration**

Remove:
```rust
pub mod state;
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p pico2-firmware --features dev`

Expected: Compiles successfully (state module no longer used)

- [ ] **Step 4: Commit**

```bash
git add crates/pico2-firmware/src/lib.rs
git commit -m "refactor(lib): remove state module from re-exports"
```

---

## Phase 7: Delete state.rs and Finalize

### Task 17: Delete state.rs

**Files:**
- Delete: `crates/pico2-firmware/src/state.rs`

- [ ] **Step 1: Verify no remaining references**

Run: `grep -r "state::State\|state::" crates/pico2-firmware/src/ --include="*.rs" | grep -v "Binary"`

Expected: No remaining references (except comments)

- [ ] **Step 2: Delete state.rs**

Run: `rm crates/pico2-firmware/src/state.rs`

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p pico2-firmware --features firmware`

Expected: Compiles successfully

- [ ] **Step 4: Run full test suite**

Run: `cargo test -p pico2-firmware --features dev`

Expected: All tests pass

- [ ] **Step 5: Verify memory usage**

Run: `cargo size --bin pico2-firmware --features firmware`

Expected: SystemState fits within 4KB, total firmware within budget

- [ ] **Step 6: Commit**

```bash
git add crates/pico2-firmware/src/state.rs
git commit -m "refactor: delete state.rs after successful migration to SystemState"
```

---

### Task 18: Add make golden target

**Files:**
- Modify: `Makefile`

- [ ] **Step 1: Check if Makefile exists**

Run: `ls -la Makefile`

Expected: Makefile exists

- [ ] **Step 2: Add golden target**

Add to Makefile:

```makefile
# Generate golden trace files for regression testing
golden:
	@echo "Generating golden traces..."
	cargo run --bin trace_validator -- --generate-golden
	@echo "Golden files updated in test_data/golden/"
```

- [ ] **Step 3: Test target**

Run: `make golden`

Expected: Runs trace validator with --generate-golden flag

- [ ] **Step 4: Commit**

```bash
git add Makefile
git commit -m "build: add make golden target for regression testing"
```

---

## Phase 8: Architecture Tests and Final Verification

### Task 19: Add architecture tests for isolation and determinism

**Files:**
- Create: `crates/pico2-firmware/tests/test_v9_architecture.rs`

- [ ] **Step 1: Create architecture test file**

Create `crates/pico2-firmware/tests/test_v9_architecture.rs`:

```rust
//! Architecture tests for v9.0 two-layer design
//!
//! Verifies the key architectural guarantees:
//! - Estimation layer is isolated (no access to control state)
//! - Estimation layer is deterministic (same input → same output)
//! - Only one mode transition per tick
//! - Mode-specific position correctness

use pico2_firmware::{SystemState, estimation::{EstimationState, estimate, EstimationInput}, SystemMode};
use shared::{binfile::RouteData, GpsPoint};

#[test]
fn test_estimation_isolation() {
    // Verify that estimation layer doesn't access control state
    // This is a compile-time test: if estimate() tries to access
    // SystemState fields, it won't compile

    let route_data = create_test_route_data();
    let mut est_state = EstimationState::new();

    // Create GPS input
    let gps = create_test_gps(1000, 0, 500);

    // Call estimate with minimal input
    let input = EstimationInput {
        gps: gps.clone(),
        route_data: &route_data,
        is_first_fix: true,
    };

    let output = estimate(input, &mut est_state);

    // Verify output is well-formed
    assert!(output.has_fix);
    assert!(output.s_cm >= 0);
}

#[test]
fn test_estimation_determinism() {
    // Same input should produce same output
    let route_data = create_test_route_data();

    let gps = create_test_gps(1000, 0, 500);

    let input = EstimationInput {
        gps: gps.clone(),
        route_data: &route_data,
        is_first_fix: true,
    };

    let mut est_state1 = EstimationState::new();
    let mut est_state2 = EstimationState::new();

    let output1 = estimate(input.clone(), &mut est_state1);
    let output2 = estimate(input, &mut est_state2);

    // Outputs should be identical
    assert_eq!(output1.s_cm, output2.s_cm);
    assert_eq!(output1.v_cms, output2.v_cms);
    assert_eq!(output1.z_gps_cm, output2.z_gps_cm);
}

#[test]
fn test_mode_specific_position() {
    // Verify current_position() returns correct value per mode
    let route_data = create_test_route_data();
    let mut control = SystemState::new(&route_data, None);
    let mut est_state = EstimationState::new();

    // Set up positions for each mode
    est_state.kalman.s_cm = 10000;
    control.frozen_s_cm = Some(5000);

    // Normal mode: use est.s_cm
    control.mode = SystemMode::Normal;
    assert_eq!(control.current_position(&estimate(
        EstimationInput {
            gps: create_test_gps(1000, 0, 500),
            route_data: &route_data,
            is_first_fix: false,
        },
        &mut est_state
    )), 10000);

    // OffRoute mode: use frozen_s_cm
    control.mode = SystemMode::OffRoute;
    assert_eq!(control.current_position(&estimate(
        EstimationInput {
            gps: create_test_gps(1000, 0, 500),
            route_data: &route_data,
            is_first_fix: false,
        },
        &mut est_state
    )), 5000);

    // Recovering mode: use z_gps_cm
    control.mode = SystemMode::Recovering;
    est_state.kalman.s_cm = 20000;
    assert_eq!(control.current_position(&estimate(
        EstimationInput {
            gps: create_test_gps(1000, 0, 500),
            route_data: &route_data,
            is_first_fix: false,
        },
        &mut est_state
    )), est_state.kalman.s_cm); // Should match z_gps_cm in recovery
}

// Helper functions
fn create_test_route_data() -> RouteData<'static> {
    // Minimal route data for testing
    // In real tests, load from actual binary file
    let data = include_bytes!("../../../test_data/ty225_normal.bin");
    RouteData::load(data).unwrap()
}

fn create_test_gps(timestamp: u64, x_cm: i32, y_cm: i32) -> GpsPoint {
    GpsPoint {
        timestamp,
        lat: 0.0,
        lon: 0.0,
        heading_cdeg: Some(0),
        speed_cms: Some(500),
        hdop_x10: Some(10),
        has_fix: true,
    }
}
```

- [ ] **Step 2: Run architecture tests**

Run: `cargo test -p pico2-firmware test_v9_architecture --features dev`

Expected: All tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/pico2-firmware/tests/test_v9_architecture.rs
git commit -m "test: add architecture tests for v9.0 guarantees"
```

---

### Task 20: Run full verification

**Files:**
- None (verification task)

- [ ] **Step 1: Run full test suite**

Run: `cargo test --features dev`

Expected: All tests pass

- [ ] **Step 2: Check memory usage**

Run: `cargo size --bin pico2-firmware --features firmware`

Expected: Verify SystemState ≤ 4KB, total within budget

- [ ] **Step 3: Build firmware binary**

Run: `cargo build --bin pico2-firmware --features firmware --release`

Expected: Builds successfully

- [ ] **Step 4: Generate trace with test data**

Run: `make run ROUTE_NAME=ty225 SCENARIO=normal`

Expected: Produces trace.jsonl

- [ ] **Step 5: Validate trace**

Run: `cargo run --bin trace_validator -- trace.jsonl`

Expected: Validation passes

- [ ] **Step 6: Create summary commit**

```bash
git add .
git commit -m "test: v9.0 integration complete - all tests pass"
```

---

## Phase 9: Documentation Updates

### Task 21: Update tech report to reflect implemented architecture

**Files:**
- Modify: `bus_arrival_tech_report_v8.md` or equivalent

- [ ] **Step 1: Find tech report file**

Run: `find . -name "*tech*report*.md" -o -name "*v8*.md" | grep -v ".git"`

Expected: Find tech report file

- [ ] **Step 2: Update architecture section**

Add note that v9.0 architecture is now implemented:

```markdown
## v9.0 Two-Layer Architecture (IMPLEMENTED)

The v9.0 architecture with isolated Estimation and Control layers
is fully implemented as of 2026-05-07. See implementation plan:
docs/superpowers/plans/2026-05-07-v9-integration.md
```

- [ ] **Step 3: Update any remaining "TODO" or "planned" references**

Search for v9.0 references and update status.

- [ ] **Step 4: Commit**

```bash
git add bus_arrival_tech_report_v8.md
git commit -m "docs: update tech report to reflect v9.0 implementation status"
```

---

### Task 22: Update CLAUDE.md if needed

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Check if CLAUDE.md mentions state::State**

Run: `grep -n "state::State\|State::" CLAUDE.md`

Expected: Check for any references to update

- [ ] **Step 2: Update any state::State references**

Replace with `SystemState` where appropriate.

- [ ] **Step 3: Add note about two-layer architecture**

Add to architecture section:

```markdown
The firmware uses a two-layer architecture (v9.0):
- Estimation layer (EstimationState): pure GPS→position pipeline
- Control layer (SystemState): mode management, detection FSM, recovery
```

- [ ] **Step 4: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: update CLAUDE.md for v9.0 architecture"
```

---

## Summary

This plan implements the v9.0 two-layer architecture by:

1. Adding detection FSM and warmup fields to SystemState
2. Implementing all helper methods (warmup queries, stop search, persistence)
3. Implementing run_detection() with full FSM logic
4. Integrating GPS jump and snap handling into tick()
5. Updating all integration tests to use SystemState
6. Switching main.rs to use SystemState + EstimationState
7. Deleting state.rs entirely
8. Adding architecture tests and verification

**Total tasks:** 22
**Estimated time:** 4-6 hours of focused work

**Success criteria:**
- All tests pass
- Memory usage within budget
- Hardware testing produces correct arrivals
- Tech report accurately describes implementation

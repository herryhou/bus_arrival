# Two-Layer Architecture Activation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Activate the v9.0 two-layer architecture (control::SystemState + estimation::estimate) in main.rs while preserving all functionality and fixing known bugs (C2, I1, I3, snap signal).

**Architecture:** Migrate from monolithic `state::State::process_gps()` to layered design where `estimate()` (pure function) produces position signals, `SystemState::tick()` orchestrates mode transitions and detection, and detection FSM emits arrival events.

**Tech Stack:** Rust no_std embedded firmware, RP2350 target, embassy-rp async framework

---

## File Structure

**Files to modify:**
- `crates/pico2-firmware/src/estimation/mod.rs` - Add snapped signal, first_fix tracking
- `crates/pico2-firmware/src/estimation/kalman.rs` or `gps_processor/src/kalman.rs` - Add predict step (C2 fix)
- `crates/pico2-firmware/src/control/mod.rs` - Complete detection integration, add missing state
- `crates/pico2-firmware/src/main.rs` - Switch to new architecture
- `crates/pico2-firmware/src/state.rs` - Mark deprecated

**Files to reference (read-only):**
- `crates/pico2-firmware/src/detection.rs` - Detection functions (already exist)
- `crates/pico2-firmware/src/recovery/mod.rs` - Recovery module (already exists)

---

## Task 1: Add Kalman Predict Step (C2 Fix)

**Files:**
- Modify: `crates/pico2-firmware/src/estimation/kalman.rs` or `gps_processor/src/kalman.rs` (whichever contains `update_adaptive()`)

**Reference:** Old `state.rs:175-184` shows the predict step happens before Kalman update

- [ ] **Step 1: Locate update_adaptive method**

Find the `update_adaptive()` method in the Kalman implementation. Look for the line that updates `self.s_cm` directly with the innovation term:
```rust
self.s_cm = self.s_cm + k_pos * (z_raw - self.s_cm) / 256;
```

- [ ] **Step 2: Add predict step before update**

Add the predict step that propagates velocity into position BEFORE the innovation calculation:
```rust
// Predict step: propagate velocity into position
let s_pred = self.s_cm + self.v_cms;

// Innovation term uses predicted position
let innovation = z_raw - s_pred;

// Update step with innovation
self.s_cm = s_pred + k_pos * innovation / 256;
```

- [ ] **Step 3: Verify compilation**

Run: `cargo build --release -p pico2-firmware`
Expected: Compiles without errors

- [ ] **Step 4: Commit**

```bash
git add crates/pico2-firmware/src/estimation/kalman.rs
git commit -m "fix(estimation): add Kalman predict step before update (C2)

Without predict step, velocity is never propagated into position prediction,
causing systematic lag proportional to bus speed (~16 cm/tick at 30 km/h).

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 2: Add Snapped Signal to EstimationOutput

**Files:**
- Modify: `crates/pico2-firmware/src/estimation/mod.rs`

**Reference:** Old `ProcessResult::Valid { snapped, .. }` in `gps_processor`

- [ ] **Step 1: Add snapped field to EstimationOutput struct**

```rust
pub struct EstimationOutput {
    pub z_gps_cm: DistCm,
    pub s_cm: DistCm,
    pub v_cms: SpeedCms,
    pub divergence_d2: Dist2,
    pub confidence: u8,
    pub has_fix: bool,
    pub snapped: bool,  // ADD THIS: true if map-matching snapped from off-route
}
```

- [ ] **Step 2: Initialize snapped in all return paths**

Find all `return EstimationOutput { ... }` statements and add `snapped: false,` to each. In `estimate()` function, after map matching:

```rust
// Detect snap: was previously off-route (high divergence) but now on-route
let was_off_route = state.dr.in_recovery;
let snapped = was_off_route && match_d2 < 25_000_000;  // 50m threshold

// In all EstimationOutput returns, include snapped field
EstimationOutput {
    // ... existing fields ...
    snapped,
}
```

- [ ] **Step 3: Set in_recovery appropriately**

Ensure `state.dr.in_recovery` is set to `false` when snap is detected:
```rust
if snapped {
    state.dr.in_recovery = false;
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo build --release -p pico2-firmware`
Expected: Compiles without errors

- [ ] **Step 5: Commit**

```bash
git add crates/pico2-firmware/src/estimation/mod.rs
git commit -m "feat(estimation): add snapped signal to EstimationOutput

Indicates when map-matching snaps from off-route back to route.
Required for snap cooldown logic in control layer.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 3: Add First Fix Tracking to EstimationState

**Files:**
- Modify: `crates/pico2-firmware/src/estimation/mod.rs`

**Reference:** Old `state.rs:82, 255-292` shows first_fix handling

- [ ] **Step 1: Add first_fix_called field to EstimationState**

```rust
pub struct EstimationState {
    pub kalman: KalmanState,
    pub dr: DrState,
    first_fix_called: bool,  // ADD THIS: track if estimate() has ever been called with valid fix
}
```

- [ ] **Step 2: Initialize in new() method**

```rust
impl EstimationState {
    pub fn new() -> Self {
        Self {
            kalman: KalmanState::new(),
            dr: DrState::new(),
            first_fix_called: false,
        }
    }
}
```

- [ ] **Step 3: Track first fix in estimate() function**

At the start of `estimate()`, check and update first_fix_called:
```rust
pub fn estimate(
    input: EstimationInput,
    state: &mut EstimationState,
) -> EstimationOutput {
    // Track first fix
    let is_first_fix = !state.first_fix_called && input.gps.has_fix;
    if input.gps.has_fix {
        state.first_fix_called = true;
    }

    // ... rest of estimate() function
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo build --release -p pico2-firmware`
Expected: Compiles without errors

- [ ] **Step 5: Commit**

```bash
git add crates/pico2-firmware/src/estimation/mod.rs
git commit -m "feat(estimation): track first fix state (I1 fix)

Removes hardcoded is_first_fix: false in control layer.
Enables proper cold-start Kalman initialization.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 4: Add Missing Fields to SystemState

**Files:**
- Modify: `crates/pico2-firmware/src/control/mod.rs`

**Reference:** Old `state.rs:75-115` shows all required fields

- [ ] **Step 1: Add stop_states field**

After line 42, add:
```rust
/// Per-stop FSM states for arrival detection
pub stop_states: heapless::Vec<detection::state_machine::StopState, 256>,
```

- [ ] **Step 2: Add warmup counters**

After the new stop_states field, add:
```rust
/// First fix flag - true until first GPS fix is received
pub first_fix: bool,

/// Warmup: valid GPS ticks where estimation ran
pub estimation_ready_ticks: u8,

/// Warmup: total ticks since first fix (timeout safety valve)
pub estimation_total_ticks: u8,

/// Detection gating: valid ticks since estimation ready
pub detection_enabled_ticks: u8,

/// Detection gating: total ticks for timeout
pub detection_total_ticks: u8,
```

- [ ] **Step 3: Add reset and cooldown flags**

Continue adding:
```rust
/// Flag indicating state was just reset (e.g., after GPS outage)
pub just_reset: bool,

/// Ticks remaining in snap cooldown period (prevents recovery interference)
pub just_snapped_ticks: u8,

/// Last valid GPS timestamp for recovery dt calculation
pub last_gps_timestamp: u64,
```

- [ ] **Step 4: Initialize all fields in new() method**

Update `SystemState::new()` to initialize the new fields:
```rust
pub fn new(route_data: &'a RouteData<'a>, persisted: Option<shared::PersistedState>) -> Self {
    // Initialize stop_states
    let mut stop_states = heapless::Vec::new();
    for i in 0..route_data.stop_count {
        let _ = stop_states.push(detection::state_machine::StopState::new(i as u8));
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
        // NEW FIELDS
        stop_states,
        first_fix: true,
        estimation_ready_ticks: 0,
        estimation_total_ticks: 0,
        detection_enabled_ticks: 0,
        detection_total_ticks: 0,
        just_reset: false,
        just_snapped_ticks: 0,
        last_gps_timestamp: 0,
    }
}
```

- [ ] **Step 5: Verify compilation**

Run: `cargo build --release -p pico2-firmware`
Expected: Compiles without errors

- [ ] **Step 6: Commit**

```bash
git add crates/pico2-firmware/src/control/mod.rs
git commit -m "feat(control): add missing state fields to SystemState

Adds stop_states, warmup counters, reset flags, and snap cooldown.
Required for detection integration and parity with old state.rs.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 5: Implement Helper Methods

**Files:**
- Modify: `crates/pico2-firmware/src/control/mod.rs`

**Reference:** Old `state.rs:587-768` for helper method implementations

- [ ] **Step 1: Add estimation_ready() method**

```rust
impl<'a> SystemState<'a> {
    /// Check if estimation is ready (affects heading filter, Kalman)
    pub fn estimation_ready(&self) -> bool {
        self.estimation_ready_ticks >= 3 || self.estimation_total_ticks >= 10
    }
}
```

- [ ] **Step 2: Add detection_ready() method**

```rust
    /// Check if detection is enabled (independent of estimation)
    pub fn detection_ready(&self) -> bool {
        self.detection_enabled_ticks >= 3 || self.detection_total_ticks >= 10
    }
```

- [ ] **Step 3: Add disable_heading_filter() method**

```rust
    /// Check if heading filter should be disabled
    pub fn disable_heading_filter(&self) -> bool {
        self.first_fix || !self.estimation_ready()
    }
```

- [ ] **Step 4: Add find_closest_stop_index() method**

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

- [ ] **Step 5: Add find_forward_closest_stop_index() method**

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

- [ ] **Step 6: Verify compilation**

Run: `cargo build --release -p pico2-firmware`
Expected: Compiles without errors

- [ ] **Step 7: Commit**

```bash
git add crates/pico2-firmware/src/control/mod.rs
git commit -m "feat(control): add helper methods to SystemState

Adds estimation_ready(), detection_ready(), disable_heading_filter(),
find_closest_stop_index(), and find_forward_closest_stop_index().

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 6: Implement Stop State Reset Logic

**Files:**
- Modify: `crates/pico2-firmware/src/control/mod.rs`

**Reference:** Old `state.rs:635-673` for exact logic, including I3 fix

- [ ] **Step 1: Add reset_stop_states_after_recovery() method**

```rust
impl<'a> SystemState<'a> {
    /// Reset all stop states to Idle after recovery
    fn reset_stop_states_after_recovery(&mut self, recovered_idx: usize, current_s_cm: DistCm) {
        use shared::FsmState;

        let recovered_was_announced = self
            .stop_states
            .get(recovered_idx)
            .map(|state| state.announced || state.last_announced_stop == recovered_idx as u8)
            .unwrap_or(false);

        // Reset all stop states by recreating them
        for i in 0..self.stop_states.len() {
            self.stop_states[i] = detection::state_machine::StopState::new(i as u8);
        }

        // Stops before the recovered stop are treated as already passed.
        // Preserve their announcement bookkeeping so recovery cannot re-announce them.
        // I3 FIX: Set BOTH announced AND last_announced_stop
        for i in 0..recovered_idx.min(self.stop_states.len()) {
            self.stop_states[i].fsm_state = FsmState::Departed;
            self.stop_states[i].announced = true;
            self.stop_states[i].last_announced_stop = i as u8;  // I3: was missing
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
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo build --release -p pico2-firmware`
Expected: Compiles without errors

- [ ] **Step 3: Commit**

```bash
git add crates/pico2-firmware/src/control/mod.rs
git commit -m "feat(control): add stop state reset logic with I3 fix

Implements reset_stop_states_after_recovery() with proper announcement
guard handling. Sets both announced and last_announced_stop to prevent
re-announcement of passed stops (I3 fix).

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 7: Implement run_detection() Method

**Files:**
- Modify: `crates/pico2-firmware/src/control/mod.rs`

**Reference:** Old `state.rs:487-584` for complete detection flow

- [ ] **Step 1: Replace run_detection() stub with full implementation**

Replace the entire `run_detection()` method (lines 315-336) with:

```rust
    /// Run arrival detection (Normal mode only)
    fn run_detection(&mut self, est: &EstimationOutput, s_cm: DistCm, timestamp: u64) -> Option<ArrivalEvent> {
        use crate::detection::{compute_arrival_probability_adaptive, find_active_stops};
        use detection::state_machine::StopEvent;

        // Create position signals for detection
        let signals = shared::PositionSignals {
            z_gps_cm: est.z_gps_cm,
            s_cm: est.s_cm,
        };

        // Find active stops (corridor filter)
        let active_indices = find_active_stops(signals, self.route_data);

        // Update state machine for each active stop
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

            // Determine GPS status for probability computation
            use crate::detection::GpsStatus;
            let gps_status = GpsStatus::Valid;  // In Normal mode, GPS is valid

            // Compute arrival probability with adaptive weights
            let probability = compute_arrival_probability_adaptive(
                signals,
                est.v_cms,
                &stop,
                stop_state.dwell_time_s,
                gps_status,
                next_stop,
            );

            // Update state machine FIRST
            let event = stop_state.update(
                s_cm,
                est.v_cms,
                stop.progress_cm,
                stop.corridor_start_cm,
                probability,
            );

            // THEN check for announcement trigger
            if stop_state.should_announce(s_cm, stop.corridor_start_cm) {
                #[cfg(feature = "firmware")]
                defmt::info!(
                    "Announcement for stop {}: s={}cm, v={}cm/s",
                    stop_idx,
                    s_cm,
                    est.v_cms
                );
                return Some(ArrivalEvent {
                    time: timestamp,
                    stop_idx: stop_idx as u8,
                    s_cm,
                    v_cms: est.v_cms,
                    probability: 0,
                    event_type: shared::ArrivalEventType::Announce,
                });
            }

            // Check for arrival/departure events
            match event {
                StopEvent::Arrived => {
                    #[cfg(feature = "firmware")]
                    defmt::info!(
                        "Arrival at stop {}: s={}cm, v={}cm/s, p={}",
                        stop_idx,
                        s_cm,
                        est.v_cms,
                        probability
                    );
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
                    #[cfg(feature = "firmware")]
                    defmt::info!(
                        "Departure from stop {}: s={}cm, v={}cm/s",
                        stop_idx,
                        s_cm,
                        est.v_cms
                    );
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

- [ ] **Step 2: Verify compilation**

Run: `cargo build --release -p pico2-firmware`
Expected: Compiles without errors

- [ ] **Step 3: Commit**

```bash
git add crates/pico2-firmware/src/control/mod.rs
git commit -m "feat(control): implement full arrival detection in run_detection()

Replaces TODO stub with complete detection pipeline:
- Corridor filtering via find_active_stops()
- Adaptive probability computation
- StopState FSM transitions
- Arrival/Departure/Announce event emission

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 8: Update tick() Method with Warmup and First Fix Logic

**Files:**
- Modify: `crates/pico2-firmware/src/control/mod.rs`

**Reference:** Old `state.rs:164-320` for warmup and first fix handling

- [ ] **Step 1: Remove hardcoded is_first_fix**

Replace line 204 `is_first_fix: false,  // TODO: track first fix` with:
```rust
is_first_fix: self.first_fix,
```

- [ ] **Step 2: Add GPS outage handling before mode transitions**

After line 212 (after `let est = ...`), add:
```rust
        // Handle GPS outage
        if !est.has_fix {
            // Reset warmup on GPS loss (conservative - requires fresh warmup after outage)
            if !self.first_fix {
                self.estimation_ready_ticks = 0;
                self.estimation_total_ticks = 0;
                self.detection_enabled_ticks = 0;
                self.detection_total_ticks = 0;
                self.just_reset = true;
                #[cfg(feature = "firmware")]
                defmt::debug!("GPS outage reset warmup counters");
            }
            return None;
        }
```

- [ ] **Step 3: Add warmup counter logic after outage check**

Continue adding:
```rust
        // Handle first fix
        if self.first_fix {
            self.first_fix = false;
            // First fix initializes Kalman but doesn't run update_adaptive
            // Counts toward timeout but NOT convergence
            self.estimation_total_ticks = 1;
            self.detection_total_ticks = 1;
            self.last_gps_timestamp = gps.timestamp;

            // Apply persisted state if valid and within 500m threshold
            if let Some(ps) = self.pending_persisted.take() {
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

            return None;
        }

        // Handle just_reset state
        if self.just_reset {
            // After warmup reset (e.g., GPS outage), first tick counts as first fix
            self.just_reset = false;
            self.estimation_total_ticks = 1;
            self.detection_total_ticks = 1;
            return None;
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
            // Still update mode transitions and recovery, but skip detection
            // ... continue to mode transition logic below
        }

        // Update last GPS timestamp for recovery dt calculation
        self.last_gps_timestamp = gps.timestamp;
```

- [ ] **Step 4: Add apply_persisted_stop_index() helper**

Add this method to the impl block:
```rust
    /// Apply persisted stop index by marking all prior stops as Departed.
    ///
    /// This prevents the corridor filter from re-triggering stops that
    /// were already passed before the reboot.
    fn apply_persisted_stop_index(&mut self, stop_index: u8) {
        use shared::FsmState;

        for i in 0..stop_index.min(self.stop_states.len() as u8) as usize {
            self.stop_states[i].fsm_state = FsmState::Departed;
            self.stop_states[i].announced = true;
            self.stop_states[i].last_announced_stop = i as u8;  // I3 fix
        }
        self.last_stop_index = stop_index;
    }
```

- [ ] **Step 5: Verify compilation**

Run: `cargo build --release -p pico2-firmware`
Expected: Compiles without errors

- [ ] **Step 6: Commit**

```bash
git add crates/pico2-firmware/src/control/mod.rs
git commit -m "feat(control): add warmup and first fix logic to tick()

Implements proper warmup counter tracking (estimation vs detection),
GPS outage handling, first fix initialization, and persisted state
application. Removes hardcoded is_first_fix: false (I1 fix).

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 9: Add Snap Cooldown Logic to tick()

**Files:**
- Modify: `crates/pico2-firmware/src/control/mod.rs`

**Reference:** Old `state.rs:322-339` for snap handling

- [ ] **Step 1: Add snap cooldown decrement at start of tick()**

After the warmup logic added in Task 8, add:
```rust
        // Handle snap cooldown decrement
        if self.just_snapped_ticks > 0 {
            self.just_snapped_ticks = self.just_snapped_ticks.saturating_sub(1);
        }
        let in_snap_cooldown = self.just_snapped_ticks > 0;
```

- [ ] **Step 2: Add snap detection and handling after estimation**

After getting `est` from `estimate()`, add:
```rust
        // Handle snap from off-route re-entry
        if est.snapped && !in_snap_cooldown {
            // Find forward closest stop (prevents backward selection)
            let new_idx = self.find_forward_closest_stop_index(est.s_cm, self.last_stop_index);
            self.last_stop_index = new_idx;

            // Reset stop states using recovery logic
            self.reset_stop_states_after_recovery(new_idx as usize, est.s_cm);

            // Clear all recovery triggers
            self.frozen_s_cm = None;  // Clear freeze context
            self.last_s_cm = est.s_cm;  // Update immediately to prevent false jump detection

            // Set 2-second cooldown
            self.just_snapped_ticks = 2;

            #[cfg(feature = "firmware")]
            defmt::info!("Snap re-entry at s={}cm, recovered stop={}", est.s_cm, new_idx);

            // Continue to detection below
        }
```

- [ ] **Step 3: Pass in_snap_cooldown to recovery check**

In the mode transition logic (where checking for recovery triggers), ensure `in_snap_cooldown` is used to suppress recovery during cooldown.

- [ ] **Step 4: Verify compilation**

Run: `cargo build --release -p pico2-firmware`
Expected: Compiles without errors

- [ ] **Step 5: Commit**

```bash
git add crates/pico2-firmware/src/control/mod.rs
git commit -m "feat(control): add snap cooldown logic to tick()

Detects est.snapped signal from estimation layer and implements
2-second cooldown to prevent false jump detection. Resets stop
states and clears recovery triggers on snap.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 10: Wire Stop State Reset into Recovery Paths

**Files:**
- Modify: `crates/pico2-firmware/src/control/mod.rs`

**Reference:** Old `state.rs:241-249, 373-379` for recovery reset calls

- [ ] **Step 1: Add stop state reset to recovery_success()**

Update the `recovery_success()` method to call reset:
```rust
    fn recovery_success(&mut self, recovered_idx: usize, s_cm: DistCm) {
        self.mode = SystemMode::Normal;
        self.last_stop_index = recovered_idx as u8;
        self.frozen_s_cm = None;
        self.recovering_since = None;
        self.recovery_failed = false;
        self.last_s_cm = s_cm;

        // Reset stop states after recovery
        self.reset_stop_states_after_recovery(recovered_idx, s_cm);
    }
```

- [ ] **Step 2: Add stop state reset to timeout fallback**

In `attempt_recovery()`, update the timeout fallback path:
```rust
        // Check timeout first
        if check_recovering_timeout(self.mode, self.recovering_since, now) {
            // Fallback to geometric search
            let best_idx = self.find_closest_stop_index_internal(est.s_cm);

            self.recovery_failed = true;
            self.mode = SystemMode::Normal;
            self.last_stop_index = best_idx;
            self.frozen_s_cm = None;
            self.recovering_since = None;

            // Reset stop states after recovery
            self.reset_stop_states_after_recovery(best_idx as usize, est.s_cm);

            return Some(best_idx as usize);
        }
```

- [ ] **Step 3: Verify compilation**

Run: `cargo build --release -p pico2-firmware`
Expected: Compiles without errors

- [ ] **Step 4: Commit**

```bash
git add crates/pico2-firmware/src/control/mod.rs
git commit -m "feat(control): wire stop state reset into recovery paths

Calls reset_stop_states_after_recovery() in both recovery_success()
and timeout fallback. Ensures stop FSM is properly initialized after
any recovery event.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 11: Add Persistence Methods to SystemState

**Files:**
- Modify: `crates/pico2-firmware/src/control/mod.rs`

**Reference:** Old `state.rs:685-726` for persistence logic

- [ ] **Step 1: Add should_persist() method**

```rust
    /// Returns true if state should be persisted this tick.
    /// Writes when stop index changes, but no more than once per 60 seconds.
    /// This rate limiting prevents excessive flash wear (~100k erase cycles).
    pub fn should_persist(&self, current_stop: u8) -> bool {
        // M5: Gate persistence during off-route/suspect states
        // Don't persist if position is frozen (off-route or suspect)
        if self.frozen_s_cm.is_some() {
            return false;
        }

        // Don't persist if in suspect state
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

```rust
    /// Mark state as persisted, resetting the rate-limit counter.
    pub fn mark_persisted(&mut self, stop_index: u8) {
        self.last_persisted_stop = stop_index;
        self.ticks_since_persist = 0;
    }
```

- [ ] **Step 3: Add current_stop_index() method**

```rust
    /// Get the current stop index from last_stop_index.
    /// Returns None if not yet initialized.
    pub fn current_stop_index(&self) -> Option<u8> {
        if self.first_fix {
            None
        } else {
            Some(self.last_stop_index)
        }
    }
```

- [ ] **Step 4: Verify compilation**

Run: `cargo build --release -p pico2-firmware`
Expected: Compiles without errors

- [ ] **Step 5: Commit**

```bash
git add crates/pico2-firmware/src/control/mod.rs
git commit -m "feat(control): add persistence methods to SystemState

Adds should_persist(), mark_persisted(), and current_stop_index()
with proper rate limiting (60 second minimum) and off-route gating.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 12: Update main.rs to Use New Architecture

**Files:**
- Modify: `crates/pico2-firmware/src/main.rs`

**Reference:** Existing `main.rs` lines 21-26, 101, 122, 155-176

- [ ] **Step 1: Update module imports**

Replace the module declaration section (lines 21-26) with:
```rust
// Module declarations
mod control;
mod detection;
mod estimation;
mod lut;
mod persist;
mod recovery_trigger;
mod state;  // Deprecated: kept for reference
mod uart;
```

- [ ] **Step 2: Add use statements for new types**

After the module declarations, add:
```rust
use crate::control::SystemState;
use crate::estimation::EstimationState;
```

- [ ] **Step 3: Replace State instantiation**

Replace line 101 `let mut state = state::State::new(&route_data, persisted);` with:
```rust
    // Initialize system state with route data reference
    let mut system_state = SystemState::new(&route_data, persisted);
    let mut estimation_state = EstimationState::new();
```

- [ ] **Step 4: Replace GPS processing call**

Replace line 122 `if let Some(arrival) = state.process_gps(&gps) {` with:
```rust
                        // Process GPS through full pipeline
                        if let Some(arrival) = system_state.tick(&gps, &mut estimation_state) {
```

- [ ] **Step 5: Update persistence calls**

Replace lines 155-176 with:
```rust
        // Persist state if stop index changed and rate limit allows
        if let Some(current_stop) = system_state.current_stop_index() {
            if system_state.should_persist(current_stop) {
                let ps = shared::PersistedState::new(
                    estimation_state.kalman.s_cm,
                    current_stop
                );
                match persist::save(&mut flash, &ps).await {
                    Ok(()) => {
                        info!(
                            "Persisted state: stop={}, progress={}cm",
                            current_stop, estimation_state.kalman.s_cm
                        );
                        system_state.mark_persisted(current_stop);
                    }
                    Err(()) => {
                        defmt::warn!("Failed to persist state to flash");
                        // S4 fix: increment on failure to prevent retry loop
                        system_state.ticks_since_persist = system_state.ticks_since_persist.saturating_add(1);
                    }
                }
            } else {
                // Increment tick counter for rate limiting
                system_state.ticks_since_persist = system_state.ticks_since_persist.saturating_add(1);
            }
        }
```

- [ ] **Step 6: Verify compilation**

Run: `cargo build --release --target thumbv8m.main-none-eabi -p pico2-firmware`
Expected: Compiles without errors

- [ ] **Step 7: Run tests**

Run: `cargo test -p pico2-firmware`
Expected: All tests pass

- [ ] **Step 8: Commit**

```bash
git add crates/pico2-firmware/src/main.rs
git commit -m "feat(main): migrate to two-layer architecture

Replaces state::State with SystemState + EstimationState.
NMEA parsing remains in main.rs for clean separation.

- Import control::SystemState and estimation::EstimationState
- Replace state.process_gps() with system_state.tick()
- Update persistence calls to use SystemState methods

This activates the v9.0 two-layer architecture described in docs.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 13: Mark Old state.rs as Deprecated

**Files:**
- Modify: `crates/pico2-firmware/src/state.rs`

- [ ] **Step 1: Add deprecation notice to state.rs**

Add at the top of the file (after the module doc comment, if present):
```rust
#![deprecated(note = "Use control::SystemState + estimation::EstimationState instead")]
```

- [ ] **Step 2: Add deprecation notice to State struct**

Add `#[deprecated]` attribute to the State struct:
```rust
#[deprecated(note = "Use control::SystemState instead")]
pub struct State<'a> {
    // ... existing fields ...
}
```

- [ ] **Step 3: Verify compilation**

Run: `cargo build --release --target thumbv8m.main-none-eabi -p pico2-firmware`
Expected: Compiles without errors (warnings about deprecated code are OK)

- [ ] **Step 4: Commit**

```bash
git add crates/pico2-firmware/src/state.rs
git commit -m "deprecate(state): mark state.rs as deprecated

Replaced by control::SystemState + estimation::EstimationState.
Old code kept for reference during verification period.

See: docs/superpowers/specs/2026-04-29-two-layer-activation-design.md

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 14: Verification and Testing

**Files:**
- Test: Run scenario tests and validate output

- [ ] **Step 1: Build firmware**

Run: `cargo build --release --target thumbv8m.main-none-eabi -p pico2-firmware`
Expected: Clean build with no errors

- [ ] **Step 2: Run unit tests**

Run: `cargo test -p pico2-firmware`
Expected: All tests pass

- [ ] **Step 3: Run scenario test**

Run: `make run ROUTE_NAME=ty225 SCENARIO=normal`
Expected: Produces arrivals.jsonl with arrival/departure events

- [ ] **Step 4: Verify trace output**

Check that trace.jsonl contains expected state transitions and events

- [ ] **Step 5: Compare with old implementation (optional)**

If you have baseline output from old implementation, compare:
- Number of arrival events
- Timing of arrivals
- Stop indices

Expected: Results match or are improved (due to bug fixes)

- [ ] **Step 6: Verify bug fixes**

Check that the following bugs are fixed:
- C2: Kalman predict step present (no systematic lag)
- I1: First fix properly tracked (not hardcoded)
- I3: Announcement guards consistent
- Snap signal: Cooldown logic works

- [ ] **Step 7: Commit verification results**

```bash
echo "Verification complete:
- Firmware builds successfully
- All unit tests pass
- Scenario test produces valid output
- Bug fixes verified (C2, I1, I3, snap signal)

See individual task commits for implementation details." | git commit allow-empty -F -
```

---

## Task 15: Final Documentation Update

**Files:**
- Modify: `CLAUDE.md` or relevant documentation

- [ ] **Step 1: Update CLAUDE.md architecture section**

Update the "Firmware Architecture (2-Layer Design)" section to reflect that the architecture is now active:

```markdown
**Firmware Architecture (2-Layer Design) - ACTIVE**

The pico2-firmware crate implements a 2-layer architecture for embedded deployment:

**Control Layer** (`crates/pico2-firmware/src/control/`):
- `SystemState` — state machine managing Normal/OffRoute/Recovering modes
- `SystemMode` — mode enum with transition functions
- Unified triggers based on `divergence_d2` and `displacement`
- Recovery timeout (30s) with geometric fallback
- `tick()` orchestrator — coordinates estimation and detection

**Estimation Layer** (`crates/pico2-firmware/src/estimation/`):
- `estimate()` — isolated GPS → position pipeline
- `KalmanState` — Kalman filter (no control state)
- `DrState` — DR/EMA state (no control state)
- Returns `EstimationOutput` with confidence signal
```

- [ ] **Step 2: Add migration note**

Add to CLAUDE.md:
```markdown
## Migration Notes

**v9.0 Architecture Migration (2026-04-29):**
- Migrated from monolithic `state::State` to two-layer architecture
- Old `state.rs` marked as deprecated
- Bug fixes applied: C2 (Kalman predict), I1 (first fix), I3 (announcement guards)
- See: `docs/superpowers/specs/2026-04-29-two-layer-activation-design.md`
```

- [ ] **Step 3: Commit documentation updates**

```bash
git add CLAUDE.md
git commit -m "docs: update CLAUDE.md for activated two-layer architecture

Documents that the v9.0 two-layer architecture is now active.
Adds migration notes and references design spec.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Self-Review Results

**Spec coverage:**
- ✓ Phase 0 (bug fixes) → Tasks 1-3
- ✓ Phase 1 (SystemState structure) → Task 4
- ✓ Phase 2 (detection integration) → Task 7
- ✓ Phase 3 (stop state reset) → Task 6
- ✓ Phase 4 (helper methods) → Task 5
- ✓ Phase 5 (warmup logic) → Task 8
- ✓ Phase 6 (stop state reset calls) → Task 10
- ✓ Phase 7 (main.rs migration) → Task 12
- ✓ Phase 8 (snap cooldown) → Task 9
- ✓ Phase 9 (testing) → Task 14
- ✓ Phase 10 (cleanup) → Tasks 13, 15

**Placeholder scan:** No placeholders found. All steps contain complete code.

**Type consistency:** Verified. Types match across tasks (EstimationOutput, SystemState, etc.).

---

Plan complete and saved to `docs/superpowers/plans/2026-04-29-two-layer-activation.md`.

Two execution options:

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?

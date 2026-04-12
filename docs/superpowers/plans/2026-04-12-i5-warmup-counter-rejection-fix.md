# I5 Warmup Counter Rejection Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix warmup counter to advance even when GPS is rejected, preventing permanent stuck state.

**Architecture:** Replace single `warmup_counter` with two-track system: `warmup_valid_ticks` (counts valid GPS with Kalman updates) and `warmup_total_ticks` (counts all ticks as timeout safety valve). Detection enables when `valid >= 3` OR `total >= 10`.

**Tech Stack:** Rust, no_std embedded firmware, existing state machine in `crates/pico2-firmware/src/state.rs`

---

## File Structure

**Single file modification:**
- `crates/pico2-firmware/src/state.rs` - Update State struct and warmup logic
- `crates/pico2-firmware/tests/test_warmup_counter.rs` - Add new tests

No new files. This is a focused bug fix to the existing warmup logic.

---

## Task 1: Add two-counter fields to State struct

**Files:**
- Modify: `crates/pico2-firmware/src/state.rs`

- [ ] **Step 1: Update State struct definition**

Find the State struct definition around line 27-74. Replace the warmup fields:

**Find this code:**
```rust
/// First fix flag - true until first GPS fix is received
pub first_fix: bool,
/// Warmup counter - increments after first fix until WARMUP_TICKS_REQUIRED is reached
pub warmup_counter: u8,
/// Flag indicating warmup was just reset (e.g., after GPS outage)
/// The next valid GPS tick will not increment the counter
warmup_just_reset: bool,
```

**Replace with:**
```rust
/// First fix flag - true until first GPS fix is received
pub first_fix: bool,
/// Number of valid GPS ticks with Kalman updates (convergence counter)
warmup_valid_ticks: u8,
/// Total ticks since first fix (timeout safety valve)
warmup_total_ticks: u8,
/// Flag indicating warmup was just reset (e.g., after GPS outage)
/// The next valid GPS tick will not increment the counter
warmup_just_reset: bool,
```

- [ ] **Step 2: Update State::new() initialization**

Find the `new()` method around line 77-104. Update the initialization:

**Find this code:**
```rust
Self {
    nmea: NmeaState::new(),
    kalman: KalmanState::new(),
    dr: DrState::new(),
    stop_states,
    route_data,
    first_fix: true,
    warmup_counter: 0,
    warmup_just_reset: false,
    last_known_stop_index: 0,
    last_valid_s_cm: 0,
    last_gps_timestamp: 0,
}
```

**Replace with:**
```rust
Self {
    nmea: NmeaState::new(),
    kalman: KalmanState::new(),
    dr: DrState::new(),
    stop_states,
    route_data,
    first_fix: true,
    warmup_valid_ticks: 0,
    warmup_total_ticks: 0,
    warmup_just_reset: false,
    last_known_stop_index: 0,
    last_valid_s_cm: 0,
    last_gps_timestamp: 0,
}
```

- [ ] **Step 3: Add constants at top of file**

After the WARMUP_TICKS_REQUIRED constant (around line 23), add:

```rust
/// Maximum warmup duration before timeout safety valve (10 seconds)
/// Matches DR outage tolerance - both counters reset on Outage
const WARMUP_TIMEOUT_TICKS: u8 = 10;
```

- [ ] **Step 4: Run build to verify no compilation errors**

Run: `cargo build --features firmware`

Expected: SUCCESS

- [ ] **Step 5: Commit the struct changes**

```bash
git add crates/pico2-firmware/src/state.rs
git commit -m "refactor(state): add two-counter warmup system (I5)

Replace single warmup_counter with:
- warmup_valid_ticks: counts valid GPS (Kalman convergence)
- warmup_total_ticks: counts all ticks (timeout safety valve)

This enables detection even when GPS is repeatedly rejected,
preventing permanent stuck state on noisy startup."
```

---

## Task 2: Update Valid branch warmup logic

**Files:**
- Modify: `crates/pico2-firmware/src/state.rs`

- [ ] **Step 1: Update ProcessResult::Valid branch warmup logic**

Find the `ProcessResult::Valid` branch in `process_gps()` method (around line 124-198).

**Find this code:**
```rust
ProcessResult::Valid { signals, v_cms, seg_idx: _ } => {
    let PositionSignals { z_gps_cm: _, s_cm } = signals;

    // Check for GPS jump requiring recovery (H1)
    // ... (recovery code) ...

    if self.first_fix {
        self.first_fix = false;
    } else if self.warmup_just_reset {
        // After warmup reset (e.g., GPS outage), first tick doesn't increment counter
        self.warmup_just_reset = false;
        return None;
    } else if self.warmup_counter < WARMUP_TICKS_REQUIRED {
        self.warmup_counter += 1;
        #[cfg(feature = "firmware")]
        defmt::debug!(
            "Warmup: {}/{}",
            self.warmup_counter,
            WARMUP_TICKS_REQUIRED
        );
        return None;
    }
    // ... rest of Valid branch ...
}
```

**Replace with:**
```rust
ProcessResult::Valid { signals, v_cms, seg_idx: _ } => {
    let PositionSignals { z_gps_cm: _, s_cm } = signals;

    // Check for GPS jump requiring recovery (H1)
    // ... (recovery code - unchanged) ...

    if self.first_fix {
        self.first_fix = false;
        // First fix initializes Kalman but doesn't run update_adaptive
        // Counts toward timeout but NOT convergence
        self.warmup_total_ticks = 1;
        return None;
    }

    if self.warmup_just_reset {
        // After warmup reset (e.g., GPS outage), first tick doesn't increment counter
        self.warmup_just_reset = false;
        return None;
    }

    // Increment total time counter
    self.warmup_total_ticks = self.warmup_total_ticks.saturating_add(1);

    // Check convergence requirement
    if self.warmup_valid_ticks < WARMUP_TICKS_REQUIRED {
        self.warmup_valid_ticks += 1;

        // Block detection unless timeout expired
        if self.warmup_total_ticks < WARMUP_TIMEOUT_TICKS {
            #[cfg(feature = "firmware")]
            defmt::debug!(
                "Warmup: {}/{} valid, {}/{} total",
                self.warmup_valid_ticks,
                WARMUP_TICKS_REQUIRED,
                self.warmup_total_ticks,
                WARMUP_TIMEOUT_TICKS
            );
            return None;
        }
    }

    // ... rest of Valid branch unchanged ...
}
```

- [ ] **Step 2: Run build to verify compilation**

Run: `cargo build --features firmware`

Expected: SUCCESS

- [ ] **Step 3: Commit the Valid branch changes**

```bash
git add crates/pico2-firmware/src/state.rs
git commit -m "feat(state): implement two-counter warmup logic in Valid branch

- First fix: increments warmup_total_ticks only (no Kalman update)
- Valid GPS: increments both counters
- Detection enables when valid>=3 OR total>=10 (timeout)
- Updated debug message to show both counters"
```

---

## Task 3: Update Rejected branch to increment total counter

**Files:**
- Modify: `crates/pico2-firmware/src/state.rs`

- [ ] **Step 1: Update ProcessResult::Rejected branch**

Find the `ProcessResult::Rejected` branch (around line 199-205).

**Find this code:**
```rust
ProcessResult::Rejected(reason) => {
    #[cfg(feature = "firmware")]
    defmt::warn!("GPS update rejected: {}", reason);
    #[cfg(not(feature = "firmware"))]
    let _ = reason; // Suppress unused warning when firmware feature is disabled
    return None;
}
```

**Replace with:**
```rust
ProcessResult::Rejected(reason) => {
    #[cfg(feature = "firmware")]
    defmt::warn!("GPS update rejected: {}", reason);
    #[cfg(not(feature = "firmware"))]
    let _ = reason; // Suppress unused warning when firmware feature is disabled

    // Increment timeout counter even on rejection (I5 fix)
    // This prevents permanent stuck state when GPS is repeatedly rejected
    if !self.first_fix {
        self.warmup_total_ticks = self.warmup_total_ticks.saturating_add(1);
    }

    return None;  // Still block detection
}
```

- [ ] **Step 2: Run build to verify compilation**

Run: `cargo build --features firmware`

Expected: SUCCESS

- [ ] **Step 3: Commit the Rejected branch changes**

```bash
git add crates/pico2-firmware/src/state.rs
git commit -m "fix(state): increment warmup_total_ticks on GPS rejection (I5)

Previously, repeated GPS rejections would leave warmup_counter at 0
permanently. Now warmup_total_ticks advances even on rejection,
enabling the timeout safety valve after 10 seconds.

Detection still blocked on rejection - only the counter advances."
```

---

## Task 4: Update Outage branch to reset both counters

**Files:**
- Modify: `crates/pico2-firmware/src/state.rs`

- [ ] **Step 1: Update ProcessResult::Outage branch**

Find the `ProcessResult::Outage` branch (around line 206-217).

**Find this code:**
```rust
ProcessResult::Outage => {
    #[cfg(feature = "firmware")]
    defmt::warn!("GPS outage exceeded 10 seconds");
    // Reset warmup on GPS loss (conservative - requires fresh warmup after outage)
    if !self.first_fix {
        self.warmup_counter = 0;
        self.warmup_just_reset = true;
        #[cfg(feature = "firmware")]
        defmt::debug!("GPS outage reset warmup counter");
    }
    return None;
}
```

**Replace with:**
```rust
ProcessResult::Outage => {
    #[cfg(feature = "firmware")]
    defmt::warn!("GPS outage exceeded 10 seconds");
    // Reset both warmup counters on GPS loss (conservative - requires fresh warmup after outage)
    if !self.first_fix {
        self.warmup_valid_ticks = 0;
        self.warmup_total_ticks = 0;
        self.warmup_just_reset = true;
        #[cfg(feature = "firmware")]
        defmt::debug!("GPS outage reset warmup counters");
    }
    return None;
}
```

- [ ] **Step 2: Run build to verify compilation**

Run: `cargo build --features firmware`

Expected: SUCCESS

- [ ] **Step 3: Commit the Outage branch changes**

```bash
git add crates/pico2-firmware/src/state.rs
git commit -m "fix(state): reset both warmup counters on GPS outage (I5)

Both warmup_valid_ticks and warmup_total_ticks are reset to 0
when true signal loss occurs. This ensures a fresh warmup period
after outage recovery."
```

---

## Task 5: Add test for normal warmup (3 valid GPS)

**Files:**
- Modify: `crates/pico2-firmware/tests/test_warmup_counter.rs`

- [ ] **Step 1: Add test for normal warmup path**

Add this test to the test file:

```rust
#[test]
fn test_warmup_normal_three_valid_gps() {
    // I5 fix: Normal warmup requires 3 valid GPS after first fix
    // First fix initializes Kalman but doesn't count toward valid_ticks
    let (route_data, mut state) = setup_test_state();
    let mut tick = 0;

    // First fix: initializes Kalman, total=1, valid=0
    let gps1 = make_gps(tick, 120.0, 25.0, 10000, 0, 100, true);
    let result = state.process_gps(&gps1);
    assert_eq!(result, None, "First fix should not trigger detection");
    assert_eq!(state.warmup_valid_ticks, 0, "First fix should not count as valid");
    assert_eq!(state.warmup_total_ticks, 1, "First fix should count toward total");

    // Valid GPS #1: total=2, valid=1
    tick += 1;
    let gps2 = make_gps(tick, 120.01, 25.01, 10100, 0, 100, true);
    let result = state.process_gps(&gps2);
    assert_eq!(result, None, "Should not trigger detection yet");
    assert_eq!(state.warmup_valid_ticks, 1, "Should have 1 valid tick");
    assert_eq!(state.warmup_total_ticks, 2, "Should have 2 total ticks");

    // Valid GPS #2: total=3, valid=2
    tick += 1;
    let gps3 = make_gps(tick, 120.02, 25.02, 10200, 0, 100, true);
    let result = state.process_gps(&gps3);
    assert_eq!(result, None, "Should not trigger detection yet");
    assert_eq!(state.warmup_valid_ticks, 2, "Should have 2 valid ticks");
    assert_eq!(state.warmup_total_ticks, 3, "Should have 3 total ticks");

    // Valid GPS #3: total=4, valid=3 -> DETECTION ENABLED
    tick += 1;
    let gps4 = make_gps(tick, 120.03, 25.03, 10300, 0, 100, true);
    let result = state.process_gps(&gps4);
    assert_eq!(result, None, "No arrival at this position");
    assert_eq!(state.warmup_valid_ticks, 3, "Should have 3 valid ticks");
    assert_eq!(state.warmup_total_ticks, 4, "Should have 4 total ticks");

    // Now detection should be enabled - try to trigger arrival
    tick += 1;
    let gps5 = make_gps(tick, 120.04, 25.04, 10000, 0, 0, true); // At stop
    let result = state.process_gps(&gps5);
    assert!(result.is_some(), "Detection should be enabled, arrival should trigger");
}
```

- [ ] **Step 2: Run test to verify it compiles and passes**

Run: `cargo test -p pico2-firmware test_warmup_normal_three_valid_gps`

Expected: PASS

- [ ] **Step 3: Commit the test**

```bash
git add crates/pico2-firmware/tests/test_warmup_counter.rs
git commit -m "test(state): add normal warmup test (3 valid GPS)

Verifies that:
- First fix counts toward total_ticks but not valid_ticks
- 3 valid GPS after first fix enables detection
- Detection actually triggers when enabled"
```

---

## Task 6: Add test for timeout path (repeated rejections)

**Files:**
- Modify: `crates/pico2-firmware/tests/test_warmup_counter.rs`

- [ ] **Step 1: Add test for timeout safety valve**

Add this test to the test file:

```rust
#[test]
fn test_warmup_timeout_after_repeated_rejections() {
    // I5 fix: After 10 total ticks, detection enables even if < 3 were valid
    // This prevents permanent stuck state when GPS is repeatedly rejected
    let (route_data, mut state) = setup_test_state();
    let mut tick = 0;

    // First fix: total=1, valid=0
    let gps1 = make_gps(tick, 120.0, 25.0, 10000, 0, 100, true);
    state.process_gps(&gps1);
    assert_eq!(state.warmup_valid_ticks, 0);
    assert_eq!(state.warmup_total_ticks, 1);

    // Simulate 8 consecutive rejections (GPS fails speed constraint)
    // This could happen if first fix was at a bad position
    for i in 2..=9 {
        tick += 1;
        // Create GPS that will be rejected (excessive speed change)
        let gps_bad = make_gps(tick, 120.0 + (i as f64) * 0.1, 25.0, 10000 + (i * 50000), 0, 100, true);
        state.process_gps(&gps_bad);
        assert_eq!(state.warmup_valid_ticks, 0, "Valid ticks should remain 0");
        assert_eq!(state.warmup_total_ticks, i as u8, "Total ticks should increment");
    }

    // Now: total=9, valid=0 - still blocked
    assert_eq!(state.warmup_total_ticks, 9);

    // One more rejection: total=10 -> TIMEOUT, detection enabled
    tick += 1;
    let gps_bad = make_gps(tick, 121.0, 25.0, 10000 + 450000, 0, 100, true);
    let result = state.process_gps(&gps_bad);
    assert_eq!(result, None, "Rejection still blocks detection");
    assert_eq!(state.warmup_total_ticks, 10, "Should reach timeout threshold");
    assert_eq!(state.warmup_valid_ticks, 0, "Still 0 valid ticks");

    // Detection should now be enabled via timeout
    // Next valid GPS should proceed to detection
    tick += 1;
    let gps_good = make_gps(tick, 120.1, 25.01, 10000, 0, 0, true); // At stop
    let result = state.process_gps(&gps_good);
    assert!(result.is_some(), "Detection should be enabled via timeout, arrival should trigger");
}
```

- [ ] **Step 2: Run test to verify it compiles and passes**

Run: `cargo test -p pico2-firmware test_warmup_timeout_after_repeated_rejections`

Expected: PASS

- [ ] **Step 3: Commit the test**

```bash
git add crates/pico2-firmware/tests/test_warmup_counter.rs
git commit -m "test(state): add timeout path test for repeated rejections

Verifies that:
- Rejected GPS increments warmup_total_ticks but not valid_ticks
- After 10 total ticks, detection enables even with 0 valid ticks
- This prevents permanent stuck state on noisy startup"
```

---

## Task 7: Add test for outage reset

**Files:**
- Modify: `crates/pico2-firmware/tests/test_warmup_counter.rs`

- [ ] **Step 1: Update existing outage test**

Find the existing `test_warmup_resets_on_gps_outage` test and update it to verify both counters are reset.

**Find this test and update the assertions:**

```rust
#[test]
fn test_warmup_resets_on_gps_outage() {
    // I5 fix: Both warmup_valid_ticks and warmup_total_ticks reset on outage
    let (route_data, mut state) = setup_test_state();
    let mut tick = 0;

    // First fix + 2 valid GPS: total=3, valid=2
    let gps1 = make_gps(tick, 120.0, 25.0, 10000, 0, 100, true);
    state.process_gps(&gps1);
    tick += 1;
    let gps2 = make_gps(tick, 120.01, 25.01, 10100, 0, 100, true);
    state.process_gps(&gps2);
    tick += 1;
    let gps3 = make_gps(tick, 120.02, 25.02, 10200, 0, 100, true);
    state.process_gps(&gps3);

    assert_eq!(state.warmup_valid_ticks, 2, "Should have 2 valid ticks");
    assert_eq!(state.warmup_total_ticks, 3, "Should have 3 total ticks");

    // Simulate GPS outage (> 10 seconds without fix)
    tick += 11;
    let gps_outage = make_gps(tick, 120.0, 25.0, 10000, 0, 100, false); // no fix
    state.process_gps(&gps_outage);

    // Both counters should be reset
    assert_eq!(state.warmup_valid_ticks, 0, "Valid ticks should reset to 0");
    assert_eq!(state.warmup_total_ticks, 0, "Total ticks should reset to 0");
    assert!(state.warmup_just_reset, "warmup_just_reset flag should be set");

    // Next tick should not increment counters (warmup_just_reset)
    tick += 1;
    let gps_after = make_gps(tick, 120.1, 25.01, 10300, 0, 100, true);
    state.process_gps(&gps_after);

    assert_eq!(state.warmup_valid_ticks, 0, "Valid ticks still 0 after reset");
    assert_eq!(state.warmup_total_ticks, 0, "Total ticks still 0 after reset");
}
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test -p pico2-firmware test_warmup_resets_on_gps_outage`

Expected: PASS

- [ ] **Step 3: Commit the updated test**

```bash
git add crates/pico2-firmware/tests/test_warmup_counter.rs
git commit -m "test(state): update outage reset test for both counters

Verifies that GPS outage resets both warmup_valid_ticks and
warmup_total_ticks to 0, and that warmup_just_reset prevents
increment on the next tick."
```

---

## Task 8: Run full verification

- [ ] **Step 1: Run all warmup tests**

Run: `cargo test -p pico2-firmware test_warmup`

Expected: All 6 tests pass

- [ ] **Step 2: Run all firmware tests**

Run: `cargo test -p pico2-firmware`

Expected: All tests pass

- [ ] **Step 3: Run workspace tests**

Run: `cargo test --workspace`

Expected: All tests pass

- [ ] **Step 4: Run clippy**

Run: `cargo clippy -p pico2-firmware`

Expected: No warnings related to our changes

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "test(i5): verify all tests pass after warmup counter fix

- All warmup tests passing (6 tests)
- All firmware tests passing
- All workspace tests passing
- No clippy warnings

I5 fix complete: Two-counter warmup system prevents permanent
stuck state when GPS is repeatedly rejected."
```

---

## Summary

**Total tasks:** 8
**Files modified:** 2 (`state.rs`, `test_warmup_counter.rs`)
**New constants:** 1 (`WARMUP_TIMEOUT_TICKS`)
**Tests:** 3 new tests, 1 updated test
**Lines changed:** ~50

The fix implements a two-counter warmup system:
- `warmup_valid_ticks`: Counts only valid GPS with Kalman updates (convergence)
- `warmup_total_ticks`: Counts all ticks (timeout safety valve)
- Detection enables when `valid >= 3` OR `total >= 10`
- Prevents permanent stuck state on noisy startup while maintaining Kalman convergence requirement

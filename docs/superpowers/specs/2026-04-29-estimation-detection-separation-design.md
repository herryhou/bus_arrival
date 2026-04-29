# Estimation Readiness and Detection Gating Separation

**Date:** 2026-04-29
**Status:** Design Pending Review
**Related:** Code review finding - temporal coupling between warmup logic and detection logic

---

## Overview

**Purpose:** Separate the conflated warmup logic into two distinct concerns:
- **Estimation readiness** - determines if GPS processing (heading filter, Kalman) should use relaxed thresholds
- **Detection gating** - determines if arrival detection is permitted

**Scope:**
- Refactor warmup state management in `state.rs`
- Add separate counters for estimation and detection readiness
- Estimation and detection operate independently (no hard dependency)
- Preserve all existing behavior (no functional changes)

**Problem Statement:**

The current system uses a single `warmup_valid_ticks` counter that affects both:
1. **Heading filter behavior** - disables heading constraint during warmup
2. **Detection blocking** - prevents arrival detection during warmup

This creates **temporal coupling** that is hard to debug:
- Warmup state is checked in 6+ locations
- Multiple interacting variables (`warmup_valid_ticks`, `warmup_total_ticks`, `warmup_just_reset`, `first_fix`)
- Confusing parameter naming (`disable_heading_filter` passed as `is_first_fix`)
- Hard to isolate which behavior (estimation vs. detection) is causing issues

**Key Design Decisions:**
- Separate counters for estimation and detection
- Same threshold values (3/10 ticks) but independently named constants
- Estimation readiness gates detection readiness (explicit dependency)
- State remains in `State` struct with better organization

---

## Architecture

### Current State (Before)

```rust
pub struct State<'a> {
    // ... existing fields ...

    /// Number of valid GPS ticks with Kalman updates (convergence counter)
    warmup_valid_ticks: u8,
    /// Total ticks since first fix (timeout safety valve)
    warmup_total_ticks: u8,
    /// Flag indicating warmup was just reset (e.g., after GPS outage)
    warmup_just_reset: bool,
    /// First fix flag - true until first GPS fix is received
    first_fix: bool,

    // ... rest of fields ...
}
```

**Current behavior:**
- Single `warmup_valid_ticks` counter affects both heading filter AND detection
- Detection enable condition: `warmup_valid_ticks >= 3 OR warmup_total_ticks >= 10`
- Heading filter disabled when: `first_fix OR warmup_valid_ticks < 3`

### Proposed State (After)

```rust
pub struct State<'a> {
    // ... existing fields ...

    // ===== Estimation Readiness (affects heading filter, Kalman) =====
    /// Valid GPS ticks where Kalman measurement update ran
    estimation_ready_ticks: u8,
    /// Total ticks since first fix (timeout safety valve)
    estimation_total_ticks: u8,

    // ===== Detection Gating (blocks arrival detection) =====
    /// Valid ticks since estimation became ready
    detection_enabled_ticks: u8,
    /// Total ticks since first fix (detection timeout)
    detection_total_ticks: u8,

    // ===== Shared flags =====
    /// First fix flag - true until first GPS fix is received
    first_fix: bool,
    /// Flag indicating state was just reset (e.g., after GPS outage)
    just_reset: bool,

    // ... rest of fields ...
}
```

### New Constants

```rust
// ===== Estimation Readiness =====
/// Valid GPS ticks required for estimation to be ready
const ESTIMATION_WARMUP_TICKS: u8 = 3;
/// Maximum ticks before estimation timeout safety valve
const ESTIMATION_TIMEOUT_TICKS: u8 = 10;

// ===== Detection Gating =====
/// Valid ticks required for detection to be enabled
const DETECTION_WARMUP_TICKS: u8 = 3;
/// Maximum ticks before detection timeout safety valve
const DETECTION_TIMEOUT_TICKS: u8 = 10;
```

**Note:** Values match current warmup thresholds. Constants are independently named for future flexibility.

---

## Component Changes

### Files Affected

1. **`crates/pico2-firmware/src/state.rs`**
   - Rename warmup fields to estimation/detection fields
   - Update initialization in `State::new()`
   - Update counter logic in `process_gps()`
   - Add helper methods for readiness checks

2. **`crates/pico2-firmware/src/control/`** (if exists)
   - Update any references to warmup state

3. **Tests** (all test files referencing warmup)
   - `test_warmup.rs`
   - `test_warmup_counter.rs`
   - `test_monotonic_invariant.rs`
   - Integration tests

### State Initialization

**Before:**
```rust
Self {
    // ...
    first_fix: true,
    warmup_valid_ticks: 0,
    warmup_total_ticks: 0,
    warmup_just_reset: false,
    // ...
}
```

**After:**
```rust
Self {
    // ...
    first_fix: true,
    estimation_ready_ticks: 0,
    estimation_total_ticks: 0,
    detection_enabled_ticks: 0,
    detection_total_ticks: 0,
    just_reset: false,
    // ...
}
```

---

## Data Flow and Behavior

### Readiness Calculation

**Helper methods to be added:**

```rust
impl<'a> State<'a> {
    /// Check if estimation is ready (affects heading filter, Kalman)
    fn estimation_ready(&self) -> bool {
        self.estimation_ready_ticks >= ESTIMATION_WARMUP_TICKS
            || self.estimation_total_ticks >= ESTIMATION_TIMEOUT_TICKS
    }

    /// Check if detection is enabled (independent of estimation)
    fn detection_ready(&self) -> bool {
        self.detection_enabled_ticks >= DETECTION_WARMUP_TICKS
            || self.detection_total_ticks >= DETECTION_TIMEOUT_TICKS
    }

    /// Check if heading filter should be disabled
    fn disable_heading_filter(&self) -> bool {
        self.first_fix || !self.estimation_ready()
    }
}
```

### Counter Update Logic

**ProcessResult::Valid:**
```rust
ProcessResult::Valid { .. } => {
    // ... existing first_fix handling ...

    if self.first_fix {
        self.first_fix = false;
        self.estimation_total_ticks = 1;
        self.detection_total_ticks = 1;
        return None;
    }

    if self.just_reset {
        self.just_reset = false;
        self.estimation_total_ticks = 1;
        self.detection_total_ticks = 1;
        return None;
    }

    // Increment total counters
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
        return None;
    }

    // ... proceed with detection ...
}
```

**ProcessResult::Rejected:**
```rust
ProcessResult::Rejected(reason) => {
    // Increment total counters only
    if !self.first_fix {
        self.estimation_total_ticks = self.estimation_total_ticks.saturating_add(1);
        self.detection_total_ticks = self.detection_total_ticks.saturating_add(1);
    }
    return None;
}
```

**ProcessResult::Outage:**
```rust
ProcessResult::Outage => {
    // Reset all counters on true signal loss
    if !self.first_fix {
        self.estimation_ready_ticks = 0;
        self.estimation_total_ticks = 0;
        self.detection_enabled_ticks = 0;
        self.detection_total_ticks = 0;
        self.just_reset = true;
    }
    return None;
}
```

**ProcessResult::DrOutage:**
```rust
ProcessResult::DrOutage { s_cm, v_cms } => {
    if self.just_reset {
        self.just_reset = false;
        self.estimation_total_ticks = 1;
        self.detection_total_ticks = 1;
        return None;
    }

    // Increment total counters but NOT valid counters
    if !self.first_fix {
        self.estimation_total_ticks = self.estimation_total_ticks.saturating_add(1);
        self.detection_total_ticks = self.detection_total_ticks.saturating_add(1);
    }

    // Block detection unless ready
    if !self.detection_ready() {
        return None;
    }

    // ... proceed with detection ...
}
```

### State Transition Diagram

```
[first_fix]
    ↓
estimation_total=1, detection_total=1
    ↓
[Valid GPS]
    ↓
estimation_total++, detection_total++
    ↓
    ├─ estimation_ready? (estimation_ready >= 3 OR estimation_total >= 10)
    │   ├─ NO → estimation_ready_ticks++
    │   └─ YES → estimation ready (heading filter enabled)
    │
    └─ detection_ready? (detection_enabled >= 3 OR detection_total >= 10)
        ├─ NO → detection_enabled_ticks++ → block detection
        └─ YES → ENABLE DETECTION
```

**Note:** Estimation and detection operate independently. Detection can enable via timeout even if estimation is not fully ready, preserving existing behavior.

---

## Error Handling & Edge Cases

### Edge Cases

1. **First fix behavior**
   - First fix initializes total counters to 1
   - Valid counters remain 0 (no Kalman update yet)
   - Detection blocked (estimation not ready)

2. **GPS outage during warmup**
   - All counters reset to 0
   - `just_reset` flag set
   - Next tick after outage starts fresh

3. **Repeated GPS rejections**
   - Total counters increment, valid counters unchanged
   - Timeout path (10 ticks) eventually enables detection
   - Estimation and detection timeout independently

4. **DrOutage during warmup**
   - Total counters increment, valid counters unchanged
   - Detection blocked until ready
   - Proceeds with DR estimates for detection when ready

5. **Estimation ready, detection not ready**
   - Heading filter enabled (strict mode)
   - Detection still blocked
   - `detection_enabled_ticks` increments until threshold

---

## Testing Strategy

### Unit Tests

1. **Helper method tests**
   - `test_estimation_ready_after_3_valid()`: Estimation ready at 3 ticks
   - `test_estimation_timeout_after_10_ticks()`: Timeout path works
   - `test_detection_ready_independent()`: Detection becomes ready independently of estimation
   - `test_detection_timeout_independent()`: Detection timeout works even when estimation not ready
   - `test_heading_filter_disabled_during_warmup()`: Filter disabled until estimation ready

2. **Counter update tests**
   - `test_first_fix_initializes_totals_only()`: Valid counters at 0
   - `test_valid_gps_increments_both()`: Both counters advance
   - `test_rejected_increments_totals_only()`: Valid counters unchanged
   - `test_outage_resets_all_counters()`: All counters reset to 0
   - `test_dr_outage_increments_totals_only()`: Valid counters unchanged

3. **State transition tests**
   - `test_normal_warmup_sequence()`: 3 valid → both estimation and detection ready
   - `test_timeout_path()`: 10 rejected → detection enabled via timeout
   - `test_estimation_detection_independent()`: Detection can timeout while estimation not ready
   - `test_mixed_valid_and_rejected()`: Handles both correctly

4. **Edge case tests**
   - `test_outage_during_warmup_resets()`: Outage causes reset
   - `test_dr_outage_during_warmup()`: DrOutage handled correctly
   - `test_just_reset_behavior()`: First tick after reset handled

### Integration Tests

1. **Full pipeline warmup**
   - Simulate NMEA sequence from cold start
   - Verify heading filter behavior changes at estimation ready
   - Verify detection enables at detection ready

2. **Outage recovery**
   - Normal → outage → recovery sequence
   - Verify counters reset and warmup restarts

3. **Noisy startup**
   - Repeated rejections followed by valid GPS
   - Verify timeout path enables detection

### Validation Criteria

- [ ] All existing tests pass (no functional regression)
- [ ] New helper methods work correctly
- [ ] Counter updates match specification
- [ ] State transitions verified
- [ ] Edge cases handled
- [ ] Integration tests pass

---

## Implementation Tasks

- [ ] Add new constants: `ESTIMATION_WARMUP_TICKS`, `ESTIMATION_TIMEOUT_TICKS`, `DETECTION_WARMUP_TICKS`, `DETECTION_TIMEOUT_TICKS`
- [ ] Rename fields in `State` struct: `warmup_valid_ticks` → `estimation_ready_ticks`, etc.
- [ ] Update `State::new()` initialization
- [ ] Add helper methods: `estimation_ready()`, `detection_ready()`, `disable_heading_filter()`
- [ ] Update `ProcessResult::Valid` branch counter logic
- [ ] Update `ProcessResult::Rejected` branch counter logic
- [ ] Update `ProcessResult::Outage` branch counter logic
- [ ] Update `ProcessResult::DrOutage` branch counter logic
- [ ] Update `process_gps_update()` call to use `disable_heading_filter()` helper
- [ ] Add unit tests for helper methods
- [ ] Add unit tests for counter updates
- [ ] Add unit tests for state transitions
- [ ] Add edge case tests
- [ ] Update existing warmup tests to use new field names
- [ ] Run full test suite and verify no regressions

---

## References

- Code Review: Warmup logic intertwined with detection logic
- Related Spec: `2026-04-12-i5-warmup-counter-rejection-fix-design.md`
- Current Implementation: `crates/pico2-firmware/src/state.rs`
- Off-Route Detection Spec: `2026-04-14-off-route-detection-design.md`

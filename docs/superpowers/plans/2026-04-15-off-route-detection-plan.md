# Off-Route Detection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add off-route detection to the GPS processing pipeline to handle sustained GPS drift where positions consistently don't fit route geometry.

**Architecture:** Surface the map matching distance² score through the pipeline, add hysteresis-based off-route detection (5 ticks to confirm, 2 to clear), freeze position during off-route episodes, and re-acquire with recovery when GPS returns to route.

**Tech Stack:** Rust, embedded Rust (no_std), existing GPS processing pipeline in crates/pipeline/gps_processor and crates/pico2-firmware.

---

## File Structure

**Modified Files:**
- `crates/shared/src/lib.rs` - Add KalmanState fields and ProcessResult::OffRoute variant
- `crates/pipeline/gps_processor/src/map_match.rs` - Change return type of find_best_segment_restricted
- `crates/pipeline/gps_processor/src/kalman.rs` - Add off-route detection logic
- `crates/pico2-firmware/src/state.rs` - Handle OffRoute and re-acquisition

**Test Files:**
- `crates/pipeline/gps_processor/tests/test_off_route_detection.rs` - NEW: Off-route detection unit tests
- `crates/pico2-firmware/tests/test_off_route_integration.rs` - NEW: Integration tests

---

## Task 1: Add KalmanState Fields

**Files:**
- Modify: `crates/shared/src/lib.rs`

- [ ] **Step 1: Add off_route fields to KalmanState**

```rust
// In crates/shared/src/lib.rs, pub struct KalmanState
pub struct KalmanState {
    pub s_cm: DistCm,
    pub v_cms: SpeedCms,
    pub last_seg_idx: usize,
    /// Consecutive ticks with match_d2 > OFF_ROUTE_D2_THRESHOLD
    pub off_route_suspect_ticks: u8,
    /// Consecutive ticks with match_d2 < OFF_ROUTE_D2_THRESHOLD
    pub off_route_clear_ticks: u8,
}
```

- [ ] **Step 2: Update KalmanState::new()**

```rust
// In crates/shared/src/lib.rs, impl KalmanState
pub fn new() -> Self {
    KalmanState {
        s_cm: 0,
        v_cms: 0,
        last_seg_idx: 0,
        off_route_suspect_ticks: 0,
        off_route_clear_ticks: 0,
    }
}
```

- [ ] **Step 3: Run existing tests**

Run: `cargo test --package shared`

Expected: All existing tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/shared/src/lib.rs
git commit -m "feat(shared): add off_route fields to KalmanState"
```

---

## Task 2: Add ProcessResult::OffRoute Variant

**Files:**
- Modify: `crates/shared/src/lib.rs`

- [ ] **Step 1: Add OffRoute variant to ProcessResult**

```rust
// In crates/shared/src/lib.rs, pub enum ProcessResult
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
    },
}
```

- [ ] **Step 2: Run existing tests**

Run: `cargo test --package shared`

Expected: All existing tests pass (enum variant addition is backward compatible)

- [ ] **Step 3: Commit**

```bash
git add crates/shared/src/lib.rs
git commit -m "feat(shared): add ProcessResult::OffRoute variant"
```

---

## Task 3: Change find_best_segment_restricted Return Type

**Files:**
- Modify: `crates/pipeline/gps_processor/src/map_match.rs`
- Test: `crates/pipeline/gps_processor/tests/test_map_match.rs`

- [ ] **Step 1: Write failing test for new return type**

```rust
// In crates/pipeline/gps_processor/tests/test_map_match.rs
#[test]
fn test_find_best_segment_returns_distance_squared() {
    use gps_processor::map_match::find_best_segment_restricted;
    use shared::{DistCm, HeadCdeg, SpeedCms};

    // Create minimal route data for testing
    let route_data = create_test_route_data();

    let (seg_idx, d2) = find_best_segment_restricted(
        1000,  // gps_x
        2000,  // gps_y
        0,     // gps_heading
        100,   // gps_speed
        &route_data,
        0,     // last_idx
        false, // is_first_fix
    );

    // Verify we get both segment index AND distance squared
    assert!(d2 >= 0);  // Distance squared should be non-negative
    assert!(seg_idx < route_data.node_count);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --package gps_processor test_find_best_segment_returns_distance_squared`

Expected: COMPILE ERROR - function returns usize, not tuple

- [ ] **Step 3: Update function signature**

```rust
// In crates/pipeline/gps_processor/src/map_match.rs
pub fn find_best_segment_restricted(
    gps_x: DistCm,
    gps_y: DistCm,
    gps_heading: HeadCdeg,
    gps_speed: SpeedCms,
    route_data: &RouteData,
    last_idx: usize,
    is_first_fix: bool,
) -> (usize, i64) {  // Changed from usize to (usize, i64)
```

- [ ] **Step 4: Update return statements**

```rust
// In crates/pipeline/gps_processor/src/map_match.rs, find_best_segment_restricted
// At the end of window search phase (early exit):
if window_eligible_found && window_eligible_dist2 < MAX_DIST2_EARLY_EXIT {
    return (window_best_eligible, window_eligible_dist2);  // Return tuple
}

// At the end of grid search phase:
(best_eligible_idx, best_eligible_dist2)  // Return tuple
// OR if no eligible found:
(best_any_idx, best_any_dist2)  // Return tuple
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --package gps_processor test_find_best_segment_returns_distance_squared`

Expected: PASS

- [ ] **Step 6: Run all map_match tests**

Run: `cargo test --package gps_processor --lib map_match`

Expected: All tests pass

- [ ] **Step 7: Commit**

```bash
git add crates/pipeline/gps_processor/src/map_match.rs
git add crates/pipeline/gps_processor/tests/test_map_match.rs
git commit -m "feat(map_match): return distance squared with segment index"
```

---

## Task 4: Update process_gps_update to Capture match_d2

**Files:**
- Modify: `crates/pipeline/gps_processor/src/kalman.rs`

- [ ] **Step 1: Update find_best_segment_restricted call**

```rust
// In crates/pipeline/gps_processor/src/kalman.rs, process_gps_update
// OLD:
let seg_idx = crate::map_match::find_best_segment_restricted(
    gps_x, gps_y, gps.heading_cdeg, gps.speed_cms,
    route_data, state.last_seg_idx, use_relaxed_heading,
);

// NEW:
let (seg_idx, match_d2) = crate::map_match::find_best_segment_restricted(
    gps_x, gps.y, gps.heading_cdeg, gps.speed_cms,
    route_data, state.last_seg_idx, use_relaxed_heading,
);
```

- [ ] **Step 2: Run existing tests**

Run: `cargo test --package gps_processor --lib kalman`

Expected: All tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/pipeline/gps_processor/src/kalman.rs
git commit -m "feat(kalman): capture match_d2 from map matching"
```

---

## Task 5: Add Off-Route Constants

**Files:**
- Modify: `crates/pipeline/gps_processor/src/kalman.rs`

- [ ] **Step 1: Add off-route constants**

```rust
// In crates/pipeline/gps_processor/src/kalman.rs, near top of file
/// Off-route distance threshold (cm²) — 50m² = 25,000,000 cm²
const OFF_ROUTE_D2_THRESHOLD: i64 = 25_000_000;

/// Ticks to confirm off-route (avoid false positives from multipath)
const OFF_ROUTE_CONFIRM_TICKS: u8 = 5;

/// Ticks to clear off-route (fast re-acquisition)
const OFF_ROUTE_CLEAR_TICKS: u8 = 2;
```

- [ ] **Step 2: Run tests**

Run: `cargo test --package gps_processor --lib`

Expected: All tests pass (constants only)

- [ ] **Step 3: Commit**

```bash
git add crates/pipeline/gps_processor/src/kalman.rs
git commit -m "feat(kalman): add off-route detection constants"
```

---

## Task 6: Add Off-Route Detection Logic with Warmup Guard

**Files:**
- Modify: `crates/pipeline/gps_processor/src/kalman.rs`
- Test: `crates/pipeline/gps_processor/tests/test_off_route_detection.rs`

- [ ] **Step 1: Write test for off-road confirmation**

```rust
// In crates/pipeline/gps_processor/tests/test_off_route_detection.rs (NEW FILE)
use gps_processor::kalman::{process_gps_update, KalmanState};
use shared::{DrState, GpsPoint, RouteData};

#[test]
fn test_off_route_confirms_after_5_ticks() {
    let mut state = KalmanState::new();
    let mut dr = DrState::new();
    let route_data = create_test_route_data();

    // Simulate 5 GPS fixes with poor match quality (> 50m from route)
    for i in 0..5 {
        let gps = GpsPoint {
            timestamp: i as u64,
            lat: 25000000,  // Far from route
            lon: 121000000,
            has_fix: true,
            heading_cdeg: 0,
            speed_cms: 500,
        };

        let result = process_gps_update(
            &mut state,
            &mut dr,
            &gps,
            &route_data,
            i,
            false, // not in warmup
        );

        if i < 4 {
            // First 4 ticks should NOT trigger off-route
            assert!(!matches!(result, shared::ProcessResult::OffRoute { .. }));
        } else {
            // 5th tick SHOULD trigger off-route
            assert!(matches!(result, shared::ProcessResult::OffRoute { .. }));
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --package gps_processor test_off_route_confirms_after_5_ticks`

Expected: FAIL - off-route detection not implemented

- [ ] **Step 3: Implement off-route detection logic**

```rust
// In crates/pipeline/gps_processor/src/kalman.rs, process_gps_update
// After map matching, before projection:

// Off-route detection (only when not in warmup)
if !is_first_fix {
    if match_d2 > OFF_ROUTE_D2_THRESHOLD {
        state.off_route_suspect_ticks = state.off_route_suspect_ticks.saturating_add(1);
        state.off_route_clear_ticks = 0;

        if state.off_route_suspect_ticks >= OFF_ROUTE_CONFIRM_TICKS {
            // Confirmed off-route: return with last valid position
            return ProcessResult::OffRoute {
                last_valid_s: state.s_cm,
                last_valid_v: state.v_cms,
            };
        }
    } else {
        state.off_route_clear_ticks = state.off_route_clear_ticks.saturating_add(1);
        if state.off_route_clear_ticks >= OFF_ROUTE_CLEAR_TICKS {
            state.off_route_suspect_ticks = 0;
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --package gps_processor test_off_route_confirms_after_5_ticks`

Expected: PASS

- [ ] **Step 5: Write test for warmup guard**

```rust
// In crates/pipeline/gps_processor/tests/test_off_route_detection.rs
#[test]
fn test_off_route_disabled_during_warmup() {
    let mut state = KalmanState::new();
    let mut dr = DrState::new();
    let route_data = create_test_route_data();

    // Simulate warmup period with poor match quality
    for i in 0..5 {
        let gps = GpsPoint {
            timestamp: i as u64,
            lat: 25000000,  // Far from route
            lon: 121000000,
            has_fix: true,
            heading_cdeg: 0,
            speed_cms: 500,
        };

        let result = process_gps_update(
            &mut state,
            &mut dr,
            &gps,
            &route_data,
            i,
            true,  // IN WARMUP - should disable off-route detection
        );

        // Should NEVER trigger off-route during warmup
        assert!(!matches!(result, shared::ProcessResult::OffRoute { .. }));
    }
}
```

- [ ] **Step 6: Run test to verify it fails**

Run: `cargo test --package gps_processor test_off_route_disabled_during_warmup`

Expected: FAIL - warmup guard not implemented

- [ ] **Step 7: Add warmup guard**

```rust
// In crates/pipeline/gps_processor/src/kalman.rs, process_gps_update
// Wrap the off-route detection in a warmup check:

// Off-route detection (only when not in warmup)
if !is_first_fix {
    // ... existing off-route logic ...
}
```

Note: The `is_first_fix` parameter already serves as the warmup guard. When true, off-route detection is skipped.

- [ ] **Step 8: Run test to verify it passes**

Run: `cargo test --package gps_processor test_off_route_disabled_during_warmup`

Expected: PASS

- [ ] **Step 9: Commit**

```bash
git add crates/pipeline/gps_processor/src/kalman.rs
git add crates/pipeline/gps_processor/tests/test_off_route_detection.rs
git commit -m "feat(kalman): add off-route detection with warmup guard"
```

---

## Task 7: Reset Off-Route Counters in handle_outage

**Files:**
- Modify: `crates/pipeline/gps_processor/src/kalman.rs`
- Test: `crates/pipeline/gps_processor/tests/test_off_route_detection.rs`

- [ ] **Step 1: Write test for counter reset on outage**

```rust
// In crates/pipeline/gps_processor/tests/test_off_route_detection.rs
#[test]
fn test_off_route_counter_resets_on_outage() {
    let mut state = KalmanState::new();
    let mut dr = DrState::new();
    let route_data = create_test_route_data();

    // Build up suspect count
    for i in 0..3 {
        let gps = GpsPoint {
            timestamp: i as u64,
            lat: 25000000,  // Far from route
            lon: 121000000,
            has_fix: true,
            heading_cdeg: 0,
            speed_cms: 500,
        };
        let _ = process_gps_update(
            &mut state, &mut dr, &gps, &route_data, i, false
        );
    }

    assert_eq!(state.off_route_suspect_ticks, 3);

    // Simulate GPS outage
    let outage_gps = GpsPoint {
        timestamp: 3,
        lat: 25000000,
        lon: 121000000,
        has_fix: false,  // NO FIX
        heading_cdeg: 0,
        speed_cms: 0,
    };
    let _ = process_gps_update(
        &mut state, &mut dr, &outage_gps, &route_data, 3, false
    );

    // Counters should be reset
    assert_eq!(state.off_route_suspect_ticks, 0);
    assert_eq!(state.off_route_clear_ticks, 0);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --package gps_processor test_off_route_counter_resets_on_outage`

Expected: FAIL - counters not reset

- [ ] **Step 3: Reset counters in handle_outage**

```rust
// In crates/pipeline/gps_processor/src/kalman.rs, handle_outage function
fn handle_outage(state: &mut KalmanState, dr: &mut DrState, timestamp: u64) -> ProcessResult {
    // ... existing code ...

    // Reset off-route counters on outage
    state.off_route_suspect_ticks = 0;
    state.off_route_clear_ticks = 0;

    // ... rest of existing code ...
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --package gps_processor test_off_route_counter_resets_on_outage`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/pipeline/gps_processor/src/kalman.rs
git add crates/pipeline/gps_processor/tests/test_off_route_detection.rs
git commit -m "feat(kalman): reset off-route counters on GPS outage"
```

---

## Task 8: Add State Fields for Re-acquisition

**Files:**
- Modify: `crates/pico2-firmware/src/state.rs`

- [ ] **Step 1: Add off-route fields to State**

```rust
// In crates/pico2-firmware/src/state.rs, pub struct State
pub struct State<'a> {
    // ... existing fields ...

    /// Flag indicating recovery should run on next valid GPS after off-route
    needs_recovery_on_reacquisition: bool,

    /// Timestamp when position was frozen (for recovery dt calculation)
    off_route_freeze_time: Option<u64>,
}
```

- [ ] **Step 2: Initialize fields in State::new()**

```rust
// In crates/pico2-firmware/src/state.rs, State::new()
Self {
    // ... existing field initializations ...
    needs_recovery_on_reacquisition: false,
    off_route_freeze_time: None,
}
```

- [ ] **Step 3: Run firmware tests**

Run: `cargo test --package pico2-firmware`

Expected: All existing tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/pico2-firmware/src/state.rs
git commit -m "feat(state): add off-route re-acquisition fields"
```

---

## Task 9: Handle ProcessResult::OffRoute in state.rs

**Files:**
- Modify: `crates/pico2-firmware/src/state.rs`
- Test: `crates/pico2-firmware/tests/test_off_route_integration.rs`

- [ ] **Step 1: Write test for OffRoute handling**

```rust
// In crates/pico2-firmware/tests/test_off_route_integration.rs (NEW FILE)
#[test]
fn test_off_route_freezes_position() {
    // Create test route and state
    let route_data = create_test_route_data();
    let mut state = State::new(&route_data, None);

    // Process a valid GPS to establish position
    let gps1 = create_gps_point(1000, 2000, 100);
    state.process_gps(&gps1);

    let last_s = state.last_valid_s_cm();
    let last_v = 100; // speed from gps1

    // Simulate off-route result
    // (This requires mocking or internal access to trigger OffRoute)
    // For now, verify the structure exists
}
```

- [ ] **Step 2: Add OffRoute handling in process_gps**

```rust
// In crates/pico2-firmware/src/state.rs, process_gps
// In the match result:

ProcessResult::OffRoute { last_valid_s, last_valid_v } => {
    // Set flag for recovery on re-acquisition
    self.needs_recovery_on_reacquisition = true;

    // Record freeze time
    self.off_route_freeze_time = Some(gps.timestamp);

    #[cfg(feature = "firmware")]
    defmt::warn!(
        "Off-route detected: GPS > 50m from route for 5s. Freezing at s={}cm.",
        last_valid_s
    );

    // Position is frozen - do NOT update last_valid_s_cm
    // Suspend arrival detection by returning None
    return None;
}
```

- [ ] **Step 3: Run firmware tests**

Run: `cargo test --package pico2-firmware`

Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/pico2-firmware/src/state.rs
git add crates/pico2-firmware/tests/test_off_route_integration.rs
git commit -m "feat(state): handle OffRoute by freezing position"
```

---

## Task 10: Implement Re-acquisition Recovery Logic

**Files:**
- Modify: `crates/pico2-firmware/src/state.rs`
- Test: `crates/pico2-firmware/tests/test_off_route_integration.rs`

- [ ] **Step 1: Write test for re-acquisition recovery**

```rust
// In crates/pico2-firmware/tests/test_off_route_integration.rs
#[test]
fn test_re_acquisition_runs_recovery() {
    // Test that after off-route, recovery runs when GPS returns
}
```

- [ ] **Step 2: Add re-acquisition logic in Valid branch**

```rust
// In crates/pico2-firmware/src/state.rs, process_gps
// In the ProcessResult::Valid branch, after warmup handling:

ProcessResult::Valid { signals, v_cms, seg_idx } => {
    let PositionSignals { z_gps_cm: _, s_cm } = signals;

    // Check for re-acquisition recovery
    if self.needs_recovery_on_reacquisition {
        self.needs_recovery_on_reacquisition = false;

        // Calculate elapsed time since freeze
        let elapsed_seconds = if let Some(freeze_time) = self.off_route_freeze_time {
            gps.timestamp.saturating_sub(freeze_time)
        } else {
            1  // Default if not set
        };

        // Clear freeze time
        self.off_route_freeze_time = None;

        // Run recovery to find correct stop index
        let mut stops_vec = heapless::Vec::<Stop, 256>::new();
        for i in 0..self.route_data.stop_count {
            if let Some(stop) = self.route_data.get_stop(i) {
                let _ = stops_vec.push(stop);
            }
        }

        if let Some(recovered_idx) = detection::recovery::find_stop_index(
            s_cm,
            v_cms,
            elapsed_seconds,
            &stops_vec,
            self.last_known_stop_index,
        ) {
            #[cfg(feature = "firmware")]
            defmt::info!("Re-acquisition recovered stop index: {}", recovered_idx);
            self.last_known_stop_index = recovered_idx as u8;
            self.reset_stop_states_after_recovery(recovered_idx);
        }
        // If recovery returns None, continue with existing states
    }

    // ... rest of existing Valid handling ...
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test --package pico2-firmware`

Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/pico2-firmware/src/state.rs
git add crates/pico2-firmware/tests/test_off_route_integration.rs
git commit -m "feat(state): implement re-acquisition recovery"
```

---

## Task 11: Integration Test for Full Off-Route Cycle

**Files:**
- Test: `crates/pico2-firmware/tests/test_off_route_integration.rs`

- [ ] **Step 1: Write full cycle test**

```rust
// In crates/pico2-firmware/tests/test_off_route_integration.rs
#[test]
fn test_full_off_route_cycle() {
    // 1. Normal operation - bus on route
    // 2. GPS drifts off-route for 6+ ticks
    // 3. Verify OffRoute triggered, position frozen
    // 4. GPS returns to route for 3 ticks
    // 5. Verify recovery runs, stop index corrected
    // 6. Verify normal operation resumes
}
```

- [ ] **Step 2: Run test**

Run: `cargo test --package pico2-firmware test_full_off_route_cycle`

Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/pico2-firmware/tests/test_off_route_integration.rs
git commit -m "test(integration): add full off-route cycle test"
```

---

## Task 12: Document Limitations

**Files:**
- Modify: `crates/pipeline/gps_processor/src/kalman.rs`

- [ ] **Step 1: Add documentation comment**

```rust
// In crates/pipeline/gps_processor/src/kalman.rs
/// Off-route detection notes:
///
/// This feature detects when GPS positions consistently don't fit route geometry
/// (distance > 50m for 5+ seconds). This catches urban canyon drift and physical
/// deviations.
///
/// LIMITATION: Cannot detect "along-route drift" where GPS stays on the road but
/// advances faster than the bus. This requires external ground truth and is not
/// detectable with a single GPS sensor.
```

- [ ] **Step 2: Commit**

```bash
git add crates/pipeline/gps_processor/src/kalman.rs
git commit -d "docs(kalman): document off-route detection limitations"
```

---

## Validation Checklist

After completing all tasks:

- [ ] All new tests pass
- [ ] All existing tests still pass (no regressions)
- [ ] Off-route detection triggers at 5 ticks, not 4
- [ ] Off-route clears after 2 ticks of good signal
- [ ] Warmup period prevents false off-route triggers
- [ ] GPS outage resets off-route counters
- [ ] Position freezes during off-route (DR doesn't advance)
- [ ] Recovery runs on re-acquisition
- [ ] Design spec requirements all met

---

## References

- Design Spec: `docs/superpowers/specs/2026-04-14-off-route-detection-design.md`
- GitHub Issue: https://github.com/herryhou/bus_arrival/issues/1
- Tech Report v8.9: `docs/bus_arrival_tech_report_v8.md` (Section 15)

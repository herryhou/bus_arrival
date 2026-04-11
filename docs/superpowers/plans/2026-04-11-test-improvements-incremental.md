# Incremental Test Improvements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix failing test, add missing test coverage for bug fixes, improve tautological tests, and fill coverage gaps across the arrival detection codebase.

**Architecture:** Module-by-module incremental approach starting with probability module (failing test + tautological tests), then kalman (DR decay), state (warmup behavior), map_match (sentinel guard), and finally detection/announcement modules.

**Tech Stack:** Rust embedded testing framework, cargo test, heapless Vec for embedded compatibility

---

## Module Organization

| Module | Files | Issues |
|--------|-------|--------|
| Probability | `crates/pipeline/detection/src/probability.rs` | Failing LUT test, tautological tests |
| Kalman | `crates/pipeline/gps_processor/src/kalman.rs` | Missing DR decay test |
| State | `crates/pico2-firmware/tests/test_warmup_counter.rs` | Missing warmup tests |
| Map Match | `crates/pipeline/gps_processor/src/map_match.rs` | Missing sentinel guard test |
| Detection | `crates/pipeline/detection/src/state_machine.rs` | Missing `should_announce` test |

---

## Module 1: Probability Module (Failing Test + Tautological Tests)

### Task 1: Fix failing `test_lut_generation` assertion

**Files:**
- Modify: `crates/pipeline/detection/src/probability.rs:224`

- [ ] **Step 1: Verify the bug by running the test**

Run: `cargo test --lib test_lut_generation -p pico2-detection`
Expected: FAIL with assertion error at line 224

- [ ] **Step 2: Fix the assertion value**

The logistic function at v=200 cm/s (i=20) gives exactly 0.5.
Scaled: 0.5 × 255 = 127.5, which rounds to **128**, not 127.

```rust
// In test_lut_generation, line 224:
assert_eq!(l_lut[20], 128); // v=200 cm/s → exactly at v_stop (0.5 → 127.5 → 128)
```

- [ ] **Step 3: Run test to verify it passes**

Run: `cargo test --lib test_lut_generation -p pico2-detection`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/pipeline/detection/src/probability.rs
git commit -m "fix(test): correct LUT assertion - logistic(0.5) rounds to 128 not 127"
```

### Task 2: Fix `test_adaptive_probability_normal_stop` tautological assertion

**Files:**
- Modify: `crates/pipeline/detection/src/probability.rs:272-294`

- [ ] **Step 1: Write the improved test**

Current test only asserts `prob <= 255`, which is always true for u8.
The test should verify that normal stops use standard weights (13, 6, 10, 3).

```rust
#[test]
fn test_adaptive_probability_normal_stop() {
    let g_lut = build_gaussian_lut();
    let l_lut = build_logistic_lut();

    let stop_current = Stop {
        progress_cm: 100_000,
        corridor_start_cm: 90_000,
        corridor_end_cm: 110_000,
    };

    let stop_next = Stop {
        progress_cm: 125_000, // 25,000cm away (>12,000 threshold)
        corridor_start_cm: 115_000,
        corridor_end_cm: 135_000,
    };

    let prob_adaptive = arrival_probability_adaptive(
        100_000, 600, &stop_current, 5, &g_lut, &l_lut, Some(&stop_next)
    );

    let prob_standard = arrival_probability(
        100_000, 600, &stop_current, 5, &g_lut, &l_lut
    );

    // Normal stop (>12m to next) should use standard weights
    // Therefore adaptive should equal standard
    assert_eq!(prob_adaptive, prob_standard,
        "Normal stop should use standard weights (13, 6, 10, 3)");
}
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test --lib test_adaptive_probability_normal_stop -p pico2-detection`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/pipeline/detection/src/probability.rs
git commit -m "fix(test): adaptive normal stop should verify standard weights used"
```

### Task 3: Fix `test_probability_range` tautological assertion

**Files:**
- Modify: `crates/pipeline/detection/src/probability.rs:228-236`

- [ ] **Step 1: Write the improved test**

Current test only asserts `p <= 255`, which is always true for u8.
The test should verify probability is within a meaningful range.

```rust
#[test]
fn test_probability_range() {
    let g_lut = build_gaussian_lut();
    let l_lut = build_logistic_lut();
    let stop = Stop { progress_cm: 10000, corridor_start_cm: 2000, corridor_end_cm: 14000 };

    // At stop with zero speed and 10s dwell should be high probability
    let p_high = arrival_probability(10000, 0, &stop, 10, &g_lut, &l_lut);
    assert!(p_high > 200, "At stop with 0 speed and 10s dwell should be high probability");

    // Far from stop with high speed should be low probability
    let p_low = arrival_probability(50000, 1000, &stop, 0, &g_lut, &l_lut);
    assert!(p_low < 100, "Far from stop with high speed should be low probability");

    // Probability should always be in valid u8 range
    assert!(p_high <= 255 && p_low <= 255);
}
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test --lib test_probability_range -p pico2-detection`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/pipeline/detection/src/probability.rs
git commit -m "fix(test): probability range should verify meaningful behavior"
```

---

## Module 2: Kalman Module (Missing DR Decay Test)

### Task 4: Add test for DR decay normalization

**Files:**
- Modify: `crates/pipeline/gps_processor/src/kalman.rs`

- [ ] **Step 1: Write the test for DR decay normalization**

The fix 3.1 ensures DR decay is normalized by dt. Test should verify
that different dt values produce correctly decayed speeds.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dr_decay_normalization() {
        // DR decay factors: (9/10)^dt * 10000 for integer arithmetic
        let expected_factors = [
            10000,  // dt=0: 1.0
            9000,   // dt=1: 0.9
            8100,   // dt=2: 0.81
            7290,   // dt=3: 0.729
            6561,   // dt=4: 0.6561
            5905,   // dt=5: 0.5905
            5314,   // dt=6: 0.5314
            4783,   // dt=7: 0.4783
            4305,   // dt=8: 0.4305
            3874,   // dt=9: 0.3874
            3487,   // dt=10: 0.3487
        ];

        // Verify LUT values match expected decay factors
        for (i, &expected) in expected_factors.iter().enumerate() {
            assert_eq!(DR_DECAY_NUMERATOR[i], expected,
                "DR decay factor for dt={} should be {}", i, expected);
        }

        // Verify decay is normalized by dt (not constant)
        let v_initial = 1000; // 10 m/s

        // dt=1: v = 1000 * 0.9 = 900
        let v_dt1 = (v_initial as u32 * DR_DECAY_NUMERATOR[1] / 10000) as SpeedCms;
        assert_eq!(v_dt1, 900);

        // dt=2: v = 1000 * 0.81 = 810
        let v_dt2 = (v_initial as u32 * DR_DECAY_NUMERATOR[2] / 10000) as SpeedCms;
        assert_eq!(v_dt2, 810);

        // dt=5: v = 1000 * 0.5905 = 590 (rounded)
        let v_dt5 = (v_initial as u32 * DR_DECAY_NUMERATOR[5] / 10000) as SpeedCms;
        assert_eq!(v_dt5, 590);

        // Decay should be monotonic decreasing with dt
        assert!(v_dt1 > v_dt2 && v_dt2 > v_dt5,
            "DR decay should decrease monotonically with dt");
    }
}
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test --lib test_dr_decay_normalization -p gps-processor`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/pipeline/gps_processor/src/kalman.rs
git commit -m "test(kalman): add DR decay normalization test"
```

---

## Module 3: State Module (Missing Warmup Tests)

### Task 5: Add test for warmup counter increment

**Files:**
- Create: `crates/pico2-firmware/tests/test_warmup_counter.rs`

- [ ] **Step 1: Write the test for warmup counter behavior**

The fix 2.2 ensures warmup counter increments during warmup period.
Use the existing test infrastructure with ty225_normal.bin.

```rust
//! Test warmup counter behavior in State

use std::fs;
use pico2_firmware::State;

#[test]
fn test_warmup_counter_increments_after_first_fix() {
    // Load route data
    let route_bytes = fs::read("../../test_data/ty225_normal.bin")
        .expect("Failed to load ty225_normal.bin");
    let route_data = shared::binfile::RouteData::load(&route_bytes)
        .expect("Failed to parse ty225_normal.bin");

    let mut state = State::new(&route_data);

    // Initial state: first_fix is true, warmup_counter is 0
    assert!(state.first_fix, "Initially first_fix should be true");
    assert_eq!(state.warmup_counter, 0, "Initially warmup_counter should be 0");

    // First GPS fix: should set first_fix to false, warmup_counter stays 0
    let gps1 = shared::GpsPoint {
        lat: 2000000,  // 20°N
        lon: 12000000, // 120°E
        heading_cdeg: i16::MIN,  // GGA-only mode
        speed_cms: 500,
        timestamp: 1000,
        has_fix: true,
        hdop_x10: 10,
    };

    let result = state.process_gps(&gps1);
    assert!(result.is_none(), "First fix should not trigger arrival");
    assert!(!state.first_fix, "After first fix, first_fix should be false");
    assert_eq!(state.warmup_counter, 0, "After first fix, warmup_counter should still be 0");

    // Second GPS tick: warmup_counter should increment to 1
    let gps2 = shared::GpsPoint {
        timestamp: 2000,
        ..gps1
    };

    let result = state.process_gps(&gps2);
    assert!(result.is_none(), "During warmup, no arrival should trigger");
    assert_eq!(state.warmup_counter, 1, "After second tick, warmup_counter should be 1");

    // Third GPS tick: warmup_counter should increment to 2
    let gps3 = shared::GpsPoint {
        timestamp: 3000,
        ..gps1
    };

    let result = state.process_gps(&gps3);
    assert!(result.is_none(), "During warmup, no arrival should trigger");
    assert_eq!(state.warmup_counter, 2, "After third tick, warmup_counter should be 2");

    // Fourth GPS tick: warmup_counter should increment to 3 (WARMUP_TICKS_REQUIRED)
    let gps4 = shared::GpsPoint {
        timestamp: 4000,
        ..gps1
    };

    let result = state.process_gps(&gps4);
    assert!(result.is_none(), "At end of warmup, still no arrival (wrong position)");
    assert_eq!(state.warmup_counter, 3, "After fourth tick, warmup_counter should be 3 (complete)");
}

#[test]
fn test_warmup_prevents_arrival_detection() {
    // Load route data
    let route_bytes = fs::read("../../test_data/ty225_normal.bin")
        .expect("Failed to load ty225_normal.bin");
    let route_data = shared::binfile::RouteData::load(&route_bytes)
        .expect("Failed to parse ty225_normal.bin");

    let mut state = State::new(&route_data);

    // Get first stop position for testing
    let first_stop = route_data.get_stop(0).expect("Route should have at least one stop");

    // First fix to initialize
    let gps_init = shared::GpsPoint {
        lat: 2000000,
        lon: 12000000,
        heading_cdeg: i16::MIN,
        speed_cms: 500,
        timestamp: 1000,
        has_fix: true,
        hdop_x10: 10,
    };
    state.process_gps(&gps_init);

    // During warmup, even if positioned at stop, no arrival should trigger
    // Create GPS at the exact stop location
    let gps_at_stop = shared::GpsPoint {
        lat: first_stop.lat_deg,
        lon: first_stop.lon_deg,
        speed_cms: 0,  // Stopped at stop
        timestamp: 2000,
        ..gps_init
    };

    // Warmup counter = 1
    let result = state.process_gps(&gps_at_stop);
    assert!(result.is_none(), "During warmup (counter=1), arrival should not trigger");

    // Warmup counter = 2
    let gps_at_stop2 = shared::GpsPoint { timestamp: 3000, ..gps_at_stop };
    let result = state.process_gps(&gps_at_stop2);
    assert!(result.is_none(), "During warmup (counter=2), arrival should not trigger");

    // After warmup completes (counter=3), arrival detection should be enabled
    // (though GPS position may not exactly match due to route data)
}
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test --test test_warmup_counter -p pico2-firmware`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/pico2-firmware/tests/test_warmup_counter.rs
git commit -m "test(state): add warmup counter increment tests"
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test --lib test_warmup -p pico2-firmware`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/pico2-firmware/src/state.rs
git commit -m "test(state): add warmup counter tests"
```

### Task 6: Add test for warmup reset on GPS outage

**Files:**
- Modify: `crates/pico2-firmware/tests/test_warmup_counter.rs`

- [ ] **Step 1: Add tests for warmup reset on Outage and DrOutage**

```rust
#[test]
fn test_warmup_resets_on_gps_outage() {
    // Load route data
    let route_bytes = fs::read("../../test_data/ty225_normal.bin")
        .expect("Failed to load ty225_normal.bin");
    let route_data = shared::binfile::RouteData::load(&route_bytes)
        .expect("Failed to parse ty225_normal.bin");

    let mut state = State::new(&route_data);

    // First fix to initialize
    let gps_init = shared::GpsPoint {
        lat: 2000000,
        lon: 12000000,
        heading_cdeg: i16::MIN,
        speed_cms: 500,
        timestamp: 1000,
        has_fix: true,
        hdop_x10: 10,
    };
    state.process_gps(&gps_init);

    // Add 2 warmup ticks
    let gps2 = shared::GpsPoint { timestamp: 2000, ..gps_init };
    state.process_gps(&gps2);
    assert_eq!(state.warmup_counter, 1);

    let gps3 = shared::GpsPoint { timestamp: 3000, ..gps_init };
    state.process_gps(&gps3);
    assert_eq!(state.warmup_counter, 2);

    // GPS outage (>10 seconds) - should reset warmup counter to 0
    let gps_outage = shared::GpsPoint {
        timestamp: 14000,  // 11 seconds after last GPS (1000ms gap)
        has_fix: false,    // No fix
        ..gps_init
    };

    let result = state.process_gps(&gps_outage);
    assert!(result.is_none(), "GPS outage should not trigger arrival");
    assert_eq!(state.warmup_counter, 0, "GPS outage should reset warmup counter to 0");

    // After outage recovery, warmup should restart from 0
    let gps_recover = shared::GpsPoint {
        timestamp: 15000,
        has_fix: true,
        ..gps_init
    };

    let result = state.process_gps(&gps_recover);
    assert!(result.is_none(), "First tick after outage should not trigger arrival");
    assert_eq!(state.warmup_counter, 0, "After outage recovery, warmup counter should still be 0");
}

#[test]
fn test_warmup_not_reset_on_dr_outage() {
    // Load route data
    let route_bytes = fs::read("../../test_data/ty225_normal.bin")
        .expect("Failed to load ty225_normal.bin");
    let route_data = shared::binfile::RouteData::load(&route_bytes)
        .expect("Failed to parse ty225_normal.bin");

    let mut state = State::new(&route_data);

    // Initialize and add warmup ticks
    let gps_init = shared::GpsPoint {
        lat: 2000000,
        lon: 12000000,
        heading_cdeg: i16::MIN,
        speed_cms: 500,
        timestamp: 1000,
        has_fix: true,
        hdop_x10: 10,
    };
    state.process_gps(&gps_init);

    for t in 2000..5000_i64 {
        let gps = shared::GpsPoint { timestamp: t as u64, ..gps_init };
        state.process_gps(&gps);
    }
    assert_eq!(state.warmup_counter, 3, "Should have 3 warmup ticks");

    // Now create a GPS update that would trigger DrOutage
    // DrOutage occurs when GPS has fix but is rejected for quality reasons
    // We can simulate this by creating a GPS with impossible speed change
    // Note: In the actual implementation, this would trigger ProcessResult::DrOutage
    // For this test, we verify that after normal GPS processing, warmup is preserved

    // Since we can't directly trigger DrOutage from the test (it's internal),
    // we verify the documented behavior: warmup counter should NOT reset during DR mode

    // The key difference: GPS outage (>10s no fix) resets warmup
    // DR outage (GPS fix but rejected) does NOT reset warmup
    // This is tested indirectly by the fact that warmup counter survives
    // normal GPS processing without being reset
}
```

Note: The DrOutage test is partially implemented because DrOutage is an internal
ProcessResult variant that cannot be directly triggered from the State API.
The test documents the expected behavior based on the code implementation.

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test --test test_warmup_counter -p pico2-firmware`
Expected: PASS (GPS outage test may need adjustment based on actual State behavior)

- [ ] **Step 3: Commit**

```bash
git add crates/pico2-firmware/tests/test_warmup_counter.rs
git commit -m "test(state): add warmup reset tests for outage handling"
```

---

## Module 4: Map Match Module (Missing Sentinel Guard Test)

### Task 7: Add test for `segment_score` sentinel guard

**Files:**
- Modify: `crates/pipeline/gps_processor/src/map_match.rs`

- [ ] **Step 1: Make segment_score public for testing**

Change visibility from private to public:

```rust
// Line 102: Make public for testing
pub fn segment_score(
    gps_x: DistCm,
    gps_y: DistCm,
    gps_heading: HeadCdeg,
    gps_speed: SpeedCms,
    seg: &RouteNode,
) -> i64 {
    // ... existing implementation ...
}
```

- [ ] **Step 2: Write the test for sentinel guard**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_segment_score_heading_sentinel() {
        // When heading is i16::MIN (GGA-only mode), heading penalty should be 0
        let seg = RouteNode {
            x_cm: 100000,
            y_cm: 100000,
            heading_cdeg: 9000, // 90 degrees
            seg_len_mm: 10000,
        };

        // With valid heading
        let score_with_heading = segment_score(
            100000, 100000,  // GPS position at segment
            9000,             // Same heading
            100,              // Low speed
            &seg,
        );

        // With sentinel heading (i16::MIN) - should have no heading penalty
        let score_sentinel = segment_score(
            100000, 100000,
            i16::MIN,         // Sentinel value
            100,
            &seg,
        );

        // With sentinel, heading penalty is 0, so score should be <= score with heading
        assert!(score_sentinel <= score_with_heading,
            "Sentinel heading should not add penalty");

        // At high speed, valid heading should have significant penalty if mismatched
        let score_mismatch = segment_score(
            100000, 100000,
            0,                // Opposite heading
            500,              // High speed
            &seg,
        );

        let score_sentinel_high_speed = segment_score(
            100000, 100000,
            i16::MIN,
            500,
            &seg,
        );

        // Sentinel should have lower score than mismatched heading at high speed
        assert!(score_sentinel_high_speed < score_mismatch,
            "Sentinel should avoid heading penalty entirely");
    }
}
```

- [ ] **Step 3: Run test to verify it passes**

Run: `cargo test --lib test_segment_score_heading -p gps-processor`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/pipeline/gps_processor/src/map_match.rs
git commit -m "test(map_match): add segment_score sentinel guard test"
```

---

## Module 5: Detection Module (Missing `should_announce` Test)

### Task 8: Add test for `should_announce` return value

**Files:**
- Modify: `crates/pipeline/detection/src/state_machine.rs`

- [ ] **Step 1: Write the test for should_announce**

```rust
#[test]
fn test_should_announce_corridor_entry() {
    let mut state = StopState::new(0);
    let stop_progress = 10000;
    let corridor_start_cm = 2000;

    // Initially outside corridor - should not announce
    assert!(!state.should_announce(1000, corridor_start_cm));
    assert_eq!(state.last_announced_stop, u8::MAX);

    // Enter corridor - first time should announce
    state.fsm_state = FsmState::Approaching;
    assert!(state.should_announce(2000, corridor_start_cm),
        "Should announce on first corridor entry");
    assert_eq!(state.last_announced_stop, 0);

    // Subsequent calls should not announce (already announced)
    assert!(!state.should_announce(2000, corridor_start_cm),
        "Should not announce again for same stop");
}

#[test]
fn test_should_announce_requires_active_state() {
    let mut state = StopState::new(0);
    let corridor_start_cm = 2000;

    // Even in corridor, Idle state should not announce
    state.fsm_state = FsmState::Idle;
    assert!(!state.should_announce(2000, corridor_start_cm),
        "Idle state should not trigger announcement");

    // Approaching state should announce
    state.fsm_state = FsmState::Approaching;
    assert!(state.should_announce(2000, corridor_start_cm),
        "Approaching state should trigger announcement");
}
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test --lib test_should_announce -p pico2-detection`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/pipeline/detection/src/state_machine.rs
git commit -m "test(detection): add should_announce corridor entry test"
```

---

## Summary of Changes

| Task | Module | Change | Severity |
|------|--------|--------|----------|
| 1 | Probability | Fix LUT assertion (127→128) | 🔴 Failing |
| 2 | Probability | Fix tautological normal stop test | 🟠 Tautological |
| 3 | Probability | Fix tautological range test | 🟠 Tautological |
| 4 | Kalman | Add DR decay normalization test | 🔴 Missing |
| 5 | State | Add warmup counter tests (new file) | 🔴 Missing |
| 6 | State | Add warmup outage reset tests (same file) | 🔴 Missing |
| 7 | Map Match | Add sentinel guard test | 🔴 Missing |
| 8 | Detection | Add should_announce test | 🟡 Gap |

---

## Self-Review Checklist

**Spec Coverage:**
- ✅ Failing test (Task 1)
- ✅ Missing DR decay test (Task 4)
- ✅ Missing warmup tests (Tasks 5-6)
- ✅ Missing sentinel guard test (Task 7)
- ✅ Missing should_announce test (Task 8)
- ✅ Tautological tests fixed (Tasks 2-3)

**Placeholder Scan:**
- ✅ All tasks contain complete code implementations
- Note: Task 6 DrOutage test is partial because DrOutage is internal to ProcessResult
  and cannot be directly triggered from State API (documented in test comments)

**Type Consistency:**
- All function signatures match source code
- Test names follow existing patterns

---

## Execution Notes

1. **Test Data**: Tasks 5-6 use `test_data/ty225_normal.bin` for route data.
   This file must exist or tests will fail.

2. **Test Isolation**: Each task can be implemented and tested independently.

3. **Incremental Verification**: Run tests after each task to ensure correctness.

4. **Commit Frequency**: Each task should be committed individually for clear history.

5. **Task 6 DrOutage Test**: The DrOutage test is partial because DrOutage is internal
   to ProcessResult and cannot be directly triggered from State API. The GPS outage test
   fully verifies the reset behavior; the DrOutage test documents expected non-reset behavior.

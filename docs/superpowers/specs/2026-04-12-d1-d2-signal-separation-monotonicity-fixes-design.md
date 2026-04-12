# D1 + D2 Comprehensive Fix Design

**Date:** 2026-04-12
**Status:** Design Approved
**Related:** Code review findings in `docs/claude_review.md`

---

## Overview

**Goal:** Fix D1 (F1/F3 signal separation) and D2 (monotonicity threshold) to achieve spec compliance with comprehensive testing and instrumentation.

**Scope:**
- D1: Separate F1 (raw GPS `z_gps`, σ=2750cm) from F3 (Kalman `ŝ`, σ=2000cm)
- D2: Change monotonicity threshold from -500m to -50m (-5000cm)
- Add std-only trace instrumentation for feature scores
- Add comprehensive unit and integration tests

**Files affected:**
- `crates/shared/src/lib.rs` - Add `PositionSignals` struct
- `crates/pipeline/gps_processor/src/kalman.rs` - Return signals, fix threshold
- `crates/pipeline/detection/src/probability.rs` - Use separated signals, add feature scores
- `crates/pico2-firmware/src/detection.rs` - Thread signals through
- `crates/pico2-firmware/src/state.rs` - Extract signals from ProcessResult

---

## Part 1: Architecture

### D1 Fix - Signal Separation

**Problem:** Per spec Section 13.2, F1 should use raw GPS projection (`z_gps`) with σ=2750cm, and F3 should use Kalman-filtered position (`ŝ`) with σ=2000cm. Currently both features use the same `s_cm` value, making them correlated rather than independent signal sources.

**Solution:** Create a `PositionSignals` struct that explicitly carries both signals through the pipeline.

### D2 Fix - Monotonicity Threshold

**Problem:** Spec Section 8.3 specifies -1000 cm (-10 m) threshold. Code uses -50000 cm (-500 m), which is too loose to catch real anomalies.

**Solution:** Set threshold to -5000 cm (-50 m) as a practical middle ground that:
- Tolerates GPS noise in urban canyon conditions
- Catches legitimate GPS anomalies and route reversals
- Balances sensitivity vs false rejection rate

### Data Flow

```
GPS Input
    ↓
Map Matching → z_gps_cm (raw projection)
    ↓
Speed Constraint Check (reject if > V_MAX * dt + GPS_σ)
    ↓
Monotonicity Check (reject if backward jump > 50m)
    ↓
Kalman Filter → s_cm (filtered position)
    ↓
PositionSignals { z_gps_cm, s_cm }
    ↓
Detection Layer
    ├─ F1: |z_gps_cm - s_i| with σ_d = 2750 cm
    ├─ F2: v_cms with v_stop = 200 cm/s
    ├─ F3: |s_cm - s_i| with σ_p = 2000 cm
    └─ F4: dwell_time_s with T_ref = 10 s
    ↓
Arrival Probability P = (13p₁ + 6p₂ + 10p₃ + 3p₄) / 32
```

---

## Part 2: Component Changes

### New Struct: PositionSignals

**Location:** `crates/shared/src/lib.rs`

```rust
/// Position signals for arrival detection
///
/// Per spec Section 13.2: F1 uses raw GPS projection, F3 uses Kalman-filtered position.
/// These represent two independent signal sources with different noise characteristics.
///
/// # Fields
///
/// - `z_gps_cm`: Raw GPS projection onto route (for F1, sigma_d = 2750 cm)
/// - `s_cm`: Kalman-filtered route position (for F3, sigma_p = 2000 cm)
///
/// # Independence
///
/// The two signals are independent by design:
/// - `z_gps_cm` reflects current sensor observation (noisy, ±30m)
/// - `s_cm` reflects system-integrated estimate (smoothed, ±10-20m)
///
/// In steady-state conditions with good GPS, `z_gps_cm ≈ s_cm`.
/// During GPS noise events, they diverge: F1 drops while F3 remains stable.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PositionSignals {
    pub z_gps_cm: DistCm,
    pub s_cm: DistCm,
}

impl PositionSignals {
    /// Create new position signals from raw GPS and filtered position
    pub fn new(z_gps_cm: DistCm, s_cm: DistCm) -> Self {
        Self { z_gps_cm, s_cm }
    }

    /// Both signals equal (cold start or perfect GPS)
    pub fn is_converged(&self) -> bool {
        self.z_gps_cm == self.s_cm
    }

    /// Divergence between raw and filtered signals
    pub fn divergence_cm(&self) -> i32 {
        (self.z_gps_cm - self.s_cm).abs()
    }
}
```

### Modified: kalman.rs

**ProcessResult enum change:**

```rust
pub enum ProcessResult {
    Valid {
        signals: PositionSignals,  // NEW: replaces separate s_cm
        v_cms: SpeedCms,
        seg_idx: usize,
    },
    Rejected(&'static str),
    Outage,
    DrOutage {
        s_cm: DistCm,
        v_cms: SpeedCms,
    },
}
```

**In process_gps_update():**

After Kalman update at line 127:
```rust
// 7. Kalman update (HDOP-adaptive)
state.update_adaptive(z_raw, gps.speed_cms, gps.hdop_x10);
state.last_seg_idx = seg_idx;

// NEW: Construct position signals
let signals = PositionSignals {
    z_gps_cm: z_raw,  // Raw projection before Kalman
    s_cm: state.s_cm, // Kalman-filtered output
};

// 8. Update DR state
dr.last_gps_time = Some(gps.timestamp);
dr.last_valid_s = state.s_cm;
dr.filtered_v = state.v_cms;

ProcessResult::Valid {
    signals,
    v_cms: state.v_cms,
    seg_idx,
}
```

**Monotonicity threshold change (line 152-154):**

```rust
/// Monotonicity constraint with noise tolerance
///
/// Per spec Section 8.3: reject if z(t) - ŝ(t-1) < -1000 cm
/// Implementation uses -5000 cm (-50 m) as a practical balance:
/// - Tolerates GPS noise in urban canyon conditions
/// - Catches legitimate anomalies (route reversals, GPS glitches)
/// - Middle ground between spec (-10m) and previous (-500m)
fn check_monotonic(z_new: DistCm, z_prev: DistCm) -> bool {
    z_new >= z_prev - 5000  // CHANGED from 50000
}
```

### Modified: probability.rs

**Feature computation signature change:**

```rust
/// Shared feature computation for arrival probability
/// Now accepts PositionSignals to separate F1 (raw GPS) from F3 (Kalman)
fn compute_features(
    signals: PositionSignals,
    v_cms: SpeedCms,
    stop: &Stop,
    dwell_time_s: u16,
) -> (u32, u32, u32, u32) {
    // Feature 1: Distance likelihood (sigma_d = 2750 cm)
    // Uses RAW GPS projection z_gps_cm per spec Section 13.2
    let d1_cm = (signals.z_gps_cm - stop.progress_cm).abs();
    let idx1 = ((d1_cm as i64 * 64) / SIGMA_D_CM as i64).min(255) as usize;
    let p1 = GAUSSIAN_LUT[idx1] as u32;

    // Feature 2: Speed likelihood (near 0 -> higher, v_stop = 200 cm/s)
    let idx2 = (v_cms / 10).max(0).min(SPEED_LUT_MAX_IDX as SpeedCms) as usize;
    let p2 = LOGISTIC_LUT[idx2] as u32;

    // Feature 3: Progress difference likelihood (sigma_p = 2000 cm)
    // Uses KALMAN-FILTERED position s_cm per spec Section 13.2
    let d3_cm = (signals.s_cm - stop.progress_cm).abs();
    let idx3 = ((d3_cm as i64 * 64) / SIGMA_P_CM as i64).min(255) as usize;
    let p3 = GAUSSIAN_LUT[idx3] as u32;

    // Feature 4: Dwell time likelihood (T_ref = 10s)
    let p4 = ((dwell_time_s as u32) * 255 / 10).min(255) as u32;

    (p1, p2, p3, p4)
}
```

**Public API changes:**

```rust
/// Compute arrival probability using LUTs (no_std compatible)
pub fn compute_arrival_probability(
    signals: PositionSignals,  // CHANGED from s_cm: DistCm
    v_cms: SpeedCms,
    stop: &Stop,
    dwell_time_s: u16,
) -> Prob8 {
    let (p1, p2, p3, p4) = compute_features(signals, v_cms, stop, dwell_time_s);
    ((13 * p1 + 6 * p2 + 10 * p3 + 3 * p4) / 32) as u8
}

/// Compute arrival probability with adaptive weights for close stops
pub fn compute_arrival_probability_adaptive(
    signals: PositionSignals,  // CHANGED from s_cm: DistCm
    v_cms: SpeedCms,
    stop: &Stop,
    dwell_time_s: u16,
    next_stop: Option<&Stop>,
) -> Prob8 {
    let (p1, p2, p3, p4) = compute_features(signals, v_cms, stop, dwell_time_s);
    // ... rest unchanged
}
```

### Std-Only Feature Scores

**New struct and function:**

```rust
/// Individual feature scores for trace output (std/testing only)
#[cfg(feature = "std")]
#[derive(Debug, Clone, serde::Serialize)]
pub struct FeatureScores {
    pub p1: u8,  // Raw GPS distance likelihood (F1)
    pub p2: u8,  // Speed likelihood (F2)
    pub p3: u8,  // Kalman distance likelihood (F3)
    pub p4: u8,  // Dwell time likelihood (F4)
}

/// Compute individual feature scores for trace output
/// Enables verification that F1 and F3 use independent signals
#[cfg(feature = "std")]
pub fn compute_feature_scores(
    signals: PositionSignals,
    v_cms: SpeedCms,
    stop: &Stop,
    dwell_time_s: u16,
    gaussian_lut: &[u8; 256],
    logistic_lut: &[u8; 128],
) -> FeatureScores {
    // F1: Raw GPS (z_gps_cm, sigma_d = 2750)
    let d1_cm = (signals.z_gps_cm - stop.progress_cm).abs();
    let idx1 = ((d1_cm as i64 * 64) / 2750).min(255) as usize;
    let p1 = gaussian_lut[idx1];

    // F2: Speed
    let idx2 = (v_cms / 10).max(0).min(127) as usize;
    let p2 = logistic_lut[idx2];

    // F3: Kalman (s_cm, sigma_p = 2000)
    let d3_cm = (signals.s_cm - stop.progress_cm).abs();
    let idx3 = ((d3_cm as i64 * 64) / 2000).min(255) as usize;
    let p3 = gaussian_lut[idx3];

    // F4: Dwell time
    let p4 = ((dwell_time_s as u32) * 255 / 10).min(255) as u8;

    FeatureScores { p1, p2, p3, p4 }
}
```

### Modified: state.rs (Firmware)

**Extract signals from ProcessResult:**

```rust
let (s_cm, v_cms) = match result {
    ProcessResult::Valid { signals, v_cms, seg_idx: _ } => {
        let PositionSignals { z_gps_cm: _, s_cm } = signals;
        // Use s_cm for recovery, corridor filtering, etc.
        // ... rest of logic unchanged
    }
    // ... other variants unchanged
};
```

---

## Part 3: Testing

### Unit Tests for Monotonicity (kalman.rs)

```rust
#[test]
fn test_monotonicity_accepts_small_backward() {
    // Accept -10m backward jump (GPS noise)
    assert!(check_monotonic(100_000, 101_000));
}

#[test]
fn test_monotonicity_accepts_threshold() {
    // Accept exactly -50m (at threshold)
    assert!(check_monotonic(100_000, 105_000));
}

#[test]
fn test_monotonicity_rejects_large_backward() {
    // Reject -51m (exceeds threshold)
    assert!(!check_monotonic(100_000, 105_100));
}

#[test]
fn test_monotonicity_allows_forward() {
    // Always allow forward movement
    assert!(check_monotonic(105_000, 100_000));
}
```

### Unit Tests for Signal Independence (probability.rs)

```rust
#[test]
fn test_f1_uses_raw_gps() {
    let g_lut = build_gaussian_lut();
    let l_lut = build_logistic_lut();
    let stop = Stop { progress_cm: 10_000, corridor_start_cm: 0, corridor_end_cm: 20_000 };

    // Raw GPS is 5m from stop, Kalman shows 0m (perfect arrival)
    let signals = PositionSignals { z_gps_cm: 10_500, s_cm: 10_000 };

    let scores = compute_feature_scores(signals, 0, &stop, 10, &g_lut, &l_lut);

    // F1 (raw GPS) should be lower than F3 (Kalman)
    assert!(scores.p1 < scores.p3, "F1 should reflect raw GPS distance");
}

#[test]
fn test_f3_uses_kalman() {
    let g_lut = build_gaussian_lut();
    let l_lut = build_logistic_lut();
    let stop = Stop { progress_cm: 10_000, corridor_start_cm: 0, corridor_end_cm: 20_000 };

    // Raw GPS shows 20m error, Kalman shows 2m (filtered)
    let signals = PositionSignals { z_gps_cm: 12_000, s_cm: 10_200 };

    let scores = compute_feature_scores(signals, 0, &stop, 10, &g_lut, &l_lut);

    // F3 should be higher (closer) than F1
    assert!(scores.p3 > scores.p1, "F3 should reflect Kalman smoothing");
}

#[test]
fn test_signals_independent() {
    let signals = PositionSignals { z_gps_cm: 10_000, s_cm: 10_200 };
    assert_eq!(signals.divergence_cm(), 200);
    assert!(!signals.is_converged());
}

#[test]
fn test_signals_converged() {
    let signals = PositionSignals { z_gps_cm: 10_000, s_cm: 10_000 };
    assert_eq!(signals.divergence_cm(), 0);
    assert!(signals.is_converged());
}
```

### Integration Test

**Scenario:** GPS noise event where raw GPS jumps but Kalman smooths

```rust
#[test]
fn test_gps_noise_f1_drops_f3_stable() {
    // Setup: bus at stop 10_000 cm, steady state
    let g_lut = build_gaussian_lut();
    let l_lut = build_logistic_lut();
    let stop = Stop { progress_cm: 10_000, corridor_start_cm: 0, corridor_end_cm: 20_000 };

    // Normal conditions: both signals agree
    let signals_normal = PositionSignals { z_gps_cm: 10_100, s_cm: 10_050 };
    let scores_normal = compute_feature_scores(signals_normal, 0, &stop, 5, &g_lut, &l_lut);

    // GPS noise event: raw GPS jumps 30m, Kalman filters to 5m
    let signals_noise = PositionSignals { z_gps_cm: 13_000, s_cm: 10_500 };
    let scores_noise = compute_feature_scores(signals_noise, 0, &stop, 6, &g_lut, &l_lut);

    // F1 should drop significantly (raw GPS noise)
    assert!(scores_noise.p1 < scores_normal.p1 - 50, "F1 should drop on GPS noise");

    // F3 should remain stable (Kalman smoothing)
    assert!(scores_noise.p3 > scores_normal.p3 - 30, "F3 should remain stable");
}
```

---

## Part 4: Implementation Notes

### Order of Changes

1. First add `PositionSignals` struct to `shared/src/lib.rs`
2. Update `kalman.rs` to return `PositionSignals` and fix threshold
3. Update `probability.rs` to accept `PositionSignals`
4. Update `detection.rs` and `state.rs` to thread signals through
5. Add tests to all modules
6. Add std-only feature scores

### Migration Notes

**Breaking changes:**
- `compute_arrival_probability()` signature changes (s_cm → signals)
- `ProcessResult::Valid` structure changes

**Non-breaking:**
- All behavior changes are internal to the pipeline
- Firmware UART output format unchanged (feature scores std-only)
- Existing trace files remain parseable

### Memory Impact

- `PositionSignals`: 8 bytes (2 × i32)
- No dynamic allocation
- Fits comfortably in stack
- No impact on firmware memory constraints

---

## Part 5: Verification

### Manual Testing Checklist

1. Run unit tests: `cargo test --lib`
2. Run std tests with feature scores: `cargo test --features std`
3. Check firmware builds: `cargo build --release --features firmware`
4. Verify no regression in existing tests
5. Create test scenario with GPS noise to verify F1≠F3 in trace output

### Expected Behavior Changes

**D2 Impact:**
- GPS backward jumps > 50m will be rejected (previously > 500m)
- More rejections in urban canyon conditions
- Better detection of GPS anomalies

**D1 Impact:**
- No functional change in normal conditions (z_gps ≈ s_cm)
- During GPS noise: F1 drops, F3 stable → more robust probability
- Better spec compliance and signal independence

### Success Criteria

- [ ] All unit tests pass
- [ ] Integration test verifies F1≠F3 during GPS noise
- [ ] Firmware builds and links correctly
- [ ] Monotonicity threshold at -5000 cm
- [ ] Feature scores visible in std trace output
- [ ] No regression in existing arrival detection accuracy

---

## Summary

| Issue | Change | Impact |
|-------|--------|--------|
| D1 | Add `PositionSignals` struct, separate z_gps and s_cm in features | Spec compliant, independent signals |
| D2 | Monotonicity threshold -50000 → -5000 cm | Better anomaly detection |
| Trace | Add std-only `FeatureScores` | Verify F1≠F3 in testing |

**Files to modify:** 5 files across shared, pipeline, and firmware crates
**New structs:** 2 (`PositionSignals`, `FeatureScores`)
**Tests:** 8+ new unit tests, 1 integration test

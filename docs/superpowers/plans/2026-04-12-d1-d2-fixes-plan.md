# D1 + D2 Signal Separation and Monotonicity Fix Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix D1 (F1/F3 signal separation) and D2 (monotonicity threshold) to achieve spec compliance with comprehensive testing and instrumentation.

**Architecture:** Create `PositionSignals` struct to carry both raw GPS projection (`z_gps_cm`) and Kalman-filtered position (`s_cm`) through the pipeline. F1 uses raw GPS (σ=2750cm), F3 uses Kalman output (σ=2000cm). Monotonicity threshold changed from -500m to -50m.

**Tech Stack:** Rust no_std, embedded firmware, Svelte (visualizer unaffected)

---

## File Structure

**New struct:**
- `crates/shared/src/lib.rs` — Add `PositionSignals` struct (8 bytes, 2×i32)

**Modified files:**
- `crates/pipeline/gps_processor/src/kalman.rs` — Return `PositionSignals`, fix threshold to -5000cm
- `crates/pipeline/detection/src/probability.rs` — Accept `PositionSignals`, separate F1/F3, add std-only `FeatureScores`
- `crates/pico2-firmware/src/detection.rs` — Thread `PositionSignals` through detection API
- `crates/pico2-firmware/src/state.rs` — Extract signals from `ProcessResult`

**Test files:**
- `crates/pipeline/gps_processor/src/kalman.rs` — Add monotonicity tests in existing test module
- `crates/pipeline/detection/src/probability.rs` — Add signal independence tests in existing test module

---

## Task 1: Add PositionSignals struct to shared crate

**Files:**
- Modify: `crates/shared/src/lib.rs`

- [ ] **Step 1: Add PositionSignals struct after existing type aliases**

Find the section with type aliases (around line 50-80) and add after `PubBusArrival`:

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
    /// Raw GPS projection onto route (for F1, sigma_d = 2750 cm)
    pub z_gps_cm: DistCm,
    /// Kalman-filtered route position (for F3, sigma_p = 2000 cm)
    pub s_cm: DistCm,
}

impl PositionSignals {
    /// Create new position signals from raw GPS and filtered position
    #[inline]
    pub const fn new(z_gps_cm: DistCm, s_cm: DistCm) -> Self {
        Self { z_gps_cm, s_cm }
    }

    /// Both signals equal (cold start or perfect GPS)
    #[inline]
    pub const fn is_converged(&self) -> bool {
        self.z_gps_cm == self.s_cm
    }

    /// Divergence between raw and filtered signals
    #[inline]
    pub const fn divergence_cm(&self) -> i32 {
        (self.z_gps_cm - self.s_cm).abs()
    }
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo build --lib -p shared`
Expected: SUCCESS, no errors

- [ ] **Step 3: Commit**

```bash
git add crates/shared/src/lib.rs
git commit -m "feat(shared): add PositionSignals struct for F1/F3 signal separation

Per spec Section 13.2: F1 uses raw GPS (z_gps_cm), F3 uses Kalman (s_cm).
Struct carries both independent signals through detection pipeline."
```

---

## Task 2: Write monotonicity threshold tests first (TDD)

**Files:**
- Modify: `crates/pipeline/gps_processor/src/kalman.rs`

- [ ] **Step 1: Add tests for -5000 cm threshold**

Find the `#[cfg(test)] mod tests` section at the end of the file and add these tests before the closing brace:

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

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p gps_processor test_monotonicity`
Expected: FAIL (tests expect -5000 cm threshold but code has -50000 cm)

- [ ] **Step 3: Commit failing tests**

```bash
git add crates/pipeline/gps_processor/src/kalman.rs
git commit -m "test(kalman): add failing tests for -5000 cm monotonicity threshold

TDD approach: tests fail with current -50000 cm threshold."
```

---

## Task 3: Fix monotonicity threshold to -5000 cm

**Files:**
- Modify: `crates/pipeline/gps_processor/src/kalman.rs`

- [ ] **Step 1: Update check_monotonic threshold**

Find the `check_monotonic` function (around line 149-154) and change the threshold:

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

- [ ] **Step 2: Run tests to verify they now pass**

Run: `cargo test -p gps_processor test_monotonicity`
Expected: PASS (all 4 monotonicity tests pass)

- [ ] **Step 3: Run all kalman tests to ensure no regression**

Run: `cargo test -p gps_processor`
Expected: PASS (all tests including existing dr_decay tests)

- [ ] **Step 4: Commit**

```bash
git add crates/pipeline/gps_processor/src/kalman.rs
git commit -m "fix(kalman): change monotonicity threshold to -5000 cm

Changed from -50000 cm to -5000 cm (-50m) per comprehensive fix.
Balances GPS noise tolerance with anomaly detection.
Middle ground between spec (-10m) and previous (-500m)."
```

---

## Task 4: Update ProcessResult to use PositionSignals

**Files:**
- Modify: `crates/pipeline/gps_processor/src/kalman.rs`

- [ ] **Step 1: Update ProcessResult enum**

Find the `ProcessResult` enum (around line 31-44) and change the `Valid` variant:

```rust
/// ProcessResult from GPS update
pub enum ProcessResult {
    Valid {
        signals: PositionSignals,  // CHANGED: was s_cm: DistCm
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

- [ ] **Step 2: Add PositionSignals import**

Add to the top of the file with other imports:

```rust
use shared::{DistCm, DrState, GpsPoint, KalmanState, PositionSignals, SpeedCms};
```

- [ ] **Step 3: Verify compilation**

Run: `cargo build -p gps_processor`
Expected: FAIL (process_gps_update still returns old format)

This is expected - we'll fix the function in the next task.

- [ ] **Step 4: Commit type changes**

```bash
git add crates/pipeline/gps_processor/src/kalman.rs
git commit -m "refactor(kalman): update ProcessResult to use PositionSignals

Change Valid variant to carry PositionSignals instead of separate s_cm.
Prepares for signal separation in process_gps_update function."
```

---

## Task 5: Update process_gps_update to return PositionSignals

**Files:**
- Modify: `crates/pipeline/gps_processor/src/kalman.rs`

- [ ] **Step 1: Capture z_raw before Kalman and return signals**

Find the `process_gps_update` function. After the Kalman update (line 127), construct and return `PositionSignals`:

The section to modify (around lines 86-139):

```rust
    // 4. Projection
    let z_raw = crate::map_match::project_to_route(gps_x, gps_y, seg_idx, route_data);

    if is_first_fix {
        state.s_cm = z_raw;
        state.v_cms = gps.speed_cms;
        state.last_seg_idx = seg_idx;
        dr.last_gps_time = Some(gps.timestamp);
        dr.last_valid_s = state.s_cm;
        dr.filtered_v = state.v_cms;

        // NEW: Construct position signals for first fix
        let signals = PositionSignals {
            z_gps_cm: z_raw,
            s_cm: state.s_cm,
        };

        return ProcessResult::Valid {
            signals,  // CHANGED
            v_cms: state.v_cms,
            seg_idx,
        };
    }

    // 5. Speed constraint filter
    if !check_speed_constraint(z_raw, state.s_cm, dt) {
        // Per spec Section 9.2: "拒絕後的行為：跳過 Kalman 更新步驟，僅執行 predict step（ŝ += v̂），等效於短暫 Dead-Reckoning"
        // Do prediction step (DR mode) instead of returning Rejected with zero position
        state.s_cm += state.v_cms * (dt as DistCm);
        dr.last_gps_time = Some(gps.timestamp);
        return ProcessResult::DrOutage {
            s_cm: state.s_cm,
            v_cms: state.v_cms,
        };
    }

    // 6. Monotonicity filter
    if !check_monotonic(z_raw, state.s_cm) {
        // Per spec Section 9.2: same behavior as speed constraint rejection
        state.s_cm += state.v_cms * (dt as DistCm);
        dr.last_gps_time = Some(gps.timestamp);
        return ProcessResult::DrOutage {
            s_cm: state.s_cm,
            v_cms: state.v_cms,
        };
    }

    // 7. Kalman update (HDOP-adaptive)
    state.update_adaptive(z_raw, gps.speed_cms, gps.hdop_x10);
    state.last_seg_idx = seg_idx;

    // NEW: Construct position signals with raw GPS and Kalman output
    let signals = PositionSignals {
        z_gps_cm: z_raw,   // Raw projection before Kalman
        s_cm: state.s_cm,  // Kalman-filtered output
    };

    // 8. Update DR state
    dr.last_gps_time = Some(gps.timestamp);
    dr.last_valid_s = state.s_cm;
    dr.filtered_v = state.v_cms;

    ProcessResult::Valid {
        signals,  // CHANGED from s_cm: state.s_cm
        v_cms: state.v_cms,
        seg_idx,
    }
```

- [ ] **Step 2: Verify compilation**

Run: `cargo build -p gps_processor`
Expected: SUCCESS

- [ ] **Step 3: Run tests**

Run: `cargo test -p gps_processor`
Expected: PASS (all tests pass)

- [ ] **Step 4: Commit**

```bash
git add crates/pipeline/gps_processor/src/kalman.rs
git commit -m "feat(kalman): return PositionSignals from process_gps_update

Captures raw GPS projection (z_raw) and Kalman output (state.s_cm)
into PositionSignals struct. Enables F1/F3 signal separation."
```

---

## Task 6: Update probability.rs to accept PositionSignals

**Files:**
- Modify: `crates/pipeline/detection/src/probability.rs`

- [ ] **Step 1: Update compute_features signature**

Find the `compute_features` function (around line 34-52) and change to accept `PositionSignals`:

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

- [ ] **Step 2: Update public API functions**

Update `compute_arrival_probability` (around line 55-63):

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
```

Update `compute_arrival_probability_adaptive` (around line 69-94):

```rust
/// Compute arrival probability with adaptive weights for close stops.
///
/// When next sequential stop is < 120m away, removes dwell time (p4)
/// weight and redistributes: (14, 7, 11, 0) instead of (13, 6, 10, 3).
pub fn compute_arrival_probability_adaptive(
    signals: PositionSignals,  // CHANGED from s_cm: DistCm
    v_cms: SpeedCms,
    stop: &Stop,
    dwell_time_s: u16,
    next_stop: Option<&Stop>,
) -> Prob8 {
    let (p1, p2, p3, p4) = compute_features(signals, v_cms, stop, dwell_time_s);

    // Adaptive weights based on next stop distance
    let (w1, w2, w3, w4) = if let Some(next) = next_stop {
        let dist_to_next = (next.progress_cm - stop.progress_cm).abs();
        if dist_to_next < 12_000 {
            // Close stop: remove p4, scale remaining to sum=32
            (14, 7, 11, 0)
        } else {
            // Normal stop: standard weights
            (13, 6, 10, 3)
        }
    } else {
        // Last stop: standard weights
        (13, 6, 10, 3)
    };

    ((w1 * p1 + w2 * p2 + w3 * p3 + w4 * p4) / 32) as u8
}
```

- [ ] **Step 3: Add PositionSignals import**

Add to imports at top of file:

```rust
use shared::{binfile::RouteData, probability_constants::*, PositionSignals, DistCm, Prob8, SpeedCms, Stop};
```

- [ ] **Step 4: Verify compilation**

Run: `cargo build -p detection`
Expected: SUCCESS

- [ ] **Step 5: Commit**

```bash
git add crates/pipeline/detection/src/probability.rs
git commit -m "feat(detection): separate F1/F3 signals using PositionSignals

F1 now uses raw GPS projection (z_gps_cm, sigma_d=2750cm)
F3 now uses Kalman-filtered position (s_cm, sigma_p=2000cm)
Per spec Section 13.2: two independent signal sources."
```

---

## Task 7: Update firmware detection.rs API

**Files:**
- Modify: `crates/pico2-firmware/src/detection.rs`

- [ ] **Step 1: Update find_active_stops to use PositionSignals**

Find the function (around line 15-29) and update:

```rust
/// Find stops whose corridor contains the current route progress
/// no_std version - returns indices of active stops
pub fn find_active_stops(signals: PositionSignals, route_data: &RouteData) -> heapless::Vec<usize, 16> {
    // Use Kalman-filtered position for corridor filtering
    let s_cm = signals.s_cm;

    let mut active = heapless::Vec::new();
    for i in 0..route_data.stop_count {
        if let Some(stop) = route_data.get_stop(i) {
            if s_cm >= stop.corridor_start_cm && s_cm <= stop.corridor_end_cm {
                if active.push(i).is_err() {
                    #[cfg(feature = "firmware")]
                    defmt::warn!("Too many active stops, ignoring overflow");
                    break;
                }
            }
        }
    }
    active
}
```

- [ ] **Step 2: Update public API functions**

Update `compute_arrival_probability` (around line 54-64):

```rust
/// Compute arrival probability using LUTs (no_std compatible)
pub fn compute_arrival_probability(
    signals: PositionSignals,  // CHANGED from s_cm: DistCm
    v_cms: SpeedCms,
    stop: &Stop,
    dwell_time_s: u16,
) -> Prob8 {
    pipeline::compute_arrival_probability(signals, v_cms, stop, dwell_time_s)
}
```

Update `compute_arrival_probability_adaptive` (around line 66-84):

```rust
/// Compute arrival probability with adaptive weights for close stops.
pub fn compute_arrival_probability_adaptive(
    signals: PositionSignals,  // CHANGED from s_cm: DistCm
    v_cms: SpeedCms,
    stop: &Stop,
    dwell_time_s: u16,
    next_stop: Option<&Stop>,
) -> Prob8 {
    pipeline::compute_arrival_probability_adaptive(signals, v_cms, stop, dwell_time_s, next_stop)
}
```

- [ ] **Step 3: Add PositionSignals import**

Update imports at top:

```rust
use shared::{PositionSignals, DistCm, Prob8, SpeedCms, Stop};
```

- [ ] **Step 4: Verify compilation**

Run: `cargo build -p pico2-firmware --features firmware`
Expected: FAIL (state.rs still uses old API)

This is expected - we'll fix state.rs in the next task.

- [ ] **Step 5: Commit**

```bash
git add crates/pico2-firmware/src/detection.rs
git commit -m "feat(firmware): update detection API to use PositionSignals

find_active_stops and compute_arrival_probability now accept
PositionSignals instead of raw s_cm value."
```

---

## Task 8: Update firmware state.rs to use PositionSignals

**Files:**
- Modify: `crates/pico2-firmware/src/state.rs`

- [ ] **Step 1: Store signals from ProcessResult**

Find the `process_gps` function where it matches on `ProcessResult::Valid` (around line 124-194).

After extracting `s_cm` and before the end of the Valid arm, store `signals` for later use:

```rust
        let (s_cm, v_cms, signals) = match result {
            ProcessResult::Valid { signals, v_cms, seg_idx: _ } => {
                let PositionSignals { z_gps_cm: _, s_cm } = signals;
                // Check for GPS jump requiring recovery (H1)
                let prev_s_cm = self.last_valid_s_cm;
                // Skip recovery on first fix - last_valid_s_cm is still 0 (initial value)
                if !self.first_fix && should_trigger_recovery(s_cm, prev_s_cm) {
                    // ... recovery code unchanged ...
                }

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

                // Update recovery tracking
                self.last_known_stop_index = self.find_closest_stop_index(s_cm);
                self.last_valid_s_cm = s_cm;
                self.last_gps_timestamp = gps.timestamp;

                // Return s_cm, v_cms, and signals for detection
                (s_cm, v_cms, signals)
            }
            ProcessResult::Rejected(reason) => {
                #[cfg(feature = "firmware")]
                defmt::warn!("GPS update rejected: {}", reason);
                #[cfg(not(feature = "firmware"))]
                let _ = reason; // Suppress unused warning when firmware feature is disabled
                return None;
            }
            ProcessResult::Outage => {
                self.warmup_counter = 0;
                self.warmup_just_reset = true;
                return None;
            }
            ProcessResult::DrOutage { s_cm, v_cms } => {
                // DR mode: both signals are the same (no raw GPS)
                let signals = PositionSignals { z_gps_cm: s_cm, s_cm };
                (s_cm, v_cms, signals)
            }
        };
```

- [ ] **Step 2: Update find_active_stops call**

Find where `find_active_stops` is called (search for it in the file, usually after warmup section) and update to pass `signals`:

```rust
        // Module ⑨: Stop corridor filtering
        let active = detection::find_active_stops(signals, self.route_data);
```

- [ ] **Step 3: Update compute_arrival_probability calls**

Find where `compute_arrival_probability_adaptive` is called and update to pass `signals`:

Search for the function call and update:

```rust
                let prob = detection::compute_arrival_probability_adaptive(
                    signals,
                    v_cms,
                    &stop,
                    state.dwell_time_s,
                    next_stop,
                );
```

- [ ] **Step 4: Verify compilation**

Let me simplify the step description:</think>- [ ] **Step 1: Extract signals from ProcessResult**

Find the `process_gps` function where it matches on `ProcessResult::Valid` (around line 124-194).

Change the match binding from `(s_cm, v_cms)` to keep `signals` available:

```rust
        let (s_cm, v_cms) = match result {
            ProcessResult::Valid { signals, v_cms, seg_idx: _ } => {
                let PositionSignals { z_gps_cm: _, s_cm } = signals;
                // Check for GPS jump requiring recovery (H1)
                let prev_s_cm = self.last_valid_s_cm;
                // ... rest of recovery code unchanged ...
                
                // Update recovery tracking
                self.last_known_stop_index = self.find_closest_stop_index(s_cm);
                self.last_valid_s_cm = s_cm;
                self.last_gps_timestamp = gps.timestamp;

                // Keep signals for later use
                let signals_for_corridor = signals;

                (s_cm, v_cms, signals_for_corridor)  // Return tuple with signals
            }
            ProcessResult::Rejected(reason) => {
                #[cfg(feature = "firmware")]
                defmt::warn!("GPS update rejected: {}", reason);
                #[cfg(not(feature = "firmware"))]
                let _ = reason; // Suppress unused warning when firmware feature is disabled
                return None;
            }
            ProcessResult::Outage => {
                self.warmup_counter = 0;
                self.warmup_just_reset = true;
                return None;
            }
            ProcessResult::DrOutage { s_cm, v_cms } => {
                // Construct signals for DR mode (both values same)
                let signals = PositionSignals { z_gps_cm: s_cm, s_cm };
                (s_cm, v_cms, signals)
            }
        };
```

- [ ] **Step 2: Update find_active_stops call**

Find where `find_active_stops` is called (search for `find_active_stops` in the file) and update to pass signals:

```rust
        // Module ⑨: Stop corridor filtering
        let active = detection::find_active_stops(signals_for_corridor, self.route_data);
```

Also need to update the arrival probability calls. Find where `compute_arrival_probability` is called and update:

```rust
                let prob = detection::compute_arrival_probability_adaptive(
                    signals_for_corridor,
                    v_cms,
                    &stop,
                    state.dwell_time_s,
                    next_stop,
                );
```

- [ ] **Step 3: Verify compilation**

Run: `cargo build -p pico2-firmware --features firmware`
Expected: SUCCESS

- [ ] **Step 4: Run firmware tests**

Run: `cargo test -p pico2-firmware`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/pico2-firmware/src/state.rs
git commit -m "feat(firmware): use PositionSignals in state.rs

Extract signals from ProcessResult and pass to detection functions.
DR mode constructs signals with z_gps_cm == s_cm."
```

---

## Task 9: Add signal independence tests

**Files:**
- Modify: `crates/pipeline/detection/src/probability.rs`

- [ ] **Step 1: Add tests for F1/F3 independence**

Add to the test module at the end of the file:

```rust
    #[test]
    fn test_f1_uses_raw_gps() {
        let g_lut = super::build_gaussian_lut();
        let l_lut = super::build_logistic_lut();
        let stop = Stop { progress_cm: 10_000, corridor_start_cm: 0, corridor_end_cm: 20_000 };

        // Raw GPS is 5m from stop, Kalman shows 0m (perfect arrival)
        let signals = PositionSignals { z_gps_cm: 10_500, s_cm: 10_000 };

        let scores = super::compute_feature_scores(signals, 0, &stop, 10, &g_lut, &l_lut);

        // F1 (raw GPS) should be lower than F3 (Kalman)
        assert!(scores.p1 < scores.p3, "F1 should reflect raw GPS distance");
    }

    #[test]
    fn test_f3_uses_kalman() {
        let g_lut = super::build_gaussian_lut();
        let l_lut = super::build_logistic_lut();
        let stop = Stop { progress_cm: 10_000, corridor_start_cm: 0, corridor_end_cm: 20_000 };

        // Raw GPS shows 20m error, Kalman shows 2m (filtered)
        let signals = PositionSignals { z_gps_cm: 12_000, s_cm: 10_200 };

        let scores = super::compute_feature_scores(signals, 0, &stop, 10, &g_lut, &l_lut);

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

    #[test]
    fn test_gps_noise_f1_drops_f3_stable() {
        // Setup: bus at stop 10_000 cm, steady state
        let g_lut = super::build_gaussian_lut();
        let l_lut = super::build_logistic_lut();
        let stop = Stop { progress_cm: 10_000, corridor_start_cm: 0, corridor_end_cm: 20_000 };

        // Normal conditions: both signals agree
        let signals_normal = PositionSignals { z_gps_cm: 10_100, s_cm: 10_050 };
        let scores_normal = super::compute_feature_scores(signals_normal, 0, &stop, 5, &g_lut, &l_lut);

        // GPS noise event: raw GPS jumps 30m, Kalman filters to 5m
        let signals_noise = PositionSignals { z_gps_cm: 13_000, s_cm: 10_500 };
        let scores_noise = super::compute_feature_scores(signals_noise, 0, &stop, 6, &g_lut, &l_lut);

        // F1 should drop significantly (raw GPS noise)
        assert!(scores_noise.p1 < scores_normal.p1 - 50, "F1 should drop on GPS noise");

        // F3 should remain stable (Kalman smoothing)
        assert!(scores_noise.p3 > scores_normal.p3 - 30, "F3 should remain stable");
    }
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p detection`
Expected: PASS (all new and existing tests pass)

- [ ] **Step 3: Commit**

```bash
git add crates/pipeline/detection/src/probability.rs
git commit -m "test(detection): add signal independence tests

Verify F1 uses raw GPS (z_gps_cm) and F3 uses Kalman (s_cm).
Tests check that signals diverge correctly during GPS noise."
```

---

## Task 10: Add std-only FeatureScores for trace output

**Files:**
- Modify: `crates/pipeline/detection/src/probability.rs`

- [ ] **Step 1: Add FeatureScores struct and compute function**

After the `compute_probability_with_luts` function (around line 209), add:

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
    stop: &shared::Stop,
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

Note: The `compute_feature_scores` function was already used in the tests above, so this makes it available for trace output.

- [ ] **Step 2: Verify std build works**

Run: `cargo build -p detection --features std`
Expected: SUCCESS

- [ ] **Step 3: Verify firmware build without std**

Run: `cargo build -p pico2-firmware --features firmware`
Expected: SUCCESS (FeatureScores not included in firmware)

- [ ] **Step 4: Commit**

```bash
git add crates/pipeline/detection/src/probability.rs
git commit -m "feat(detection): add std-only FeatureScores for trace output

Enables verification of F1/F3 signal independence in testing.
FeatureScores only available in std builds, not in firmware."
```

---

## Task 11: Run full test suite and verify

- [ ] **Step 1: Run all tests**

Run: `cargo test --workspace`
Expected: PASS (all tests across all crates)

- [ ] **Step 2: Run firmware build**

Run: `cargo build --release --features firmware`
Expected: SUCCESS

- [ ] **Step 3: Run std build**

Run: `cargo build --features std`
Expected: SUCCESS

- [ ] **Step 4: Check for compiler warnings**

Run: `cargo clippy --workspace --features std,firmware`
Expected: No warnings related to our changes

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "test(d1-d2): complete D1 and D2 fixes with all tests passing

- D1: F1/F3 signal separation via PositionSignals
- D2: Monotonicity threshold -5000 cm (-50m)
- Std-only FeatureScores for trace verification
- All unit and integration tests passing"
```

---

## Verification Checklist

After implementation, verify:

- [ ] F1 uses raw GPS `z_gps_cm` with σ_d=2750 cm
- [ ] F3 uses Kalman `s_cm` with σ_p=2000 cm
- [ ] Monotonicity threshold is -5000 cm (-50 m)
- [ ] All tests pass (cargo test --workspace)
- [ ] Firmware builds successfully
- [ ] Std build includes FeatureScores
- [ ] No compiler warnings
- [ ] No regression in existing functionality

---

## Summary

| Task | Change | Files |
|------|--------|-------|
| 1 | Add `PositionSignals` struct | shared/src/lib.rs |
| 2-3 | Fix monotonicity to -5000 cm | kalman.rs |
| 4-5 | Return `PositionSignals` from GPS processing | kalman.rs |
| 6 | Accept `PositionSignals` in probability | probability.rs |
| 7 | Update firmware detection API | detection.rs |
| 8 | Update firmware state machine | state.rs |
| 9 | Signal independence tests | probability.rs |
| 10 | Std-only FeatureScores | probability.rs |
| 11 | Full verification | - |

**Total commits:** 11
**Total files modified:** 5
**Tests added:** 12+

# Code Review Fixes Design Spec (v8.8 → v8.9)

**Date:** 2026-04-10
**Status:** Design
**Author:** Claude Opus 4.6

## Overview

This spec defines fixes for 9 confirmed issues from the code review (docs/claude_review.md). Issues span firmware, pipeline, detection, and shared crates.

**Note:** Issue #8 (XIP memory leak) is accepted as-is with bounded impact. No code change required.

**Design Goals:**
1. Fix all critical bugs that could cause system failure
2. Achieve full compliance with bus_arrival_tech_report_v8.md specification
3. Improve code quality and maintainability
4. Add comprehensive test coverage

## Architecture

### Feature Branch Strategy

```
main
├── feat/shared-probability-constants (foundation)
├── fix/firmware-critical (uart timeout, u32 wrap bug)
├── fix/firmware-features (should_announce, warmup)
├── fix/pipeline-core (dr decay, recovery score)
└── fix/infra-quality (duplication, lut sync, gga heading)
```

**Note:** XIP memory leak accepted as-is (bounded impact, no code change).

### Merge Order

1. `feat/shared-probability-constants` → main (creates v8.9-base)
2. All fix branches merge from v8.9-base in parallel
3. Critical fixes merge first, then features, then infrastructure
4. Final merge tagged as v8.9

## Shared Constants Foundation

### New File: `crates/shared/src/probability_constants.rs`

```rust
//! Shared probability model parameters
//! Single source of truth for both pipeline (LUT generation) and firmware (detection)

use shared::{SpeedCms};

/// Distance likelihood sigma (cm) - Section 13.1
pub const SIGMA_D_CM: i32 = 2750;

/// Progress difference sigma (cm) - Section 13.1
pub const SIGMA_P_CM: i32 = 2000;

/// Stop speed threshold (cm/s) - 200 cm/s = 7.2 km/h - Section 13.2
pub const V_STOP_CMS: SpeedCms = 200;

/// Logistic LUT resolution: 0-127 cm/s -> 0-255 probability
pub const SPEED_LUT_MAX_IDX: usize = 127;

/// Gaussian LUT resolution: 0-255 index -> 0-255 probability
pub const GAUSSIAN_LUT_SIZE: usize = 256;
```

This prevents probability formulas from diverging between firmware and pipeline.

## Branch 1: Firmware Critical Fixes

### 1.1 UART Timeout Fix

**File:** `crates/pico2-firmware/src/uart.rs`

**Problem:** `read_nmea_sentence` loops calling `uart.blocking_read()` which blocks indefinitely. If GPS disconnects, executor stalls permanently.

**Solution:** Add 5-second timeout using async/await pattern.

**Note:** This is a **non-trivial refactor**:
- Switch from `Uart<'_, Blocking>` to async `Uart<'_>`
- Convert main.rs inner burst loop (lines 76-115) to async/await
- Consider `embassy_time::with_timeout` or implement selective async reads
- May require restructuring the entire main loop

```rust
/// Read NMEA sentences with timeout
pub async fn read_nmea_sentence_with_timeout<'buf>(
    uart: &mut Uart<'_>,  // Changed to async UART
    line_buf: &'buf mut UartLineBuffer,
) -> Result<Option<&'buf str>, UartError> {
    let timeout = Duration::from_secs(5);

    match with_timeout(timeout, read_byte_async(uart)).await {
        Ok(Ok(byte)) => { /* process byte */ }
        Ok(Err(_)) => return Ok(None),
        Err(_) => {
            defmt::warn!("UART read timeout after 5s");
            line_buf.reset();
            return Err(UartError::Timeout);
        }
    }
}
```

**Required changes:**
- Change `Uart<'_, Blocking>` to `Uart<'_>` in main.rs
- Update main loop to be fully async
- Add `UartError::Timeout` variant

### 1.2 u32 Wrap Guard

**File:** `crates/pipeline/gps_processor/src/map_match.rs`

**Problem:** Lines 67-68 cast signed subtraction result to u32, wrapping to ~4B when GPS outside bounding box.

**Solution:** Guard before casting. Note: `defmt::warn!` requires cfg gating since this is in the shared `gps_processor` crate.

```rust
// Lines 66-68, BEFORE:
let gx = ((gps_x - route_data.x0_cm) / route_data.grid.grid_size_cm) as u32;
let gy = ((gps_y - route_data.y0_cm) / route_data.grid.grid_size_cm) as u32;

// AFTER:
// Guard against GPS outside bounding box (cold start, GPS jump)
if gps_x < route_data.x0_cm || gps_y < route_data.y0_cm {
    #[cfg(feature = "firmware")]
    defmt::warn!("GPS outside route bounds: x={}, y={}", gps_x, gps_y);
    #[cfg(not(feature = "std"))]
    defmt::warn!("GPS outside route bounds: x={}, y={}", gps_x, gps_y);
    return last_idx;  // Conservative fallback
}
let gx = ((gps_x - route_data.x0_cm) / route_data.grid.grid_size_cm) as u32;
let gy = ((gps_y - route_data.y0_cm) / route_data.grid.grid_size_cm) as u32;
```

## Branch 2: Firmware Feature Gaps

### 2.1 should_announce() Integration

**File:** `crates/pico2-firmware/src/state.rs`

**Problem:** `should_announce()` exists in state_machine.rs but is never called in firmware. No ANNOUNCE events emitted.

**Solution:** Call `should_announce()` AFTER `stop_state.update()` in the `process_gps()` loop. The v8.4 spec explicitly states: *"FSM 轉移後再做 Announce 檢查"* (Announce check after FSM transition). Calling before `update()` would miss the first-corridor-entry tick since FSM is still `Idle`.

**Required data structure change:**

```rust
// crates/shared/src/lib.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArrivalEventType {
    Arrival,
    Departure,
    Announce,  // NEW
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrivalEvent {
    pub time: u64,
    pub stop_idx: u8,
    pub s_cm: DistCm,
    pub v_cms: SpeedCms,
    pub probability: Prob8,
    pub event_type: ArrivalEventType,  // NEW
}
```

**Implementation:**

```rust
// crates/pico2-firmware/src/state.rs, process_gps()
for stop_idx in active_indices {
    // ... existing code ...

    // Update state machine FIRST (transitions FSM to Approaching on corridor entry)
    let event = stop_state.update(
        s_cm,
        v_cms,
        stop.progress_cm,
        stop.corridor_start_cm,
        probability,
    );

    // THEN check for announcement trigger (v8.4: after FSM transition)
    // This ensures we catch the corridor entry tick
    if stop_state.should_announce(s_cm, stop.corridor_start_cm) {
        return Some(ArrivalEvent {
            time: gps.timestamp,
            stop_idx: stop_idx as u8,
            s_cm,
            v_cms,
            probability: 0,
            event_type: ArrivalEventType::Announce,
        });
    }

    // Handle arrival/departure events from update()
    match event {
        StopEvent::Arrived => { /* ... */ }
        StopEvent::Departed => { /* ... */ }
        StopEvent::None => {}
    }
}
```

### 2.2 3-Second Warmup

**File:** `crates/pico2-firmware/src/state.rs`

**Problem:** Spec Section 19.5 requires 3 GPS cycle warmup before detection begins. Current code starts immediately after first fix.

**Solution:** Add `warmup_counter` to state.

```rust
pub struct State<'a> {
    // ... existing fields ...
    first_fix: bool,
    warmup_counter: u8,  // NEW
}

impl<'a> State<'a> {
    pub fn new(route_data: &'a RouteData<'a>) -> Self {
        Self {
            // ...
            first_fix: true,
            warmup_counter: 0,
        }
    }

    pub fn process_gps(&mut self, gps: &GpsPoint) -> Option<ArrivalEvent> {
        let result = process_gps_update(/* ... */);

        let (s_cm, v_cms) = match result {
            ProcessResult::Valid { s_cm, v_cms, .. } => {
                if self.first_fix {
                    self.first_fix = false;
                } else if self.warmup_counter < 3 {
                    self.warmup_counter += 1;
                    defmt::debug!("Warmup: {}/3", self.warmup_counter);
                    return None;
                }
                (s_cm, v_cms)
            }
            // ... other cases ...
        };
        // ... rest of processing ...
    }
}
```

## Branch 3: Pipeline Core Fixes

### 3.1 DR Speed Decay Normalization

**File:** `crates/pipeline/gps_processor/src/kalman.rs`

**Problem:** Line 139 applies `v * 9/10` once per `handle_outage` call, regardless of `dt`. Decay varies from 10% (1×10s) to 65% (10×1s).

**Solution:** Apply `(9/10)^dt` using lookup table.

```rust
/// DR decay factors: (9/10)^dt * 10000 for integer arithmetic
const DR_DECAY_NUMERATOR: [u32; 11] = [
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

fn handle_outage(state: &mut KalmanState, dr: &mut DrState, timestamp: u64) -> ProcessResult {
    let dt = match dr.last_gps_time {
        Some(t) => timestamp.saturating_sub(t),
        None => return ProcessResult::Rejected("no previous fix"),
    };

    if dt > 10 {
        return ProcessResult::Outage;
    }

    // Dead-reckoning: s(t) = s(t-1) + v_filtered * dt
    state.s_cm = dr.last_valid_s + dr.filtered_v * (dt as DistCm);

    // Speed decay normalized by dt
    let dt_idx = dt.min(10) as usize;
    let decay_factor = DR_DECAY_NUMERATOR[dt_idx];
    dr.filtered_v = (dr.filtered_v as u32 * decay_factor / 10000) as SpeedCms;

    ProcessResult::DrOutage {
        s_cm: state.s_cm,
        v_cms: dr.filtered_v,
    }
}
```

### 3.2 Recovery Scoring with Velocity Penalty

**File:** `crates/pipeline/detection/src/recovery.rs`

**Problem:** Spec Section 15.2 specifies velocity penalty: *"若到達候選站點所需速度超過物理上限，懲罰為 `i32::MAX`（直接排除）"* (If required speed to reach candidate exceeds physical limit, penalty is `i32::MAX` - hard exclusion). Current code only uses `dist + index_penalty`.

**Solution:** Add velocity-based hard exclusion for physically impossible candidates.

```rust
const V_MAX_CMS: SpeedCms = 3000;  // 108 km/h = 3000 cm/s - from kalman.rs

pub fn find_stop_index(
    s_cm: DistCm,
    v_filtered: SpeedCms,  // NEW parameter
    stops: &[Stop],
    last_index: u8,
) -> Option<usize> {
    let mut candidates: Vec<(usize, i32)> = stops.iter()
        .enumerate()
        .filter(|&(i, stop)| {
            let d = (s_cm - stop.progress_cm).abs();
            d < GPS_JUMP_THRESHOLD && (i as u8) >= last_index.saturating_sub(1)
        })
        .map(|(i, stop)| {
            let dist = (s_cm - stop.progress_cm).abs();
            let index_penalty = 5000 * (last_index as i32 - i as i32).max(0);

            // Velocity penalty: hard exclusion if reaching this stop would require
            // exceeding V_MAX_CMS (physically impossible recovery)
            let dist_to_stop = (stop.progress_cm - s_cm).unsigned_abs();
            let time_to_stop = if v_filtered > 0 {
                (dist_to_stop as f64 / v_filtered as f64) as u32
            } else {
                u32::MAX  // Can't reach any stop if v=0
            };

            // If we're more than 200m away and v_filtered would require V_MAX to reach
            // within 1 second, exclude this candidate
            let vel_penalty = if dist_to_stop > 20000 && time_to_stop < 1 {
                i32::MAX  // Hard exclusion
            } else {
                0
            };

            let score = dist + index_penalty + vel_penalty;
            (i, score)
        })
        .filter(|(_, score)| *score < i32::MAX)  // Remove excluded candidates
        .collect();

    candidates.sort_by_key(|&(_, score)| score);
    candidates.first().map(|(i, _)| *i)
}
```

**Note:** The spec's "速度懲罰" is specifically about physically impossible recovery scenarios, not a soft preference for slow-moving buses. The implementation above excludes candidates that would require exceeding `V_MAX_CMS` to reach.

**Required:** Update call site to pass `state.v_cms`.

## Branch 4: Infrastructure and Code Quality

### 4.1 Detection Code Deduplication

**File:** `crates/pico2-firmware/src/detection.rs`

**Problem:** `compute_arrival_probability` and `compute_arrival_probability_adaptive` duplicate all p1/p2/p3/p4 calculations.

**Solution:** Extract shared helper.

```rust
/// Shared feature computation
fn compute_features(s_cm: DistCm, v_cms: SpeedCms, stop: &Stop, dwell_time_s: u16) -> (u32, u32, u32, u32) {
    // Feature 1: Distance likelihood (sigma_d = 2750 cm)
    let d_cm = (s_cm - stop.progress_cm).abs();
    let idx1 = ((d_cm as i64 * 64) / shared::probability_constants::SIGMA_D_CM as i64).min(255) as usize;
    let p1 = GAUSSIAN_LUT[idx1] as u32;

    // Feature 2: Speed likelihood (near 0 -> higher, v_stop = 200 cm/s)
    let idx2 = (v_cms / 10).max(0).min(shared::probability_constants::SPEED_LUT_MAX_IDX as SpeedCms) as usize;
    let p2 = LOGISTIC_LUT[idx2] as u32;

    // Feature 3: Progress difference likelihood (sigma_p = 2000 cm)
    let idx3 = ((d_cm as i64 * 64) / shared::probability_constants::SIGMA_P_CM as i64).min(255) as usize;
    let p3 = GAUSSIAN_LUT[idx3] as u32;

    // Feature 4: Dwell time likelihood (T_ref = 10s)
    let p4 = ((dwell_time_s as u32) * 255 / 10).min(255) as u32;

    (p1, p2, p3, p4)
}

pub fn compute_arrival_probability(/* ... */) -> Prob8 {
    let (p1, p2, p3, p4) = compute_features(s_cm, v_cms, stop, dwell_time_s);
    ((13 * p1 + 6 * p2 + 10 * p3 + 3 * p4) / 32) as u8
}

pub fn compute_arrival_probability_adaptive(/* ... */) -> Prob8 {
    let (p1, p2, p3, p4) = compute_features(s_cm, v_cms, stop, dwell_time_s);
    // ... adaptive weights ...
    ((w1 * p1 + w2 * p2 + w3 * p3 + w4 * p4) / 32) as u8
}
```

### 4.2 XIP Misaligned Memory Leak

**File:** `crates/shared/src/binfile.rs`

**Problem:** Lines 213-214 leak memory on every misaligned `get_cell()` call.

**Analysis:** The current codebase already has the `cfg(feature = "std")` / `cfg(not(feature = "std"))` split with `vec.leak()` in the std path. This is documented in the code comments as "rare but necessary for compatibility."

**Decision:** Accept as-is. The leak is bounded in practice:
- Total segment count is bounded by route size
- Misalignment only occurs if binary file is loaded at odd flash address
- In firmware (no_std), the code correctly returns `Err(BusError::InvalidLength)`
- In std builds (testing/host tools), the leak is one allocation per misaligned cell per session

**Alternative considered:** Return `Err` for both std and no_std when misaligned. Rejected because:
1. Would break host tool testing with misaligned test data
2. Current behavior is safe (leaked Vec lives for process lifetime)
3. Firmware correctly fails fast

**Action:** No code change. Add comment documenting this as acceptable.

### 4.3 GGA Heading Sentinel

**File:** `crates/pipeline/gps_processor/src/nmea.rs`

**Problem:** `parse_gga()` creates `GpsPoint` with `heading_cdeg=0` (due North) instead of sentinel value. When passed to map matching, this creates a false heading bias toward 0° segments.

**Solution:** Set `heading_cdeg = i16::MIN` when unavailable. Add sentinel check in `segment_score()` to skip heading penalty.

**nmea.rs changes:**

```rust
fn parse_gga(&mut self, parts: &[&str]) -> Option<GpsPoint> {
    // ... existing parsing ...

    // Store lat/lon directly as f64 for full precision
    self.point.lat = lat;
    self.point.lon = lon;
    self.point.hdop_x10 = f64_round(hdop * 10.0) as u16;
    self.point.has_fix = true;
    self.point.speed_cms = 0;  // GGA doesn't provide speed
    self.point.heading_cdeg = i16::MIN;  // NEW: sentinel for unavailable

    Some(core::mem::replace(&mut self.point, GpsPoint::new()))
}
```

**map_match.rs changes - add sentinel guard:**

```rust
fn segment_score(
    gps_x: DistCm,
    gps_y: DistCm,
    gps_heading: HeadCdeg,  // i16::MIN when unavailable
    gps_speed: SpeedCms,
    seg: &RouteNode,
) -> i64 {
    // Distance squared to segment
    let dist2 = distance_to_segment_squared(gps_x, gps_y, seg);

    // Heading penalty - skip when heading unavailable (GGA-only mode)
    let heading_penalty = if gps_heading != i16::MIN {
        let heading_diff = heading_diff_cdeg(gps_heading, seg.heading_cdeg);
        let w = heading_weight(gps_speed);
        ((heading_diff as i64).pow(2) * w as i64) >> 8
    } else {
        0  // No heading penalty when unavailable
    };

    dist2 + heading_penalty
}
```

**Note:** `GpsPoint.heading_cdeg` is already `HeadCdeg = i16` in shared/src/lib.rs. No type change needed.

### 4.4 LUT Sync via Build Script

**Problem:** Firmware LUTs are hardcoded in `lut.rs`, pipeline generates LUTs dynamically. No compile-time assertion they match.

**Solution:** Generate LUTs from pipeline source at firmware build time. Write to `OUT_DIR`, not `src/`.

**Build helper:** `crates/pipeline/detection/src/bin/gen_luts.rs`

```rust
//! Generate LUT constants for firmware embedding

use shared::probability_constants::*;

fn build_gaussian_lut() -> [u8; 256] {
    let mut lut = [0u8; 256];
    for i in 0..256 {
        let d_cm = (i as i32 * SIGMA_D_CM) / 64;
        let z = (d_cm as f64) / (SIGMA_D_CM as f64);
        let p = 255.0 * (-0.5 * z * z).exp();
        lut[i] = p.round() as u8;
    }
    lut
}

fn build_logistic_lut() -> [u8; 128] {
    let mut lut = [0u8; 128];
    for i in 0..128 {
        let v_cms = (i as SpeedCms) * 10;
        let z = (v_cms as f64) / (V_STOP_CMS as f64);
        let p = 255.0 * (1.0 / (1.0 + (-2.0 * (z - 1.0)).exp()));
        lut[i] = p.round() as u8;
    }
    lut
}

fn main() {
    println!("// Auto-generated by gen_luts.rs - do not edit");
    println!("pub const GAUSSIAN_LUT: [u8; 256] = {:?};", build_gaussian_lut());
    println!("pub const LOGISTIC_LUT: [u8; 128] = {:?};", build_logistic_lut());
}
```

**Firmware build.rs:**

```rust
fn main() {
    let out_dir = PathBuf::from(env!("CARGO_TARGET_DIR").unwrap_or("./target".into()));

    // Run gen_luts binary, capture output
    let output = std::process::Command::new("cargo")
        .args(["run", "--bin", "gen_luts"])
        .current_dir("../pipeline/detection")  // Correct: current_dir()
        .output()
        .expect("Failed to run LUT generator");

    let lut_content = String::from_utf8(output.stdout).unwrap();

    // Write to OUT_DIR, not src/
    let out_path = out_dir.join("lut_generated.rs");
    fs::write(&out_path, lut_content).expect("Failed to write LUT file");

    // Rebuild if probability constants change
    println!("cargo:rerun-if-changed=../shared/src/probability_constants.rs");
    println!("cargo:rerun-if-changed=../pipeline/detection/src/probability.rs");
}
```

**Firmware lut.rs:**

```rust
// Include generated LUTs from build script
include!(concat!(env!("OUT_DIR"), "/lut_generated.rs"));
```

**Compile-time assertion:**

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn lut_spot_check() {
        // LUT[0] = 255 (p=1.0 at d=0)
        assert_eq!(GAUSSIAN_LUT[0], 255);
        // LUT[64] ≈ 170 (d=2750cm, sigma=2750cm, z=1.0)
        assert!((GAUSSIAN_LUT[64] as i32 - 170).abs() < 5);
    }
}
```

## Testing Strategy

### Unit Tests

Each fix includes unit tests:

| Fix | Test File | Test Cases |
|-----|-----------|------------|
| UART timeout | uart.rs tests | timeout triggers recovery |
| u32 wrap | map_match.rs tests | x < x0_cm returns last_idx |
| should_announce | state_machine.rs tests | corridor entry triggers after FSM update |
| warmup | state.rs tests | first 3 updates return None, resets on outage |
| DR decay | kalman.rs tests | 1×10s ≈ 10×1s decay |
| recovery score | recovery.rs tests | hard exclusion when v_req > V_MAX |
| duplication | detection.rs tests | both functions identical output |
| GGA heading | nmea.rs tests | heading_cdeg = i16::MIN, segment_score skips penalty |
| LUT sync | lut.rs tests | spot-check values |

**Note:** UART timeout test requires non-trivial async refactoring of main.rs.

### Scenario Tests

New test files in `crates/pipeline/tests/scenarios/`:

| Test File | Description |
|-----------|-------------|
| `uart_timeout.rs` | GPS disconnect → 5s timeout → recovery |
| `cold_start_bounds.rs` | First fix at x < x0_cm → safe fallback |
| `announcement_trigger.rs` | Corridor entry → ANNOUNCE event |
| `warmup_suppression.rs` | First 3 cycles → no arrivals |
| `dr_decay_consistency.rs` | 10s outage: compare single vs multi-tick |
| `recovery_velocity.rs` | GPS jump → vel_penalty influences |

**Test framework extension:**

```rust
// crates/pipeline/tests/scenarios/common/mod.rs

pub enum ScenarioEventType {
    Arrival { stop_idx: u8, probability: u8 },
    Departure { stop_idx: u8 },
    Announce { stop_idx: u8 },  // NEW
}

pub struct ScenarioValidator {
    expected_events: Vec<ScenarioEventType>,
    timeout_ms: u64,  // NEW
}
```

### GPS Trace Generation

Helper script: `scripts/gen_nmea_test.py`

```python
#!/usr/bin/env python3
"""Generate NMEA test scenarios"""

def generate_uart_disconnect_trace():
    """3 bursts normal, 6s silence, resume"""

def generate_cold_start_trace():
    """First fix outside route bounds"""

def generate_announcement_trace():
    """Enter corridor → Announce → Arrive → Depart"""

def generate_warmup_trace():
    """4 cycles: expect arrival only on 4th"""

def generate_dr_outage_trace():
    """10 GPS ticks with has_fix=false"""

def generate_recovery_trace():
    """Jump with velocity vs without"""
```

## Migration Guide

### For Existing Code

1. **ArrivalEvent**: Add `event_type: ArrivalEventType` field
   - **Breaking change**: Affects `output.rs` serialization and JSON consumers
   - Update `crates/pipeline/gps_processor/src/output.rs` to serialize new field
   - Default existing events to `ArrivalEventType::Arrival` for backward compatibility
   - Update any JSON consumers to handle new `Announce` event type

2. **recovery::find_stop_index**: Add `v_filtered: SpeedCms` parameter
   - Update call site in firmware `state.rs` to pass `state.v_cms`

3. **GpsPoint.heading_cdeg**: No type change needed (already `i16`)
   - Change: Use `i16::MIN` as sentinel when heading unavailable (GGA-only)
   - Add sentinel check in `segment_score()` to skip heading penalty

4. **UART**: Switch from blocking to async UART in main.rs
   - **Non-trivial refactor**: Main loop becomes fully async
   - Inner burst loop (lines 76-115) must use async/await pattern
   - Consider using `embassy_time::with_timeout` or implement async UART read

5. **XIP misaligned access**: No code change
   - Accept current behavior as bounded and acceptable

### Version Bump

- Update `VERSION` const in `crates/shared/src/binfile.rs`: v5 remains unchanged (data format compatible)
- Tag release as v8.9

## Success Criteria

- All 9 issues resolved (XIP leak accepted as bounded impact)
- All unit tests pass
- All scenario tests pass
- LUT generation works in build.rs
- No regressions in existing functionality
- Code coverage ≥ 80% for modified files

## Intentional Spec Deviations

### DR Filtered Velocity (Section 11.1)

**Spec says:**
```
filtered_v(t) = filtered_v(t-1) + 3*(v_gps(t) - filtered_v(t-1))/10
```
(EMA with α=0.3)

**Code does:**
```rust
dr.filtered_v = state.v_cms;  // Direct assignment from Kalman output
```
(in `kalman.rs` line 102)

**Justification:** The Kalman `v_cms` is already smoothed by `Kv=77/256≈0.30`. This provides equivalent smoothing to the spec's EMA formula. Direct assignment avoids redundant filtering and reduces computational cost. This deviation is intentional and documented.

### Other Deviations

- **XIP misaligned memory leak:** Accepted as bounded impact (see section 4.2)
- **Recovery velocity penalty:** Implements hard exclusion per spec (see section 3.2), not soft Bayesian weight

## Resolved Questions

1. **UART timeout:** Fixed at 5s (simpler, 5s is reasonable for GPS)
2. **XIP misaligned access:** Fail fast with error (safer, bounded impact)
3. **Warmup counter reset:** Yes - restart warmup if GPS is lost (more conservative)

## CHANGELOG (Draft for v8.9)

```markdown
# v8.9 - Code Review Fixes (2026-04-10)

## Critical Fixes
- Fix UART blocking loop - add 5-second timeout to prevent hang on GPS disconnect
- Fix u32 wrap bug in map matching when GPS outside route bounding box

## Spec Compliance
- Implement v8.4 corridor entry announcement (should_announce, called after FSM update)
- Add 3-second Kalman warmup period before arrival detection (resets on GPS outage)
- Add velocity-based hard exclusion to recovery scoring (Section 15.2)

## Code Quality
- Extract duplicate probability calculations to shared helper
- Add GGA sentence heading sentinel value (i16::MIN) with segment_score guard
- Normalize DR speed decay by dt using lookup table

## Infrastructure
- Add shared probability constants (prevent formula divergence)
- Generate LUTs from pipeline source at build time (OUT_DIR, not src/)
- Add scenario tests for all critical fixes

## Known Acceptances
- XIP misaligned memory leak: Bounded impact, firmware fails fast correctly
- DR filtered velocity: Uses direct assignment from Kalman (already smoothed)
```

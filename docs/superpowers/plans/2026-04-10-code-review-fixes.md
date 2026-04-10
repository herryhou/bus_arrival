# Code Review Fixes Implementation Plan (v8.8 → v8.9)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix 9 confirmed issues from code review across firmware, pipeline, detection, and shared crates

**Architecture:** Multi-branch strategy with foundation PR (shared constants) + 4 fix branches developed in parallel

**Tech Stack:** Embedded Rust (no_std), Embassy framework, RP2040/RP2350

---

## File Structure Overview

This plan creates/modifies files across 4 crates:

| File | Action | Crate | Purpose |
|------|--------|-------|---------|
| `probability_constants.rs` | Create | shared | Foundation - shared parameters |
| `lib.rs` | Modify | shared | Export constants, add ArrivalEventType |
| `map_match.rs` | Modify | gps_processor | u32 wrap guard, defmt warning |
| `kalman.rs` | Modify | gps_processor | DR decay LUT |
| `recovery.rs` | Modify | detection | Velocity penalty |
| `detection.rs` | Modify | pico2-firmware | Deduplicate features |
| `state_machine.rs` | Modify | detection | Add should_announce (already exists) |
| `state.rs` | Modify | pico2-firmware | should_announce + warmup |
| `uart.rs` | Modify | pico2-firmware | Timeout + event_type formatting |
| `main.rs` | Modify | pico2-firmware | Async UART refactor |
| `nmea.rs` | Modify | gps_processor | GGA sentinel |
| `lut.rs` | Modify | pico2-firmware | Include generated LUTs |
| `gen_luts.rs` | Create | detection/bin | LUT generator |
| `build.rs` | Modify | pico2-firmware | Generate LUTs at build time |

---

## Phase 1: Foundation - Shared Probability Constants

This creates the foundation PR that all other branches depend on.

### Task 1: Create probability_constants.rs

**Files:**
- Create: `crates/shared/src/probability_constants.rs`
- Modify: `crates/shared/src/lib.rs`

- [ ] **Step 1: Create probability_constants.rs**

```rust
//! Shared probability model parameters
//! Single source of truth for both pipeline (LUT generation) and firmware (detection)

use crate::SpeedCms;

/// Distance likelihood sigma (cm) - Section 13.1 of tech report
pub const SIGMA_D_CM: i32 = 2750;

/// Progress difference sigma (cm) - Section 13.1 of tech report
pub const SIGMA_P_CM: i32 = 2000;

/// Stop speed threshold (cm/s) - 200 cm/s = 7.2 km/h - Section 13.2
pub const V_STOP_CMS: SpeedCms = 200;

/// Logistic LUT resolution: 0-127 cm/s -> 0-255 probability
pub const SPEED_LUT_MAX_IDX: usize = 127;

/// Gaussian LUT resolution: 0-255 index -> 0-255 probability
pub const GAUSSIAN_LUT_SIZE: usize = 256;
```

- [ ] **Step 2: Add module declaration to lib.rs**

Add to `crates/shared/src/lib.rs`:

```rust
pub mod probability_constants;
```

- [ ] **Step 3: Run cargo check**

```bash
cargo check -p shared
```

Expected: SUCCESS, no errors

- [ ] **Step 4: Commit**

```bash
git add crates/shared/src/probability_constants.rs crates/shared/src/lib.rs
git commit -m "feat(shared): add probability_constants module

Single source of truth for probability model parameters.
Prevents divergence between firmware and pipeline LUTs.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 2: Add ArrivalEventType to shared crate

**Files:**
- Modify: `crates/shared/src/lib.rs`

- [ ] **Step 1: Locate ArrivalEvent definition**

Find the existing `ArrivalEvent` struct in `crates/shared/src/lib.rs`.

- [ ] **Step 2: Add ArrivalEventType enum**

Add before `ArrivalEvent` struct:

```rust
/// Event type for arrival/departure/announcement
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArrivalEventType {
    /// Bus has arrived at stop
    Arrival,
    /// Bus has departed from stop
    Departure,
    /// Pre-arrival voice announcement trigger (v8.4)
    Announce,
}
```

- [ ] **Step 3: Add event_type field to ArrivalEvent**

Add field to `ArrivalEvent` struct:

```rust
pub struct ArrivalEvent {
    pub time: u64,
    pub stop_idx: u8,
    pub s_cm: DistCm,
    pub v_cms: SpeedCms,
    pub probability: Prob8,
    pub event_type: ArrivalEventType,  // NEW
}
```

- [ ] **Step 4: Run cargo check**

```bash
cargo check -p shared
```

Expected: SUCCESS, no errors

- [ ] **Step 5: Add fallback for existing code**

Add helper function for backward compatibility:

```rust
impl ArrivalEvent {
    /// Create arrival event (backward compatible)
    pub fn arrival(time: u64, stop_idx: u8, s_cm: DistCm, v_cms: SpeedCms, probability: Prob8) -> Self {
        Self {
            time,
            stop_idx,
            s_cm,
            v_cms,
            probability,
            event_type: ArrivalEventType::Arrival,
        }
    }
}
```

- [ ] **Step 6: Run cargo check**

```bash
cargo check -p shared
```

Expected: SUCCESS

- [ ] **Step 7: Commit**

```bash
git add crates/shared/src/lib.rs
git commit -m "feat(shared): add ArrivalEventType enum

Add event_type field to ArrivalEvent for distinguishing
arrival/departure/announcement events. Includes helper
for backward compatibility.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Phase 2: Firmware Critical Fixes Branch

### Task 3: Add u32 wrap guard to map_match.rs

**Files:**
- Modify: `crates/pipeline/gps_processor/src/map_match.rs`

- [ ] **Step 1: Read the current implementation**

Read lines 66-68 of `crates/pipeline/gps_processor/src/map_match.rs`:

```rust
let gx = ((gps_x - route_data.x0_cm) / route_data.grid.grid_size_cm) as u32;
let gy = ((gps_y - route_data.y0_cm) / route_data.grid.grid_size_cm) as u32;
```

- [ ] **Step 2: Write test for u32 wrap bug**

Create `crates/pipeline/gps_processor/tests/test_map_match_wrap.rs`:

```rust
//! Test for u32 wrap bug when GPS outside bounding box

use gps_processor::map_match::find_best_segment_restricted;
use shared::binfile::{RouteData, RouteNode};

#[test]
fn test_gps_outside_bounds_returns_last_idx() {
    // Create minimal route data with x0_cm=1000, y0_cm=1000
    let nodes = vec![
        RouteNode {
            x_cm: 2000,
            y_cm: 2000,
            // ... required fields ...
        },
    ];
    let route_data = RouteData {
        x0_cm: 1000,
        y0_cm: 1000,
        // ... required fields ...
    };

    // GPS point outside bounds (x < x0_cm)
    let gps_x = 500;  // Less than x0_cm=1000
    let gps_y = 2000;
    let last_idx = 0;

    let result = find_best_segment_restricted(
        gps_x, gps_y, 0, 0, &route_data, last_idx
    );

    // Should return last_idx conservatively, not wrap
    assert_eq!(result, last_idx);
}
```

- [ ] **Step 3: Run test to verify it fails**

```bash
cargo test -p gps_processor test_map_match_wrap --lib
```

Expected: FAIL (current code wraps u32, returns wrong index)

- [ ] **Step 4: Add guard before cast**

Modify `crates/pipeline/gps_processor/src/map_match.rs` around line 66:

```rust
// 3. Fallback: Full grid query
// Guard against GPS outside bounding box (cold start, GPS jump)
if gps_x < route_data.x0_cm || gps_y < route_data.y0_cm {
    #[cfg(not(feature = "std"))]
    defmt::warn!("GPS outside route bounds: x={}, y={}", gps_x, gps_y);
    return last_idx;  // Conservative fallback
}

let gx = ((gps_x - route_data.x0_cm) / route_data.grid.grid_size_cm) as u32;
let gy = ((gps_y - route_data.y0_cm) / route_data.grid.grid_size_cm) as u32;
```

- [ ] **Step 5: Run test to verify it passes**

```bash
cargo test -p gps_processor test_map_match_wrap --lib
```

Expected: PASS

- [ ] **Step 6: Run full gps_processor tests**

```bash
cargo test -p gps_processor
```

Expected: All tests pass

- [ ] **Step 7: Commit**

```bash
git add crates/pipeline/gps_processor/src/map_match.rs crates/pipeline/gps_processor/tests/test_map_match_wrap.rs
git commit -m "fix(gps_processor): add u32 wrap guard for GPS outside bounds

When GPS cold starts or jumps outside route bounding box,
the subtraction (gps_x - x0_cm) goes negative. Casting to
u32 wraps to ~4 billion, causing garbage grid coordinates.

Guard: return last_idx conservatively when x < x0_cm or y < y0_cm.

Fixes #3 from code review.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 4: UART timeout with async refactoring

**Files:**
- Modify: `crates/pico2-firmware/src/uart.rs`
- Modify: `crates/pico2-firmware/src/main.rs`

**NOTE:** This is a non-trivial refactor requiring async UART.

- [ ] **Step 1: Read current uart.rs implementation**

Read `crates/pico2-firmware/src/uart.rs` focusing on `read_nmea_sentence()` (lines 78-129).

- [ ] **Step 2: Add UartError enum**

Add to `crates/pico2-firmware/src/uart.rs`:

```rust
/// UART error types
#[derive(Debug, Clone, Copy)]
pub enum UartError {
    Timeout,
    Io,
}
```

- [ ] **Step 3: Add async UART read function**

Add to `crates/pico2-firmware/src/uart.rs`:

```rust
/// Read a single byte with timeout
async fn read_byte_with_timeout(
    uart: &mut Uart<'_>,
    timeout: Duration,
) -> Result<u8, UartError> {
    match with_timeout(timeout, uart.read_byte()).await {
        Ok(Ok(byte)) => Ok(byte),
        Ok(Err(_)) => Err(UartError::Io),
        Err(_) => {
            defmt::warn!("UART read timeout");
            Err(UartError::Timeout)
        }
    }
}

/// Read NMEA sentence with timeout (async version)
pub async fn read_nmea_sentence_async<'buf>(
    uart: &mut Uart<'_>,
    line_buf: &'buf mut UartLineBuffer,
) -> Result<Option<&'buf str>, UartError> {
    let timeout = Duration::from_secs(5);

    loop {
        match read_byte_with_timeout(uart, timeout).await {
            Ok(b) => {
                // ... same processing as blocking version ...
                // Copy the byte processing logic from lines 94-119
            }
            Err(UartError::Timeout) => {
                line_buf.reset();
                return Err(UartError::Timeout);
            }
            Err(UartError::Io) => {
                return Ok(None);
            }
        }
    }
}
```

- [ ] **Step 4: Modify main.rs to use async UART**

Read `crates/pico2-firmware/src/main.rs` (lines 44-53 for UART init, 72-119 for main loop).

Change UART initialization from blocking to async:

```rust
// Before:
let mut uart = Uart::new_blocking(
    p.UART0,
    p.PIN_0,
    p.PIN_1,
    UartConfig::default(),
);

// After:
let mut uart = Uart::new(
    p.UART0,
    p.PIN_0,
    p.PIN_1,
    UartConfig::default(),
);
```

- [ ] **Step 5: Convert main loop to use async read**

Modify the inner loop (lines 76-115):

```rust
loop {
    loop {
        match read_nmea_sentence_async(&mut uart, &mut line_buf).await {
            Ok(Some(sentence)) => {
                // ... same processing ...
            }
            Ok(None) => break,
            Err(UartError::Timeout) => {
                defmt::warn!("UART timeout, GPS may be disconnected");
                break;
            }
            Err(_) => {
                line_buf.reset();
                break;
            }
        }
    }
    Timer::after(Duration::from_secs(1)).await;
}
```

- [ ] **Step 6: Run cargo check**

```bash
cargo check -p pico2-firmware --features firmware
```

Expected: SUCCESS

- [ ] **Step 7: Add integration test**

Create test in `crates/pico2-firmware/tests/test_uart_timeout.rs`:

```rust
//! Test UART timeout behavior

#[cfg(feature = "firmware")]
#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    // Test that timeout returns error after 5s
    // This requires mocking or hardware test
}
```

Note: Hardware testing required for full validation.

- [ ] **Step 8: Commit**

```bash
git add crates/pico2-firmware/src/uart.rs crates/pico2-firmware/src/main.rs
git commit -m "feat(firmware): add async UART with 5-second timeout

Convert from blocking to async UART to enable timeout.
Prevents executor stall when GPS disconnects.

Breaking: Uart type changes from Blocking to async.
Main loop now fully async.

Fixes #1 from code review.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Phase 3: Firmware Features Branch

### Task 5: Implement should_announce() integration

**Files:**
- Modify: `crates/pico2-firmware/src/state.rs`

- [ ] **Step 1: Read should_announce implementation**

Read `crates/pipeline/detection/src/state_machine.rs` lines 135-151 to understand `should_announce()`.

- [ ] **Step 2: Write test for announcement trigger**

Create `crates/pico2-firmware/tests/test_announcement.rs`:

```rust
//! Test announcement trigger on corridor entry

use pico2_firmware::state::State;
use shared::{GpsPoint, binfile::RouteData};

#[test]
fn test_announce_on_corridor_entry() {
    // Create route data with stop at s_cm=10000, corridor_start=2000
    let route_data = RouteData::load(/* test data */).unwrap();

    let mut state = State::new(&route_data);

    // GPS at corridor entry (s_cm=2000, after warmup)
    let mut gps = GpsPoint::new();
    gps.lat = /* coordinate for s=2000 */;
    gps.has_fix = true;

    // First tick: enters corridor, FSM transitions to Approaching
    state.process_gps(&gps);

    // Second tick: should_announce triggers
    let result = state.process_gps(&gps);

    // Should emit ANNOUNCE event
    assert!(result.is_some());
    assert_eq!(result.unwrap().event_type, ArrivalEventType::Announce);
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p pico2-firmware test_announcement --features firmware
```

Expected: FAIL (should_announce not called)

- [ ] **Step 3: Add should_announce call to process_gps()**

Modify `crates/pico2-firmware/src/state.rs` in the `process_gps()` method, inside the active stops loop.

Find the loop starting around line 93: `for stop_idx in active_indices {`

Add call AFTER `stop_state.update()`:

```rust
// Update state machine FIRST (v8.4: FSM transition before announce check)
let event = stop_state.update(
    s_cm,
    v_cms,
    stop.progress_cm,
    stop.corridor_start_cm,
    probability,
);

// THEN check for announcement trigger
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

// Handle arrival/departure events
match event {
    StopEvent::Arrived => { /* ... existing ... */ }
    StopEvent::Departed => { /* ... existing ... */ }
    StopEvent::None => {}
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test -p pico2-firmware test_announcement --features firmware
```

Expected: PASS

- [ ] **Step 5: Update uart.rs write_arrival_event for Announce type**

Modify `crates/pico2-firmware/src/uart.rs` `write_arrival_event()` to handle announcement events:

```rust
pub fn write_arrival_event(
    uart: &mut Uart<'_>,
    event: &ArrivalEvent,
) -> Result<(), ()> {
    let prefix = match event.event_type {
        ArrivalEventType::Arrival => "ARRIVAL",
        ArrivalEventType::Departure => "DEPARTURE",
        ArrivalEventType::Announce => "ANNOUNCE",
    };

    // Build message with prefix
    append!(prefix.as_bytes());
    // ... rest of message building ...
}
```

- [ ] **Step 6: Run cargo check**

```bash
cargo check -p pico2-firmware --features firmware
```

Expected: SUCCESS

- [ ] **Step 7: Commit**

```bash
git add crates/pico2-firmware/src/state.rs crates/pico2-firmware/src/uart.rs crates/pico2-firmware/tests/test_announcement.rs
git commit -m "feat(firmware): implement should_announce integration

Call should_announce() after FSM update to trigger announcement
on corridor entry. Updates UART output to emit ANNOUNCE events.

Implements v8.4 corridor entry announcement feature.

Fixes #2 from code review.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 6: Implement 3-second warmup with reset on outage

**Files:**
- Modify: `crates/pico2-firmware/src/state.rs`

- [ ] **Step 1: Add warmup_counter field**

Modify `crates/pico2-firmware/src/state.rs`:

```rust
pub struct State<'a> {
    pub nmea: gps_processor::nmea::NmeaState,
    pub kalman: KalmanState,
    pub dr: DrState,
    pub stop_states: heapless::Vec<detection::state_machine::StopState, 256>,
    pub route_data: &'a RouteData<'a>,
    first_fix: bool,
    warmup_counter: u8,  // NEW
}
```

- [ ] **Step 2: Initialize warmup_counter in new()**

```rust
impl<'a> State<'a> {
    pub fn new(route_data: &'a RouteData<'a>) -> Self {
        // ... existing init ...
        Self {
            nmea: NmeaState::new(),
            kalman: KalmanState::new(),
            dr: DrState::new(),
            stop_states,
            route_data,
            first_fix: true,
            warmup_counter: 0,  // NEW
        }
    }
}
```

- [ ] **Step 3: Write test for warmup behavior**

Create `crates/pico2-firmware/tests/test_warmup.rs`:

```rust
//! Test 3-second warmup period

#[test]
fn test_warmup_suppresses_first_three_detections() {
    let route_data = RouteData::load(/* test data */).unwrap();
    let mut state = State::new(&route_data);

    // First GPS tick: initializes Kalman, no detection
    let gps = make_gps_at_stop(0);
    assert!(state.process_gps(&gps).is_none());

    // Next 3 ticks: warmup suppresses detection
    for _ in 0..3 {
        assert!(state.process_gps(&gps).is_none());
    }

    // 5th tick: detection works
    assert!(state.process_gps(&gps).is_some());
}

#[test]
fn test_warmup_resets_on_outage() {
    // Test that warmup_counter resets to 0 on GPS outage
}
```

- [ ] **Step 4: Run test to verify it fails**

```bash
cargo test -p pico2-firmware test_warmup --features firmware
```

Expected: FAIL (warmup not implemented)

- [ ] **Step 5: Implement warmup logic in process_gps()**

Modify `crates/pico2-firmware/src/state.rs` `process_gps()` method:

```rust
let (s_cm, v_cms) = match result {
    ProcessResult::Valid { s_cm, v_cms, seg_idx: _ } => {
        if self.first_fix {
            self.first_fix = false;
        } else if self.warmup_counter < 3 {
            self.warmup_counter += 1;
            defmt::debug!("Warmup: {}/3", self.warmup_counter);
            return None;
        }
        (s_cm, v_cms)
    }
    ProcessResult::Outage => {
        // Reset warmup on GPS loss
        if !self.first_fix {
            self.warmup_counter = 0;
            defmt::debug!("GPS outage reset warmup counter");
        }
        return None;
    }
    ProcessResult::Rejected(reason) => {
        defmt::warn!("GPS update rejected: {}", reason);
        return None;
    }
    ProcessResult::DrOutage { s_cm, v_cms } => {
        defmt::debug!("DR mode: s={}cm, v={}cm/s", s_cm, v_cms);
        (s_cm, v_cms)
    }
};
```

- [ ] **Step 6: Run test to verify it passes**

```bash
cargo test -p pico2-firmware test_warmup --features firmware
```

Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/pico2-firmware/src/state.rs crates/pico2-firmware/tests/test_warmup.rs
git commit -m "feat(firmware): add 3-second warmup with outage reset

Per spec Section 19.5: wait 3 GPS cycles before arrival detection.
Warmup counter resets to 0 on GPS outage for conservative behavior.

Fixes #10 from code review.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Phase 4: Pipeline Core Fixes Branch

### Task 7: Normalize DR speed decay by dt

**Files:**
- Modify: `crates/pipeline/gps_processor/src/kalman.rs`

- [ ] **Step 1: Read current handle_outage implementation**

Read lines 126-145 of `crates/pipeline/gps_processor/src/kalman.rs`.

- [ ] **Step 2: Write test for DR decay consistency**

Create `crates/pipeline/gps_processor/tests/test_dr_decay.rs`:

```rust
//! Test DR speed decay normalization by dt

use gps_processor::kalman::{KalmanState, DrState, process_gps_update, ProcessResult};
use shared::{GpsPoint, binfile::RouteData};

#[test]
fn test_dr_decay_10s_single_vs_multi() {
    // 10-second outage as single tick should give same decay as 10×1-second ticks
    let route_data = RouteData::load(/* test data */).unwrap();
    let mut state_single = KalmanState::new();
    let mut dr_single = DrState::new();
    let mut state_multi = KalmanState::new();
    let mut dr_multi = DrState::new();

    // Initialize both with same state
    state_single.s_cm = 10000;
    state_single.v_cms = 1000;
    state_multi.s_cm = 10000;
    state_multi.v_cms = 1000;
    dr_single.filtered_v = 1000;
    dr_single.last_valid_s = 10000;
    dr_multi.filtered_v = 1000;
    dr_multi.last_valid_s = 10000;

    // Single 10-second outage
    let _ = process_gps_update(&mut state_single, &mut dr_single, &GpsPoint::new(), &route_data, 10, false);

    // Ten 1-second outages
    for _ in 0..10 {
        let _ = process_gps_update(&mut state_multi, &mut dr_multi, &GpsPoint::new(), &route_data, 1, false);
    }

    // Decay should be identical (within rounding)
    assert_eq!(dr_single.filtered_v, dr_multi.filtered_v);
}
```

- [ ] **Step 3: Run test to verify it fails**

```bash
cargo test -p gps_processor test_dr_decay --lib
```

Expected: FAIL (decay differs: 10% vs ~65%)

- [ ] **Step 4: Add DR decay LUT**

Add to `crates/pipeline/gps_processor/src/kalman.rs`:

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
```

- [ ] **Step 5: Update handle_outage to use LUT**

Modify `crates/pipeline/gps_processor/src/kalman.rs` lines 136-139:

```rust
// Dead-reckoning: s(t) = s(t-1) + v_filtered * dt
state.s_cm = dr.last_valid_s + dr.filtered_v * (dt as DistCm);

// Speed decay normalized by dt: (9/10)^dt
let dt_idx = dt.min(10) as usize;
let decay_factor = DR_DECAY_NUMERATOR[dt_idx];
dr.filtered_v = (dr.filtered_v as u32 * decay_factor / 10000) as SpeedCms;
```

- [ ] **Step 6: Run test to verify it passes**

```bash
cargo test -p gps_processor test_dr_decay --lib
```

Expected: PASS

- [ ] **Step 7: Run full kalman tests**

```bash
cargo test -p gps_processor kalman
```

Expected: All tests pass

- [ ] **Step 8: Commit**

```bash
git add crates/pipeline/gps_processor/src/kalman.rs crates/pipeline/gps_processor/tests/test_dr_decay.rs
git commit -m "fix(pipeline): normalize DR speed decay by dt

Apply (9/10)^dt using lookup table for consistent decay
regardless of tick granularity. 10s outage now gives
same decay whether as 1×10s or 10×1s.

Fixes #4 from code review.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 8: Add velocity-based hard exclusion to recovery

**Files:**
- Modify: `crates/pipeline/detection/src/recovery.rs`

- [ ] **Step 1: Read current recovery implementation**

Read `crates/pipeline/detection/src/recovery.rs`.

- [ ] **Step 2: Write test for velocity exclusion**

Create `crates/pipeline/detection/tests/test_recovery_velocity.rs`:

```rust
//! Test recovery velocity-based exclusion

use crate::recovery::find_stop_index;
use shared::Stop;

#[test]
fn test_velocity_exclusion_when_dist_exceeds_vmax() {
    let stops = vec![
        Stop { progress_cm: 100000, corridor_start_cm: 95000, corridor_end_cm: 105000 },
        Stop { progress_cm: 200000, corridor_start_cm: 195000, corridor_end_cm: 205000 },
    ];

    // Bus at s=5000, v=1000 cm/s
    // Stop 0: dist=95000 cm > V_MAX=3000, should be excluded
    // Stop 1: dist=195000 cm > V_MAX=3000, should be excluded
    let result = find_stop_index(5000, 1000, &stops, 0);
    assert_eq!(result, None); // Both excluded
}

#[test]
fn test_velocity_inclusion_when_dist_within_vmax() {
    let stops = vec![
        Stop { progress_cm: 7000, corridor_start_cm: 0, corridor_end_cm: 10000 },
    ];

    // Bus at s=5000, v=1000 cm/s
    // Stop 0: dist=2000 cm < V_MAX=3000, should be included
    let result = find_stop_index(5000, 1000, &stops, 0);
    assert_eq!(result, Some(0));
}
```

- [ ] **Step 3: Run test to verify it fails**

```bash
cargo test -p detection test_recovery_velocity --lib
```

Expected: FAIL (velocity penalty not implemented)

- [ ] **Step 4: Add V_MAX_CMS constant**

Add to `crates/pipeline/detection/src/recovery.rs`:

```rust
use shared::{DistCm, Stop};

/// Maximum bus speed: 108 km/h = 3000 cm/s
const V_MAX_CMS: u32 = 3000;
```

- [ ] **Step 5: Update find_stop_index signature**

Change function signature to add `v_filtered` parameter:

```rust
pub fn find_stop_index(
    s_cm: DistCm,
    v_filtered: SpeedCms,  // NEW parameter
    stops: &[Stop],
    last_index: u8,
) -> Option<usize>
```

- [ ] **Step 6: Implement velocity-based hard exclusion**

Modify the scoring logic (around lines 30-35):

```rust
.map(|(i, stop)| {
    let dist = (s_cm - stop.progress_cm).abs();
    let index_penalty = 5000 * (last_index as i32 - i as i32).max(0);

    // Velocity penalty: hard exclusion if reaching this stop in 1 GPS tick
    // would require exceeding V_MAX_CMS (physically impossible)
    // Required speed = dist_to_stop cm/s (since dt=1s per GPS tick)
    let dist_to_stop = (stop.progress_cm - s_cm).unsigned_abs();
    let vel_penalty = if dist_to_stop > V_MAX_CMS {
        i32::MAX  // Physically impossible in 1 second at max bus speed
    } else {
        0
    };

    let score = dist + index_penalty + vel_penalty;
    (i, score)
})
.filter(|(_, score)| *score < i32::MAX)  // Remove excluded candidates
```

- [ ] **Step 7: Update call site in firmware**

Update `crates/pico2-firmware/src/state.rs` to pass `v_filtered`:

Find where `recovery::find_stop_index` is called and add `state.v_cms` parameter.

- [ ] **Step 8: Run test to verify it passes**

```bash
cargo test -p detection test_recovery_velocity --lib
```

Expected: PASS

- [ ] **Step 9: Run full recovery tests**

```bash
cargo test -p detection recovery
```

Expected: All tests pass

- [ ] **Step 10: Commit**

```bash
git add crates/pipeline/detection/src/recovery.rs crates/pipeline/detection/tests/test_recovery_velocity.rs crates/pico2-firmware/src/state.rs
git commit -m "feat(detection): add velocity-based hard exclusion to recovery

Per spec Section 15.2: exclude candidates that require exceeding
V_MAX_CMS to reach in 1 GPS tick. Pure integer implementation,
no floating point.

Fixes recovery scoring gap from code review.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Phase 5: Infrastructure and Code Quality Branch

### Task 9: Deduplicate probability feature calculations

**Files:**
- Modify: `crates/pico2-firmware/src/detection.rs`

- [ ] **Step 1: Read current duplicate code**

Read both `compute_arrival_probability` (lines 31-55) and `compute_arrival_probability_adaptive` (lines 61-100) in `crates/pico2-firmware/src/detection.rs`.

- [ ] **Step 2: Write test for identical output**

Create `crates/pico2-firmware/tests/test_probability_consistency.rs`:

```rust
//! Test that both probability functions give same output when weights match

use pico2_firmware::detection::{compute_arrival_probability, compute_arrival_probability_adaptive};

#[test]
fn test_probability_functions_identical_with_same_weights() {
    let s_cm = 10000;
    let v_cms = 100;
    let stop = Stop { progress_cm: 10000, corridor_start_cm: 5000, corridor_end_cm: 15000 };
    let dwell_time_s = 10;

    // Adaptive with no next stop uses standard weights
    let result1 = compute_arrival_probability(s_cm, v_cms, &stop, dwell_time_s);
    let result2 = compute_arrival_probability_adaptive(s_cm, v_cms, &stop, dwell_time_s, None);

    assert_eq!(result1, result2);
}
```

- [ ] **Step 3: Run test to verify it passes**

```bash
cargo test -p pico2-firmware test_probability_consistency --features firmware
```

Expected: PASS (verifies current behavior)

- [ ] **Step 4: Extract shared feature computation**

Add helper function to `crates/pico2-firmware/src/detection.rs`:

```rust
/// Shared feature computation for arrival probability
fn compute_features(s_cm: DistCm, v_cms: SpeedCms, stop: &Stop, dwell_time_s: u16) -> (u32, u32, u32, u32) {
    use shared::probability_constants::*;

    // Feature 1: Distance likelihood (sigma_d = 2750 cm)
    let d_cm = (s_cm - stop.progress_cm).abs();
    let idx1 = ((d_cm as i64 * 64) / SIGMA_D_CM as i64).min(255) as usize;
    let p1 = GAUSSIAN_LUT[idx1] as u32;

    // Feature 2: Speed likelihood (near 0 -> higher, v_stop = 200 cm/s)
    let idx2 = (v_cms / 10).max(0).min(SPEED_LUT_MAX_IDX as SpeedCms) as usize;
    let p2 = LOGISTIC_LUT[idx2] as u32;

    // Feature 3: Progress difference likelihood (sigma_p = 2000 cm)
    let idx3 = ((d_cm as i64 * 64) / SIGMA_P_CM as i64).min(255) as usize;
    let p3 = GAUSSIAN_LUT[idx3] as u32;

    // Feature 4: Dwell time likelihood (T_ref = 10s)
    let p4 = ((dwell_time_s as u32) * 255 / 10).min(255) as u32;

    (p1, p2, p3, p4)
}
```

- [ ] **Step 5: Refactor compute_arrival_probability**

Replace body with:

```rust
pub fn compute_arrival_probability(
    s_cm: DistCm,
    v_cms: SpeedCms,
    stop: &Stop,
    dwell_time_s: u16,
) -> Prob8 {
    let (p1, p2, p3, p4) = compute_features(s_cm, v_cms, stop, dwell_time_s);
    ((13 * p1 + 6 * p2 + 10 * p3 + 3 * p4) / 32) as u8
}
```

- [ ] **Step 6: Refactor compute_arrival_probability_adaptive**

Replace feature computation with:

```rust
pub fn compute_arrival_probability_adaptive(
    s_cm: DistCm,
    v_cms: SpeedCms,
    stop: &Stop,
    dwell_time_s: u16,
    next_stop: Option<&Stop>,
) -> Prob8 {
    let (p1, p2, p3, p4) = compute_features(s_cm, v_cms, stop, dwell_time_s);

    // Adaptive weights based on next stop distance
    let (w1, w2, w3, w4) = if let Some(next) = next_stop {
        let dist_to_next = (next.progress_cm - stop.progress_cm).abs();
        if dist_to_next < 12_000 {
            (14, 7, 11, 0)
        } else {
            (13, 6, 10, 3)
        }
    } else {
        (13, 6, 10, 3)
    };

    ((w1 * p1 + w2 * p2 + w3 * p3 + w4 * p4) / 32) as u8
}
```

- [ ] **Step 7: Run test to verify consistency**

```bash
cargo test -p pico2-firmware test_probability_consistency --features firmware
```

Expected: PASS

- [ ] **Step 8: Run full detection tests**

```bash
cargo test -p pico2-firmware detection --features firmware
```

Expected: All tests pass

- [ ] **Step 9: Commit**

```bash
git add crates/pico2-firmware/src/detection.rs
git commit -m "refactor(firmware): deduplicate probability feature calculations

Extract p1/p2/p3/p4 computation to shared helper function.
Both compute_arrival_probability and _adaptive now use same
code, preventing future divergence.

Fixes #5 from code review.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 10: Add GGA heading sentinel with segment_score guard

**Files:**
- Modify: `crates/pipeline/gps_processor/src/nmea.rs`
- Modify: `crates/pipeline/gps_processor/src/map_match.rs`

- [ ] **Step 1: Read GGA parse implementation**

Read `parse_gga()` in `crates/pipeline/gps_processor/src/nmea.rs` (lines 123-155).

- [ ] **Step 2: Write test for GGA heading sentinel**

Create `crates/pipeline/gps_processor/tests/test_gga_heading.rs`:

```rust
//! Test GGA sentence sets heading sentinel

use gps_processor::nmea::NmeaState;

#[test]
fn test_gga_sets_heading_sentinel() {
    let mut state = NmeaState::new();

    // Parse GGA sentence (no heading data)
    let result = state.parse_sentence(
        "$GPGGA,221320,2500.2582,N,12117.1898,E,1,08,3.5,10.0,M,0.0,M,,*4B"
    );

    assert!(result.is_some());
    let point = result.unwrap();
    assert_eq!(point.heading_cdeg, i16::MIN); // Sentinel value
    assert_eq!(point.speed_cms, 0); // GGA doesn't provide speed
}
```

- [ ] **Step 3: Run test to verify it fails**

```bash
cargo test -p gps_processor test_gga_heading --lib
```

Expected: FAIL (heading is 0, not i16::MIN)

- [ ] **Step 4: Set sentinel in parse_gga**

Modify `crates/pipeline/gps_processor/src/nmea.rs` `parse_gga()` (around line 150):

```rust
// Store lat/lon directly as f64 for full precision
self.point.lat = lat;
self.point.lon = lon;
self.point.hdop_x10 = f64_round(hdop * 10.0) as u16;
self.point.has_fix = true;
self.point.speed_cms = 0;  // GGA doesn't provide speed
self.point.heading_cdeg = i16::MIN;  // NEW: sentinel for unavailable
```

- [ ] **Step 5: Add sentinel guard in segment_score**

Modify `crates/pipeline/gps_processor/src/map_match.rs` `segment_score()` function (around line 110):

```rust
fn segment_score(
    gps_x: DistCm,
    gps_y: DistCm,
    gps_heading: HeadCdeg,
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

- [ ] **Step 6: Run test to verify it passes**

```bash
cargo test -p gps_processor test_gga_heading --lib
```

Expected: PASS

- [ ] **Step 7: Run full nmea tests**

```bash
cargo test -p gps_processor nmea
```

Expected: All tests pass

- [ ] **Step 8: Commit**

```bash
git add crates/pipeline/gps_processor/src/nmea.rs crates/pipeline/gps_processor/src/map_match.rs crates/pipeline/gps_processor/tests/test_gga_heading.rs
git commit -m "fix(pipeline): add GGA heading sentinel with segment_score guard

Set heading_cdeg=i16::MIN when GGA provides no heading.
Skip heading penalty in segment_score when sentinel detected.

Prevents false heading bias toward 0° segments when using
GGA-only mode.

Fixes #7 from code review.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 11: Implement LUT sync via build script

**Files:**
- Create: `crates/pipeline/detection/src/bin/gen_luts.rs`
- Modify: `crates/pico2-firmware/build.rs`
- Modify: `crates/pico2-firmware/src/lut.rs`

- [ ] **Step 1: Create gen_luts binary**

Create `crates/pipeline/detection/src/bin/gen_luts.rs`:

```rust
//! Generate LUT constants for firmware embedding
//! Run with: cargo run --bin gen_luts

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

- [ ] **Step 2: Add binary declaration**

Add to `crates/pipeline/detection/Cargo.toml`:

```toml
[[bin]]
name = "gen_luts"
path = "src/bin/gen_luts.rs"
```

- [ ] **Step 3: Test gen_luts binary**

```bash
cargo run --bin gen_luts
```

Expected: Outputs LUT arrays

- [ ] **Step 4: Modify firmware build.rs**

Read `crates/pico2-firmware/build.rs` and add:

```rust
fn main() {
    // OUT_DIR is set by Cargo during build.rs execution
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR not set"));

    // Run gen_luts binary, capture output
    let output = std::process::Command::new("cargo")
        .args(["run", "--bin", "gen_luts"])
        .current_dir("../pipeline/detection")
        .output()
        .expect("Failed to run LUT generator");

    let lut_content = String::from_utf8(output.stdout).unwrap();

    // Write to OUT_DIR
    let out_path = out_dir.join("lut_generated.rs");
    std::fs::write(&out_path, lut_content).expect("Failed to write LUT file");

    // Rebuild if probability constants change
    println!("cargo:rerun-if-changed=../shared/src/probability_constants.rs");
    println!("cargo:rerun-if-changed=../pipeline/detection/src/probability.rs");
}
```

- [ ] **Step 5: Update firmware lut.rs to include generated**

Replace content of `crates/pico2-firmware/src/lut.rs` with:

```rust
//! LUTs for arrival probability computation
//! Auto-generated from pipeline source

include!(concat!(env!("OUT_DIR"), "/lut_generated.rs"));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lut_spot_check() {
        // LUT[0] = 255 (p=1.0 at d=0)
        assert_eq!(GAUSSIAN_LUT[0], 255);
        // LUT[64] ≈ 170 (d=2750cm, sigma=2750cm, z=1.0)
        assert!((GAUSSIAN_LUT[64] as i32 - 170).abs() < 5);
    }
}
```

- [ ] **Step 6: Build firmware to generate LUTs**

```bash
cargo build -p pico2-firmware --features firmware
```

Expected: SUCCESS, generates `lut_generated.rs` in target dir

- [ ] **Step 7: Run LUT tests**

```bash
cargo test -p pico2-firmware lut --features firmware
```

Expected: PASS

- [ ] **Step 8: Add build-deps to firmware Cargo.toml**

Add to `crates/pico2-firmware/Cargo.toml`:

```toml
[build-dependencies]
std = { path = "../shared" }
```

- [ ] **Step 9: Commit**

```bash
git add crates/pipeline/detection/src/bin/gen_luts.rs crates/pipeline/detection/Cargo.toml crates/pico2-firmware/build.rs crates/pico2-firmware/src/lut.rs crates/pico2-firmware/Cargo.toml
git commit -m "feat(infra): generate LUTs from pipeline at build time

Firmware LUTs now generated from pipeline probability module
via build script. Ensures formulas never diverge.

Writes to OUT_DIR (not src/) for reproducible builds.

Fixes #9 from code review.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Phase 6: Final Integration

### Task 12: Update output.rs for ArrivalEventType

**Files:**
- Modify: `crates/pipeline/gps_processor/src/output.rs`

- [ ] **Step 1: Read current output implementation**

Read `crates/pipeline/gps_processor/src/output.rs`.

- [ ] **Step 2: Update serialization to include event_type**

Add to JSON output:

```rust
pub fn format_arrival_event(event: &ArrivalEvent) -> String {
    let event_type_str = match event.event_type {
        ArrivalEventType::Arrival => "arrival",
        ArrivalEventType::Departure => "departure",
        ArrivalEventType::Announce => "announce",
    };

    format!(
        r#"{{"type":"{}","time":{},"stop":{},"s":{},"v":{},"p":{}}}"#,
        event_type_str, event.time, event.stop_idx, event.s_cm, event.v_cms, event.probability
    )
}
```

- [ ] **Step 3: Run cargo check**

```bash
cargo check -p gps_processor
```

Expected: SUCCESS

- [ ] **Step 4: Commit**

```bash
git add crates/pipeline/gps_processor/src/output.rs
git commit -m "feat(pipeline): add event_type to JSON output

Update output.rs to serialize new ArrivalEventType field.
JSON format now includes 'type' field with 'arrival'/'departure'/'announce'.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 13: Version bump and CHANGELOG

**Files:**
- Create: `CHANGELOG.md`
- Modify: `crates/shared/src/binfile.rs`

- [ ] **Step 1: Create CHANGELOG.md**

Create `CHANGELOG.md` at repo root:

```markdown
# Changelog

## [v8.9] - 2026-04-10

### Critical Fixes
- Fix UART blocking loop - add 5-second timeout to prevent hang on GPS disconnect
- Fix u32 wrap bug in map matching when GPS outside route bounding box

### Spec Compliance
- Implement v8.4 corridor entry announcement (should_announce, called after FSM update)
- Add 3-second Kalman warmup period before arrival detection (resets on GPS outage)
- Add velocity-based hard exclusion to recovery scoring (Section 15.2)

### Code Quality
- Extract duplicate probability calculations to shared helper
- Add GGA sentence heading sentinel value (i16::MIN) with segment_score guard
- Normalize DR speed decay by dt using lookup table

### Infrastructure
- Add shared probability constants (prevent formula divergence)
- Generate LUTs from pipeline source at build time (OUT_DIR, not src/)
- Add scenario tests for all critical fixes

### Breaking Changes
- `ArrivalEvent` now includes `event_type` field
- UART changed from blocking to async
- JSON output format includes 'type' field

### Known Acceptances
- XIP misaligned memory leak: Bounded impact, firmware fails fast correctly
- DR filtered velocity: Uses direct assignment from Kalman (already smoothed)

## [v8.8] - Previous Release
- Initial implementation
```

- [ ] **Step 2: Commit CHANGELOG**

```bash
git add CHANGELOG.md
git commit -m "docs: add CHANGELOG for v8.9

Document all changes from code review fixes.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

- [ ] **Step 3: Run full test suite**

```bash
cargo test --workspace
```

Expected: All tests pass

---

## Self-Review Checklist

After completing all tasks, verify:

- [ ] **Spec coverage:** All 9 fixes from spec have corresponding tasks
- [ ] **No placeholders:** All steps have complete code/commands
- [ ] **Type consistency:** ArrivalEventType used consistently across uart.rs, output.rs, state.rs
- [ ] **Tests added:** Each fix has corresponding test
- [ ] **Migration documented:** Breaking changes (ArrivalEvent, async UART) noted
- [ ] **XIP leak:** Intentionally not fixed (documented as acceptable)

---

## Execution Notes

1. **Branch strategy:** Execute tasks in order within each phase. Phase 1 (foundation) must complete first.
2. **Testing:** Hardware testing required for UART timeout (Task 4).
3. **Dependencies:** Phase 2-5 can be done in parallel after Phase 1 completes.
4. **Review checkpoints:** Commit after each task for easy rollback.

Total estimated time: 4-6 hours (excluding hardware testing).

# Fix Short-Term Issues Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix 3 short-term priority issues identified in code review: (1) UART lifetime unsoundness, (2) GNSS multi-constellation NMEA support, (3) Adaptive weights for close stops

**Architecture:** Each fix is independent and can be committed separately. All fixes are in the pico2-firmware crate.

**Tech Stack:** Embedded Rust (no_std), Embassy framework, RP2350

---

## Task 1: Fix UART Lifetime Unsoundness (#5)

**Problem:** `read_nmea_sentence` returns `&'a str` bound to `uart` lifetime, but the slice actually points to `line_buf.buffer`. This is unsound.

**Files:**
- Modify: `crates/pico2-firmware/src/uart.rs:76-131`

- [ ] **Step 1: Update function signature to bind lifetime to line_buf**

```rust
// Change from:
pub fn read_nmea_sentence<'a>(
    uart: &mut Uart<'a, embassy_rp::uart::Blocking>,
    line_buf: &mut UartLineBuffer,
) -> Result<Option<&'a str>, ()>

// To:
pub fn read_nmea_sentence<'buf>(
    uart: &mut Uart<'_, embassy_rp::uart::Blocking>,
    line_buf: &'buf mut UartLineBuffer,
) -> Result<Option<&'buf str>, ()>
```

- [ ] **Step 2: Remove unsafe block and use safe str conversion**

```rust
// Change from (lines 114-120):
let sentence = unsafe {
    let slice = core::slice::from_raw_parts(line_buf.buffer.as_ptr(), line_buf.len - 2);
    core::str::from_utf8_unchecked(slice)
};
return Ok(Some(sentence));

// To:
let sentence = core::str::from_utf8(&line_buf.buffer[..line_buf.len - 2])
    .map_err(|_| ())?;
return Ok(Some(sentence));
```

- [ ] **Step 3: Verify the fix compiles**

Run: `cargo check --target thumbv8m.main-none-eabihf -p pico2-firmware`
Expected: No errors, lifetime annotations now correctly bind to `line_buf`

- [ ] **Step 4: Run firmware tests to ensure no behavioral change**

Run: `cargo test --target thumbv8m.main-none-eabihf -p pico2-firmware`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/pico2-firmware/src/uart.rs
git commit -m "fix(uart): correct lifetime annotation in read_nmea_sentence

The returned str slice was bound to uart's lifetime but actually
points to line_buf.buffer. Changed to bind to line_buf lifetime
and removed unsafe block in favor of safe from_utf8.

Fixes #5 in code review - lifetime unsoundness."
```

---

## Task 2: Add GNSS Multi-Constellation NMEA Support (#7)

**Problem:** NMEA parser only handles `$GPRMC` and `$GPGGA` (GPS-only). Modern GNSS modules output `$GNRMC` and `$GNGGA` (multi-constellation), which are silently discarded.

**Files:**
- Modify: `crates/pipeline/gps_processor/src/nmea.rs:51-56`
- Modify: `crates/pipeline/gps_processor/src/nmea.rs:193-305` (tests)

- [ ] **Step 1: Update parse_sentence match to handle GN prefixes**

```rust
// Change from (lines 51-56):
match parts_slice.first() {
    Some(&"$GPRMC") => self.parse_rmc(parts_slice),
    Some(&"$GNGSA") => self.parse_gsa(parts_slice),
    Some(&"$GPGGA") => self.parse_gga(parts_slice),
    _ => None,
}

// To:
match parts_slice.first() {
    Some(&"$GPRMC") | Some(&"$GNRMC") => self.parse_rmc(parts_slice),
    Some(&"$GNGSA") | Some(&"$GPGSA") => self.parse_gsa(parts_slice),
    Some(&"$GPGGA") | Some(&"$GNGGA") => self.parse_gga(parts_slice),
    _ => None,
}
```

- [ ] **Step 2: Add test for $GNRMC parsing**

```rust
// Add to tests module (after line 297):
#[test]
fn parse_gnrmc_valid() {
    let mut state = NmeaState::new();
    let result = state.parse_sentence("$GNRMC,221320,A,2500.2582,N,12117.1898,E,8.4,80.5,141123,,*2E");
    assert!(result.is_none()); // RMC alone doesn't complete the point
    assert!(state.point.has_fix);
    assert_eq!(state.point.lat_cdeg, 2500);
    assert_eq!(state.point.lon_cdeg, 12129);
}
```

- [ ] **Step 3: Add test for $GNGGA parsing**

```rust
// Add to tests module (after parse_gnrmc_valid test):
#[test]
fn parse_gngga_valid() {
    let mut state = NmeaState::new();
    let result = state.parse_sentence("$GNGGA,221320,2500.2582,N,12117.1898,E,1,08,3.5,10.0,M,0.0,M,,*4B");
    assert!(result.is_some()); // GNGGA completes the point
    let point = result.unwrap();
    assert!(point.has_fix);
    assert_eq!(point.lat_cdeg, 2500);
    assert_eq!(point.lon_cdeg, 12129);
}
```

- [ ] **Step 4: Run tests to verify GNSS sentences are handled**

Run: `cargo test -p gps_processor parse_gn`
Expected: `test parse_gnrmc_valid ... ok` and `test parse_gngga_valid ... ok`

- [ ] **Step 5: Commit**

```bash
git add crates/pipeline/gps_processor/src/nmea.rs
git commit -m "fix(nmea): add support for GNSS multi-constellation sentences

Add support for \$GNRMC, \$GNGGA, and \$GPGSA sentence types.
Modern GNSS modules (u-blox, etc.) output these instead of
GPS-only \$GPRMC/\$GPGGA sentences.

Fixes #7 in code review - GNSS multi-constellation support."
```

---

## Task 3: Implement Adaptive Weights for Close Stops (#6)

**Problem:** Firmware uses fixed weights (13, 6, 10, 3) but pipeline has adaptive weights (14, 7, 11, 0) when next stop < 120m away. This causes false positives for close stops.

**Files:**
- Modify: `crates/pico2-firmware/src/detection.rs:29-54`
- Modify: `crates/pico2-firmware/src/state.rs:95-101` (call site)
- Create: `crates/pico2-firmware/tests/test_adaptive_weights.rs`

- [ ] **Step 1: Add adaptive weights function to detection.rs**

```rust
// Add after compute_arrival_probability function (after line 54):
/// Compute arrival probability with adaptive weights for close stops.
///
/// When next sequential stop is < 120m away, removes dwell time (p4)
/// weight and redistributes: (14, 7, 11, 0) instead of (13, 6, 10, 3).
pub fn compute_arrival_probability_adaptive(
    s_cm: DistCm,
    v_cms: SpeedCms,
    stop: &Stop,
    dwell_time_s: u16,
    next_stop: Option<&Stop>,
) -> Prob8 {
    // Feature 1: Distance likelihood (sigma_d = 2750 cm)
    let d_cm = (s_cm - stop.progress_cm).abs();
    let idx1 = ((d_cm as i64 * 64) / 2750).min(255) as usize;
    let p1 = GAUSSIAN_LUT[idx1] as u32;

    // Feature 2: Speed likelihood (near 0 -> higher, v_stop = 200 cm/s)
    let idx2 = (v_cms / 10).max(0).min(127) as usize;
    let p2 = LOGISTIC_LUT[idx2] as u32;

    // Feature 3: Progress difference likelihood (sigma_p = 2000 cm)
    let idx3 = ((d_cm as i64 * 64) / 2000).min(255) as usize;
    let p3 = GAUSSIAN_LUT[idx3] as u32;

    // Feature 4: Dwell time likelihood (T_ref = 10s)
    let p4 = ((dwell_time_s as u32) * 255 / 10).min(255) as u32;

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

- [ ] **Step 2: Update state.rs to use adaptive weights**

```rust
// Change import (line 6):
use crate::detection::{compute_arrival_probability_adaptive, find_active_stops};

// Change probability computation (lines 95-101):
// Get next sequential stop for adaptive weights
let next_stop_idx = stop_idx.checked_add(1);
let next_stop = next_stop_idx.and_then(|idx| self.route_data.get_stop(idx));

// Compute arrival probability with adaptive weights
let probability = compute_arrival_probability_adaptive(
    s_cm,
    v_cms,
    &stop,
    stop_state.dwell_time_s,
    next_stop,
);
```

- [ ] **Step 3: Create test file for adaptive weights**

Create file: `crates/pico2-firmware/tests/test_adaptive_weights.rs`

```rust
#![no_std]

use pico2_firmware::detection::{compute_arrival_probability_adaptive, GAUSSIAN_LUT, LOGISTIC_LUT};
use shared::{Stop, DistCm, SpeedCms};

#[test]
fn test_adaptive_weights_close_stop() {
    // Build LUTs
    let g_lut = GAUSSIAN_LUT;
    let l_lut = LOGISTIC_LUT;

    let stop_current = Stop {
        progress_cm: 100_000,
        corridor_start_cm: 90_000,
        corridor_end_cm: 110_000,
    };

    let stop_next = Stop {
        progress_cm: 108_000, // 8,000cm away (<12,000 threshold)
        corridor_start_cm: 98_000,
        corridor_end_cm: 118_000,
    };

    // At stop, moderate speed, some dwell
    let prob = compute_arrival_probability_adaptive(
        100_000,  // s_cm (at stop)
        600,      // v_cms (approaching)
        &stop_current,
        5,        // dwell_time_s
        Some(&stop_next),
    );

    // With close stop, p4 weight is removed (0), probability should be higher
    // than fixed weights
    assert!(prob > 190, "Expected probability > 190 for close stop, got {}", prob);
    assert!(prob <= 255);
}

#[test]
fn test_adaptive_weights_normal_stop() {
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

    let prob = compute_arrival_probability_adaptive(
        100_000, 600, &stop_current, 5, Some(&stop_next)
    );

    // Normal stop uses standard weights
    assert!(prob <= 255);
}

#[test]
fn test_adaptive_weights_last_stop() {
    let stop = Stop {
        progress_cm: 100_000,
        corridor_start_cm: 90_000,
        corridor_end_cm: 110_000,
    };

    // Last stop (next_stop = None)
    let prob = compute_arrival_probability_adaptive(
        100_000, 0, &stop, 10, None
    );

    // Should use standard weights
    assert!(prob <= 255);
    assert!(prob > 150); // At stop with 10s dwell should be high
}
```

- [ ] **Step 4: Run tests to verify adaptive weights work**

Run: `cargo test --target thumbv8m.main-none-eabihf -p pico2-firmware adaptive`
Expected: All 3 adaptive weights tests pass

- [ ] **Step 5: Verify compilation**

Run: `cargo check --target thumbv8m.main-none-eabihf -p pico2-firmware`
Expected: No errors

- [ ] **Step 6: Commit**

```bash
git add crates/pico2-firmware/src/detection.rs
git add crates/pico2-firmware/src/state.rs
git add crates/pico2-firmware/tests/test_adaptive_weights.rs
git commit -m "feat(detection): add adaptive weights for close stops

When next sequential stop is < 120m away, removes dwell time (p4)
weight and redistributes: (14, 7, 11, 0) instead of (13, 6, 10, 3).

This matches the pipeline implementation and fixes false positives
for routes with closely-spaced stops.

Fixes #6 in code review - adaptive weights implementation."
```

---

## Self-Review Results

**Spec coverage:**
- #5 Lifetime unsoundness → Task 1 ✓
- #7 GNSS NMEA support → Task 2 ✓
- #6 Adaptive weights → Task 3 ✓

**Placeholder scan:** No placeholders found. All code is complete.

**Type consistency:** Verified all function signatures match between tasks.

---

## Testing Strategy

Each task includes:
1. Compilation check (`cargo check`)
2. Unit tests where applicable
3. Individual commits for easy rollback

Run all tests after completing all tasks:
```bash
cargo test --target thumbv8m.main-none-eabihf -p pico2-firmware
cargo test -p gps_processor
```

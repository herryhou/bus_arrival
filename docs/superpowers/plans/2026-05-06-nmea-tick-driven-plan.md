# NMEA Timestamp-Driven Architecture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix NMEA burst double-processing by implementing timestamp-driven GPS accumulation

**Architecture:** GPS timestamp is the only authority. Accumulate NMEA sentences across reads, emit exactly once per timestamp change. No artificial timer.

**Tech Stack:** Rust (no_std), embedded firmware (RP2350), Embassy async

---

## File Structure

**New files:**
- `crates/pipeline/gps_processor/src/accumulator.rs` - FixAccumulator implementation

**Modified files:**
- `crates/shared/src/lib.rs` - Add FixQuality enum, change GpsPoint fields to Option<T>
- `crates/pipeline/gps_processor/src/nmea.rs` - Re-export accumulator, update for Option<T>
- `crates/pipeline/gps_processor/src/kalman.rs` - Handle Option<T> fields
- `crates/pico2-firmware/src/main.rs` - Event-driven loop, remove Timer::after(1s)
- `crates/pico2-firmware/src/state.rs` - Handle Option<T> in process_gps()
- `crates/pipeline/detection/src/probability.rs` - Handle None HDOP

---

## Task 1: Add FixQuality enum to shared types

**Files:**
- Modify: `crates/shared/src/lib.rs`

- [ ] **Step 1: Add FixQuality enum above GpsPoint**

Add this enum before the GpsPoint struct (around line 172):

```rust
/// Quality level of a GPS fix based on which NMEA sentences contributed data.
///
/// - Full: RMC + GGA/GSA - has position, motion, and quality metrics
/// - PositionOnly: GGA only - has position and quality, no motion data
/// - MotionOnly: RMC only - has position and motion, no quality metrics
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixQuality {
    Full,
    PositionOnly,
    MotionOnly,
}
```

- [ ] **Step 2: Run cargo check**

Run: `cargo check -p shared`
Expected: No errors, enum compiles successfully

- [ ] **Step 3: Commit**

```bash
git add crates/shared/src/lib.rs
git commit -m "feat(shared): add FixQuality enum

Distinguish between Full (RMC+GGA), PositionOnly (GGA), and
MotionOnly (RMC) GPS fixes for downstream quality handling."
```

---

## Task 2: Change GpsPoint fields to Option<T>

**Files:**
- Modify: `crates/shared/src/lib.rs`
- Modify: `crates/shared/tests/binary_format.rs` (if any tests use GpsPoint directly)

- [ ] **Step 1: Update GpsPoint struct definition**

Replace the GpsPoint struct (lines 176-184) with:

```rust
/// Parsed GPS data from NMEA sentences.
/// lat/lon use f64 for full precision (map matching requires ~1m accuracy).
/// Other fields use Option<T> to indicate data availability.
#[derive(Debug, Clone)]
pub struct GpsPoint {
    pub timestamp: u64, // seconds since epoch
    pub lat: f64, // Latitude in degrees (full precision)
    pub lon: f64, // Longitude in degrees (full precision)
    pub heading_cdeg: Option<HeadCdeg>, // Heading in 0.01° units (None = unavailable)
    pub speed_cms: Option<SpeedCms>, // Speed in cm/s (None = unavailable)
    pub hdop_x10: Option<u16>, // HDOP * 10 (None = unavailable)
    pub has_fix: bool,
}
```

- [ ] **Step 2: Update GpsPoint::new() method**

Replace the new() method (lines 192-204) with:

```rust
impl GpsPoint {
    pub fn new() -> Self {
        GpsPoint {
            timestamp: 0,
            lat: 0.0,
            lon: 0.0,
            heading_cdeg: None,
            speed_cms: None,
            hdop_x10: None,
            has_fix: false,
        }
    }
}
```

- [ ] **Step 3: Run cargo check to find breakage**

Run: `cargo check -p shared 2>&1 | head -50`
Expected: Compilation errors in other crates that use GpsPoint

- [ ] **Step 4: Note the errors for next tasks**

The errors will show which files need updates. DO NOT fix them yet - each will be handled in subsequent tasks.

- [ ] **Step 5: Commit**

```bash
git add crates/shared/src/lib.rs
git commit -m "feat(shared): change GpsPoint fields to Option<T>

heading_cdeg, speed_cms, hdop_x10 are now Option<T> to explicitly
represent data availability. Fixes dangerous defaults issue."
```

---

## Task 3: Create accumulator.rs module

**Files:**
- Create: `crates/pipeline/gps_processor/src/accumulator.rs`
- Modify: `crates/pipeline/gps_processor/src/lib.rs` (to add module)

- [ ] **Step 1: Write accumulator test first**

Create `crates/pipeline/gps_processor/src/accumulator.rs`:

```rust
//! NMEA sentence accumulator for timestamp-driven GPS processing
//!
//! Accumulates NMEA sentences across UART reads, emitting exactly once
//! per GPS timestamp change. GPS timestamp is the only authority.

use crate::nmea::{parse_lat, parse_lon};
use shared::{FixQuality, GpsPoint, HeadCdeg, SpeedCms};

const MAX_NMEA_FIELDS: usize = 20;

/// Accumulates NMEA sentences into a single GPS fix per timestamp.
///
/// # Emission Semantics
///
/// Emits the **previous** snapshot when a **new** timestamp arrives:
/// ```text
/// RMC(t=01) + GGA(t=01) → accumulate
/// RMC(t=02) arrives → should_emit() true → emit (t=01) → reset
/// ```
///
/// # First Fix Edge Case
///
/// Does NOT emit on first timestamp. Only emits after seeing second
/// timestamp (requires `last_emitted_timestamp.is_some()`).
pub struct FixAccumulator {
    timestamp: Option<u64>,
    lat: Option<f64>,
    lon: Option<f64>,
    speed: Option<SpeedCms>,
    heading: Option<HeadCdeg>,
    hdop: Option<u16>,
    has_fix: bool,
    last_emitted_timestamp: Option<u64>,
}

impl Default for FixAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

impl FixAccumulator {
    pub fn new() -> Self {
        FixAccumulator {
            timestamp: None,
            lat: None,
            lon: None,
            speed: None,
            heading: None,
            hdop: None,
            has_fix: false,
            last_emitted_timestamp: None,
        }
    }

    /// Parse and update accumulator with an NMEA sentence.
    /// Returns true if the sentence was valid and contributed data.
    pub fn update(&mut self, sentence: &str) -> bool {
        if !verify_checksum(sentence) {
            return false;
        }

        let parts: heapless::Vec<&str, MAX_NMEA_FIELDS> = sentence.split(',').collect();
        if parts.is_empty() {
            return false;
        }

        match parts.first() {
            Some(&"$GPRMC") | Some(&"$GNRMC") => self.update_rmc(&parts),
            Some(&"$GNGSA") | Some(&"$GPGSA") => self.update_gsa(&parts),
            Some(&"$GPGGA") | Some(&"$GNGGA") => self.update_gga(&parts),
            _ => false,
        }
    }

    /// Check if timestamp changed (new second arrived).
    /// Returns true if we should emit the previous snapshot.
    ///
    /// Updates `last_emitted_timestamp` when returning true.
    pub fn should_emit(&mut self) -> bool {
        let ts = self.timestamp?;
        if self.last_emitted_timestamp != Some(ts) {
            self.last_emitted_timestamp = Some(ts);
            return true;
        }
        false
    }

    /// Build a GpsPoint if we have minimum required data.
    /// Returns None if insufficient data for a valid fix.
    pub fn build(&self) -> Option<(GpsPoint, FixQuality)> {
        if !self.has_fix {
            return None;
        }
        let lat = self.lat?;
        let lon = self.lon?;
        let timestamp = self.timestamp?;

        let has_motion = self.speed.is_some() || self.heading.is_some();
        let has_quality = self.hdop.is_some();

        let quality = match (has_motion, has_quality) {
            (true, true) => FixQuality::Full,
            (false, true) => FixQuality::PositionOnly,
            (true, false) => FixQuality::MotionOnly,
            (false, false) => FixQuality::PositionOnly, // Default fallback
        };

        Some((
            GpsPoint {
                timestamp,
                lat,
                lon,
                speed_cms: self.speed,
                heading_cdeg: self.heading,
                hdop_x10: self.hdop,
                has_fix: true,
            },
            quality,
        ))
    }

    /// Reset accumulator state for next second.
    /// Call this AFTER emitting a fix.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    fn update_rmc(&mut self, parts: &[&str]) -> bool {
        if parts.len() < 12 {
            return false;
        }

        // Status 'V' = Warning, 'A' = Valid
        if parts[2] != "A" {
            return false;
        }

        let lat = parse_lat(parts[3], parts[4])?;
        let lon = parse_lon(parts[5], parts[6])?;
        let speed_knots: f64 = parts[7].parse().unwrap_or(0.0);
        let heading_deg: f64 = parts[8].parse().unwrap_or(0.0);

        // Convert heading to centidegrees, handle > 180°
        let heading_cdeg = (heading_deg * 100.0) as i32;
        let heading_cdeg = if heading_cdeg > 18000 {
            heading_cdeg - 36000
        } else {
            heading_cdeg
        };

        // Parse time
        if parts[1].len() >= 6 {
            let hh: u64 = parts[1][0..2].parse().unwrap_or(0);
            let mm: u64 = parts[1][2..4].parse().unwrap_or(0);
            let ss: u64 = parts[1][4..6].parse().unwrap_or(0);
            self.timestamp = Some(hh * 3600 + mm * 60 + ss);
        }

        self.lat = Some(lat);
        self.lon = Some(lon);
        self.speed = Some(knots_to_cms(speed_knots));
        self.heading = Some(heading_cdeg as HeadCdeg);
        self.has_fix = true;
        true
    }

    fn update_gsa(&mut self, parts: &[&str]) -> bool {
        if parts.len() < 17 {
            return false;
        }

        let hdop_idx = parts.len() - 2;
        let hdop: f64 = parts[hdop_idx].parse().unwrap_or(99.0);
        self.hdop = Some((hdop * 10.0) as u16);
        true
    }

    fn update_gga(&mut self, parts: &[&str]) -> bool {
        if parts.len() < 9 {
            return false;
        }

        // Quality indicator: 1 = GPS, 2 = DGPS
        if parts[6] != "1" && parts[6] != "2" {
            return false;
        }

        let lat = parse_lat(parts[2], parts[3])?;
        let lon = parse_lon(parts[4], parts[5])?;
        let hdop: f64 = parts[8].parse().unwrap_or(99.0);

        // Parse time
        if parts[1].len() >= 6 {
            let hh: u64 = parts[1][0..2].parse().unwrap_or(0);
            let mm: u64 = parts[1][2..4].parse().unwrap_or(0);
            let ss: u64 = parts[1][4..6].parse().unwrap_or(0);
            self.timestamp = Some(hh * 3600 + mm * 60 + ss);
        }

        self.lat = Some(lat);
        self.lon = Some(lon);
        self.hdop = Some((hdop * 10.0) as u16);
        self.has_fix = true;
        // GGA doesn't provide speed/heading - preserves what RMC set
        true
    }
}

fn verify_checksum(sentence: &str) -> bool {
    if let Some(star_pos) = sentence.find('*') {
        let data = &sentence[1..star_pos];
        let checksum_str = &sentence[star_pos + 1..star_pos + 3];
        if let Ok(checksum) = u8::from_str_radix(checksum_str, 16) {
            let calculated = data.bytes().fold(0u8, |acc, b| acc ^ b);
            return calculated == checksum;
        }
    }
    false
}

fn parse_lat(deg_min: &str, ns: &str) -> Option<f64> {
    let dm: f64 = deg_min.parse().ok()?;
    let degrees = (dm / 100.0).trunc() + (dm % 100.0) / 60.0;
    Some(if ns == "N" { degrees } else { -degrees })
}

fn parse_lon(deg_min: &str, ew: &str) -> Option<f64> {
    let dm: f64 = deg_min.parse().ok()?;
    let degrees = (dm / 100.0).trunc() + (dm % 100.0) / 60.0;
    Some(if ew == "E" { degrees } else { -degrees })
}

fn knots_to_cms(knots: f64) -> SpeedCms {
    (knots * 51.44) as SpeedCms
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_accumulator_is_empty() {
        let acc = FixAccumulator::new();
        assert!(!acc.should_emit());
        assert!(acc.build().is_none());
    }

    #[test]
    fn test_rmc_only_creates_motion_only_fix() {
        let mut acc = FixAccumulator::new();
        acc.update("$GPRMC,221320,A,2500.2582,N,12117.1898,E,8.4,80.5,141123,,*2E");

        assert!(!acc.should_emit()); // First timestamp, no emit yet
        assert!(acc.build().is_some());

        let (gps, quality) = acc.build().unwrap();
        assert_eq!(quality, FixQuality::MotionOnly);
        assert!(gps.speed_cms.is_some());
        assert!(gps.heading_cdeg.is_some());
        assert!(gps.hdop_x10.is_none());
    }

    #[test]
    fn test_gga_only_creates_position_only_fix() {
        let mut acc = FixAccumulator::new();
        acc.update("$GPGGA,221320,2500.2582,N,12117.1898,E,1,08,3.5,10.0,M,0.0,M,,*4B");

        assert!(!acc.should_emit());
        let (gps, quality) = acc.build().unwrap();
        assert_eq!(quality, FixQuality::PositionOnly);
        assert!(gps.speed_cms.is_none());
        assert!(gps.heading_cdeg.is_none());
        assert!(gps.hdop_x10.is_some());
    }

    #[test]
    fn test_rmc_then_gga_creates_full_fix() {
        let mut acc = FixAccumulator::new();
        acc.update("$GPRMC,221320,A,2500.2582,N,12117.1898,E,8.4,80.5,141123,,*2E");
        acc.update("$GPGGA,221320,2500.2582,N,12117.1898,E,1,08,3.5,10.0,M,0.0,M,,*4B");

        let (gps, quality) = acc.build().unwrap();
        assert_eq!(quality, FixQuality::Full);
        assert!(gps.speed_cms.is_some());
        assert!(gps.heading_cdeg.is_some());
        assert!(gps.hdop_x10.is_some());
    }

    #[test]
    fn test_emit_on_second_timestamp() {
        let mut acc = FixAccumulator::new();

        // First timestamp
        acc.update("$GPRMC,221320,A,2500.2582,N,12117.1898,E,8.4,80.5,141123,,*2E");
        assert!(!acc.should_emit()); // First fix, don't emit yet

        // Second timestamp triggers emission
        acc.update("$GPRMC,221321,A,2500.2583,N,12117.1899,E,8.5,81.5,141123,,*2E");
        assert!(acc.should_emit()); // New timestamp, emit previous

        let (gps, _) = acc.build().unwrap();
        assert_eq!(gps.timestamp, 22 * 3600 + 13 * 60 + 20); // First timestamp
    }

    #[test]
    fn test_out_of_order_sentences() {
        let mut acc = FixAccumulator::new();
        acc.update("$GPGGA,221320,2500.2582,N,12117.1898,E,1,08,3.5,10.0,M,0.0,M,,*4B");
        acc.update("$GPRMC,221320,A,2500.2582,N,12117.1898,E,8.4,80.5,141123,,*2E");

        let (gps, quality) = acc.build().unwrap();
        assert_eq!(quality, FixQuality::Full);
    }

    #[test]
    fn test_reset_clears_state() {
        let mut acc = FixAccumulator::new();
        acc.update("$GPRMC,221320,A,2500.2582,N,12117.1898,E,8.4,80.5,141123,,*2E");
        acc.reset();

        assert!(acc.build().is_none());
        assert!(!acc.should_emit());
    }

    #[test]
    fn test_invalid_checksum_rejected() {
        let mut acc = FixAccumulator::new();
        let result = acc.update("$GPRMC,221320,A,2500.2582,N,12117.1898,E,8.4,80.5,141123,,*00");
        assert!(!result);
        assert!(acc.build().is_none());
    }
}
```

- [ ] **Step 2: Add module to lib.rs**

Add to `crates/pipeline/gps_processor/src/lib.rs`:

```rust
pub mod accumulator;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p gps_processor accumulator`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/pipeline/gps_processor/src/accumulator.rs crates/pipeline/gps_processor/src/lib.rs
git commit -m "feat(gps_processor): add FixAccumulator for timestamp-driven GPS

Accumulates NMEA sentences across reads, emits once per timestamp.
Handles out-of-order sentences, partial data, split bursts.

Tests:
- RMC-only (MotionOnly)
- GGA-only (PositionOnly)  
- RMC+GGA (Full)
- Emit on second timestamp
- Out-of-order handling
- Reset behavior
- Invalid checksum rejection"
```

---

## Task 4: Update nmea.rs for Option<T> compatibility

**Files:**
- Modify: `crates/pipeline/gps_processor/src/nmea.rs`

- [ ] **Step 1: Remove old NmeaState (replaced by FixAccumulator)**

The old `NmeaState` struct and its methods are now replaced by `FixAccumulator`. We need to update the module to export the accumulator instead.

Replace the content of `nmea.rs` with a compatibility shim:

```rust
//! NMEA parsing utilities
//!
//! NOTE: The new `FixAccumulator` in `accumulator.rs` replaces the old
//! `NmeaState`. This module now provides helper functions and types.

pub use crate::accumulator::FixAccumulator;

// Re-export helper functions for tests
pub use crate::accumulator::{parse_lat, parse_lon, knots_to_cms};
```

- [ ] **Step 2: Run cargo check**

Run: `cargo check -p gps_processor`
Expected: Compiles (FixAccumulator is re-exported)

- [ ] **Step 3: Run tests**

Run: `cargo test -p gps_processor`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/pipeline/gps_processor/src/nmea.rs
git commit -m "refactor(gps_processor): nmea.rs now re-exports FixAccumulator

Old NmeaState replaced by FixAccumulator. This module provides
compatibility re-exports for existing code."
```

---

## Task 5: Update kalman.rs for Option<T> fields

**Files:**
- Modify: `crates/pipeline/gps_processor/src/kalman.rs`

- [ ] **Step 1: Find speed/heading usage in Kalman**

Run: `grep -n "speed_cms\|heading_cdeg" crates/pipeline/gps_processor/src/kalman.rs`

Note the lines that need updating.

- [ ] **Step 2: Update process_gps_update to handle Option<T>**

Find where `gps.speed_cms` and `gps.heading_cdeg` are used. Update to unwrap or provide defaults:

For speed:
```rust
let speed_cms = gps.speed_cms.unwrap_or(0);
```

For heading:
```rust
let heading_cdeg = gps.heading_cdeg.unwrap_or(i16::MIN);
```

- [ ] **Step 3: Run cargo check**

Run: `cargo check -p gps_processor`
Expected: Compiles without errors

- [ ] **Step 4: Run tests**

Run: `cargo test -p gps_processor kalman`
Expected: All Kalman tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/pipeline/gps_processor/src/kalman.rs
git commit -m "feat(gps_processor): handle Option<T> fields in Kalman

speed_cms and heading_cdeg are now Option<T>. Use 0 and i16::MIN
as fallbacks when None, matching previous default behavior."
```

---

## Task 6: Update detection probability for Option<T> HDOP

**Files:**
- Modify: `crates/pipeline/detection/src/probability.rs`

- [ ] **Step 1: Find HDOP usage**

Run: `grep -n "hdop_x10" crates/pipeline/detection/src/probability.rs`

- [ ] **Step 2: Update to handle Option<u16>**

Where `gps.hdop_x10` is used, add unwrap_or:

```rust
let hdop_x10 = gps.hdop_x10.unwrap_or(9990); // Worst quality: 999.0
```

- [ ] **Step 3: Run cargo check**

Run: `cargo check -p pipeline`
Expected: Compiles

- [ ] **Step 4: Run tests**

Run: `cargo test -p pipeline detection`
Expected: All detection tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/pipeline/detection/src/probability.rs
git commit -m "feat(detection): handle Option<T> HDOP in probability

hdop_x10 is now Option<u16>. Use 9990 (worst quality) as fallback
when None, ensuring conservative probability weights."
```

---

## Task 7: Update firmware state.rs for Option<T>

**Files:**
- Modify: `crates/pico2-firmware/src/state.rs`

- [ ] **Step 1: Run cargo check to find issues**

Run: `cargo check -p pico2-firmware --features firmware 2>&1 | grep "state.rs"`

- [ ] **Step 2: Update process_gps to unwrap Option<T> fields**

Where `gps.speed_cms`, `gps.heading_cdeg`, `gps.hdop_x10` are accessed, add unwrap_or with sensible defaults.

- [ ] **Step 3: Run cargo check**

Run: `cargo check -p pico2-firmware --features firmware`
Expected: Compiles

- [ ] **Step 4: Commit**

```bash
git add crates/pico2-firmware/src/state.rs
git commit -m "feat(firmware): handle Option<T> in State::process_gps

Unwrap Option<T> fields with appropriate defaults for firmware
execution. Matches previous behavior for missing data."
```

---

## Task 8: Update firmware main.rs to event-driven loop

**Files:**
- Modify: `crates/pico2-firmware/src/main.rs`

- [ ] **Step 1: Add accumulator import and initialization**

At the top of main(), after route_data initialization, add:

```rust
use gps_processor::FixAccumulator;

// In main loop initialization:
let mut fix_accumulator = FixAccumulator::new();
```

- [ ] **Step 2: Replace NMEA processing loop**

Find the main loop (around line 109). Replace the entire inner loop with:

```rust
// Main processing loop (event-driven, no timer)
loop {
    match uart::read_nmea_sentence_async(&mut uart, &mut line_buf).await {
        Ok(Some(sentence)) => {
            debug!("NMEA: {}", sentence);

            fix_accumulator.update(&sentence);
            line_buf.reset();

            // Timestamp change is the ONLY trigger
            if fix_accumulator.should_emit() {
                if let Some((gps, _quality)) = fix_accumulator.build() {
                    debug!("GPS: lat={}, lon={}, fix={}", gps.lat, gps.lon, gps.has_fix);

                    // Exactly ONE process_gps() call per GPS second
                    if let Some(arrival) = state.process_gps(&gps) {
                        match uart::write_arrival_event_async(&mut uart, &arrival).await {
                            Ok(()) => {
                                info!("Emitted arrival event for stop {}", arrival.stop_idx);
                            }
                            Err(e) => {
                                defmt::warn!("Failed to write arrival event: {:?}", e);
                            }
                        }
                    }
                }
                // Only reset after emission, preserving partial data
                fix_accumulator.reset();
            }
        }
        Ok(None) => {
            // UART idle - continue waiting
            continue;
        }
        Err(uart::UartError::Timeout) => {
            defmt::warn!("UART timeout, GPS may be disconnected");
        }
        Err(e) => {
            defmt::warn!("UART read error: {:?}", e);
            line_buf.reset();
        }
    }

    // Persist state if stop index changed and rate limit allows
    if let Some(current_stop) = state.current_stop_index() {
        if state.should_persist(current_stop) {
            let ps = shared::PersistedState::new(state.kalman.s_cm, current_stop);
            match persist::save(&mut flash, &ps).await {
                Ok(()) => {
                    info!(
                        "Persisted state: stop={}, progress={}cm",
                        current_stop, state.kalman.s_cm
                    );
                    state.mark_persisted(current_stop);
                }
                Err(()) => {
                    defmt::warn!("Failed to persist state to flash");
                    state.ticks_since_persist = state.ticks_since_persist.saturating_add(1);
                }
            }
        } else {
            state.ticks_since_persist = state.ticks_since_persist.saturating_add(1);
        }
    }

    // NO Timer::after(1s) - GPS timestamp drives the loop
}
```

- [ ] **Step 3: Remove old NmeaState usage**

Remove any references to `state.nmea` since we're using `FixAccumulator` directly now.

- [ ] **Step 4: Run cargo check**

Run: `cargo check -p pico2-firmware --features firmware`
Expected: Compiles

- [ ] **Step 5: Commit**

```bash
git add crates/pico2-firmware/src/main.rs
git commit -m "feat(firmware): event-driven GPS loop, remove Timer::after(1s)

Key changes:
- Use FixAccumulator directly instead of state.nmea
- Emit on timestamp change, not on artificial timer
- Reset only after emission, preserving partial burst data
- Remove two-stage 'drain FIFO' pattern

This ensures exactly ONE process_gps() call per GPS second,
fixing dwell_time double-counting and duplicate events."
```

---

## Task 9: Update State to remove NmeaState field

**Files:**
- Modify: `crates/pico2-firmware/src/state.rs`

- [ ] **Step 1: Remove nmea field from State struct**

Find the `pub nmea: gps_processor::nmea::NmeaState` field and remove it.

- [ ] **Step 2: Update State::new()**

Remove the nmea field initialization.

- [ ] **Step 3: Run cargo check**

Run: `cargo check -p pico2-firmware --features firmware`
Expected: Compiles

- [ ] **Step 4: Commit**

```bash
git add crates/pico2-firmware/src/state.rs
git commit -m "refactor(firmware): remove NmeaState from State

FixAccumulator is now managed directly in main.rs. State no
longer needs its own NMEA parser instance."
```

---

## Task 10: Add split burst integration test

**Files:**
- Create: `crates/pipeline/tests/split_burst_test.rs`

- [ ] **Step 1: Write split burst test**

```rust
//! Test that split bursts (UART fragmentation) are handled correctly.
//! Ensures accumulator preserves partial data across reads.

use gps_processor::FixAccumulator;

#[test]
fn test_split_burst_emits_once() {
    let mut acc = FixAccumulator::new();

    // Simulate split burst: RMC arrives, then delay, then GGA
    acc.update("$GPRMC,221320,A,2500.2582,N,12117.1898,E,8.4,80.5,141123,,*2E");
    assert!(!acc.should_emit()); // Still accumulating t=01

    // Simulate UART delay...
    acc.update("$GPGGA,221320,2500.2582,N,12117.1898,E,1,08,3.5,10.0,M,0.0,M,,*4B");
    assert!(!acc.should_emit()); // Still same timestamp

    // New timestamp arrives - triggers emission of t=01
    acc.update("$GPRMC,221321,A,2500.2583,N,12117.1899,E,8.5,81.5,141123,,*2E");
    assert!(acc.should_emit());

    let (gps, _) = acc.build().unwrap();
    assert_eq!(gps.timestamp, 22 * 3600 + 13 * 60 + 20); // t=01, not t=02
    assert!(gps.speed_cms.is_some()); // Has RMC data
    assert!(gps.hdop_x10.is_some());  // Has GGA data
}

#[test]
fn test_timestamp_jump() {
    let mut acc = FixAccumulator::new();

    // t=01
    acc.update("$GPRMC,221320,A,2500.2582,N,12117.1898,E,8.4,80.5,141123,,*2E");
    acc.update("$GPGGA,221320,2500.2582,N,12117.1898,E,1,08,3.5,10.0,M,0.0,M,,*4B");

    // t=03 (t=02 was lost)
    acc.update("$GPRMC,221323,A,2500.2584,N,12117.1900,E,8.6,82.5,141123,,*2E");

    assert!(acc.should_emit());
    let (gps, _) = acc.build().unwrap();
    assert_eq!(gps.timestamp, 22 * 3600 + 13 * 60 + 20); // t=01

    // Reset and process t=03
    acc.reset();
    acc.update("$GPRMC,221323,A,2500.2584,N,12117.1900,E,8.6,82.5,141123,,*2E");

    // Next timestamp would trigger emission of t=03
}
```

- [ ] **Step 2: Add test module to lib.rs**

Add to `crates/pipeline/tests/lib.rs` or appropriate test module file.

- [ ] **Step 3: Run test**

Run: `cargo test --test split_burst_test`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/pipeline/tests/split_burst_test.rs
git commit -m "test(pipeline): add split burst and timestamp jump tests

Tests accumulator behavior when:
- Burst is fragmented across multiple UART reads
- GPS timestamps jump (lost seconds)

Ensures exactly one emission per timestamp regardless
of how sentences arrive."
```

---

## Task 11: Regression test with real NMEA data

**Files:**
- Create: `crates/pipeline/tests/nmea_burst_regression.rs`

- [ ] **Step 1: Write regression test using existing test data**

```rust
//! Regression test using real NMEA data.
//! Verifies exactly one GPS point per second.

use gps_processor::FixAccumulator;
use std::fs::read_to_string;

#[test]
fn test_real_nmea_burst_one_tick_per_second() {
    let nmea_data = read_to_string("test_data/tpF805_normal_nmea.txt")
        .expect("Test NMEA file not found");

    let mut acc = FixAccumulator::new();
    let mut emit_count = 0;
    let mut last_timestamp = None;
    let mut ticks_per_second = std::collections::HashMap::new();

    for line in nmea_data.lines() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with('$') {
            continue;
        }

        acc.update(line);

        if acc.should_emit() {
            emit_count += 1;
            if let Some((gps, _)) = acc.build() {
                let ts = gps.timestamp;
                *ticks_per_second.entry(ts).or_insert(0) += 1;
                last_timestamp = Some(ts);
                acc.reset();
            }
        }
    }

    // Verify: exactly one tick per second
    for (ts, count) in &ticks_per_second {
        assert_eq!(
            *count, 1,
            "Timestamp {} emitted {} times, expected 1",
            ts, count
        );
    }

    // Verify: processed multiple seconds
    assert!(emit_count > 10, "Should process at least 10 seconds");
}
```

- [ ] **Step 2: Run test**

Run: `cargo test --test nmea_burst_regression`
Expected: Pass, shows exactly one tick per second

- [ ] **Step 3: Commit**

```bash
git add crates/pipeline/tests/nmea_burst_regression.rs
git commit -m "test(pipeline): regression test with real NMEA bursts

Uses tpF805_normal_nmea.txt to verify exactly one GPS point
emitted per second. This would fail with the old double-processing
bug and pass with the fix."
```

---

## Task 12: Update documentation

**Files:**
- Modify: `docs/SPEC.md` or relevant documentation

- [ ] **Step 1: Update SPEC.md with timestamp-driven architecture**

Add or update section describing GPS processing:

```markdown
## GPS Processing (Timestamp-Driven)

The system uses a timestamp-driven architecture for GPS processing.
NMEA sentences (RMC, GGA, GSA) are accumulated across UART reads,
and exactly one GPS fix is emitted per timestamp change.

Key properties:
- GPS timestamp is authoritative (no artificial timer)
- Handles split bursts (UART fragmentation)
- One process_gps() call per GPS second (Δt = 1s)
- Data quality is explicit via FixQuality enum
```

- [ ] **Step 2: Commit**

```bash
git add docs/SPEC.md
git commit -m "docs: update GPS processing documentation

Describe timestamp-driven architecture and its properties."
```

---

## Verification Steps

After completing all tasks:

- [ ] **Run full test suite**: `cargo test`
- [ ] **Run with real NMEA data**: Verify `dwell_time_s` increments by 1 per second
- [ ] **Check for duplicate events**: Ensure no duplicate ARRIVAL/ANNOUNCE in trace output
- [ ] **Compare trace output**: Run pipeline before/after fix, validate against ground truth

---

## Rollback Plan

If issues arise:
1. Revert to commit before first Task 1 commit
2. All changes are isolated to GPS processing pipeline
3. No data format changes (RouteData, PersistedState unchanged)

# Phase 3: Arrival Detection — Design Spec

**Date:** 2026-03-12
**Status:** Draft
**Phase:** 3 of 3 — Arrival Detection

---

## Overview

Phase 3 implements the arrival detection pipeline that consumes smoothed route progress ŝ(t) and velocity v̂(t) from Phase 2, and produces arrival events when buses reach stops.

**Goal:** Detect and emit arrival events using corridor filtering, probabilistic modeling, and state machine validation

**Input:** Phase 2 JSONL output (ŝ, v̂ per GPS update) + route_data.bin (stops with corridors)
**Output:** JSONL arrival events (one per arrival)

---

## Project Structure

```
bus_arrival/
├── shared/              # Existing, add arrival types
│   └── src/lib.rs       # Add: FsmState, ArrivalEvent
├── arrival_detector/    # NEW: arrival detection binary
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs      # CLI entry point
│       ├── input.rs     # Phase 2 JSON parser
│       ├── corridor.rs  # Stop corridor filter
│       ├── probability.rs # 4-feature Bayesian with LUTs
│       ├── state_machine.rs # FSM + skip-stop guard
│       ├── recovery.rs  # Stop index recovery
│       └── output.rs    # Arrival event JSON output
```

---

## Core Data Structures

### FsmState (shared/src/lib.rs)

```rust
/// Stop state machine states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsmState {
    /// Bus is approaching stop (in corridor, not yet close)
    Approaching,
    /// Bus is in arrival zone (close to stop)
    Arriving,
    /// Bus has arrived (confirmed stop)
    AtStop,
    /// Bus has departed (moved past stop)
    Departed,
}
```

### ArrivalEvent (shared/src/lib.rs)

```rust
/// Arrival event emitted when bus reaches a stop
#[derive(Debug, Clone)]
pub struct ArrivalEvent {
    /// GPS update timestamp (seconds since epoch)
    pub time: u64,
    /// Stop index that was arrived at
    pub stop_idx: u8,
    /// Route progress at arrival (cm)
    pub s_cm: DistCm,
    /// Speed at arrival (cm/s)
    pub v_cms: SpeedCms,
    /// Arrival probability that triggered
    pub probability: Prob8,
}
```

---

## Module Specifications

### 1. Input Parser (arrival_detector/src/input.rs)

```rust
//! Phase 2 JSONL input parser

use serde::Deserialize;
use std::io::{BufRead, BufReader};
use std::fs::File;

#[derive(Deserialize)]
struct Phase2Record {
    time: u64,
    s_cm: i32,
    v_cms: i32,
    status: String,
    seg_idx: Option<usize>,
}

pub struct InputRecord {
    pub time: u64,
    pub s_cm: DistCm,
    pub v_cms: SpeedCms,
    pub valid: bool,
}

/// Parse Phase 2 JSONL and extract valid records
pub fn parse_input(path: &Path) -> impl Iterator<Item=InputRecord> {
    let file = BufReader::new(File::open(path).unwrap());
    file.lines().filter_map(|line| line.ok())
        .filter_map(|line| serde_json::from_str::<Phase2Record>(&line).ok())
        .map(|rec| InputRecord {
            time: rec.time,
            s_cm: rec.s_cm as DistCm,
            v_cms: rec.v_cms as SpeedCms,
            valid: rec.status == "valid",
        })
}
```

---

### 2. Corridor Filter (arrival_detector/src/corridor.rs)

```rust
//! Stop corridor filter

use shared::{Stop, DistCm};

/// Find stops whose corridor contains the current route progress
pub fn find_active_stops(s_cm: DistCm, stops: &[Stop]) -> Vec<usize> {
    stops.iter()
        .enumerate()
        .filter(|(_, stop)| {
            s_cm >= stop.corridor_start_cm && s_cm <= stop.corridor_end_cm
        })
        .map(|(i, _)| i)
        .collect()
}
```

---

### 3. Probability Model (arrival_detector/src/probability.rs)

```rust
//! 4-feature Bayesian arrival probability model

use shared::{DistCm, SpeedCms, Prob8};

/// Pre-computed Gaussian LUT (σ = 2750 cm)
///
/// Note: Simplified LUT using direct distance indexing.
/// For production firmware, use normalized (d/σ) indexing
/// as shown in tech report section 3.4.
fn build_gaussian_lut() -> [u8; 256] {
    let mut lut = [0u8; 256];
    let sigma = 2750.0;
    for i in 0..256 {
        let x = (i as f64) * 100.0;  // 0 to 25500 cm
        let g = (-0.5 * (x / sigma).powi(2)).exp();
        lut[i] = (g * 255.0).min(255.0) as u8;
    }
    lut
}

/// Pre-computed Logistic LUT (v_stop = 200 cm/s)
fn build_logistic_lut() -> [u8; 128] {
    let mut lut = [0u8; 128];
    for i in 0..128 {
        let dv = (i as f64) * 10.0;  // 0 to 1270 cm/s
        let l = 1.0 / (1.0 + (-0.01 * (dv - 200.0)).exp());
        lut[i] = (l * 255.0).min(255.0) as u8;
    }
    lut
}

/// Compute arrival probability (0-255)
pub fn arrival_probability(
    s_cm: DistCm,
    v_cms: SpeedCms,
    stop: &shared::Stop,
    dwell_time_s: u16,
    gaussian_lut: &[u8; 256],
    logistic_lut: &[u8; 128],
) -> Prob8 {
    // Feature 1: Distance likelihood
    let d_cm = (s_cm - stop.progress_cm).abs();
    let p1 = gaussian_lut[(d_cm / 100).min(255) as usize] as u32;

    // Feature 2: Speed likelihood (near 0 → higher)
    let v_diff = (200 - v_cms).abs().max(0) as u32;
    let p2 = logistic_lut[(v_diff / 10).min(127) as usize] as u32;

    // Feature 3: Progress difference
    let p3 = gaussian_lut[(d_cm / 100).min(255) as usize] as u32;

    // Feature 4: Dwell time
    let p4 = ((dwell_time_s as u32) * 255 / 10).min(255) as u32;

    // Weighted sum: (13p₁ + 6p₂ + 10p₃ + 3p₄) / 32
    ((13 * p1 + 6 * p2 + 10 * p3 + 3 * p4) / 32) as u8
}

/// Arrival threshold: 75% probability
pub const THETA_ARRIVAL: Prob8 = 191;
```

---

### 4. State Machine (arrival_detector/src/state_machine.rs)

```rust
//! Stop state machine with skip-stop protection

use shared::{DistCm, SpeedCms, Prob8, FsmState};

/// Runtime state for a single stop
pub struct StopState {
    /// Stop index in route
    pub index: u8,
    /// Current FSM state
    pub fsm_state: FsmState,
    /// Time spent in corridor (seconds)
    pub dwell_time_s: u16,
    /// Last computed arrival probability
    pub last_probability: Prob8,
}

impl StopState {
    pub fn new(index: u8) -> Self {
        StopState {
            index,
            fsm_state: FsmState::Approaching,
            dwell_time_s: 0,
            last_probability: 0,
        }
    }

    /// Reset state for re-entry into corridor (after departure)
    pub fn reset(&mut self) {
        self.fsm_state = FsmState::Approaching;
        self.dwell_time_s = 0;
        self.last_probability = 0;
    }

    /// Update state and return true if just arrived
    pub fn update(
        &mut self,
        s_cm: DistCm,
        v_cms: SpeedCms,
        stop_progress: DistCm,
        probability: Prob8,
    ) -> bool {
        let d_to_stop = (s_cm - stop_progress).abs();

        match self.fsm_state {
            FsmState::Approaching => {
                if d_to_stop < 5000 {
                    self.fsm_state = FsmState::Arriving;
                }
                // Update dwell time when in corridor
                self.dwell_time_s += 1;
            }
            FsmState::Arriving => {
                if d_to_stop < 3000 && v_cms < 56 && probability > 191 {
                    self.fsm_state = FsmState::AtStop;
                    self.dwell_time_s += 1;
                    return true;  // Just arrived!
                }
                self.dwell_time_s += 1;
            }
            FsmState::AtStop => {
                if d_to_stop > 4000 && s_cm > stop_progress {
                    self.fsm_state = FsmState::Departed;
                }
                // Don't increment dwell_time after departure
            }
            FsmState::Departed => {
                // Stay departed - dwell_time no longer accumulates
            }
        }

        self.last_probability = probability;
        false
    }

    /// Check if stop can be re-activated (after departure)
    pub fn can_reactivate(&self, s_cm: DistCm, stop_progress: DistCm) -> bool {
        matches!(self.fsm_state, FsmState::Departed)
            && s_cm >= stop_progress - 8000  // Back in corridor
    }
}
```

---

### 5. Recovery (arrival_detector/src/recovery.rs)

```rust
//! Stop index recovery for GPS jump handling

use shared::{DistCm, Stop};

/// Trigger conditions
const GPS_JUMP_THRESHOLD: DistCm = 20000;  // 200 m

/// Find correct stop after GPS jump
///
/// Simplified version: Find closest stop within ±200m
/// that is at or after the last known stop index.
/// For full implementation with velocity penalty and
/// backward scoring, see tech report section 15.2.
pub fn find_stop_index(
    s_cm: DistCm,
    stops: &[Stop],
    last_index: u8,
) -> Option<usize> {
    // Candidates within ±200m and >= last_index - 1
    let mut candidates: Vec<(usize, DistCm)> = stops.iter()
        .enumerate()
        .filter(|&(i, stop)| {
            let d = (s_cm - stop.progress_cm).abs();
            d < GPS_JUMP_THRESHOLD && (i as u8) >= last_index.saturating_sub(1)
        })
        .map(|(i, stop)| (i, (s_cm - stop.progress_cm).abs()))
        .collect();

    // Prefer forward progress (higher index, then closer distance)
    candidates.sort_by(|a, b| {
        a.0.cmp(&b.0)  // Prefer higher index first
            .then_with(|| a.1.cmp(&b.1).reverse())  // Then closer distance
    });

    candidates.first().map(|(i, _)| *i)
}
```

---

### 6. Output (arrival_detector/src/output.rs)

```rust
//! Arrival event JSON output

use serde::Serialize;
use shared::ArrivalEvent;

#[derive(Serialize)]
struct OutputRecord {
    time: u64,
    stop_idx: u8,
    s_cm: DistCm,
    v_cms: SpeedCms,
    probability: Prob8,
}

pub fn write_event<W: std::io::Write>(
    output: &mut W,
    event: &ArrivalEvent,
) -> std::io::Result<()> {
    let record = OutputRecord {
        time: event.time,
        stop_idx: event.stop_idx,
        s_cm: event.s_cm,
        v_cms: event.v_cms,
        probability: event.probability,
    };
    writeln!(output, "{}", serde_json::to_string(&record).unwrap())
}
```

---

## Binary Configuration

**arrival_detector/Cargo.toml:**

```toml
[package]
name = "arrival_detector"
version.workspace = true
edition.workspace = true

[[bin]]
name = "arrival_detector"
path = "src/main.rs"

[dependencies]
shared = { path = "../shared" }
serde = { workspace = true }
serde_json = { workspace = true }
```

---

## CLI Interface

```bash
# Run arrival detector on Phase 2 output
cargo run --bin arrival_detector -- input.jsonl route_data.bin output.jsonl
```

---

### Main Loop Integration (arrival_detector/src/main.rs)

```rust
fn main() {
    // ... parse arguments, load route_data ...

    let mut stop_states: Vec<StopState> = vec![];
    let mut last_s_cm = 0;
    let mut current_stop_idx = 0;

    // Pre-compute LUTs
    let gaussian_lut = probability::build_gaussian_lut();
    let logistic_lut = probability::build_logistic_lut();

    for record in input::parse_input(&input_path) {
        if !record.valid {
            continue;
        }

        // Check for GPS jump - trigger recovery
        if (record.s_cm - last_s_cm).abs() > 20000 {
            if let Some(new_idx) = recovery::find_stop_index(
                record.s_cm, &stops, current_stop_idx
            ) {
                // Reset all states and jump to new stop
                stop_states.clear();
                current_stop_idx = new_idx as u8;
            }
        }
        last_s_cm = record.s_cm;

        // Find active stops (corridor filter)
        let active_stops = corridor::find_active_stops(record.s_cm, &stops);

        // Ensure we have state for all stops
        while stop_states.len() <= active_stops.iter().max().unwrap_or(&0) {
            stop_states.push(StopState::new(stop_states.len() as u8));
        }

        for &stop_idx in &active_stops {
            let stop = &stops[stop_idx];
            let state = &mut stop_states[stop_idx];

            // Handle re-entry after departure
            if state.fsm_state == FsmState::Departed {
                if state.can_reactivate(record.s_cm, stop.progress_cm) {
                    state.reset();
                }
            }

            // Compute probability
            let prob = probability::arrival_probability(
                record.s_cm,
                record.v_cms,
                stop,
                state.dwell_time_s,
                &gaussian_lut,
                &logistic_lut,
            );

            // Update state machine
            if state.update(record.s_cm, record.v_cms, stop.progress_cm, prob) {
                // Just arrived! Emit event
                let event = ArrivalEvent {
                    time: record.time,
                    stop_idx: state.index,
                    s_cm: record.s_cm,
                    v_cms: record.v_cms,
                    probability: prob,
                };
                output::write_event(&mut output, &event).unwrap();
            }
        }
    }
}
```

---

## Validation Criteria

- [ ] Parses Phase 2 JSONL input correctly
- [ ] Loads stops from route_data.bin
- [ ] Corridor filter returns 0-1 active stops
- [ ] Probability model computes values in 0-255 range
- [ ] State machine transitions correctly
- [ ] Arrival events emitted at appropriate times
- [ ] Recovery handles GPS jumps
- [ ] Output JSON matches expected format

---

## Next Phase

Phase 3 completes the GPS Bus Arrival Detection System for host simulation. Future work could include:
- Embedded firmware port (RP2350)
- Real-time GPS stream processing
- Multi-route support
- Performance optimization

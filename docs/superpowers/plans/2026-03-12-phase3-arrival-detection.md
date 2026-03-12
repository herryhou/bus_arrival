# Phase 3: Arrival Detection — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Rust host simulator that reads Phase 2 JSONL output and detects bus arrivals at stops using corridor filtering, probabilistic modeling, and state machine validation.

**Architecture:** Pipeline: Phase 2 JSONL → Parse → Corridor Filter → Probability Model → State Machine → Arrival Events

**Tech Stack:** Rust 2021 edition, existing shared crate, new arrival_detector binary

---

## File Structure

```
bus_arrival/
├── shared/src/lib.rs       # Add: FsmState, ArrivalEvent
├── arrival_detector/        # NEW: arrival detection binary
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs          # CLI entry point + main loop
│       ├── input.rs         # Phase 2 JSONL parser
│       ├── corridor.rs      # Stop corridor filter
│       ├── probability.rs   # 4-feature Bayesian + LUTs
│       ├── state_machine.rs  # FSM + per-stop state
│       ├── recovery.rs      # GPS jump recovery
│       └── output.rs        # Arrival event JSON output
```

---

## Chunk 1: Shared Types & Arrival Detector Scaffold

**Goal:** Add new types to shared crate and create arrival_detector binary scaffold.

---

### Task 1: Add FsmState and ArrivalEvent to Shared

**Files:**
- Modify: `shared/src/lib.rs`

- [ ] **Step 1: Add FsmState enum to shared/src/lib.rs**

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

- [ ] **Step 2: Add ArrivalEvent struct to shared/src/lib.rs**

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

- [ ] **Step 3: Export new types**

```rust
pub use {RouteNode, Stop, GridOrigin};
pub use {DistCm, SpeedCms, HeadCdeg, Prob8, Dist2};
pub use {GpsPoint, KalmanState, SpatialGrid, DrState};
pub use {FsmState, ArrivalEvent};
```

- [ ] **Step 4: Run tests to verify**

Run: `cargo test -p shared`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add shared/src/lib.rs && git commit -m "feat(shared): add FsmState and ArrivalEvent for arrival detection"
```

---

### Task 2: Create Arrival Detector Binary Scaffold

**Files:**
- Create: `arrival_detector/Cargo.toml`
- Create: `arrival_detector/src/main.rs`

- [ ] **Step 1: Create arrival_detector/Cargo.toml**

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

- [ ] **Step 2: Create arrival_detector/src/main.rs with CLI**

```rust
use std::env;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 4 {
        eprintln!("Usage: arrival_detector <input.jsonl> <route_data.bin> <output.jsonl>");
        eprintln!();
        eprintln!("Arguments:");
        eprintln!("  input.jsonl     - Phase 2 JSONL output");
        eprintln!("  route_data.bin  - Binary route data from Phase 1");
        eprintln!("  output.jsonl   - Arrival event output");
        std::process::exit(1);
    }

    let input_path = PathBuf::from(&args[1]);
    let route_path = PathBuf::from(&args[2]);
    let output_path = PathBuf::from(&args[3]);

    println!("Phase 3: Arrival Detection");
    println!("  Input:  {}", input_path.display());
    println!("  Route:  {}", route_path.display());
    println!("  Output: {}", output_path.display());
    println!();
    println!("TODO: Implement pipeline");
}
```

- [ ] **Step 3: Run to verify CLI**

Run: `cargo build --bin arrival_detector`
Expected: Compiles without errors

- [ ] **Step 4: Update workspace members**

```bash
# Add arrival_detector to workspace members in root Cargo.toml if needed
```

- [ ] **Step 5: Commit**

```bash
git add arrival_detector/ && git commit -m "feat(arrival_detector): add CLI scaffold"
```

---

## Chunk 2: Input Parser

**Goal:** Parse Phase 2 JSONL output and extract valid GPS updates.

---

### Task 3: Implement Input Parser

**Files:**
- Create: `arrival_detector/src/input.rs`

- [ ] **Step 1: Add input parser module**

```rust
//! Phase 2 JSONL input parser

use serde::Deserialize;
use shared::{DistCm, SpeedCms};
use std::io::{BufRead, BufReader};

#[derive(Deserialize)]
struct Phase2Record {
    time: u64,
    s_cm: i32,
    v_cms: i32,
    status: String,
    seg_idx: Option<usize>,
}

/// Parsed input record
pub struct InputRecord {
    pub time: u64,
    pub s_cm: DistCm,
    pub v_cms: SpeedCms,
    pub valid: bool,
}

/// Parse Phase 2 JSONL file and return iterator of records
pub fn parse_input(path: &std::path::Path) -> impl Iterator<Item=InputRecord> {
    let file = std::fs::File::open(path).unwrap();
    let reader = BufReader::new(file);

    reader.lines().filter_map(|line| line.ok())
        .filter_map(|line| serde_json::from_str::<Phase2Record>(&line).ok())
        .map(|rec| InputRecord {
            time: rec.time,
            s_cm: rec.s_cm,
            v_cms: rec.v_cms,
            valid: rec.status == "valid",
        })
}
```

- [ ] **Step 2: Add mod input to main.rs**

```rust
mod input;
```

- [ ] **Step 3: Run cargo check to verify**

Run: `cargo check -p arrival_detector`
Expected: Compiles without errors

- [ ] **Step 4: Commit**

```bash
git add arrival_detector/src/input.rs arrival_detector/src/main.rs && git commit -m "feat(arrival_detector): add Phase 2 JSONL parser"
```

---

## Chunk 3: Corridor Filter

**Goal:** Find stops whose corridor contains the current route progress.

---

### Task 4: Implement Corridor Filter

**Files:**
- Create: `arrival_detector/src/corridor.rs`

- [ ] **Step 1: Implement corridor filter**

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

- [ ] **Step 2: Add mod corridor to main.rs**

```rust
mod corridor;
```

- [ ] **Step 3: Add tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use shared::Stop;

    #[test]
    fn test_no_active_stops() {
        let stops = vec![
            Stop { progress_cm: 10000, corridor_start_cm: 2000, corridor_end_cm: 14000 },
        ];
        // s_cm = 0 is outside corridor
        assert!(find_active_stops(0, &stops).is_empty());
    }

    #[test]
    fn test_one_active_stop() {
        let stops = vec![
            Stop { progress_cm: 10000, corridor_start_cm: 2000, corridor_end_cm: 14000 },
        ];
        // s_cm = 10000 is inside corridor
        let result = find_active_stops(10000, &stops);
        assert_eq!(result, vec![0]);
    }
}
```

- [ ] **Step 4: Run tests to verify**

Run: `cargo test -p arrival_detector`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add arrival_detector/src/corridor.rs arrival_detector/src/main.rs && git commit -m "feat(arrival_detector): add corridor filter"
```

---

## Chunk 4: Probability Model

**Goal:** Implement 4-feature Bayesian probability model with LUTs.

---

### Task 5: Implement Probability Model with LUTs

**Files:**
- Create: `arrival_detector/src/probability.rs`

- [ ] **Step 1: Add probability module with LUT generation**

```rust
//! 4-feature Bayesian arrival probability model

use shared::{DistCm, SpeedCms, Prob8};

/// Pre-computed Gaussian LUT (σ = 2750 cm)
pub fn build_gaussian_lut() -> [u8; 256] {
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
pub fn build_logistic_lut() -> [u8; 128] {
    let mut lut = [0u8; 128];
    for i in 0..128 {
        let dv = (i as f64) * 10.0;  // 0 to 1270 cm/s
        let l = 1.0 / (1.0 + (-0.01 * (dv - 200.0)).exp());
        lut[i] = (l * 255.0).min(255.0) as u8;
    }
    lut
}

/// Arrival threshold: 75% probability
pub const THETA_ARRIVAL: Prob8 = 191;
```

- [ ] **Step 2: Add arrival_probability function**

```rust
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
```

- [ ] **Step 3: Add tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use shared::Stop;

    #[test]
    fn test_lut_generation() {
        let g_lut = build_gaussian_lut();
        assert_eq!(g_lut[0], 255);  // d=0 → max probability
        assert!(g_lut[255], 0);   // d=25500 → min probability

        let l_lut = build_logistic_lut();
        assert!(l_lut[20] > 200);  // v=200 cm/s → high probability
    }

    #[test]
    fn test_probability_range() {
        let g_lut = build_gaussian_lut();
        let l_lut = build_logistic_lut();
        let stop = Stop { progress_cm: 10000, corridor_start_cm: 2000, corridor_end_cm: 14000 };

        let p = arrival_probability(10000, 100, &stop, 5, &g_lut, &l_lut);
        assert!(p <= 255);
    }
}
```

- [ ] **Step 4: Add mod probability to main.rs**

```rust
mod probability;
```

- [ ] **Step 5: Run tests to verify**

Run: `cargo test -p arrival_detector`
Expected: All tests pass

- [ ] **Step 6: Commit**

```bash
git add arrival_detector/src/probability.rs arrival_detector/src/main.rs && git commit -m "feat(arrival_detector): add probability model with LUTs"
```

---

## Chunk 5: State Machine

**Goal:** Implement stop state machine with FSM transitions and per-stop state tracking.

---

### Task 6: Implement State Machine

**Files:**
- Create: `arrival_detector/src/state_machine.rs`

- [ ] **Step 1: Add StopState struct and implementation**

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

    /// Check if stop can be re-activated after departure
    pub fn can_reactivate(&self, s_cm: DistCm, stop_progress: DistCm) -> bool {
        matches!(self.fsm_state, FsmState::Departed)
            && s_cm >= stop_progress - 8000  // Back in corridor
    }
}
```

- [ ] **Step 2: Add tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stop_state_creation() {
        let state = StopState::new(5);
        assert_eq!(state.index, 5);
        assert_eq!(matches!(state.fsm_state, FsmState::Approaching), true);
    }

    #[test]
    fn test_approaching_to_arriving() {
        let mut state = StopState::new(0);
        let stop_progress = 10000;

        // Start at 6000cm (in corridor, not close)
        state.update(6000, 100, stop_progress, 100);
        assert!(matches!(state.fsm_state, FsmState::Approaching));

        // Move to 4000cm (close enough for Arriving)
        state.update(4000, 100, stop_progress, 100);
        assert!(matches!(state.fsm_state, FsmState::Arriving));
    }

    #[test]
    fn test_arriving_to_atstop() {
        let mut state = StopState::new(0);
        state.fsm_state = FsmState::Arriving;
        let stop_progress = 10000;

        // Arriving conditions: d<3000, v<56, P>191
        let arrived = state.update(2500, 50, stop_progress, 200);
        assert!(arrived);
        assert!(matches!(state.fsm_state, FsmState::AtStop));
    }

    #[test]
    fn test_atstop_to_departed() {
        let mut state = StopState::new(0);
        state.fsm_state = FsmState::AtStop;
        let stop_progress = 10000;

        // Departed conditions: d>4000, s > stop
        state.update(1500, 100, stop_progress, 100);  // Past stop
        assert!(matches!(state.fsm_state, FsmState::Departed));
    }
}
```

- [ ] **Step 3: Add mod state_machine to main.rs**

```rust
mod state_machine;
```

- [ ] **Step 4: Run tests to verify**

Run: `cargo test -p arrival_detector`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add arrival_detector/src/state_machine.rs arrival_detector/src/main.rs && git commit -m "feat(arrival_detector): add state machine with FSM"
```

---

## Chunk 6: Recovery Module

**Goal:** Handle GPS jumps and stop index recovery.

---

### Task 7: Implement Recovery Module

**Files:**
- Create: `arrival_detector/src/recovery.rs`

- [ ] **Step 1: Add recovery module**

```rust
//! Stop index recovery for GPS jump handling

use shared::{DistCm, Stop};

/// Trigger conditions
pub const GPS_JUMP_THRESHOLD: DistCm = 20000;  // 200 m

/// Find correct stop after GPS jump
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
        a.0.cmp(&b.0)
            .then_with(|| a.1.cmp(&b.1).reverse())
    });

    candidates.first().map(|(i, _)| *i)
}
```

- [ ] **Step 2: Add tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_candidates() {
        let stops = vec![
            Stop { progress_cm: 0, corridor_start_cm: 0, corridor_end_cm: 0 },
        ];
        // s_cm = 50000 is outside threshold
        assert!(find_stop_index(50000, &stops, 0).is_none());
    }

    #[test]
    fn test_find_closest_stop() {
        let stops = vec![
            Stop { progress_cm: 0, corridor_start_cm: 0, corridor_end_cm: 0 },
            Stop { progress_cm: 10000, corridor_start_cm: 2000, corridor_end_cm: 14000 },
            Stop { progress_cm: 20000, corridor_start_cm: 12000, corridor_end_cm: 16000 },
        ];

        // At 9500cm, should find stop 1 (closest within threshold)
        let result = find_stop_index(9500, &stops, 0);
        assert_eq!(result, Some(1));
    }

    #[test]
    fn test_prefer_forward_progress() {
        let stops = vec![
            Stop { progress_cm: 0, corridor_start_cm: 0, corridor_end_cm: 0 },
            Stop { progress_cm: 10000, corridor_start_cm: 2000, corridor_end_cm: 14000 },
            Stop { progress_cm: 20000, corridor_start_cm: 12000, corridor_end_cm: 16000 },
        ];

        // At 15000cm (between stops 1 and 2), last_index=1, prefer forward (stop 2)
        let result = find_stop_index(15000, &stops, 1);
        assert_eq!(result, Some(2));
    }
}
```

- [ ] **Step 3: Add mod recovery to main.rs**

```rust
mod recovery;
```

- [ ] **Step 4: Run tests to verify**

Run: `cargo test -p arrival_detector`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add arrival_detector/src/recovery.rs arrival_detector/src/main.rs && git commit -m "feat(arrival_detector): add GPS jump recovery"
```

---

## Chunk 7: Output Module

**Goal:** JSON output for arrival events.

---

### Task 8: Implement Output Module

**Files:**
- Create: `arrival_detector/src/output.rs`

- [ ] **Step 1: Add output module**

```rust
//! Arrival event JSON output

use serde::Serialize;
use std::io::Write;

#[derive(Serialize)]
struct OutputRecord {
    time: u64,
    stop_idx: u8,
    s_cm: DistCm,
    v_cms: SpeedCms,
    probability: Prob8,
}

/// Write arrival event to JSON output
pub fn write_event<W: Write>(
    output: &mut W,
    event: &shared::ArrivalEvent,
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

- [ ] **Step 2: Add mod output to main.rs**

```rust
mod output;
```

- [ ] **Step 3: Run cargo check to verify**

Run: `cargo check -p arrival_detector`
Expected: Compiles without errors

- [ ] **Step 4: Commit**

```bash
git add arrival_detector/src/output.rs arrival_detector/src/main.rs && git commit -m "feat(arrival_detector): add JSON output module"
```

---

## Chunk 8: Full Pipeline Integration

**Goal:** Integrate all modules into main processing loop.

---

### Task 9: Integrate Full Pipeline in Main

**Files:**
- Modify: `arrival_detector/src/main.rs`

- [ ] **Step 1: Update main.rs with route_data reader**

```rust
use shared::{RouteData, Stop, FsmState, ArrivalEvent};
use state_machine::StopState;
use std::fs::File;
use std::io::{BufWriter, Write};

mod route_data;  // Reuse from simulator crate or add local module
mod input;
mod corridor;
mod probability;
mod state_machine;
mod recovery;
mod output;

fn main() {
    // ... CLI parsing ...

    // Load route data (reuse simulator module or local copy)
    let route_data = route_data::load_route_data(&route_path)
        .expect("Failed to load route_data.bin");

    // Initialize state
    let mut stop_states: Vec<StopState> = vec![];
    let mut last_s_cm: DistCm = 0;
    let mut current_stop_idx: u8 = 0;

    // Pre-compute LUTs
    let gaussian_lut = probability::build_gaussian_lut();
    let logistic_lut = probability::build_logistic_lut();

    // Open output
    let mut output = BufWriter::new(File::create(&output_path).unwrap());

    // Process each input record
    for record in input::parse_input(&input_path) {
        if !record.valid {
            continue;
        }

        // Check for GPS jump - trigger recovery
        if (record.s_cm - last_s_cm).unsigned_abs() > 20000 {
            if let Some(new_idx) = recovery::find_stop_index(
                record.s_cm, &route_data.stops, current_stop_idx
            ) {
                // Reset all states and jump to new stop
                stop_states.clear();
                current_stop_idx = new_idx as u8;
                eprintln!("GPS jump detected, recovered to stop {}", current_stop_idx);
            }
        }
        last_s_cm = record.s_cm;

        // Find active stops (corridor filter)
        let active_stops = corridor::find_active_stops(record.s_cm, &route_data.stops);

        // Ensure we have state for all stops
        while stop_states.len() <= active_stops.iter().max().unwrap_or(&0) {
            stop_states.push(StopState::new(stop_states.len() as u8));
        }

        for &stop_idx in &active_stops {
            let stop = &route_data.stops[stop_idx];
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
                eprintln!("Arrival at stop {}: time={}, s={}m, P={}",
                    state.index, record.time, record.s_cm / 100, prob);
            }
        }
    }

    output.flush().unwrap();
    eprintln!("Processing complete");
}
```

- [ ] **Step 2: Add route_data module (reuse or copy)**

Option A: Copy `simulator/src/route_data.rs` to `arrival_detector/src/`
Option B: Add shared route_data loading

```bash
# Option A - copy the module
cp simulator/src/route_data.rs arrival_detector/src/
```

- [ ] **Step 3: Run cargo build to verify**

Run: `cargo build --release --bin arrival_detector`
Expected: Compiles without errors

- [ ] **Step 4: Test with sample data**

Run: `cargo run --bin arrival_detector -- test_output.jsonl route_data.bin arrivals.jsonl`
Expected: Creates arrivals.jsonl with arrival events

- [ ] **Step 5: Commit**

```bash
git add arrival_detector/src/main.rs arrival_detector/src/route_data.rs && git commit -m "feat(arrival_detector): integrate full pipeline"
```

---

## Completion Checklist

After all tasks complete:

- [ ] `cargo test --workspace` passes (all modules)
- [ ] `cargo build --release --bin arrival_detector` succeeds
- [ ] End-to-end test produces arrival events
- [ ] All commits follow conventional commit format
- [ ] Final `git log` shows clean progression

---

## Testing with Real Data

```bash
# Run full pipeline
cargo run --release --bin arrival_detector -- \
    phase2_output.jsonl \
    route_data.bin \
    arrivals.jsonl

# Check arrival events
head arrivals.jsonl
# Expected: {"time":123, "stop_idx": 5, "s_cm": 12345, "v_cms": 50, "probability": 200}
```

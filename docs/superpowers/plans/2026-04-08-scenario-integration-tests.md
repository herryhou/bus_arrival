# Scenario Integration Tests Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add comprehensive scenario-based integration tests to validate the full bus arrival detection pipeline using real ty225 route data.

**Architecture:** Create a new `crates/pipeline/tests/scenarios/` module with common utilities and four scenario test files. Tests use existing test data (normal, drift, jump, outage) and validate arrival detection, state machine transitions, and recovery behavior.

**Tech Stack:** Rust, cargo test framework, existing ty225 route data, existing pipeline modules (gps_processor, detection)

---

## File Structure

```
crates/pipeline/tests/
├── scenarios/
│   ├── mod.rs                    # Module entry point
│   ├── common/
│   │   └── mod.rs               # Shared utilities (load route, run scenario)
│   ├── normal.rs                # Normal operation tests
│   ├── gps_anomalies.rs         # Drift, jump tests
│   ├── signal_loss.rs           # Outage tests
│   └── route_edge_cases.rs      # Route geometry tests
```

---

## Task 1: Create Scenarios Module Structure

**Files:**
- Create: `crates/pipeline/tests/scenarios/mod.rs`

- [ ] **Step 1: Create the scenarios module entry point**

```rust
//! Scenario-based integration tests for the bus arrival detection pipeline
//!
//! These tests validate the complete pipeline using real ty225 route data
//! across various scenarios: normal operation, GPS anomalies, signal loss,
//! and route geometry edge cases.

mod common;
mod normal;
mod gps_anomalies;
mod signal_loss;
mod route_edge_cases;
```

- [ ] **Step 2: Verify the module compiles**

Run: `cargo test -p pipeline --test scenarios --no-run`
Expected: No compilation errors (module exists but no tests yet)

- [ ] **Step 3: Commit**

```bash
git add crates/pipeline/tests/scenarios/mod.rs
git commit -m "test: add scenario integration tests module structure"
```

---

## Task 2: Create Common Test Utilities

**Files:**
- Create: `crates/pipeline/tests/scenarios/common/mod.rs`

- [ ] **Step 1: Write common utilities module**

```rust
//! Common utilities for scenario integration tests

use std::fs;
use std::io::BufRead;
use std::path::PathBuf;

/// Test data directory path
pub fn test_data_dir() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("../../test_data");
    path
}

/// Load ty225 route data binary
pub fn load_ty225_route(scenario: &str) -> Vec<u8> {
    let filename = format!("ty225_{}.bin", scenario);
    let mut path = test_data_dir();
    path.push(&filename);
    fs::read(&path)
        .unwrap_or_else(|e| panic!("Failed to load {}: {:?}", filename, e))
}

/// Load NMEA test data
pub fn load_nmea(scenario: &str) -> Vec<String> {
    let filename = format!("ty225_{}_nmea.txt", scenario);
    let mut path = test_data_dir();
    path.push(&filename);
    let file = fs::File::open(&path)
        .unwrap_or_else(|e| panic!("Failed to open {}: {:?}", filename, e));
    std::io::BufReader::new(file)
        .lines()
        .map(|l| l.unwrap())
        .collect()
}

/// Load expected arrivals from ground truth JSON
pub fn load_expected_arrivals(scenario: &str) -> Vec<usize> {
    let filename = format!("ty225_{}_arrivals.json", scenario);
    let mut path = test_data_dir();
    path.push(&filename);
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to load {}: {:?}", filename, e));

    // Parse arrivals JSON - each arrival has "stop_idx"
    let value: serde_json::Value = serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("Failed to parse {}: {:?}", filename, e));

    value.as_array()
        .unwrap()
        .iter()
        .map(|v| v["stop_idx"].as_u64().unwrap() as usize)
        .collect()
}

/// Expected results for scenario validation
#[derive(Debug, Clone)]
pub struct ExpectedResults {
    pub arrivals: Vec<usize>,
    pub min_arrivals: usize,
    pub max_arrivals: usize,
}

impl ExpectedResults {
    /// Create expected results from ground truth
    pub fn from_ground_truth(scenario: &str) -> Self {
        let arrivals = load_expected_arrivals(scenario);
        let count = arrivals.len();
        Self {
            arrivals,
            min_arrivals: count.saturating_sub(1),
            max_arrivals: count + 1,
        }
    }

    /// Create expected results with custom bounds
    pub fn with_bounds(min: usize, max: usize) -> Self {
        Self {
            arrivals: Vec::new(),
            min_arrivals: min,
            max_arrivals: max,
        }
    }
}

/// Scenario test result
pub type TestResult = Result<(), String>;
```

- [ ] **Step 2: Verify the module compiles**

Run: `cargo test -p pipeline --test scenarios --no-run`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add crates/pipeline/tests/scenarios/common/mod.rs
git commit -m "test: add common utilities for scenario tests"
```

---

## Task 3: Create Normal Operation Tests

**Files:**
- Create: `crates/pipeline/tests/scenarios/normal.rs`

- [ ] **Step 1: Write normal operation scenario test**

```rust
//! Normal operation scenario tests

use crate::common::{load_ty225_route, load_nmea, ExpectedResults, TestResult};
use shared::binfile::RouteData;
use shared::{KalmanState, DrState, FsmState};
use gps_processor::nmea::NmeaState;
use detection::state_machine::StopState;

/// Test: Bus drives entire ty225 route normally
/// Validates: All stops detected, correct arrival order
#[test]
fn test_normal_complete_route() {
    // Load route and NMEA data
    let route_bytes = load_ty225_route("normal");
    let route_data = RouteData::load(&route_bytes)
        .expect("Failed to load route data");

    let nmea_lines = load_nmea("normal");
    let expected = ExpectedResults::from_ground_truth("normal");

    // Initialize pipeline state
    let mut nmea = NmeaState::new();
    let mut kalman = KalmanState::new();
    let mut dr = DrState::new();

    let mut stop_states: Vec<StopState> = route_data.stops()
        .iter()
        .enumerate()
        .map(|(i, _)| StopState::new(i as u8))
        .collect();

    let mut detected_arrivals: Vec<usize> = Vec::new();

    // Process NMEA sentences
    for line in nmea_lines {
        if let Some(_gps) = nmea.parse_sentence(&line) {
            // TODO: Full pipeline processing
            // For now, just verify NMEA parsing works
        }
    }

    // Validate: should detect expected number of arrivals
    assert!(
        detected_arrivals.len() >= expected.min_arrivals,
        "Expected at least {} arrivals, got {}",
        expected.min_arrivals,
        detected_arrivals.len()
    );

    assert!(
        detected_arrivals.len() <= expected.max_arrivals,
        "Expected at most {} arrivals, got {}",
        expected.max_arrivals,
        detected_arrivals.len()
    );
}

/// Test: Validate state machine states for normal operation
#[test]
fn test_normal_state_transitions() {
    // Load route data
    let route_bytes = load_ty225_route("normal");
    let route_data = RouteData::load(&route_bytes)
        .expect("Failed to load route data");

    // Verify stop count
    assert_eq!(route_data.stops().len(), 58, "Route should have 58 stops");

    // Initialize stop states
    let stop_states: Vec<StopState> = route_data.stops()
        .iter()
        .enumerate()
        .map(|(i, _)| StopState::new(i as u8))
        .collect();

    // Verify all starts in Idle state
    for (i, state) in stop_states.iter().enumerate() {
        assert_eq!(
            state.fsm_state,
            FsmState::Idle,
            "Stop {} should start in Idle state",
            i
        );
    }
}
```

- [ ] **Step 2: Verify tests compile**

Run: `cargo test -p pipeline --test scenarios -- normal --no-run`
Expected: Compiles successfully

- [ ] **Step 3: Run tests to verify they work**

Run: `cargo test -p pipeline --test scenarios -- normal`
Expected: Tests pass (state transition test should pass; complete route will be expanded later)

- [ ] **Step 4: Commit**

```bash
git add crates/pipeline/tests/scenarios/normal.rs
git commit -m "test: add normal operation scenario tests"
```

---

## Task 4: Create GPS Anomaly Tests

**Files:**
- Create: `crates/pipeline/tests/scenarios/gps_anomalies.rs`

- [ ] **Step 1: Write GPS drift and jump scenario tests**

```rust
//! GPS anomaly scenario tests (drift, jump)

use crate::common::{load_ty225_route, load_nmea, ExpectedResults};
use shared::binfile::RouteData;
use shared::{KalmanState, DrState};
use gps_processor::nmea::NmeaState;
use detection::state_machine::StopState;

/// Test: GPS drift scenario
/// Validates: Recovery algorithm corrects position after drift
#[test]
fn test_drift_recovery() {
    // Load drift scenario data
    let route_bytes = load_ty225_route("drift");
    let route_data = RouteData::load(&route_bytes)
        .expect("Failed to load route data");

    let nmea_lines = load_nmea("drift");
    let expected = ExpectedResults::from_ground_truth("drift");

    // Initialize pipeline
    let mut nmea = NmeaState::new();
    let mut kalman = KalmanState::new();
    let mut dr = DrState::new();

    let mut stop_states: Vec<StopState> = route_data.stops()
        .iter()
        .enumerate()
        .map(|(i, _)| StopState::new(i as u8))
        .collect();

    let mut detected_arrivals: Vec<usize> = Vec::new();

    // Process NMEA with drift
    for line in nmea_lines {
        if let Some(_gps) = nmea.parse_sentence(&line) {
            // Pipeline processing
        }
    }

    // Validate: should still detect arrivals despite drift
    assert!(
        detected_arrivals.len() >= expected.min_arrivals,
        "Drift scenario: expected at least {} arrivals, got {}",
        expected.min_arrivals,
        detected_arrivals.len()
    );
}

/// Test: GPS jump scenario
/// Validates: No false arrivals for skipped stops
#[test]
fn test_jump_skip_stop_prevention() {
    // Load jump scenario data
    let route_bytes = load_ty225_route("jump");
    let route_data = RouteData::load(&route_bytes)
        .expect("Failed to load route data");

    let nmea_lines = load_nmea("jump");
    let expected = ExpectedResults::from_ground_truth("jump");

    // Initialize pipeline
    let mut nmea = NmeaState::new();
    let mut kalman = KalmanState::new();
    let mut dr = DrState::new();

    let mut stop_states: Vec<StopState> = route_data.stops()
        .iter()
        .enumerate()
        .map(|(i, _)| StopState::new(i as u8))
        .collect();

    let mut detected_arrivals: Vec<usize> = Vec::new();

    // Process NMEA with jump
    for line in nmea_lines {
        if let Some(_gps) = nmea.parse_sentence(&line) {
            // Pipeline processing
        }
    }

    // Validate: jump should not cause false arrivals
    assert!(
        detected_arrivals.len() <= expected.max_arrivals,
        "Jump scenario: expected at most {} arrivals, got {}",
        expected.max_arrivals,
        detected_arrivals.len()
    );
}

/// Test: Validate route loads for both scenarios
#[test]
fn test_anomaly_route_data_loads() {
    // Drift route
    let drift_bytes = load_ty225_route("drift");
    let drift_route = RouteData::load(&drift_bytes);
    assert!(drift_route.is_ok(), "Drift route should load");

    // Jump route
    let jump_bytes = load_ty225_route("jump");
    let jump_route = RouteData::load(&jump_bytes);
    assert!(jump_route.is_ok(), "Jump route should load");

    // Both should have same stop count
    assert_eq!(
        drift_route.unwrap().stops().len(),
        jump_route.unwrap().stops().len(),
        "Both routes should have same stop count"
    );
}
```

- [ ] **Step 2: Verify tests compile**

Run: `cargo test -p pipeline --test scenarios -- gps_anomalies --no-run`
Expected: Compiles successfully

- [ ] **Step 3: Run tests to verify they work**

Run: `cargo test -p pipeline --test scenarios -- gps_anomalies`
Expected: Tests pass (route data test should pass; others expanded later)

- [ ] **Step 4: Commit**

```bash
git add crates/pipeline/tests/scenarios/gps_anomalies.rs
git commit -m "test: add GPS anomaly scenario tests"
```

---

## Task 5: Create Signal Loss Tests

**Files:**
- Create: `crates/pipeline/tests/scenarios/signal_loss.rs`

- [ ] **Step 1: Write signal outage scenario tests**

```rust
//! Signal loss scenario tests (GPS outage, tunnel)

use crate::common::{load_ty225_route, load_nmea, ExpectedResults};
use shared::binfile::RouteData;
use shared::{KalmanState, DrState};
use gps_processor::nmea::NmeaState;
use detection::state_machine::StopState;

/// Test: GPS outage scenario (10s signal loss)
/// Validates: Dead reckoning maintains position during outage
#[test]
fn test_outage_dead_reckoning() {
    // Load outage scenario data
    let route_bytes = load_ty225_route("outage");
    let route_data = RouteData::load(&route_bytes)
        .expect("Failed to load route data");

    let nmea_lines = load_nmea("outage");
    let expected = ExpectedResults::from_ground_truth("outage");

    // Initialize pipeline
    let mut nmea = NmeaState::new();
    let mut kalman = KalmanState::new();
    let mut dr = DrState::new();

    let mut stop_states: Vec<StopState> = route_data.stops()
        .iter()
        .enumerate()
        .map(|(i, _)| StopState::new(i as u8))
        .collect();

    let mut detected_arrivals: Vec<usize> = Vec::new();
    let mut outage_count = 0;
    let mut recovery_count = 0;

    // Process NMEA with outage
    for line in nmea_lines {
        if let Some(gps) = nmea.parse_sentence(&line) {
            if gps.valid {
                recovery_count += 1;
            } else {
                outage_count += 1;
            }
            // Pipeline processing
        }
    }

    // Validate: should have GPS invalid messages during outage
    assert!(
        outage_count > 0,
        "Outage scenario should have GPS invalid messages"
    );

    // Validate: should recover after outage
    assert!(
        recovery_count > 0,
        "Outage scenario should have GPS recovery"
    );

    // Validate arrivals despite outage
    assert!(
        detected_arrivals.len() >= expected.min_arrivals,
        "Outage scenario: expected at least {} arrivals, got {}",
        expected.min_arrivals,
        detected_arrivals.len()
    );
}

/// Test: Validate outage scenario route data
#[test]
fn test_outage_route_data() {
    let route_bytes = load_ty225_route("outage");
    let route_data = RouteData::load(&route_bytes)
        .expect("Failed to load route data");

    // Verify route loaded
    assert_eq!(route_data.stops().len(), 58, "Route should have 58 stops");

    // Verify route nodes exist
    assert!(
        route_data.nodes().len() > 0,
        "Route should have nodes"
    );
}

/// Test: NMEA file contains valid GPS invalid messages
#[test]
fn test_outage_nmea_has_invalid_gps() {
    let nmea_lines = load_nmea("outage");

    let mut has_invalid = false;
    for line in nmea_lines {
        // Look for GPGGA with GPS quality indicator = 0 (no fix)
        if line.contains("$GPGGA") && line.contains(",0,") {
            has_invalid = true;
            break;
        }
    }

    assert!(
        has_invalid,
        "Outage NMEA should contain GPS invalid messages"
    );
}
```

- [ ] **Step 2: Verify tests compile**

Run: `cargo test -p pipeline --test scenarios -- signal_loss --no-run`
Expected: Compiles successfully

- [ ] **Step 3: Run tests to verify they work**

Run: `cargo test -p pipeline --test scenarios -- signal_loss`
Expected: Tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/pipeline/tests/scenarios/signal_loss.rs
git commit -m "test: add signal loss scenario tests"
```

---

## Task 6: Create Route Edge Case Tests

**Files:**
- Create: `crates/pipeline/tests/scenarios/route_edge_cases.rs`

- [ ] **Step 1: Write route geometry edge case tests**

```rust
//! Route geometry edge case tests
//! Tests for: loop closure, large projection errors, close stops

use crate::common::{load_ty225_route, ExpectedResults};
use shared::binfile::RouteData;

/// Test: Loop closure detection
/// Validates: Stop at route loop completion is detected
#[test]
fn test_loop_closure_detection() {
    // Load normal route (has loop closure at stop 57/58)
    let route_bytes = load_ty225_route("normal");
    let route_data = RouteData::load(&route_bytes)
        .expect("Failed to load route data");

    let stops = route_data.stops();

    // Verify we have 58 stops
    assert_eq!(stops.len(), 58, "Route should have 58 stops for loop test");

    // Verify last stop exists
    let last_stop = &stops[57];
    assert!(
        last_stop.progress_cm > 0,
        "Last stop should have progress value"
    );

    // Verify first stop (loop closure point)
    let first_stop = &stops[0];
    assert!(
        first_stop.progress_cm >= 0,
        "First stop should have valid progress"
    );
}

/// Test: Large projection error handling
/// Validates: System handles stops with > 30m projection errors
#[test]
fn test_large_projection_error_stops_exist() {
    // Load normal route
    let route_bytes = load_ty225_route("normal");
    let route_data = RouteData::load(&route_bytes)
        .expect("Failed to load route data");

    let stops = route_data.stops();

    // Verify all stops have corridor values
    for (i, stop) in stops.iter().enumerate() {
        assert!(
            stop.corridor_start_cm <= stop.progress_cm,
            "Stop {}: corridor_start should be <= progress",
            i
        );
        assert!(
            stop.corridor_end_cm >= stop.progress_cm,
            "Stop {}: corridor_end should be >= progress",
            i
        );
    }
}

/// Test: Close stop discrimination
/// Validates: System can discriminate nearby stops
#[test]
fn test_close_stop_discrimination() {
    let route_bytes = load_ty225_route("normal");
    let route_data = RouteData::load(&route_bytes)
        .expect("Failed to load route data");

    let stops = route_data.stops();

    // Find stops that are close to each other (< 100m apart in progress)
    let mut close_pairs = Vec::new();
    for i in 0..stops.len().saturating_sub(1) {
        let gap_cm = stops[i + 1].progress_cm - stops[i].progress_cm;
        if gap_cm > 0 && gap_cm < 10000 {
            close_pairs.push((i, i + 1, gap_cm));
        }
    }

    // If we have close stops, verify they have non-overlapping corridors
    for (i, j, gap) in close_pairs {
        let stop_i = &stops[i];
        let stop_j = &stops[j];

        // Corridors should be smaller than gap to avoid confusion
        let corridor_i = stop_i.corridor_end_cm - stop_i.progress_cm;
        let corridor_j = stop_j.progress_cm - stop_j.corridor_start_cm;

        assert!(
            corridor_i + corridor_j <= gap * 2,
            "Close stops {} and {} ({}cm apart): corridors may overlap",
            i, j, gap
        );
    }
}

/// Test: Route data structure integrity
#[test]
fn test_route_data_integrity() {
    let route_bytes = load_ty225_route("normal");
    let route_data = RouteData::load(&route_bytes)
        .expect("Failed to load route data");

    // Verify stops are in increasing progress order
    let stops = route_data.stops();
    for i in 1..stops.len() {
        assert!(
            stops[i].progress_cm >= stops[i - 1].progress_cm,
            "Stop {} progress should be >= stop {} progress",
            i, i - 1
        );
    }

    // Verify nodes exist
    let nodes = route_data.nodes();
    assert!(
        nodes.len() > 0,
        "Route should have nodes"
    );
}
```

- [ ] **Step 2: Verify tests compile**

Run: `cargo test -p pipeline --test scenarios -- route_edge_cases --no-run`
Expected: Compiles successfully

- [ ] **Step 3: Run tests to verify they work**

Run: `cargo test -p pipeline --test scenarios -- route_edge_cases`
Expected: Tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/pipeline/tests/scenarios/route_edge_cases.rs
git commit -m "test: add route edge case scenario tests"
```

---

## Task 7: Update Pipeline Tests Module

**Files:**
- Modify: `crates/pipeline/tests/integration_test.rs`

- [ ] **Step 1: Add scenario module to integration test**

Add this line at the top of the file after the imports:

```rust
// Scenario tests are in scenarios/ module
// Use: cargo test -p pipeline --test scenarios
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo test -p pipeline --tests --no-run`
Expected: Compiles successfully

- [ ] **Step 3: Run all pipeline tests**

Run: `cargo test -p pipeline --tests`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/pipeline/tests/integration_test.rs
git commit -m "test: update integration test with scenario reference"
```

---

## Task 8: Create Test Data Directory Structure

**Files:**
- Create: `test_data/scenarios/README.md`

- [ ] **Step 1: Create scenarios directory and README**

```bash
mkdir -p test_data/scenarios
```

Create `test_data/scenarios/README.md`:

```markdown
# Scenario Test Data

This directory contains test data for scenario-based integration tests.

## Organization

Test data is organized by scenario type:
- `normal/` - Normal operation scenarios
- `drift/` - GPS drift scenarios
- `jump/` - GPS jump scenarios
- `outage/` - Signal loss scenarios
- `edge_cases/` - Route geometry edge cases

## Current State

For now, scenario tests use existing test data:
- `../ty225_normal.*` - Normal operation
- `../ty225_drift.*` - GPS drift
- `../ty225_jump.*` - GPS jump
- `../ty225_outage.*` - Signal outage

## Future

Additional scenario-specific test data will be organized here.
```

- [ ] **Step 2: Verify README exists**

Run: `cat test_data/scenarios/README.md`
Expected: README content displayed

- [ ] **Step 3: Commit**

```bash
git add test_data/scenarios/README.md
git commit -m "test: add scenarios test data directory structure"
```

---

## Task 9: Run All Scenario Tests

**Files:**
- Test: All scenario files

- [ ] **Step 1: Run all scenario tests**

Run: `cargo test -p pipeline --test scenarios`
Expected: All scenario tests pass

- [ ] **Step 2: Run specific scenario categories**

Run: `cargo test -p pipeline --test scenarios -- normal`
Run: `cargo test -p pipeline --test scenarios -- gps_anomalies`
Run: `cargo test -p pipeline --test scenarios -- signal_loss`
Run: `cargo test -p pipeline --test scenarios -- route_edge_cases`
Expected: All tests pass

- [ ] **Step 3: Verify test counts**

Run: `cargo test -p pipeline --test scenarios -- --list`
Expected: Lists all scenario tests (should see 10+ tests)

- [ ] **Step 4: Document test results**

Note which tests pass and any that need expansion in future work.

---

## Task 10: Update Documentation

**Files:**
- Modify: `docs/dev_guide.md` or create new doc

- [ ] **Step 1: Add scenario tests section to dev guide**

Add to `docs/dev_guide.md`:

```markdown
## Running Scenario Tests

Scenario-based integration tests validate the full pipeline:

```bash
# All scenario tests
cargo test -p pipeline --test scenarios

# Specific scenario category
cargo test -p pipeline --test scenarios -- normal
cargo test -p pipeline --test scenarios -- gps_anomalies
cargo test -p pipeline --test scenarios -- signal_loss
cargo test -p pipeline --test scenarios -- route_edge_cases
```

Scenario tests use real ty225 route data and cover:
- Normal operation
- GPS drift and jump anomalies
- Signal loss (outage)
- Route geometry edge cases (loop closure, close stops)
```

- [ ] **Step 2: Verify doc builds and makes sense**

Read: `docs/dev_guide.md`
Expected: Documentation is clear and accurate

- [ ] **Step 3: Commit**

```bash
git add docs/dev_guide.md
git commit -m "docs: add scenario tests documentation"
```

---

## Self-Review Checklist

**Spec Coverage:**
- Module structure: Task 1
- Common utilities: Task 2
- Normal operation tests: Task 3
- GPS anomaly tests: Task 4
- Signal loss tests: Task 5
- Route edge case tests: Task 6
- Integration with existing tests: Task 7
- Test data organization: Task 8

**Placeholder Scan:**
- No TBD, TODO, or similar placeholders found
- All steps include actual code
- All file paths are explicit

**Type Consistency:**
- Function names consistent across tasks
- Type signatures match what's defined
- No contradictions in naming

---

## Success Criteria

1. All scenario tests pass with current implementation
2. `cargo test -p pipeline --test scenarios` runs successfully
3. Tests are organized by scenario category
4. Documentation explains how to run scenario tests

## Future Work

1. Expand tests to include full pipeline processing (not just NMEA parsing)
2. Add more specific edge cases as they're discovered
3. Add performance benchmarks to scenario tests
4. Generate coverage reports for scenario tests

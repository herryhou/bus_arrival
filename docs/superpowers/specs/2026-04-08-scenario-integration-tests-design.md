# Scenario Integration Tests Design

**Date:** 2026-04-08
**Status:** Draft
**Priority:** High

## Overview

Add comprehensive scenario-based integration tests to validate the full bus arrival detection pipeline using real route data. The tests will cover normal operation, GPS anomalies, signal loss, and route geometry edge cases.

## Problem Statement

Recent debugging revealed gaps in test coverage:
- Route geometry issues (stops 56-57 with large projection errors)
- Loop closure handling for circular routes
- GPS anomaly scenarios (drift, jump, outage)
- Lack of end-to-end integration validation

Existing tests are primarily unit-level or fragmented across multiple modules. We need integration-level tests that validate the complete pipeline with real route data.

## Design

### Module Structure

```
crates/pipeline/tests/scenarios/
├── mod.rs                    # Test module entry point
├── common/
│   └── mod.rs               # Shared scenario test utilities
├── normal.rs                # Normal operation scenarios
├── gps_anomalies.rs         # Drift, jump scenarios
├── signal_loss.rs           # Outage, tunnel scenarios
└── route_edge_cases.rs      # Route geometry edge cases
```

### Common Utilities

**File:** `crates/pipeline/tests/scenarios/common/mod.rs`

```rust
/// Load ty225 route data for scenario testing
pub fn load_ty225_route() -> RouteData {
    // Loads test_data/ty225_normal.bin or similar
}

/// Scenario execution and validation
pub fn run_scenario(
    name: &str,
    nmea_file: &str,
    expected: &ExpectedResults
) -> TestResult {
    // Runs NMEA through pipeline, validates against expected
}

/// Expected results for scenario validation
pub struct ExpectedResults {
    pub arrivals: Vec<ArrivalEvent>,
    pub active_stops_count: usize,
    pub max_projection_error: i32,  // cm
}

/// Result type for scenario tests
pub type TestResult = Result<(), String>;
```

### Scenario Test Files

#### normal.rs - Normal Operation

```rust
#[test]
fn test_normal_complete_route() {
    // Bus drives entire ty225 route
    // Validates: all stops detected, correct arrival order
}

#[test]
fn test_normal_with_dwell_times() {
    // Validates dwell time detection at each stop
}

#[test]
fn test_normal_speed_profile() {
    // Validates arrival probability with varying speeds
}
```

#### gps_anomalies.rs - GPS Drift and Jump

```rust
#[test]
fn test_drift_recovery() {
    // GPS drifts 100m over time
    // Validates: recovery algorithm corrects position
}

#[test]
fn test_jump_skip_stop_prevention() {
    // GPS jumps past a stop
    // Validates: no false arrival for skipped stops
}

#[test]
fn test_jump_backward_recovery() {
    // GPS jumps backward (progress decreases)
    // Validates: recovery handles reverse jumps
}
```

#### signal_loss.rs - GPS Outage

```rust
#[test]
fn test_outage_dead_reckoning() {
    // 10s GPS outage
    // Validates: dead reckoning maintains position
}

#[test]
fn test_outage_threshold_exceeded() {
    // GPS lost > 10s
    // Validates: enters GPS_LOST state
}

#[test]
fn test_outage_recovery() {
    // GPS returns after outage
    // Validates: system resyncs correctly
}
```

#### route_edge_cases.rs - Route Geometry

```rust
#[test]
fn test_loop_closure_detection() {
    // Stop at route loop closure
    // Validates: stop at completion point detected
}

#[test]
fn test_large_projection_error_handling() {
    // Stops with > 30m projection errors
    // Validates: system handles gracefully
}

#[test]
fn test_close_stop_discrimination() {
    // Two stops < 100m apart
    // Validates: correct stop discrimination
}
```

### Test Data Organization

```
test_data/scenarios/
├── normal/
│   ├── complete_route.nmea
│   ├── complete_route_gt.json
│   └── ...
├── drift/
│   ├── drift_50m.nmea
│   ├── drift_100m.nmea
│   └── ...
├── jump/
│   ├── jump_forward.nmea
│   ├── jump_backward.nmea
│   └── ...
├── outage/
│   ├── outage_5s.nmea
│   ├── outage_10s.nmea
│   ├── outage_15s.nmea
│   └── ...
└── edge_cases/
    ├── loop_closure.nmea
    ├── close_stops.nmea
    └── ...
```

### Integration with Existing Tests

1. **Reuse existing utilities:** `crates/pipeline/*/tests/common/mod.rs`
2. **Reuse existing data:** `test_data/ty225_*` files where applicable
3. **New data generation:** Use `tools/gen_nmea/gen_nmea.js` for missing scenarios

**Running tests:**
```bash
# All scenario tests
cargo test -p pipeline --test scenarios

# Specific scenario file
cargo test -p pipeline --test scenarios -- normal

# Individual test
cargo test -p pipeline --test scenarios -- test_normal_complete_route
```

## Implementation Notes

### Data Generation

Use existing `gen_nmea.js` tool with scenario parameters:

```bash
# Drift scenario
node tools/gen_nmea/gen_nmea.js generate \
  --route test_data/ty225_route.json \
  --stops test_data/ty225_stops.json \
  --scenario drift \
  --out_nmea test_data/scenarios/drift/drift_100m.nmea

# Jump scenario
node tools/gen_nmea/gen_nmea.js generate \
  --scenario jump \
  --out_nmea test_data/scenarios/jump/jump_forward.nmea

# Outage scenario
node tools/gen_nmea/gen_nmea.js generate \
  --scenario outage \
  --outage_start_seg 15 \
  --out_nmea test_data/scenarios/outage/outage_10s.nmea
```

### Validation Criteria

Each scenario test validates:
1. **Arrival detection:** Correct stops detected at correct times
2. **State machine:** Proper state transitions
3. **Probability scores:** Bayesian fusion outputs reasonable values
4. **Recovery behavior:** Proper handling of anomalies
5. **Performance:** Execution time within budget (optional)

### Dependencies

- Existing `crates/pipeline` modules
- Existing `crates/shared` types
- `test_data/ty225_*` route and stop files
- `tools/gen_nmea/gen_nmea.js` for data generation

## Success Criteria

1. All scenario tests pass with current implementation
2. Tests catch regressions when code changes break functionality
3. Test execution completes in reasonable time (< 30s for all scenarios)
4. CI pipeline includes scenario tests

## Future Enhancements

1. Add performance benchmarks to scenario tests
2. Generate coverage reports for scenario tests
3. Add visual trace output for failed tests
4. Parametrize tests for multiple route types (beyond ty225)

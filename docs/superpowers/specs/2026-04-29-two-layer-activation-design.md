# Activate Two-Layer Architecture Design

## Context

The firmware codebase contains two divergent full-pipeline implementations:

1. **OLD (active)**: `state.rs` - Monolithic `State::process_gps()` (770 lines)
2. **NEW (dead code)**: `control/mod.rs` + `estimation/mod.rs` - Two-layer architecture

The new v9.0 architecture exists but is never invoked from `main.rs`. The `SystemState::run_detection()` method is a TODO stub that unconditionally returns `None`. This design activates the new architecture while preserving all functionality.

## Goal

Activate the documented v9.0 two-layer architecture with complete feature parity to the old implementation. The new design provides better separation of concerns and matches the architecture described in technical documentation.

## Architecture

### Layer Separation

```
main.rs
  │
  ├─> NMEA parsing → GpsPoint
  │
  └─> SystemState::tick(gps, estimation_state)
        │
        ├─> Estimation Layer (pure function)
        │     └─> estimate(input, state) → EstimationOutput
        │
        ├─> Control Layer (state machine)
        │     ├─> Mode transitions (Normal/OffRoute/Recovering)
        │     ├─> Recovery orchestration
        │     └─> Monotonic enforcement
        │
        └─> Detection Layer (FSM)
              ├─> find_active_stops()
              ├─> compute_arrival_probability_adaptive()
              └─> StopState transitions → ArrivalEvent
```

### Data Flow

```
GPS NMEA sentence
  → NmeaState::parse_sentence() → Option<GpsPoint>
  → SystemState::tick(gps, estimation_state)
    → EstimationInput::new(gps, route_data, is_first_fix)
    → estimate(input, &mut estimation_state) → EstimationOutput
      { s_cm, v_cms, divergence_d2, confidence, has_fix }
    → Check mode transitions (Normal ↔ OffRoute ↔ Recovering)
    → Enforce monotonic invariant
    → If Recovering: attempt_recovery() → Option<usize>
    → If Normal: run_detection() → Option<ArrivalEvent>
  → Some(ArrivalEvent) → UART output
```

## Components

### Estimation Layer (`estimation/mod.rs`)

**Status:** Already complete

**Purpose:** Pure function converting GPS + route data to position signals

**Contract:**
- Input: GPS + route (no control layer state)
- Output: Position signals (no side effects to control layer)
- Internal state: Kalman + DR (opaque to control layer)
- Deterministic: Same GPS input → same EstimationOutput

**Output Structure:**
```rust
pub struct EstimationOutput {
    pub z_gps_cm: DistCm,      // Raw GPS projection (for F1 probability)
    pub s_cm: DistCm,          // Kalman-filtered position
    pub v_cms: SpeedCms,       // Filtered velocity
    pub divergence_d2: Dist2,  // Divergence from route
    pub confidence: u8,        // Quality signal (0-255)
    pub has_fix: bool,         // GPS validity
}
```

### Control Layer (`control/mod.rs`)

**Status:** Partially complete, needs detection integration

**Purpose:** State machine orchestration and detection coordination

**Key Responsibilities:**
- Mode management (Normal/OffRoute/Recovering)
- Recovery orchestration
- Monotonic invariant enforcement
- Detection gating (warmup/timeout)
- Stop state persistence

**State Fields to Add:**
```rust
pub stop_states: heapless::Vec<StopState, 256>,  // Per-stop FSM
pub first_fix: bool,                              // First GPS fix flag
pub estimation_ready_ticks: u8,                   // Estimation warmup
pub detection_enabled_ticks: u8,                  // Detection warmup
pub just_reset: bool,                             // Post-reset flag
pub just_snapped_ticks: u8,                       // Snap cooldown
pub last_gps_timestamp: u64,                      // For recovery dt
```

**Methods to Implement:**
- `run_detection()` - Full arrival detection FSM (currently a TODO stub)
- `reset_stop_states_after_recovery()` - Stop state reset logic
- `estimation_ready()` - Check if estimation converged
- `detection_ready()` - Check if detection allowed
- `find_closest_stop_index()` - Geometric fallback
- `find_forward_closest_stop_index()` - Forward-only search

### Detection Layer

**Status:** Functions exist in `detection.rs`, needs integration into SystemState

**Purpose:** Bayesian arrival detection with finite state machine

**Integration Point:** `SystemState::run_detection()`

**Flow:**
1. Find active stops via corridor filter
2. For each active stop:
   - Compute arrival probability with adaptive weights
   - Update StopState FSM
   - Check for Arrival/Departure/Announce events
3. Return first event (early exit on event emission)

## Implementation

### Phase 1: SystemState Structure

Add missing fields to `SystemState` struct in `control/mod.rs`:
- `stop_states` array
- Warmup counters
- Snap cooldown tracking
- First fix and reset flags
- GPS timestamp tracking

### Phase 2: Detection Integration

Implement `run_detection()` method:
- Call `detection::find_active_stops()` with position signals
- For each active stop:
  - Get next stop for adaptive weights
  - Call `compute_arrival_probability_adaptive()`
  - Update `StopState` via `stop_state.update()`
  - Check `should_announce()` for announcement events
  - Check `StopEvent` for arrival/departure events
- Return first event (early exit)

### Phase 3: Stop State Reset

Implement `reset_stop_states_after_recovery()`:
- Reset all stops to `Idle`
- Mark stops before recovered index as `Departed` (preserve announced flag)
- Set recovered stop to `Approaching` if within corridor
- Preserve `announced` flag if stop was previously announced

### Phase 4: Helper Methods

Add helper methods to `SystemState`:
- `estimation_ready() -> bool` - Check warmup or timeout
- `detection_ready() -> bool` - Check warmup or timeout
- `disable_heading_filter() -> bool` - First fix or not ready
- `find_closest_stop_index(s_cm)` - Geometric search
- `find_forward_closest_stop_index(s_cm, last_idx)` - Forward-only search

### Phase 5: Warmup Logic in tick()

Add warmup tracking to `SystemState::tick()`:
- Track `estimation_ready_ticks` and `estimation_total_ticks`
- Track `detection_enabled_ticks` and `detection_total_ticks`
- Reset counters on GPS outage (> 10 seconds)
- Block detection until ready (3 ticks or 10 tick timeout)
- Handle `first_fix` initialization

### Phase 6: Stop State Reset Calls

Wire stop state reset into recovery paths:
- Call in `recovery_success()` after successful recovery
- Call in timeout fallback path
- Use same logic as old `state.rs:635-673`

### Phase 7: main.rs Migration

Update `main.rs` to use new architecture:
- Import `control::SystemState` and `estimation::EstimationState`
- Replace `state::State` instantiation with `SystemState` + `EstimationState`
- Replace `state.process_gps(&gps)` with `system_state.tick(&gps, &mut estimation_state)`
- Update persistence calls to use `SystemState` methods

### Phase 8: Snap Cooldown

Add snap cooldown logic:
- Set `just_snapped_ticks = 2` on successful off-route snap re-entry
- Decrement counter each tick when > 0
- Pass `in_snap_cooldown` flag to recovery logic to prevent false jump detection

### Phase 9: Testing

- Unit tests for new `SystemState` methods
- Integration tests for mode transitions
- Scenario validation: `make run ROUTE_NAME=ty225 SCENARIO=normal`
- Trace comparison against old implementation
- Verify arrivals/departures match expected output

### Phase 10: Cleanup

Once migration is verified working:
- Mark `state.rs` as deprecated with `#[allow(dead_code)]`
- Document migration in commit message
- Consider removing `state.rs` in future commit

## Key Design Decisions

1. **NMEA parsing stays in main.rs** - I/O concerns separated from business logic
2. **Stop states live in SystemState** - Detection needs persistent FSM state per tick
3. **Estimation is pure** - No access to control layer state, deterministic output
4. **Single transition per tick** - Prevents race conditions in mode changes
5. **Detection blocked until ready** - 3 tick warmup or 10 tick timeout safety valve
6. **Preserve exact behavior** - Match old implementation for warmup, recovery, persistence

## Testing Strategy

- **Unit tests**: New `SystemState` methods (ready checks, stop finding)
- **Integration tests**: Mode transitions, recovery paths
- **Scenario tests**: Full pipeline with test data
- **Trace validation**: Compare output against old implementation
- **Behavioral verification**: Arrivals/departures match expected

## Success Criteria

1. Code compiles for target: `cargo build --release --target thumbv8m.main-none-eabi -p pico2-firmware`
2. All tests pass: `cargo test -p pico2-firmware`
3. Scenario tests produce valid output: `make run ROUTE_NAME=ty225 SCENARIO=normal`
4. Trace matches old implementation (within expected variance)
5. No dead code warnings for `SystemState` and `estimate()`
6. Old `state.rs` marked as deprecated

## Files Modified

- `crates/pico2-firmware/src/control/mod.rs` - Add detection integration and missing state
- `crates/pico2-firmware/src/estimation/mod.rs` - Already complete (no changes)
- `crates/pico2-firmware/src/main.rs` - Switch to new architecture
- `crates/pico2-firmware/src/state.rs` - Mark deprecated (future: remove)

## References

- Old implementation: `crates/pico2-firmware/src/state.rs` (lines 1-770)
- New control layer: `crates/pico2-firmware/src/control/mod.rs`
- New estimation layer: `crates/pico2-firmware/src/estimation/mod.rs`
- Detection module: `crates/pico2-firmware/src/detection.rs`
- Tech documentation: `docs/bus_arrival_tech_report_v8.md`

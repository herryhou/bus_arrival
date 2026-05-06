# v9.0 Architecture Integration Design

**Date:** 2026-05-06
**Status:** Draft
**Author:** AI + Human collaboration

## Problem Statement

The v9.0 two-layer architecture (described in the tech report) is implemented but **completely unintegrated**:

- `SystemState::run_detection()` is a stub that always returns `None`
- `main.rs` uses `state::State` (old monolithic path), not `SystemState`
- The new `EstimationState` layer exists but isn't used in production
- Tech report describes v9.0 as current, but firmware runs pre-v9.0 code

This is a critical documentation/code drift issue that must be resolved.

## Solution

Complete the v9.0 integration by:
1. Implementing full detection logic in `SystemState::run_detection()`
2. Migrating all control logic from `state::State` to `SystemState`
3. Switching `main.rs` to use the two-layer architecture
4. Deleting `state::State` entirely (aggressive migration)

## Architecture

### Target Structure

```
┌─────────────────────────────────────────────────────────────┐
│                        main.rs                               │
│  ┌──────────────────────────────────────────────────────┐   │
│  │              SystemState (control layer)              │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌──────────────┐  │   │
│  │  │   Modes     │  │  Warmup     │  │  Detection   │  │   │
│  │  │ (Normal/    │  │  Logic      │  │  FSM +       │  │   │
│  │  │  OffRoute/  │  │             │  │  stop_states │  │   │
│  │  │ Recovering) │  │             │  │              │  │   │
│  │  └─────────────┘  └─────────────┘  └──────────────┘  │   │
│  │  ┌─────────────────────────────────────────────────┐  │   │
│  │  │         GPS Jump Recovery + Snap Handling       │  │   │
│  │  └─────────────────────────────────────────────────┘  │   │
│  │  ┌─────────────────────────────────────────────────┐  │   │
│  │  │         Persistence + Rate Limiting             │  │   │
│  │  └─────────────────────────────────────────────────┘  │   │
│  └──────────────────────────────────────────────────────┘   │
│                            │                                 │
│                            ▼                                 │
│  ┌──────────────────────────────────────────────────────┐   │
│  │        EstimationState (estimation layer)            │   │
│  │  ┌─────────────┐  ┌─────────────┐                    │   │
│  │  │   Kalman    │  │      DR     │                    │   │
│  │  │   Filter    │  │  (EMA, DR)  │                    │   │
│  │  └─────────────┘  └─────────────┘                    │   │
│  │             Pure function: estimate(input) → output  │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

### Layer Responsibilities

**Estimation Layer (EstimationState):**
- Pure GPS → position pipeline
- Kalman filtering + dead-reckoning
- No access to mode, stops, or control state
- Contract: Same input → same output (deterministic)

**Control Layer (SystemState):**
- Mode management (Normal/OffRoute/Recovering)
- Warmup logic (estimation readiness, detection gating)
- Detection FSM with stop_states
- GPS jump recovery (reactive)
- Off-route snap handling (reactive)
- Persistence + rate limiting
- Calls estimation layer, never accesses its internals directly

### Recovery Separation

Two distinct recovery paths with different semantics:

**Recovery Path Matrix:**

| Trigger | Mode | Function | Search Direction | Called From |
|---------|------|----------|------------------|-------------|
| Timeout (>30s off-route) | Recovering | `recovery::recover(RecoveryInput)` | Bidirectional ±10 stops | `SystemState::attempt_recovery()` |
| GPS jump (sudden large displacement) | Normal | `detection::recovery::find_stop_index()` | Bidirectional, all stops | `SystemState::tick()` after jump detected |
| Off-route snap (re-entry with snap=true) | Normal → Normal | `find_forward_closest_stop_index()` | Forward-only from last_idx | `SystemState::tick()` when snap detected |
| Re-acquisition (after OffRoute without snap) | Normal | `detection::recovery::find_stop_index()` | Bidirectional ±10 stops | `SystemState::tick()` when `needs_recovery_on_reacquisition` set |

**1. Timeout Recovery (OffRoute → Recovering mode):**
- Triggered: Lost for >30 seconds
- Function: `recovery::recover(RecoveryInput)`
- Semantics: Fallback best-effort search
- Search: Bidirectional ±10 stops from hint

**2. Reactive Recovery (GPS jump / snap / re-acquisition):**
- Triggered: Sudden position jump or off-route re-entry
- Function: Direct call to `detection::recovery::find_stop_index()` or forward search
- Semantics: Fast reactive fix with fresh GPS
- Search: Jump = bidirectional, Snap = forward-only

## Component Specification

### EstimationState

No changes needed — already correctly implemented.

```rust
pub struct EstimationState {
    pub kalman: KalmanState,
    pub dr: DrState,
}

pub fn estimate(
    input: EstimationInput,
    state: &mut EstimationState,
) -> EstimationOutput
```

### SystemState

**New fields to add:**

```rust
pub struct SystemState<'a> {
    // === Existing fields ===
    pub mode: SystemMode,
    pub last_stop_index: u8,
    pub frozen_s_cm: Option<DistCm>,
    pub off_route_clear_ticks: u8,
    pub off_route_suspect_ticks: u8,
    pub off_route_since: Option<u64>,
    pub recovering_since: Option<u64>,
    pub recovery_failed: bool,
    pub route_data: &'a RouteData<'a>,
    pub pending_persisted: Option<shared::PersistedState>,
    pub last_persisted_stop: u8,
    pub ticks_since_persist: u16,
    pub last_s_cm: DistCm,
    pub backward_jump_count: u32,
    has_received_first_fix: bool,

    // === NEW: Detection FSM ===
    pub stop_states: heapless::Vec<detection::state_machine::StopState, 256>,

    // === NEW: Warmup counters ===
    estimation_ready_ticks: u8,
    estimation_total_ticks: u8,
    detection_enabled_ticks: u8,
    detection_total_ticks: u8,
    just_reset: bool,

    // === NEW: GPS jump recovery tracking ===
    last_valid_s_cm: DistCm,
    last_gps_timestamp: u64,
    needs_recovery_on_reacquisition: bool,

    // === NEW: Snap cooldown ===
    just_snapped_ticks: u8,
}
```

**Memory constraint (compile-time verified):**
```rust
const _: () = assert!(size_of::<SystemState>() <= 4096, "SystemState exceeds 4KB SRAM budget");
```

**New methods to implement:**

```rust
// Warmup queries
pub fn estimation_ready(&self) -> bool
pub fn detection_ready(&self) -> bool
pub fn disable_heading_filter(&self) -> bool

// Stop search helpers
pub fn find_closest_stop_index(&self, s_cm: DistCm) -> u8
pub fn find_forward_closest_stop_index(&self, s_cm: DistCm, last_idx: u8) -> u8

// FSM management
fn reset_stop_states_after_recovery(&mut self, recovered_idx: usize, current_s_cm: DistCm)

// Persistence (interface already exists, implementation needs migration)
pub fn should_persist(&self, current_stop: u8) -> bool
pub fn mark_persisted(&mut self, stop_index: u8)
pub fn current_stop_index(&self) -> Option<u8>

// Detection (currently a stub, needs full implementation)
fn run_detection(&mut self, est: &EstimationOutput, s_cm: DistCm, timestamp: u64) -> Option<ArrivalEvent>
```

**`run_detection()` implementation steps:**

1. Find active stops via `detection::find_active_stops(signals, route_data)`
2. For each active stop:
   - Get stop and stop_state
   - Get next_stop for adaptive weights
   - Compute probability via `compute_arrival_probability_adaptive()`
   - Update FSM via `stop_state.update()` → `StopEvent`
   - Check `should_announce()` → return `ArrivalEvent::Announce` if true
   - Return `ArrivalEvent::Arrived` or `ArrivalEvent::Departure` if event
3. Return `None` if no events

**Async persistence interface:**

```rust
/// Return value from tick() - separates sync logic from async persistence
pub struct TickResult {
    /// Arrival/departure/announce event if any
    pub event: Option<ArrivalEvent>,
    /// Persist request if stop index changed and rate limit allows
    pub persist_request: Option<shared::PersistedState>,
}

/// Main tick function - returns TickResult for async handling in main()
pub fn tick(&mut self, gps: &GpsPoint, est_state: &mut EstimationState) -> TickResult
```

## Data Flow

**Per GPS tick:**

```
1. main() receives NMEA
   └─> Accumulate → GpsPoint
       └─> SystemState::tick(gps, &mut EstimationState) → TickResult

2. SystemState::tick():
   ├─> Estimation.estimate() → EstimationOutput
   ├─> enforce_monotonic() → s_cm_for_detection
   ├─> Mode transition checks (Normal ↔ OffRoute ↔ Recovering)
   ├─> If Recovering: recovery::recover()
   ├─> If Normal:
   │   ├─> Check warmup
   │   ├─> Check GPS jump → find_stop_index() if needed
   │   ├─> Check snap → forward_closest_stop() if needed
   │   └─> run_detection() → FSM → ArrivalEvent
   ├─> Check persistence → build PersistedState if needed
   └─> Return TickResult { event, persist_request }

3. main() handles TickResult:
   ├─> If event: uart::write_arrival_event_async(event)
   └─> If persist_request: persist::save(persist_request).await

4. main() calls mark_persisted() after successful save
```

**Invariants:**
- Estimation layer is pure (no side effects to control state)
- Only ONE mode transition per tick
- Detection ONLY in Normal mode after warmup
- Recovery ONLY in Recovering mode (timeout) or after jump/snap (reactive)

## Migration Strategy

### Phase 1: Prepare SystemState

1. Add new fields to `SystemState` (stop_states, warmup counters, recovery tracking, needs_recovery_on_reacquisition)
2. Implement helper methods (warmup, stop search, persistence)
3. Implement `run_detection()` with full FSM logic (5-step process documented above)
4. Integrate GPS jump and snap handling into `tick()`
5. Add `first_fix` handling and persisted state application
6. Change `tick()` return type to `TickResult` for async persistence
7. Add compile-time size check: `assert!(size_of::<SystemState>() <= 4096)`

### Phase 2: Port and Extend Tests

1. Port all tests from `state.rs` to `control/mod.rs`
2. Add new architecture tests:
   - Estimation isolation/determinism
   - Single transition invariant
   - Mode-specific position correctness
   - Recovery path separation
3. Add integration tests for full pipeline

### Phase 3: Switch and Delete

1. Update `main.rs` to use `SystemState` + `EstimationState`
2. Update main loop to handle `TickResult`:
   ```rust
   let result = control.tick(&gps, &mut est_state);
   if let Some(event) = result.event {
       uart::write_arrival_event_async(&mut uart, &event).await;
   }
   if let Some(ps) = result.persist_request {
       persist::save(&mut flash, &ps).await;
       control.mark_persisted(ps.last_stop_index);
   }
   ```
3. Update `lib.rs` re-exports (remove state::State)
4. Delete `state.rs` entirely
5. Run full test suite
6. Verify memory usage with `cargo size --bin pico2-firmware`
7. Flash and test on hardware

### Phase 4: Cleanup

1. Remove dead code and unused imports
2. Update tech report to reflect implemented architecture
3. Update CLAUDE.md if needed

## Testing Strategy

### Unit Tests (in-module)

- `estimation/`: Existing tests, expand coverage
- `control/`: Mode transitions, monotonic enforcement, warmup, stop search, persistence

### Integration Tests (port from state.rs)

- GPS → detection → arrival happy path
- GPS jump recovery
- Off-route snap handling
- Recovery timeout fallback
- Warmup blocking
- Persistence rate limiting
- Backward jump detection

### Architecture Tests (new)

- Estimation isolation (no access to control state)
- Estimation determinism (same input → same output)
- Single transition invariant
- Mode-specific position correctness
- Recovery path separation

### Host-based Tests

- Full trace generation with test NMEA
- Golden file comparison
- Memory usage verification (<1KB SRAM)

### Test Execution

```bash
# Unit tests
cargo test --lib

# Integration tests
cargo test

# Trace validation
cargo run --bin trace_validator

# Generate golden files for regression testing
make golden

# Hardware test
elf2flash && flash && monitor
```

### Golden File Generation

Add to `Makefile`:

```makefile
# Generate golden trace files for regression testing
golden:
	@echo "Generating golden traces..."
	cargo run --bin trace_validator -- --generate-golden
	@echo "Golden files updated in test_data/golden/"
```

This target:
- Runs the firmware on test NMEA files
- Captures output trace.jsonl
- Saves to `test_data/golden/` for regression comparison

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Breaking working detection | Port ALL existing tests before deleting state::State |
| Missing edge cases | Add architecture tests for isolation/determinism |
| Memory regression | Compile-time `assert!(size_of::<SystemState>() <= 4096)`, verify with `cargo size` |
| Async persistence mismatch | `TickResult` struct separates sync logic from async save |
| Hardware bugs | Real-world testing after host tests pass |
| Persistence after reboot | Test power cycle recovery explicitly |

## Success Criteria

**Functional:**
1. All existing tests pass with `SystemState`
2. New architecture tests pass (isolation, determinism, single transition)
3. Trace validation matches reference (golden file comparison)
4. Hardware testing produces correct arrivals
5. Persistence works after reboot (state survives power cycle)
6. `state::State` is deleted
7. Tech report accurately describes implementation

**Performance:**
8. Memory usage ≤ current (verify with `cargo size --bin pico2-firmware`)
9. No performance regression (CPU < 8% @ 150MHz, 1Hz GPS)
10. Compile-time size check passes (`assert!(size_of::<SystemState>() <= 4096)`)

**Compatibility:**
11. Backward compatible with existing `route_data.bin` format
12. No changes to NMEA input format
13. No changes to UART arrival event output format

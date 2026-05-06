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

**1. Timeout Recovery (OffRoute → Recovering mode):**
- Triggered: Lost for >30 seconds
- Function: `recovery::recover(RecoveryInput)`
- Semantics: Fallback best-effort search
- Search: Bidirectional ±10 stops from hint

**2. Reactive Recovery (GPS jump / snap):**
- Triggered: Sudden position jump or off-route re-entry
- Function: Direct call to `detection::recovery::find_stop_index()`
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

    // === NEW: Snap cooldown ===
    just_snapped_ticks: u8,
}
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

## Data Flow

**Per GPS tick:**

```
1. main() receives NMEA
   └─> Accumulate → GpsPoint
       └─> SystemState::tick(gps, &mut EstimationState)

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
   └─> Return Option<ArrivalEvent>

3. main() emits ArrivalEvent via UART
4. main() persists state if needed
```

**Invariants:**
- Estimation layer is pure (no side effects to control state)
- Only ONE mode transition per tick
- Detection ONLY in Normal mode after warmup
- Recovery ONLY in Recovering mode (timeout) or after jump/snap (reactive)

## Migration Strategy

### Phase 1: Prepare SystemState

1. Add new fields to `SystemState`
2. Implement helper methods (warmup, stop search, persistence)
3. Implement `run_detection()` with full FSM logic
4. Integrate GPS jump and snap handling into `tick()`
5. Add `first_fix` handling and persisted state application

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
2. Update `lib.rs` re-exports
3. Delete `state.rs` entirely
4. Run full test suite
5. Flash and test on hardware

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

# Hardware test
elf2flash && flash && monitor
```

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Breaking working detection | Comprehensive test coverage before deletion |
| Missing edge cases | Port ALL existing tests, add architecture tests |
| Memory regression | SRAM budget verification in tests |
| Hardware bugs | Real-world testing after host tests pass |

## Success Criteria

1. All existing tests pass with `SystemState`
2. New architecture tests pass
3. Trace validation matches reference
4. Hardware testing produces correct arrivals
5. `state::State` is deleted
6. Tech report accurately describes implementation

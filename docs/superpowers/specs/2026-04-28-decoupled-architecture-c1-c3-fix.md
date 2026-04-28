# Decoupled Architecture for C1/C3 Fix

## Overview

This design addresses **C1 (hidden coupling)** and **C3 (recovery overlap)** issues from the code review by introducing a **2-layer architecture**:

1. **Pure Estimation Layer** — Stateless GPS → position pipeline
2. **Control Layer** — State machine managing Normal/OffRoute/Recovering modes

**Key insight:** Recovery becomes a first-class system mode, not an inline side-effect.

---

## Problem Statement

### C1: Hidden Coupling Between Modules

**Current issues:**
- `last_known_stop_index` (detection state) passed into map matching
- `freeze_ctx` (Kalman state) used in recovery module
- Circular dependency: Detection → Kalman → Recovery → Detection

**Impact:** Non-local reasoning, hard to validate correctness, emergent bugs

### C3: Recovery Logic Overlaps with Normal Tracking

**Current issues:**
- Three recovery mechanisms in `state.rs`:
  1. GPS jump recovery (line 180-227)
  2. Off-route snap recovery (line 296-312)
  3. Re-acquisition recovery (line 323-360)
- All can potentially run in same tick
- No mutual exclusion enforcement

**Impact:** Oscillation, double-correction, index jumps

---

## Solution: 2-Layer Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Control Layer                          │
│  ┌──────────────────────────────────────────────────────┐  │
│  │              SystemState (state machine)              │  │
│  │  mode: SystemMode (Normal | OffRoute | Recovering)    │  │
│  │  last_stop_index: u8                                  │  │
│  │  frozen_s_cm: Option<DistCm}  ← Only OffRoute/Recov   │  │
│  │  off_route_clear_ticks: u8                            │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                         │
                         │ pure call
                         ▼
┌─────────────────────────────────────────────────────────────┐
│                   Estimation Layer (Pure)                    │
│  estimate() → EstimationOutput { s_cm, v_cms, z_gps_cm }    │
└─────────────────────────────────────────────────────────────┘
```

---

## Section 1: Core Data Structures

### SystemMode Enum

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemMode {
    Normal,
    OffRoute,
    Recovering,
}
```

### SystemState (Control Layer)

```rust
pub struct SystemState<'a> {
    pub mode: SystemMode,
    pub last_stop_index: u8,
    pub frozen_s_cm: Option<DistCm>,
    pub off_route_clear_ticks: u8,
    pub off_route_since: Option<u64>,
    pub route_data: &'a RouteData<'a>,
    pub stop_states: heapless::Vec<StopState, 256>,
    pub pending_persisted: Option<PersistedState>,
}
```

### EstimationInput/Output

```rust
pub struct EstimationInput<'a> {
    pub gps: GpsPoint,
    pub route_data: &'a RouteData<'a>,
    pub is_first_fix: bool,
}

pub struct EstimationOutput {
    pub z_gps_cm: DistCm,
    pub s_cm: DistCm,
    pub v_cms: SpeedCms,
    pub divergence_d2: Dist2,
    pub has_fix: bool,
}
```

### RecoveryInput

```rust
pub struct RecoveryInput<'a> {
    pub s_cm: DistCm,
    pub v_cms: SpeedCms,
    pub dt_seconds: u64,
    pub stops: heapless::Vec<Stop, 256>,
    pub hint_idx: u8,              // From control layer
    pub frozen_s_cm: Option<DistCm>,  // From control layer
}
```

---

## Section 2: State Machine Transitions

```
                    ┌───────────────────┐
                    │      Normal       │
                    └─────────┬─────────┘
                              │
               ┌──────────────┼──────────────┐
               │              │              │
    divergence  │              │              │  GPS re-acquired
    > threshold │              │              │  + hysteresis
    for 5 ticks │              │              │
               ▼              │              ▼
        ┌──────────────┐      │      ┌──────────────┐
        │   OffRoute   │◄─────┴─────┤ │  Recovering  │
        └──────────────┘             │ └──────────────┘
                                     │       │
                                     │       │ recovery
                                     │       │ success
                                     │       ▼
                                     │  ┌─────────┐
                                     │  │ Normal  │
                                     └───────────┘
```

### Transition Rules

**Normal → OffRoute:** `divergence_d2 > 25_000_000` for 5 ticks

**OffRoute → Normal:** `divergence_d2 <= 25_000_000` for 2 ticks

**OffRoute → Recovering:** GPS jumped >50m from frozen position

**Recovering → Normal:** Recovery successfully finds stop index

---

## Section 3: Pure Estimation Layer

### estimate() Function

```rust
pub fn estimate(
    input: EstimationInput,
    state: &mut EstimationState,
) -> EstimationOutput {
    // 1. Map matching (pure)
    let (seg_idx, match_d2) = map_match::find_best_segment_restricted(...);

    // 2. Project to route (pure)
    let z_gps_cm = map_match::project_to_route(...);

    // 3. Kalman filter (internal state only)
    let (s_cm, v_cms) = kalman::process_update(...);

    EstimationOutput { z_gps_cm, s_cm, v_cms, divergence_d2: match_d2, has_fix: true }
}
```

### Constraints

🚫 **The estimation layer MUST NOT:**
- Access `last_stop_index`
- Access `frozen_s_cm`
- Access `mode`
- Trigger recovery

✅ **The estimation layer MUST:**
- Be a pure function
- Return all derived signals explicitly

---

## Section 4: Pure Recovery Function

```rust
pub fn recover(input: RecoveryInput) -> Option<u8> {
    // Scoring: distance + index penalty + spatial anchor penalty
    // Uses input.hint_idx for index penalty
    // Uses input.frozen_s_cm for spatial anchoring (if Some)
}
```

### Key Changes

**Before (Coupled):**
```rust
find_stop_index(..., &kalman.freeze_ctx)  // ❌ Depends on Kalman
```

**After (Decoupled):**
```rust
recover(RecoveryInput {
    hint_idx: state.last_stop_index,  // ✅ From control layer
    frozen_s_cm: state.frozen_s_cm,    // ✅ From control layer
})
```

---

## Section 5: Tick Orchestrator

```rust
impl SystemState {
    pub fn tick(&mut self, gps: &GpsPoint, est_state: &mut EstimationState) -> Option<ArrivalEvent> {
        // STEP 1: Pure estimation
        let est = estimate(EstimationInput {...}, est_state);

        // STEP 2: State machine transitions
        match self.mode {
            Normal => { /* Check Normal → OffRoute */ }
            OffRoute => { /* Check OffRoute → Normal or → Recovering */ }
            Recovering => { /* Recovery handling */ }
        }

        // STEP 3: Recovery (ONLY in Recovering mode)
        if self.mode == Recovering {
            if let Some(idx) = self.attempt_recovery(&est) {
                self.recovery_success(idx);
            }
        }

        // STEP 4: Detection (ONLY in Normal mode)
        if self.mode == Normal {
            return self.run_detection(&est, gps);
        }
        None
    }
}
```

### Critical Invariant

**Recovery ONLY runs in `Recovering` mode:**

```rust
match state.mode {
    Normal => { /* 🚫 NO recovery */ }
    OffRoute => { /* 🚫 NO recovery */ }
    Recovering => { /* ✅ Recovery ONLY here */ }
}
```

---

## Section 6: Module Structure

### Before (Current)

```
crates/pico2-firmware/src/
├── state.rs              ❌ 730 lines — everything mixed
├── recovery_trigger.rs   ❌ GPS jump detection
└── detection/recovery.rs ❌ Depends on Kalman::freeze_ctx
```

### After (New)

```
crates/pico2-firmware/src/
├── control/              ✅ NEW: Control layer
│   ├── mod.rs            → SystemState, tick()
│   ├── mode.rs           → SystemMode + transitions
│   └── state_machine.rs  → Transition logic
├── estimation/           ✅ NEW: Estimation layer
│   ├── mod.rs            → estimate(), EstimationState
│   ├── kalman.rs         → Pure Kalman (no control state)
│   └── dr.rs             → Pure DR (no control state)
├── recovery/             ✅ NEW: Pure recovery
│   └── mod.rs            → recover() function
├── detection/            ✅ Unchanged (pure)
└── state.rs              ❌ DELETE (split into control/estimation)
```

---

## Section 7: Single Source of Truth

### Per-Mode Position Selection

```rust
fn current_position(state: &SystemState, est: &EstimationOutput) -> DistCm {
    match state.mode {
        Normal => est.s_cm,           // Kalman-filtered
        OffRoute => state.frozen_s_cm.expect(...),  // Frozen
        Recovering => est.z_gps_cm,   // Raw projection
    }
}
```

### Enforcement

```rust
// In control layer — frozen_s_cm only passed in OffRoute/Recovering
let input = RecoveryInput {
    frozen_s_cm: match state.mode {
        Normal => None,  // 🚫 Never used
        OffRoute | Recovering => state.frozen_s_cm,
    },
    ...
};
```

---

## Section 8: Migration Strategy

### Phase 1: Create New Structure (Non-Breaking)
- Add `control/`, `estimation/`, `recovery/` modules
- Add feature flag `decoupled_architecture` (disabled by default)

### Phase 2: Implement Pure Functions
- Implement `estimate()` and `recover()`
- Add unit tests for pure functions

### Phase 3: Implement Control Layer
- Implement `SystemState` and `tick()`
- Add integration tests

### Phase 4: Parallel Testing
- Run both old and new implementations
- Compare outputs, log discrepancies

### Phase 5: Switch Over
- Enable feature flag by default
- Remove old code

### Phase 6: Clean Up
- Remove parallel testing code
- Update documentation

---

## What This Fixes

### ✅ C1: Hidden Coupling

- `last_known_stop_index` no longer passed to map matching
- `freeze_ctx` removed from KalmanState
- Recovery receives all inputs explicitly

### ✅ C3: Recovery Overlap

- Recovery ONLY runs in `Recovering` mode
- Explicit state machine with mutual exclusion
- No double-trigger possible

---

## Additional Benefits

1. **Testability:** Pure functions are trivially testable
2. **Reasonability:** Clear separation of concerns
3. **Maintainability:** Each module has single responsibility
4. **Verifiability:** Invariants are enforceable
5. **Performance:** No regression expected (same algorithms)

---

## Constraints

- Integer-only arithmetic (per `00-constraints.md`)
- Memory budget: <1 KB SRAM, ~34 KB Flash
- CPU budget: <8% @ 150MHz for 1Hz GPS
- XIP (Execute-in-Place) for route data

---

## Testing

### Unit Tests
- `control/mode`: Transition conditions
- `estimation`: Pure function determinism
- `recovery`: Scoring with hint_idx, spatial anchor

### Integration Tests
- Normal operation
- Off-route detection and recovery
- GPS jump recovery
- GPS outage handling

### Regression Tests
- All existing integration tests must pass
- Scenario tests: normal, drift, outage, jump, off_route

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2026-04-28 | Initial design for C1/C3 fix |

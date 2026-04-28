# Decoupled Architecture for C1/C3 Fix

## Overview

This design addresses **C1 (hidden coupling)** and **C3 (recovery overlap)** issues from the code review by introducing a **2-layer architecture**:

1. **Isolated Estimation Layer** — GPS → position pipeline with internal Kalman/DR state
2. **Control Layer** — State machine managing Normal/OffRoute/Recovering modes

**Key insight:** Recovery becomes a first-class system mode, not an inline side-effect.

### Critical Design Decisions

**Fix 1: Transition Priority** — OffRoute mode checks transitions in priority order (Recovering > Normal) to prevent race conditions.

**Fix 2: Isolation Contract** — Estimation layer is "isolated" (bounded state) not "pure" (no state). Kalman maintains internal state but doesn't access control layer state.

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
                         │ isolated call (bounded internal state)
                         ▼
┌─────────────────────────────────────────────────────────────┐
│                 Estimation Layer (Isolated)                  │
│  estimate() → EstimationOutput { s_cm, v_cms, z_gps_cm }    │
│  Internal: KalmanState + DrState (opaque to control layer)  │
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
                    divergence │
                    > 50m for │
                    5 ticks   │
                              ▼
        ┌─────────────────────────────────────┐
        │           OffRoute                  │
        │                                     │
        │  ┌─────────────────────────────┐   │
        │  │   Check in PRIORITY order:   │   │
        │  │                             │   │
        │  │  1. Jump > 50m? → Recovering│   │
        │  │  2. Low divergence? → Normal │   │
        │  │  3. Else → Stay              │   │
        │  └─────────────────────────────┘   │
        └─────────────────────────────────────┘
                              │
                              │ recovery success
                              ▼
                    ┌───────────────────┐
                    │   Recovering      │
                    │   (find stop)     │
                    └─────────┬─────────┘
                              │
                              │ success
                              ▼
                    ┌───────────────────┐
                    │      Normal       │
                    └───────────────────┘
```

### Transition Rules

**Normal → OffRoute:** `divergence_d2 > 25_000_000` for 5 ticks

**OffRoute → Recovering (Priority 1):** GPS jumped >50m from frozen position

**OffRoute → Normal (Priority 2):** `divergence_d2 <= 25_000_000` for 2 ticks

**Recovering → Normal:** Recovery successfully finds stop index

### Critical: Priority Enforcement

In OffRoute mode, transitions are checked **in priority order**:

```rust
match state.mode {
    SystemMode::OffRoute => {
        // Priority 1: Check for large GPS jump FIRST
        if check_offroute_to_recovering(state, &est) {
            transition_to_recovering(state);
            return None;  // Don't check Normal transition
        }

        // Priority 2: Only check Normal if NOT going to Recovering
        if check_offroute_to_normal(state, &est) {
            transition_offroute_to_normal(state);
            return None;
        }

        // Priority 3: Stay in OffRoute
        return None;
    }
    // ...
}
```

**This ensures mutual exclusion** — only ONE transition executes per tick, preventing C3 overlap.

---

## Section 3: Isolated Estimation Layer

### estimate() Function

```rust
/// Isolated estimation layer
///
/// # Contract
/// - Input: GPS + route (no control layer state)
/// - Output: Position signals (no side effects to control layer)
/// - Internal state: Kalman + DR (opaque to control layer)
///
/// # Guarantees
/// - Does NOT access: mode, last_stop_index, frozen_s_cm
/// - Does NOT trigger: recovery, mode changes
/// - Same GPS input → same EstimationOutput (deterministic)
pub fn estimate(
    input: EstimationInput,
    state: &mut EstimationState,
) -> EstimationOutput {
    // 1. Map matching (stateless geometric operation)
    let (seg_idx, match_d2) = map_match::find_best_segment_restricted(...);

    // 2. Project to route (stateless geometric operation)
    let z_gps_cm = map_match::project_to_route(...);

    // 3. Kalman filter (internal state: s_cm, v_cms)
    // 4. DR EMA (internal state: filtered_v)
    let (s_cm, v_cms) = kalman::process_update(...);

    EstimationOutput { z_gps_cm, s_cm, v_cms, divergence_d2: match_d2, has_fix: true }
}
```

### Constraints

🚫 **The estimation layer MUST NOT:**
- Access control layer state: `mode`, `last_stop_index`, `frozen_s_cm`
- Trigger recovery or mode changes
- Modify control layer state

✅ **The estimation layer MUST:**
- Maintain only internal Kalman + DR state
- Return all derived signals explicitly
- Be deterministic (same GPS → same output)

### Isolation vs Purity

| Aspect | "Pure" (Functional) | "Isolated" (This Design) |
|--------|---------------------|--------------------------|
| Internal state | ❌ No mutable state | ✅ Has Kalman + DR state |
| Side effects | ❌ No side effects at all | ✅ No side effects TO CONTROL LAYER |
| Determinism | ✅ Same input → same output | ✅ Same input → same output |
| Coupling | ❌ Ambiguous claim | ✅ Explicit boundary |

**Key insight:** Coupling is about **boundaries**, not mutability. The estimation layer maintains internal state but is **isolated** from control layer concerns.

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
        // STEP 1: Isolated estimation
        let est = estimate(EstimationInput {...}, est_state);

        // STEP 2: State machine transitions (with priority)
        match self.mode {
            SystemMode::Normal => {
                if check_normal_to_offroute(&est) {
                    transition_to_offroute(self, &est, gps.timestamp);
                    return None;
                }
            }
            SystemMode::OffRoute => {
                // PRIORITY ORDER: Check Recovering BEFORE Normal
                if check_offroute_to_recovering(self, &est) {
                    transition_to_recovering(self);
                    // Fall through to recovery handling
                } else if check_offroute_to_normal(self, &est) {
                    transition_offroute_to_normal(self);
                    return None;
                } else {
                    // Stay in OffRoute
                    return None;
                }
            }
            SystemMode::Recovering => {
                // Recovery handling below
            }
        }

        // STEP 3: Recovery (ONLY in Recovering mode)
        if self.mode == SystemMode::Recovering {
            if let Some(idx) = self.attempt_recovery(&est, gps.timestamp) {
                self.recovery_success(idx);
                // Continue to detection
            } else {
                return None;  // Recovery failed, stay in Recovering
            }
        }

        // STEP 4: Detection (ONLY in Normal mode)
        if self.mode == SystemMode::Normal {
            return self.run_detection(&est, gps);
        }
        None
    }
}
```

### Critical Invariants

**1. Recovery ONLY runs in `Recovering` mode:**

```rust
match state.mode {
    SystemMode::Normal => { /* 🚫 NO recovery */ }
    SystemMode::OffRoute => { /* 🚫 NO recovery */ }
    SystemMode::Recovering => { /* ✅ Recovery ONLY here */ }
}
```

**2. OffRoute transitions have priority (Recovering > Normal):**

```rust
// In OffRoute mode, check in THIS ORDER:
if check_offroute_to_recovering(...) {
    // Priority 1: Go to Recovering, skip Normal check
    transition_to_recovering(state);
} else if check_offroute_to_normal(...) {
    // Priority 2: Only checked if NOT going to Recovering
    transition_offroute_to_normal(state);
}
// This prevents both transitions in same tick (C3 fix)
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

1. **Testability:** Isolated functions are testable with explicit inputs
2. **Reasonability:** Clear separation of concerns (control vs estimation)
3. **Maintainability:** Each module has single responsibility
4. **Verifiability:** Invariants are enforceable (priority order, mode checks)
5. **Performance:** No regression expected (same algorithms, reorganized)

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
| 1.1 | 2026-04-28 | Fix 1: Add transition priority (Recovering > Normal) to prevent C3 overlap |
| 1.1 | 2026-04-28 | Fix 2: Rename "pure" → "isolated" for accuracy (Kalman has internal state) |

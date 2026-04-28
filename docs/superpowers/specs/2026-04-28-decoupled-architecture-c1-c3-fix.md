# Decoupled Architecture for C1/C3 Fix

## Overview

This design addresses **C1 (hidden coupling)** and **C3 (recovery overlap)** issues from the code review by introducing a **2-layer architecture**:

1. **Isolated Estimation Layer** — GPS → position pipeline with internal Kalman/DR state
2. **Control Layer** — State machine managing Normal/OffRoute/Recovering modes

**Key insight:** Recovery becomes a first-class system mode, not an inline side-effect.

### Critical Design Decisions

**Fix 1: Unified Trigger System** — ALL transitions use estimation signals only (`divergence_d2`, `displacement`). No separate "GPS jump" heuristic.

**Fix 2: Transition Priority** — OffRoute mode checks transitions in priority order (Recovering > Normal) to prevent race conditions.

**Fix 3: Isolation Contract** — Estimation layer is "isolated" (bounded state) not "pure" (no state). Kalman maintains internal state but doesn't access control layer state.

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

## Section 2: Unified Trigger System

### Key Principle

**ALL transitions use estimation signals ONLY:**

- `divergence_d2`: GPS distance from route (perpendicular, from map matching)
- `displacement`: Distance traveled since frozen (along-route, |z_gps_cm - frozen_s_cm|)

No separate "GPS jump" heuristic — everything derives from `EstimationOutput`.

---

### State Machine Diagram

```
                    ┌───────────────────┐
                    │      Normal       │
                    └─────────┬─────────┘
                              │
              divergence > 50m │
              for 5 ticks     │
                              ▼
        ┌─────────────────────────────────────┐
        │           OffRoute                  │
        │   (position frozen at s_cm)         │
        │                                     │
        │  Wait for: divergence ≤ 50m for 2  │
        │           consecutive ticks         │
        │                                     │
        │  Then check DISPLACEMENT:           │
        │  ┌─────────────────────────────┐   │
        │  │  displacement ≤ 50m?        │   │
        │  │    → Normal (direct)        │   │
        │  │                             │   │
        │  │  displacement > 50m?         │   │
        │  │    → Recovering             │   │
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

---

### Transition Rules (Unified)

**Normal → OffRoute:**
- Condition: `divergence_d2 > 25_000_000` for 5 consecutive ticks
- Signal: `divergence_d2` from `EstimationOutput`

**OffRoute → Normal (Direct):**
- Condition 1: `divergence_d2 <= 25_000_000` for 2 consecutive ticks
- Condition 2: `displacement = |z_gps_cm - frozen_s_cm| <= 5000` (50m)
- Signals: Both from `EstimationOutput`

**OffRoute → Recovering:**
- Condition 1: `divergence_d2 <= 25_000_000` for 2 consecutive ticks
- Condition 2: `displacement = |z_gps_cm - frozen_s_cm| > 5000` (50m)
- Signals: Both from `EstimationOutput`

**Recovering → Normal:**
- Condition: Recovery successfully finds stop index
- Action: Update `last_stop_index`, clear frozen state

---

### Unified Transition Logic

```rust
// In OffRoute mode — unified trigger check
fn handle_offroute_mode(
    state: &mut SystemState,
    est: &EstimationOutput,
) -> TransitionAction {
    const OFF_ROUTE_D2_THRESHOLD: Dist2 = 25_000_000;
    
    // Both paths require: divergence resolved (low for 2 ticks)
    if est.divergence_d2 > OFF_ROUTE_D2_THRESHOLD {
        state.off_route_clear_ticks = 0;
        return TransitionAction::Stay;  // Still diverging
    }
    
    state.off_route_clear_ticks += 1;
    if state.off_route_clear_ticks < 2 {
        return TransitionAction::Stay;  // Need 2 consecutive good ticks
    }
    
    // Divergence resolved — check displacement
    let displacement = state.frozen_s_cm
        .map(|f| (est.z_gps_cm - f).abs())
        .unwrap_or(0);
    
    if displacement > 5000 {
        // Large displacement → Recovering
        TransitionAction::ToRecovering
    } else {
        // Small displacement → Normal (direct)
        TransitionAction::ToNormal
    }
}

enum TransitionAction {
    ToNormal,
    ToRecovering,
    Stay,
}
```

---

### Edge Case Coverage

| Scenario | Divergence | Displacement | Result |
|----------|-----------|-------------|--------|
| Multipath spike | >50m (1 tick) | N/A | Stay Normal (need 5 ticks) |
| Sustained multipath | >50m (5 ticks) | N/A | → OffRoute |
| Return near frozen | ≤50m (2 ticks) | ≤50m | → Normal (direct) |
| Return with jump | ≤50m (2 ticks) | >50m | → Recovering |
| Never returns | >50m (forever) | N/A | Stay OffRoute |
| Divergence oscillates | 30m → 60m → 30m | Any | Reset clear ticks, stay OffRoute |

---

### Benefits of Unified Triggers

**Before (Flawed):**
```
Normal → OffRoute:   divergence_d2 > 25M
OffRoute → Recov:   |z_gps - frozen| > 5000  ❌ Different signal
```

**After (Unified):**
```
Normal → OffRoute:   divergence_d2 > 25M for 5 ticks
OffRoute → Normal:  divergence_d2 ≤ 25M for 2 ticks AND displacement ≤ 50m
OffRoute → Recov:   divergence_d2 ≤ 25M for 2 ticks AND displacement > 50m
```

**All transitions use the SAME signal set from `EstimationOutput`:**
- No separate "GPS jump" heuristic
- Consistent criteria across all transitions
- No edge case gaps

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

        // STEP 2: State machine transitions (unified triggers)
        match self.mode {
            SystemMode::Normal => {
                // Check: divergence > 50m for 5 ticks
                if self.divergence_d2 > 25_000_000 {
                    self.off_route_suspect_ticks += 1;
                    if self.off_route_suspect_ticks >= 5 {
                        transition_to_offroute(self, &est, gps.timestamp);
                        return None;
                    }
                } else {
                    self.off_route_suspect_ticks = 0;
                }
            }
            SystemMode::OffRoute => {
                const OFF_ROUTE_D2_THRESHOLD: Dist2 = 25_000_000;
                
                // Both paths require: divergence ≤ 50m for 2 ticks
                if est.divergence_d2 <= OFF_ROUTE_D2_THRESHOLD {
                    self.off_route_clear_ticks += 1;
                    
                    if self.off_route_clear_ticks >= 2 {
                        // Divergence resolved — check displacement
                        let displacement = self.frozen_s_cm
                            .map(|f| (est.z_gps_cm - f).abs())
                            .unwrap_or(0);
                        
                        if displacement > 5000 {
                            // Large displacement → Recovering
                            transition_to_recovering(self);
                            // Fall through to recovery handling
                        } else {
                            // Small displacement → Normal (direct)
                            transition_offroute_to_normal(self);
                            return None;
                        }
                    }
                } else {
                    // Still diverging — reset clear counter
                    self.off_route_clear_ticks = 0;
                }
                
                // Stay in OffRoute (waiting for divergence to resolve)
                return None;
            }
            SystemMode::Recovering => {
                // Recovery handling below
            }
        }

        // STEP 3: Recovery (ONLY in Recovering mode)
        if self.mode == SystemMode::Recovering {
            if let Some(idx) = self.attempt_recovery(&est, gps.timestamp) {
                self.recovery_success(idx);
                // Continue to detection with new stop index
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

**2. Unified triggers — ALL use estimation signals:**

```rust
// All transitions derive from EstimationOutput:
let divergence_d2 = est.divergence_d2;     // GPS to route distance
let displacement = |est.z_gps_cm - frozen|; // Along-route displacement

// No separate "GPS jump" heuristic — everything is unified
```

**3. OffRoute → Normal/Recovering are mutually exclusive:**

```rust
// In OffRoute mode, only ONE transition executes:
if divergence_resolved_for_2_ticks {
    if displacement > 50m {
        transition_to_recovering();  // Only this
    } else {
        transition_offroute_to_normal();  // Only this
    }
}
// Can't execute both — prevents C3 overlap
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
| 1.2 | 2026-04-28 | Fix 3: Unified trigger system — ALL transitions use estimation signals only (no separate "jump" heuristic) |

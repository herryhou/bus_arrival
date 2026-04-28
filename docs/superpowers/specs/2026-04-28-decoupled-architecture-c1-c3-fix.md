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

**All transitions use the SAME signal set from `EstimationOutput`:**
- `divergence_d2`: GPS distance from route
- `displacement`: |z_gps_cm - frozen_s_cm|
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

## Section 4: Recovery Function

```rust
pub fn recover(input: RecoveryInput) -> Option<u8> {
    // Scoring: distance + index penalty + spatial anchor penalty
    // Uses input.hint_idx for index penalty
    // Uses input.frozen_s_cm for spatial anchoring (if Some)
    // Searches limited window around hint_idx for performance
}
```

### RecoveryInput Structure

```rust
pub struct RecoveryInput<'a> {
    pub s_cm: DistCm,
    pub v_cms: SpeedCms,
    pub dt_seconds: u64,
    pub stops: heapless::Vec<Stop, 256>,
    pub hint_idx: u8,              // From control layer
    pub frozen_s_cm: Option<DistCm>,  // From control layer
    pub search_window: u8,          // NEW: ±N stops from hint_idx (default 10)
}
```

### Search Window Limitation

**Performance optimization:** Recovery only searches `hint_idx ± search_window` instead of all stops.

```rust
pub fn recover(input: RecoveryInput) -> Option<u8> {
    let min_idx = input.hint_idx.saturating_sub(input.search_window);
    let max_idx = (input.hint_idx + input.search_window).min(input.stops.len() as u8);
    
    for i in min_idx..max_idx {
        // Score stops within window only
    }
    // ...
}
```

**Rationale:**
- Dense urban routes: ~100-200m stop spacing, ±10 stops = ~1-2km search range
- Velocity constraint already limits physically reachable stops
- Prevents O(N) search on long routes

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

### Critical Invariants (Enforced as Assertions)

**1. Recovery ONLY runs in `Recovering` mode:**

```rust
// In tick() — assertion for debug builds
#[cfg(debug_assertions)]
if self.mode == SystemMode::Recovering {
    // ✅ Recovery code here
} else {
    debug_assert!(!self.attempt_recovery_called, "Recovery called outside Recovering mode");
}
```

**2. Only ONE transition executes per tick:**

```rust
// In handle_offroute_mode() — assertion after transition
#[cfg(debug_assertions)]
match action {
    TransitionAction::ToNormal | TransitionAction::ToRecovering => {
        debug_assert!(self.mode == old_mode, "Only one transition per tick");
    }
    TransitionAction::Stay => {
        debug_assert!(self.mode == SystemMode::OffRoute, "Stay means mode unchanged");
    }
}
```

**3. frozen_s_cm ONLY accessed in OffRoute/Recovering:**

```rust
fn current_position(state: &SystemState, est: &EstimationOutput) -> DistCm {
    match state.mode {
        SystemMode::Normal => est.s_cm,
        SystemMode::OffRoute => state.frozen_s_cm.expect("Invariant: frozen_s_cm set in OffRoute"),
        SystemMode::Recovering => est.z_gps_cm,
    }
}
```

**4. Unified triggers — ALL use estimation signals:**

```rust
// All transitions derive from EstimationOutput:
let divergence_d2 = est.divergence_d2;     // GPS to route distance
let displacement = |est.z_gps_cm - frozen|; // Along-route displacement

// No separate "GPS jump" heuristic — everything is unified
```

---

### Transition Priority Rationale

**Why Recovering > Normal in OffRoute mode?**

When GPS returns to route after being off-route:
1. **Safety first:** Large displacement (>50m) indicates GPS jumped significantly
   - If we go directly to Normal, we might use wrong stop index
   - Recovery ensures we find correct stop before resuming detection
2. **No harm:** Small displacement (≤50m) goes directly to Normal
   - If GPS is near frozen position, no recovery needed
3. **Deterministic:** Single path eliminates race conditions

**Priority is encoded in the transition logic:**
```rust
// Check Recovering BEFORE Normal
if displacement > 50m {
    return TransitionAction::ToRecovering;  // Priority 1
}
return TransitionAction::ToNormal;  // Priority 2 (only if above didn't trigger)
```

---

## Section 6: High-Impact Recommendations

### 1. Recovery Timeout (Avoid Stuck State)

**Problem:** Recovering mode can get stuck if recovery repeatedly fails.

**Solution:** Add timeout with fallback to Normal.

```rust
pub struct SystemState<'a> {
    // ... existing fields ...
    pub recovering_since: Option<u64>,  // When Recovering mode entered
    pub recovering_timeout_ticks: u16,   // Timeout counter
}

const RECOVERING_TIMEOUT_SECONDS: u64 = 30;  // 30 seconds max

impl<'a> SystemState<'a> {
    fn check_recovering_timeout(&mut self, now: u64) -> bool {
        if self.mode != SystemMode::Recovering {
            return false;
        }
        
        let elapsed = self.recovering_since
            .map(|t| now.saturating_sub(t))
            .unwrap_or(0);
        
        if elapsed > RECOVERING_TIMEOUT_SECONDS {
            // Timeout: give up on recovery, return to Normal
            self.mode = SystemMode::Normal;
            self.frozen_s_cm = None;
            self.recovering_since = None;
            // Keep last_stop_index as-is (best effort)
            return true;
        }
        
        false
    }
}
```

**Rationale:**
- 30 seconds is long enough for most recovery scenarios
- Prevents infinite stuck state
- Falls back to existing stop index (better than never resuming detection)

---

### 2. Confidence Signal from Estimation

**Problem:** Control layer doesn't know estimation quality (e.g., HDOP, outage status).

**Solution:** Add `confidence` field to `EstimationOutput`.

```rust
pub struct EstimationOutput {
    pub z_gps_cm: DistCm,
    pub s_cm: DistCm,
    pub v_cms: SpeedCms,
    pub divergence_d2: Dist2,
    pub has_fix: bool,
    pub confidence: u8,  // NEW: 0-255 quality signal
}
```

**Confidence calculation:**
```rust
pub fn calculate_confidence(
    hdop_x10: u16,
    is_in_outage: bool,
    divergence_d2: Dist2,
) -> u8 {
    if is_in_outage {
        return 0;  // Lowest confidence during outage
    }
    
    // HDOP contribution: 1.0 (good) to 0.0 (bad)
    let hdop_factor = if hdop_x10 < 20 {
        255
    } else if hdop_x10 > 100 {
        0
    } else {
        255 - (hdop_x10 - 20) * 255 / 80
    };
    
    // Divergence contribution: penalize high divergence
    let div_factor = if divergence_d2 < 10_000_000 {
        255  // Good match
    } else if divergence_d2 > 100_000_000 {
        0    // Very bad match
    } else {
        255 - (divergence_d2 - 10_000_000) / 360_000
    };
    
    // Combined: minimum of both factors
    hdop_factor.min(div_factor)
}
```

**Control layer usage:**
```rust
// Use confidence to weight decisions
if est.confidence < 50 {
    // Low confidence: be conservative
    // Don't trigger recovery, stay in current mode
    return None;
}
```

---

### 3. Assertions for Invariants

**Problem:** Invariants are documented but not enforced.

**Solution:** Add debug_assert! for critical invariants.

```rust
impl<'a> SystemState<'a> {
    pub fn tick(&mut self, gps: &GpsPoint, est_state: &mut EstimationState) -> Option<ArrivalEvent> {
        let est = estimate(...);
        
        // INVARIANT: Only one transition per tick
        let old_mode = self.mode;
        
        match self.mode {
            SystemMode::OffRoute => {
                // Transition logic...
            }
            // ...
        }
        
        // INVARIANT CHECK (debug builds only)
        #[cfg(debug_assertions)]
        {
            if old_mode != self.mode {
                // Mode changed — ensure it happened exactly once
                debug_assert!(
                    self.transition_count == 1,
                    "Invariant violated: {} transitions in single tick",
                    self.transition_count
                );
            }
        }
        
        // INVARIANT: frozen_s_cm consistency
        #[cfg(debug_assertions)]
        {
            match self.mode {
                SystemMode::Normal => {
                    debug_assert!(
                        self.frozen_s_cm.is_none(),
                        "Invariant violated: frozen_s_cm set in Normal mode"
                    );
                }
                SystemMode::OffRoute | SystemMode::Recovering => {
                    debug_assert!(
                        self.frozen_s_cm.is_some(),
                        "Invariant violated: frozen_s_cm not set in OffRoute/Recovering"
                    );
                }
            }
        }
        
        // ... rest of tick()
    }
}
```

---

### 4. Search Window Limitation in Recovery

**Problem:** Recovery searches all stops (O(N)) — expensive on long routes.

**Solution:** Limit search to `hint_idx ± window_size`.

```rust
pub struct RecoveryInput<'a> {
    // ... existing fields ...
    pub search_window: u8,  // ±N stops from hint_idx (default 10)
}

pub fn recover(input: RecoveryInput) -> Option<u8> {
    let min_idx = input.hint_idx.saturating_sub(input.search_window);
    let max_idx = (input.hint_idx + input.search_window).min(input.stops.len() as u8);
    
    // Only search within window
    for i in min_idx..max_idx {
        let stop = &input.stops[i as usize];
        // Score this stop...
    }
    // ...
}
```

**Rationale:**
- Dense urban routes: ~100-200m stop spacing, ±10 stops = ~1-2km range
- Velocity constraint (V_MAX × dt) already limits physically reachable stops
- Prevents O(N) search on routes with 100+ stops
- Window size configurable for different route densities

---

### 5. Documentation for Priority Rationale

**Why is Recovering > Normal?**

Document directly in code:

```rust
/// OffRoute mode transition handler
///
/// # Priority Rationale
///
/// When GPS returns to route (divergence ≤ 50m for 2 ticks), we check
/// displacement to decide between direct Normal vs. Recovering:
///
/// - **Priority 1: Recovering** (displacement > 50m)
///   - Large jump indicates GPS position changed significantly
///   - Recovery finds correct stop index before resuming detection
///   - Safety: prevents wrong stop announcements
///
/// - **Priority 2: Normal** (displacement ≤ 50m)
///   - GPS near frozen position, no significant movement
///   - Safe to resume detection immediately
///   - Avoids unnecessary recovery overhead
///
/// # Mutual Exclusion
///
/// Only ONE transition executes per tick. The if/else structure
/// ensures Recovering and Normal paths are mutually exclusive.
fn handle_offroute_mode(...) -> TransitionAction {
    // ...
}
```

---

## Section 7: Module Structure

### Target Structure

```
crates/pico2-firmware/src/
├── control/              ✅ NEW: Control layer
│   ├── mod.rs            → SystemState, tick(), invariants
│   ├── mode.rs           → SystemMode + transitions with assertions
│   └── timeout.rs        → Recovery timeout logic
├── estimation/           ✅ NEW: Estimation layer
│   ├── mod.rs            → estimate(), EstimationState
│   ├── kalman.rs         → Kalman filter (no control state)
│   └── dr.rs             → DR/EMA (no control state)
├── recovery/             ✅ NEW: Pure recovery
│   ├── mod.rs            → recover() function
│   └── search.rs         → Search window logic
├── detection/            ✅ Unchanged (pure)
│   ├── state_machine.rs  → Stop FSM
│   ├── probability.rs    → Arrival probability
│   └── corridors.rs      → Stop corridor filter
└── lib.rs                → Main entry point
```

### Files to Remove

```
crates/pico2-firmware/src/
├── state.rs              ❌ DELETE (split into control/estimation)
├── recovery_trigger.rs   ❌ DELETE (logic moved to control/mode.rs)
└── detection/recovery.rs ❌ DELETE (moved to recovery/ module)
```

---

## Section 8: Single Source of Truth

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

## Section 9: Migration Strategy

### Phase 1: Create New Structure (Non-Breaking)
- Add `control/`, `estimation/`, `recovery/` modules
- Add feature flag `decoupled_architecture` (disabled by default)
- Implement confidence calculation in estimation layer

### Phase 2: Implement Isolated Functions
- Implement `estimate()` with confidence output
- Implement `recover()` with search window
- Add unit tests for isolated functions
- Add debug_assert! invariants

### Phase 3: Implement Control Layer
- Implement `SystemState` with timeout fields
- Implement `tick()` with unified triggers
- Add assertions for invariants (debug builds)
- Add integration tests for transitions

### Phase 4: Implement Timeout
- Add `recovering_since` and timeout logic
- Test timeout behavior (30 second fallback)
- Document timeout rationale in code

### Phase 5: Parallel Testing
- Run both old and new implementations
- Compare outputs, log discrepancies
- Test all edge cases (timeout, confidence, search window)

### Phase 6: Switch Over
- Enable feature flag by default
- Run full test suite
- Remove old code

### Phase 7: Clean Up
- Remove parallel testing code
- Update all documentation
- Add inline documentation for priority rationale

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
- `control/mode`: Transition conditions, priority enforcement
- `estimation`: Isolation contract, confidence calculation
- `recovery`: Search window, hint_idx usage, spatial anchor
- `control/timeout`: Recovery timeout behavior

### Integration Tests
- Normal operation
- Off-route detection and recovery
- GPS jump recovery with displacement check
- GPS outage handling
- Recovery timeout (30 second fallback)
- Confidence threshold gating

### Regression Tests
- All existing integration tests must pass
- Scenario tests: normal, drift, outage, jump, off_route

### Invariant Testing (Debug Builds)
- Run tests with debug_assertions enabled
- Verify frozen_s_cm consistency across modes
- Verify single-transition-per-tick invariant
- Verify recovery-only-in-Recovering-mode invariant

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2026-04-28 | Initial design for C1/C3 fix |
| 1.1 | 2026-04-28 | Fix 1: Add transition priority (Recovering > Normal) to prevent C3 overlap |
| 1.1 | 2026-04-28 | Fix 2: Rename "pure" → "isolated" for accuracy (Kalman has internal state) |
| 1.2 | 2026-04-28 | Fix 3: Unified trigger system — ALL transitions use estimation signals only |
| 1.3 | 2026-04-28 | Add high-impact recommendations: invariants as assertions, recovery timeout, confidence signal, priority rationale documentation, search window limitation |

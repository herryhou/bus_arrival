# Off-Route State Machine Refactoring Design

**Date:** 2026-04-25
**Status:** Proposed
**Author:** Claude Opus 4.6

## Overview

This document describes refactoring changes to simplify the off-route detection state machine, eliminate redundancy, and improve testability.

## Problems Addressed

1. **Duplicate state:** `off_route_freeze_time` exists in both `KalmanState` and `State`
2. **Scattered logic:** Hysteresis logic spread across 3 locations in `process_gps_update()`
3. **Implicit states:** State is combination of multiple counters and flags
4. **Undocumented transitions:** No clear documentation of state machine behavior

## Changes

### 1. Remove Duplicate `off_route_freeze_time`

**Before:**
```rust
// In KalmanState
pub off_route_freeze_time: Option<u64>

// In State (DUPLICATE)
pub off_route_freeze_time: Option<u64>
```

**After:**
```rust
// Only in KalmanState (source of truth)
pub off_route_freeze_time: Option<u64>
```

**Impact:**
- Remove field from `State` struct
- Update `process_gps()` to extract freeze_time from `ProcessResult::OffRoute`
- Update test assertions

### 2. Add `OffRouteState` Enum

**Before:**
```rust
pub off_route_suspect_ticks: u8  // 0-5
pub off_route_clear_ticks: u8    // 0-2
pub frozen_s_cm: Option<DistCm>
```

**After:**
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OffRouteState {
    Normal,       // Normal operation
    Suspect(u8),  // Position frozen, N ticks so far (1-4)
    OffRoute,     // Confirmed off-route
}

pub off_route_state: OffRouteState
pub frozen_s_cm: Option<DistCm>
pub off_route_freeze_time: Option<u64>
```

**Benefits:**
- Impossible states are unrepresentable (e.g., suspect=5 with clear=2)
- Clear state transitions
- Easier pattern matching

### 3. Consolidate Hysteresis Logic

**Before:** Logic scattered in `process_gps_update()` lines 124-163

**After:** Extract to dedicated function:

```rust
fn update_off_route_hysteresis(
    state: &mut KalmanState,
    match_d2: i64,
    gps_timestamp: u64,
) -> OffRouteStatus {
    const OFF_ROUTE_D2_THRESHOLD: i64 = 25_000_000;
    const OFF_ROUTE_CONFIRM_TICKS: u8 = 5;
    const OFF_ROUTE_CLEAR_TICKS: u8 = 2;

    if match_d2 > OFF_ROUTE_D2_THRESHOLD {
        // Bad match: increment suspect counter
        match state.off_route_state {
            OffRouteState::Normal => {
                state.frozen_s_cm = Some(state.s_cm);
                state.off_route_freeze_time = Some(gps_timestamp);
                state.off_route_state = OffRouteState::Suspect(1);
            }
            OffRouteState::Suspect(n) => {
                let new_n = n.saturating_add(1);
                if new_n >= OFF_ROUTE_CONFIRM_TICKS {
                    state.off_route_state = OffRouteState::OffRoute;
                } else {
                    state.off_route_state = OffRouteState::Suspect(new_n);
                }
            }
            OffRouteState::OffRoute => {
                // Already in off-route state, stay there
            }
        }
        OffRouteStatus::Frozen
    } else {
        // Good match: check clear counter
        match state.off_route_state {
            OffRouteState::Normal => OffRouteStatus::Normal,
            OffRouteState::Suspect(_) => {
                // TODO: Need to track clear counter separately
                OffRouteStatus::Suspect
            }
            OffRouteState::OffRoute => OffRouteStatus::Frozen,
        }
    }
}

enum OffRouteStatus {
    Normal,   // Process GPS normally
    Suspect,  // Return DrOutage with frozen position
    Frozen,   // Return OffRoute result
}
```

**Note:** The clear counter still needs tracking. Options:
- Add `clear_ticks: u8` field to KalmanState (simpler)
- Track within Suspect variant (more complex)

### 4. State Transition Diagram

Create `docs/off_route_state_machine.md` documenting:
- All states and their meanings
- All transitions with guard conditions
- Interactions with other modules (warmup, outage, DR, etc.)

## Implementation Order

1. **Add `OffRouteState` enum** to `shared/src/lib.rs`
2. **Update `KalmanState`** to use the new enum
3. **Refactor `process_gps_update()`** to use consolidated hysteresis function
4. **Remove duplicate `off_route_freeze_time`** from `State`
5. **Update all tests** to use new enum
6. **Create state transition documentation**
7. **Run full test suite** and fix any failures

## Testing Strategy

After refactoring:
- Update existing tests to use new enum
- Add tests for previously uncovered transitions
- Verify no behavior changes (test results should be identical)

## Migration Notes

**Breaking changes:**
- Public API of `KalmanState` changes (fields removed/added)
- Tests need to update assertions

**Non-breaking:**
- `ProcessResult` enum unchanged
- External behavior identical

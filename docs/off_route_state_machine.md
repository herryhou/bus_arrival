# Mode State Machine (v9.0 - Clear Boundaries)

## Overview

The mode state machine detects when GPS consistently doesn't match route geometry (>50m for 5+ ticks), implementing hysteresis with position freezing. It manages three modes: Normal, OffRoute, and Recovering.

**Architecture:** Part of the Control Layer in `crates/pico2-firmware/src/control/machine.rs`

## States

### Normal
- GPS matches route within 50m
- Position updates normally via Kalman filter
- Arrival detection enabled
- `off_route_suspect_ticks = 0`
- `off_route_clear_ticks = 0`

### OffRoute
- GPS has been >50m from route for 5+ consecutive ticks
- Position frozen at last known good location
- Arrival detection disabled
- `off_route_suspect_ticks >= 5`
- `off_route_clear_ticks = 0`

### Recovering
- Transitioned from OffRoute with large GPS displacement
- Recovery search active to find correct stop index
- Uses raw GPS position (not Kalman-filtered)
- Arrival detection disabled

## Component Boundary

**Input (`ModeInput`):**
- `divergence_d2`: Squared distance from route (for transition triggers)
- `has_gps_fix`: Whether GPS has valid fix
- `frozen_s_cm`: Frozen position (when in OffRoute mode, None in Normal)
- `current_z_gps_cm`: Current raw GPS projection (for displacement calculation)

**Output (`ModeOutput`):**
- `mode`: Next mode after transition
- `action`: Action to take (FreezePosition, BeginRecovery, ResumeNormal, None)
- `detection_enabled`: Whether arrival detection should be enabled

**Invariants:**
- Does NOT contain route data
- Does NOT contain stop indices
- Does NOT contain Kalman state
- Pure state machine: same input → same output (given internal state)

## State Transitions

```
Normal → OffRoute
    Guard: divergence_d2 > 25,000,000 (50m threshold) for 5 consecutive ticks
    Action: FreezePosition, disable detection

OffRoute → Normal
    Guard: divergence_d2 ≤ 25,000,000 for 2 consecutive ticks AND small displacement
    Action: ResumeNormal, enable detection

OffRoute → Recovering
    Guard: divergence_d2 ≤ 25,000,000 for 2 consecutive ticks AND large displacement
    Action: BeginRecovery, trigger recovery search

Recovering → Normal
    Guard: Recovery succeeds (stop index found)
    Action: ResumeNormal, enable detection
```

## Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `OFF_ROUTE_D2_THRESHOLD` | 25,000,000 cm² | 50m distance threshold |
| `OFF_ROUTE_CONFIRM_TICKS` | 5 | Ticks to confirm off-route |
| `OFF_ROUTE_CLEAR_TICKS` | 2 | Ticks to clear off-route |
| `RECOVERY_DISPLACEMENT_THRESHOLD` | 5000 cm | 50m displacement for recovery |

## Interactions with Other Components

### Estimation Layer
- Estimation layer provides `divergence_d2` and `z_gps_cm` via `EstimationOutput`
- Estimation does NOT have access to mode (enforced by architecture)
- Control layer uses estimation outputs to construct `ModeInput`

### Control Layer
- `SystemState` orchestrates the mode machine with estimation and recovery
- `ModeMachine` owns ONLY mode and hysteresis counters
- Position state (`frozen_s_cm`) is owned by `SystemState`, not `ModeMachine`

### Recovery
- Recovery is triggered by `ModeAction::BeginRecovery`
- Recovery function is pure: `recover(RecoveryInput) -> Option<usize>`
- Recovery uses `z_gps_cm` (raw GPS) not `s_cm` (Kalman-filtered)

## Key Implementation Details

### Pure State Machine
The `ModeMachine` is a pure state machine with no side effects:
- `update(ModeInput) -> ModeOutput` is deterministic
- No access to external state (position, stops, route data)
- Single transition per tick (enforced by design)

### Hysteresis Prevents Flapping
The 5-tick confirmation and 2-tick clear thresholds prevent false positives from transient multipath while allowing fast re-acquisition.

### Position Freezing
Position is frozen **immediately** on first suspect tick (N=1), not when OffRoute is confirmed (tick 5). This prevents position drift during the confirmation period.

## Testing

### Unit Tests (`control/machine.rs`)
- `test_new_machine_in_normal_mode` - Initial state
- `test_normal_to_offroute_requires_5_ticks` - Hysteresis
- `test_normal_to_offroute_resets_on_good_divergence` - Reset behavior
- `test_offroute_to_normal_with_small_displacement` - Direct recovery
- `test_offroute_to_recovering_with_large_displacement` - Recovery trigger
- `test_detection_enabled_only_in_normal` - Detection gating

### Boundary Tests
- `test_mode_input_excludes_route_data` - Compile-time check for isolation
- `test_mode_machine_is_pure` - Verifies no external state access

## Related Files

- **Implementation:** `crates/pico2-firmware/src/control/machine.rs`
- **Mode definitions:** `crates/pico2-firmware/src/control/mode.rs`
- **Control layer:** `crates/pico2-firmware/src/control/mod.rs`
- **Design document:** `docs/clear_boundaries_refactoring_plan.md`

## Migration from v8.x

### Old Implementation (v8.x)
- State managed in `state::State` with `ProcessResult` enum
- Off-route logic mixed with Kalman filter in `kalman.rs`
- Return values: `Valid`, `DrOutage`, `OffRoute`

### New Implementation (v9.0)
- Pure state machine in `control::machine::ModeMachine`
- Clear input/output contracts via `ModeInput`/`ModeOutput`
- Actions via `ModeAction` enum (None, FreezePosition, BeginRecovery, ResumeNormal)
- Isolated from estimation layer (enforced by type system)

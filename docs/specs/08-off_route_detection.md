# Off-Route Detection Specification (v9.0)

## Overview

Detects when GPS consistently doesn't fit route geometry (>50m for 5+ ticks). Implements hysteresis (5 to confirm, 2 to clear) with position freezing.

**Architecture:** Implemented as `ModeMachine` in the Control Layer (`crates/pico2-firmware/src/control/machine.rs`)

## Purpose

Off-route detection handles sustained GPS drift where positions consistently don't fit route geometry. This feature detects:

- **Urban canyon multipath** causing GPS drift away from road
- **Physical deviations** (detour, depot, wrong route loaded)

### Limitations

Cannot detect "along-route drift" where GPS stays on the road but advances faster than the bus. This requires external ground truth and is not detectable with a single GPS sensor.

## Invariants (MUST)

- [ ] **Threshold:** `OFF_ROUTE_D2_THRESHOLD = 25,000,000 cm²` (50 m)
- [ ] **Confirm ticks:** 5 consecutive ticks (avoid false positives from multipath)
- [ ] **Clear ticks:** 2 consecutive good ticks (fast re-acquisition)
- [ ] **Position frozen immediately** when entering suspect state
- [ ] **Arrival suppressed** during off-route episodes
- [ ] **GPS jump recovery:** find nearest stop ahead of frozen position

## Component Boundary

### ModeMachine Input (`ModeInput`)

```rust
pub struct ModeInput {
    /// Divergence from route (squared distance in cm²)
    pub divergence_d2: Dist2,

    /// Whether GPS has valid fix
    pub has_gps_fix: bool,

    /// Frozen position (when in OffRoute mode, None in Normal)
    pub frozen_s_cm: Option<DistCm>,

    /// Current raw GPS projection (for displacement calculation)
    pub current_z_gps_cm: DistCm,
}
```

**What's NOT included (enforcing isolation):**
- ❌ Route data
- ❌ Stop indices
- ❌ Kalman state

### ModeMachine Output (`ModeOutput`)

```rust
pub struct ModeOutput {
    /// Next mode after this transition
    pub mode: SystemMode,

    /// Action to take (if any)
    pub action: ModeAction,

    /// Whether arrival detection should be enabled
    pub detection_enabled: bool,
}
```

## Mode Transitions

```
Normal → OffRoute
    Guard: divergence_d2 > 25,000,000 for 5 consecutive ticks
    Action: FreezePosition, detection_enabled=false

OffRoute → Normal
    Guard: divergence_d2 ≤ 25,000,000 for 2 ticks AND displacement < 50m
    Action: ResumeNormal, detection_enabled=true

OffRoute → Recovering
    Guard: divergence_d2 ≤ 25,000,000 for 2 ticks AND displacement ≥ 50m
    Action: BeginRecovery, detection_enabled=false

Recovering → Normal
    Guard: Recovery succeeds (stop index found)
    Action: ResumeNormal, detection_enabled=true
```

### During Normal Mode

- Kalman filter updates normally
- Arrival detection enabled
- `off_route_suspect_ticks = 0`
- `off_route_clear_ticks = 0`

### During OffRoute Mode

- Position frozen (`frozen_s_cm` set by control layer)
- Skip Kalman filter updates
- Arrival detection disabled (`detection_enabled = false`)
- Track clear ticks for transition back to Normal

### During Recovering Mode

- Recovery search active
- Use raw GPS position (`z_gps_cm`), not Kalman-filtered
- Arrival detection disabled
- Transition to Normal when recovery succeeds

## Constants

```rust
/// Off-route distance threshold (cm²) — 50m² = 25,000,000 cm²
const OFF_ROUTE_D2_THRESHOLD: i64 = 25_000_000;

/// Ticks to confirm off-route (avoid false positives from multipath)
const OFF_ROUTE_CONFIRM_TICKS: u8 = 5;

/// Ticks to clear off-route (fast re-acquisition)
const OFF_ROUTE_CLEAR_TICKS: u8 = 2;

/// Recovery displacement threshold (cm) - 50m jump triggers recovery
const RECOVERY_DISPLACEMENT_THRESHOLD: i32 = 5000;
```

## Integration Points

### Control Layer

The control layer (`SystemState`) orchestrates mode transitions:

```rust
impl SystemState {
    pub fn tick(&mut self, gps: Option<GpsPoint>, est_state: &mut EstimationState) -> Option<ArrivalEvent> {
        // 1. Run estimation (isolated, no access to mode)
        let est_output = estimate(est_input, est_state);

        // 2. Construct mode input from estimation output
        let mode_input = ModeInput {
            divergence_d2: est_output.divergence_d2,
            has_gps_fix: est_output.has_fix,
            frozen_s_cm: self.frozen_s_cm,
            current_z_gps_cm: est_output.z_gps_cm,
        };

        // 3. Update mode machine
        let mode_output = self.mode_machine.update(mode_input);

        // 4. Handle mode actions
        match mode_output.action {
            ModeAction::FreezePosition => {
                self.frozen_s_cm = Some(est_output.s_cm);
            }
            ModeAction::BeginRecovery => {
                // Trigger recovery
            }
            ModeAction::ResumeNormal => {
                self.frozen_s_cm = None;
            }
            ModeAction::None => {}
        }

        // 5. Run detection if enabled
        if mode_output.detection_enabled {
            // ... arrival detection
        }

        None
    }
}
```

### Estimation Layer

The estimation layer provides divergence for mode detection:

```rust
pub struct EstimationOutput {
    /// Raw GPS projection onto route (for recovery input)
    pub z_gps_cm: DistCm,

    /// Kalman-filtered position (primary position in Normal mode)
    pub s_cm: DistCm,

    /// Filtered velocity (cm/s)
    pub v_cms: SpeedCms,

    /// Divergence from route (squared distance, for mode transitions)
    pub divergence_d2: Dist2,

    /// Confidence signal (0-255, higher is better)
    pub confidence: u8,

    /// Whether GPS has valid fix
    pub has_fix: bool,
}
```

**Key:** Estimation does NOT have access to mode. The control layer constructs `ModeInput` from `EstimationOutput`.

## GPS Jump Recovery

When returning from off-route with a large GPS jump (>50m):

1. Check displacement from frozen position
2. If displacement ≥ 50m, trigger recovery mode
3. Recovery function scans stops ahead of frozen position
4. If valid stop found, update stop index and return to Normal
5. If no valid stop, clear frozen state and continue

## Testing Considerations

### Test Scenarios

1. **Normal operation:** GPS consistently < 50m from route
2. **Transient multipath:** Single tick > 50m (should not trigger)
3. **Sustained multipath:** 5+ consecutive ticks > 50m (should trigger)
4. **Fast recovery:** Return to < 50m for 2 ticks (should clear)
5. **GPS jump recovery:** Large position jump after off-route
6. **Stop index recovery:** Find correct stop after jump

### Edge Cases

- GPS outage during off-route (reset counters)
- First fix warm-up period (skip off-route detection)
- Large jumps without valid stops (continue normal processing)
- Velocity constraint violations during recovery

### Unit Tests

Located in `crates/pico2-firmware/src/control/machine.rs`:

- `test_new_machine_in_normal_mode` - Initial state verification
- `test_normal_to_offroute_requires_5_ticks` - Hysteresis verification
- `test_normal_to_offroute_resets_on_good_divergence` - Counter reset
- `test_offroute_to_normal_with_small_displacement` - Direct recovery
- `test_offroute_to_recovering_with_large_displacement` - Recovery trigger
- `test_detection_enabled_only_in_normal` - Detection gating

### Boundary Tests

Compile-time checks enforcing architecture:

- `test_mode_input_excludes_route_data` - Verifies no route data access
- `test_mode_machine_is_pure` - Verifies isolation from external state

## Related Files

- **Implementation:** `crates/pico2-firmware/src/control/machine.rs`
- **Mode definitions:** `crates/pico2-firmware/src/control/mode.rs`
- **Control layer:** `crates/pico2-firmware/src/control/mod.rs`
- **Estimation layer:** `crates/pico2-firmware/src/estimation/mod.rs`
- **Design document:** `docs/clear_boundaries_refactoring_plan.md`
- **Architecture overview:** `docs/off_route_state_machine.md`

## Migration from v8.x

### Changes in v9.0

| Aspect | v8.x | v9.0 |
|--------|------|------|
| Location | `kalman.rs` (mixed with estimation) | `control/machine.rs` (isolated) |
| State | `KalmanState` fields | `ModeMachine` struct |
| Input/Output | `ProcessResult` enum | `ModeInput`/`ModeOutput` structs |
| Return values | `Valid`, `DrOutage`, `OffRoute` | `ModeAction` enum |
| Isolation | Mixed with estimation | Enforced by type system |
| Testability | Hard to test in isolation | Pure function, easily testable |

### Key Improvements

1. **Clear boundaries:** Mode machine is isolated from estimation
2. **Explicit contracts:** `ModeInput` and `ModeOutput` define interfaces
3. **Testability:** Pure state machine can be tested without estimation
4. **Type safety:** Compile-time checks prevent architectural violations

# Hard Monotonic Invariant Design

**Date:** 2026-04-28
**Status:** Draft
**Reviewer Feedback:** C5. Lack of formal guarantees for monotonic progress

## Problem Statement

The current system lacks a hard invariant guaranteeing `s(t+1) >= s(t)`. While soft mitigations exist (speed constraint, Kalman smoothing, -50m monotonic tolerance in kalman.rs), backward jumps can still occur and cause false stop transitions in the detection layer.

## Architecture Overview

The hard monotonic invariant will be enforced at the **control layer boundary** between estimation and detection:

```
GPS → estimate() → EstimationOutput → enforce_monotonic() → Detection
                                           ↑
                                    mode-aware check
```

**Key design decisions:**
1. Estimation layer remains unchanged (keeps its -50m soft check)
2. Control layer adds hard 0m invariant at system boundary
3. Two-tier defense: kalman handles GPS noise, control guarantees strict monotonic
4. Mode-aware enforcement differs by system state
5. **Critical:** Monotonic is enforced on `current_position()` output (mode-specific signal), not raw `est.s_cm`

**Spatial Contract consistency:**
- The design leverages `current_position()` which already implements the mode-specific position selection
- This ensures monotonic enforcement works on the same signal that detection sees
- Maintains the "single source of truth per mode" invariant (C2)

## Components

### `enforce_monotonic()` Function

**Location:** `crates/pico2-firmware/src/control/mod.rs`

**Signature:**
```rust
/// Enforce hard monotonic invariant at system boundary.
///
/// # Returns
/// * (s_cm, false) - position is valid, use as-is
/// * (s_prev, true) - backward jump detected, clamped to previous
///
/// # Mode behavior
/// * Normal: strict monotonic (s_new >= s_prev)
/// * Recovering: allow backward (re-localization may need it)
/// * OffRoute: frozen (returns s_prev, no jump counted)
pub fn enforce_monotonic(
    s_new: DistCm,
    s_prev: DistCm,
    mode: SystemMode,
) -> (DistCm, bool)
```

**Implementation:**
```rust
pub fn enforce_monotonic(
    s_new: DistCm,
    s_prev: DistCm,
    mode: SystemMode,
) -> (DistCm, bool) {
    match mode {
        SystemMode::Normal => {
            if s_new < s_prev {
                (s_prev, true)  // Backward jump detected
            } else {
                (s_new, false)
            }
        }
        SystemMode::Recovering => {
            (s_new, false)  // Allow backward during recovery
        }
        SystemMode::OffRoute => {
            (s_prev, false)  // Frozen position
        }
    }
}
```

### SystemState Extensions

**New fields added to `SystemState`:**
```rust
pub struct SystemState<'a> {
    // ... existing fields ...

    /// Previous position for monotonic checking
    pub last_s_cm: DistCm,

    /// Counter for backward jump events (GPS health monitoring)
    pub backward_jump_count: u32,
}
```

**Initialization in `SystemState::new()`:**
```rust
pub fn new(route_data: &'a RouteData<'a>, persisted: Option<shared::PersistedState>) -> Self {
    Self {
        // ... existing fields ...
        last_s_cm: 0,  // First GPS fix will set this
        backward_jump_count: 0,
    }
}
```

**First-fix handling:** When `last_s_cm == 0`, skip monotonic check and directly set `last_s_cm = est.s_cm`. This prevents false clamping on startup.

## Data Flow

### Normal Mode Operation

```
1. GPS arrives
2. estimate() returns EstimationOutput { s_cm: 10500, ... }
3. enforce_monotonic(10500, 10000, Normal) → (10500, false)
4. last_s_cm = 10500
5. Detection runs with s_cm = 10500
```

### Backward Jump Detection (Normal Mode)

```
1. GPS arrives (noise causes backward projection)
2. estimate() returns EstimationOutput { s_cm: 9800, ... }
3. enforce_monotonic(9800, 10500, Normal) → (10500, true)
4. backward_jump_count += 1
5. last_s_cm = 10500 (unchanged)
6. Detection runs with s_cm = 10500 (clamped)
```

### Recovering Mode (Backward Allowed)

```
1. In Recovering mode, GPS re-acquires with new position
2. estimate() returns EstimationOutput { s_cm: 9500, z_gps_cm: 9500, ... }
3. current_position() returns z_gps_cm = 9500 (mode-specific)
4. enforce_monotonic(9500, 10500, Recovering) → (9500, false)
5. last_s_cm = 9500 (backward allowed)
6. Recovery continues with new position
```

### OffRoute Mode (Frozen)

```
1. In OffRoute mode, position is frozen at entry
2. estimate() returns EstimationOutput { s_cm: 11000, z_gps_cm: 11000, ... }
3. current_position() returns frozen_s_cm = 10000 (mode-specific)
4. enforce_monotonic(10000, 10000, OffRoute) → (10000, false)
5. last_s_cm = 10000 (unchanged, frozen)
6. Detection is suppressed in OffRoute anyway
```

## Integration in tick()

The enforcement happens in `SystemState::tick()` after estimation but before detection:

```rust
pub fn tick(&mut self, gps: &GpsPoint, est_state: &mut EstimationState) -> Option<ArrivalEvent> {
    // STEP 1: Isolated estimation
    let est = estimate(input, est_state);

    // Handle GPS outage
    if !est.has_fix {
        return None;
    }

    // STEP 1.5: Enforce monotonic invariant
    // CRITICAL: Use current_position() to get mode-specific position
    // Normal → est.s_cm, Recovering → est.z_gps_cm, OffRoute → frozen_s_cm
    let s_raw = self.current_position(&est);
    let (s_cm_for_detection, did_jump) = if self.last_s_cm == 0 {
        // First fix: skip check, initialize directly
        (s_raw, false)
    } else {
        enforce_monotonic(s_raw, self.last_s_cm, self.mode)
    };
    if did_jump {
        self.backward_jump_count += 1;
    }
    self.last_s_cm = s_cm_for_detection;

    // STEP 2: State machine transitions
    // ... (existing code, but use s_cm_for_detection where needed)

    // STEP 3: Recovery
    // ... (existing code)

    // STEP 4: Detection (ONLY in Normal mode)
    if self.mode == SystemMode::Normal {
        return self.run_detection(&est, s_cm_for_detection, timestamp);
    }

    None
}
```

**Note:** `run_detection()` signature changes to accept `s_cm` parameter instead of deriving from `est`:

```rust
// OLD:
fn run_detection(&mut self, est: &EstimationOutput, timestamp: u64) -> Option<ArrivalEvent>

// NEW:
fn run_detection(&mut self, est: &EstimationOutput, s_cm: DistCm, timestamp: u64) -> Option<ArrivalEvent>
```

Inside `run_detection()`, replace `self.current_position(est)` with the passed `s_cm` value.

## Mode-Specific Behavior

| Mode      | Monotonic Enforcement | Rationale                                    |
|-----------|----------------------|----------------------------------------------|
| Normal    | Strict (s_new >= s_prev) | Prevents false stop transitions          |
| Recovering| Disabled (allow backward) | Recovery search may move backward       |
| OffRoute  | Frozen (returns s_prev)  | Position frozen, detection suppressed    |

## Testing Strategy

### Unit Tests

**File:** `crates/pico2-firmware/src/control/mod.rs` (test module)

1. **Normal mode - forward movement**
   - Input: s_new=10500, s_prev=10000, mode=Normal
   - Expected: (10500, false)

2. **Normal mode - backward jump**
   - Input: s_new=9800, s_prev=10500, mode=Normal
   - Expected: (10500, true)

3. **Normal mode - exact equality**
   - Input: s_new=10000, s_prev=10000, mode=Normal
   - Expected: (10000, false)

4. **Normal mode - first fix (s_prev=0)**
   - Input: s_new=10000, s_prev=0, mode=Normal
   - Expected: (10000, false) - handled in tick(), not enforce_monotonic()

5. **Recovering mode - backward allowed**
   - Input: s_new=9800, s_prev=10500, mode=Normal
   - Expected: (10500, true)

3. **Normal mode - exact equality**
   - Input: s_new=10000, s_prev=10000, mode=Normal
   - Expected: (10000, false)

4. **Recovering mode - backward allowed**
   - Input: s_new=9500, s_prev=10500, mode=Recovering
   - Expected: (9500, false)

5. **OffRoute mode - frozen**
   - Input: s_new=11000, s_prev=10000, mode=OffRoute
   - Expected: (10000, false)

### Integration Tests

**Scenario:** GPS noise causing backward jump in Normal mode
1. Start at position s=10000
2. GPS projects to s=9800 (noise)
3. Verify: detection sees s=10000 (clamped)
4. Verify: backward_jump_count incremented
5. Next GPS at s=10200
6. Verify: detection sees s=10200 (resumes)

**Scenario:** Recovery with backward movement
1. Enter Recovering mode at s=10000
2. GPS re-acquires at s=9500
3. Verify: position updates to 9500 (backward allowed)
4. Verify: backward_jump_count NOT incremented

## Trace Output

The `backward_jump_count` should be included in trace output for GPS health monitoring:

```jsonl
{"type": "position", "s_cm": 10500, "mode": "Normal", "backward_jumps": 0}
{"type": "position", "s_cm": 10500, "mode": "Normal", "backward_jumps": 1}  // clamped
{"type": "position", "s_cm": 10200, "mode": "Normal", "backward_jumps": 1}
```

## Error Handling

No error conditions - the function always returns a valid position. Backward jumps are silently clamped but counted for monitoring.

Persistent high `backward_jump_count` values indicate GPS quality issues and may warrant:
- Route data validation (wrong route loaded?)
- Hardware check (antenna placement?)
- Environmental assessment (urban canyon?)

## Relationship to Existing Code

### Unchanged

- `crates/pipeline/gps_processor/src/kalman.rs::check_monotonic()` - keeps -50m tolerance
- Estimation layer (`estimate()`) - no modifications
- Detection layer - only sees different input values

### Modified

- `crates/pico2-firmware/src/control/mod.rs`:
  - Add `enforce_monotonic()` function
  - Add `last_s_cm` and `backward_jump_count` to `SystemState`
  - Modify `tick()` to call `enforce_monotonic()`
  - Modify `run_detection()` signature to accept `s_cm` parameter

## Implementation Checklist

- [ ] Add `enforce_monotonic()` function to `control/mod.rs`
- [ ] Add `last_s_cm: DistCm` field to `SystemState`
- [ ] Add `backward_jump_count: u32` field to `SystemState`
- [ ] Initialize new fields in `SystemState::new()`
- [ ] Integrate enforcement in `tick()` after `estimate()`
- [ ] Update `run_detection()` signature to accept `s_cm` parameter
- [ ] Add unit tests for `enforce_monotonic()`
- [ ] Add integration test for backward jump scenario
- [ ] Add trace output for `backward_jump_count`
- [ ] Update `recovery_success()` to reset `last_s_cm` to recovered position (the s_cm parameter passed to recovery_success)

## Future Considerations

1. **Threshold-based logging:** If `backward_jump_count` exceeds threshold in a time window, emit warning event
2. **Flash persistence:** Consider persisting `backward_jump_count` for long-term GPS health analysis
3. **Dynamic threshold:** Could make the strictness configurable based on GPS quality (HDOP)

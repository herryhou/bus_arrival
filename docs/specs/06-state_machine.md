# Arrival State Machine Specification

## Overview

Finite state machine for stop arrival/departure detection with skip-stop protection. Each stop maintains an independent state machine instance that tracks the bus's progression through arrival zones, preventing duplicate announcements and managing dwell time tracking.

The FSM also emits one-shot lifecycle events for UI/progress surfaces. These lifecycle events are separate from confirmed arrival/departure domain events: `Approaching` and `Arriving` are progress signals, `Arrived` maps to the existing confirmed arrival event, and `Departed` maps to the existing departure event.

## States

| State | Description | Transition Condition |
|-------|-------------|---------------------|
| **Idle** | Bus is outside the corridor (before corridor entry) | `s_cm >= corridor_start_cm` |
| **Approaching** | Bus is in corridor, >50m from stop | `d_to_stop < 5000` (enter Arriving)<br>`s_cm < corridor_start_cm` (return to Idle) |
| **Arriving** | Bus is close to stop (<50m) | `d_to_stop < 5000 AND probability > 191` (AtStop)<br>`d_to_stop > 4000 AND s_cm > stop_progress` (Departed)<br>`s_cm < corridor_start_cm` (Idle) |
| **AtStop** | Confirmed arrival at stop | `d_to_stop > 4000 AND s_cm > stop_progress` |
| **Departed** | Bus has left stop (past stop) | Terminal state - no further transitions |
| **TripComplete** | Past last stop (terminal) | Terminal state - no further transitions |

## State Transitions

### Forward Progression (Normal Flow)
```
Idle → Approaching → Arriving → AtStop → Departed
```

### Backward Transitions (GPS Noise Recovery)
```
Approaching → Idle (if bus exits corridor before arriving)
Arriving → Idle (if bus exits corridor before arrival confirmation)
```

### Terminal States
- **Departed**: Stop has been visited and exited - no reactivation allowed
- **TripComplete**: Final stop state - no further state changes

## Invariants (MUST)

### Arrival Conditions
- **Distance constraint**: `d_to_stop < 5000` (within 50m of stop)
- **Probability threshold**: `probability > THETA_ARRIVAL` (191)
- Both conditions MUST be satisfied simultaneously for arrival confirmation

### Departure Conditions
- **Distance constraint**: `d_to_stop > 4000` (more than 40m from stop)
- **Progress constraint**: `s_cm > stop_progress` (moved past stop position)
- **State constraint**: Must be in `AtStop` or `Arriving` state
- All three conditions MUST be satisfied for departure trigger

### Skip-Stop Protection
- **Per-stop state machine**: Each stop maintains independent FSM state
- **No cross-stop interference**: State transitions for one stop don't affect others
- **One-time announcement**: `announced` flag never resets within a trip
- **Reactivation prevention**: `can_reactivate()` always returns `false` (v8.6+)

### Dwell Time Tracking
- **Start counting**: From corridor entry (`s_cm >= corridor_start_cm`)
- **Stop counting**: After departure (Departed state) or corridor exit (Idle state)
- **Reset conditions**: Returns to 0 when transitioning to Idle state
- **No accumulation**: Does not increment in Idle, Departed, or TripComplete states

### Corridor Boundaries
- **Entry threshold**: `s_cm >= corridor_start_cm` (typically 80m before stop)
- **Exit detection**: `s_cm < corridor_start_cm` (backward transition to Idle)
- **Active states**: Approaching, Arriving, AtStop (dwell time accumulates)

## Event Emission

### Lifecycle Events
The state machine emits the following internal lifecycle events at most once per stop per pipeline session:

| Transition | Lifecycle Event |
|------------|-----------------|
| `Idle → Approaching` | `Approaching` |
| `Approaching → Arriving` | `Arriving` |
| `Arriving → AtStop` | `Arrived` |
| `Arriving → Departed` | `Departed` |
| `AtStop → Departed` | `Departed` |

No lifecycle event is emitted for transitions back to `Idle`, terminal no-op updates, or repeated state updates. The once-per-stop rule is enforced with per-stop flags:

- `approaching_emitted`
- `arriving_emitted`
- `arrived_emitted`
- `departed_emitted`

These flags reset when stop states are recreated, such as pipeline initialization, route change, explicit reset, or recovery logic that rebuilds stop states from a recovered stop index. They do not reset on ordinary `Idle` fallback, suspect/off-route ticks, or clean GPS re-entry that preserves stop state.

### Arrival Event
Emitted when transitioning to `AtStop` state:
- **Trigger**: `d_to_stop < 5000 AND probability > 191`
- **Side effect**: Sets `announced = true` (permanent for trip duration)
- **State change**: `Arriving → AtStop`

### Departure Event
Emitted when transitioning to `Departed` state:
- **Trigger**: `d_to_stop > 4000 AND s_cm > stop_progress`
- **From states**: `AtStop` or `Arriving`
- **State change**: `AtStop/Arriving → Departed`

### Android Lifecycle Boundary
The Android pipeline exposes lifecycle events through `PipelineResult.Success.stopEvents`:

```kotlin
data class StopUiEvent(
    val stopIndex: Int,
    val event: StopLifecycleEvent,
    val timestamp: TimestampMs
)
```

`DetectionPipeline` remains UI-neutral: it emits `StopUiEvent` values but does not depend on Android `Context`, toast APIs, Compose, or UI classes. `DetectionService` forwards events through `StopEventCallback`, and `DetectionViewModel` maps them to `EventHint` values for the current toast UI.

The Android detection loop MUST keep stops in `Arriving` or `AtStop` eligible for FSM updates even after `s_cm` passes `corridorEndCm`; otherwise the first tick satisfying `d_to_stop > 4000` can be filtered out before the FSM emits `Departed`.

Host Rust and firmware orchestration follow the same lifecycle eligibility rule: stops in `Arriving` or `AtStop` continue receiving FSM updates after `corridor_end_cm` so `Departed` can be observed. The shared corridor filter remains strict and inclusive; idle or already departed stops outside their corridors are not selected by this lifecycle rule. This behavior does not add a stale-state timeout.

## Implementation Details

### StopState Structure
```rust
pub struct StopState {
    pub index: u8,                    // Stop index in route
    pub fsm_state: FsmState,          // Current FSM state
    pub dwell_time_s: u16,            // Time spent in corridor (seconds)
    pub last_probability: Prob8,      // Last computed arrival probability
    pub last_announced_stop: u8,      // Announcement tracking (v8.4)
    pub announced: bool,              // One-time announcement flag (v8.6)
    pub previous_distance_cm: Option<i32>, // For re-acquisition detection
    pub approaching_emitted: bool,     // Lifecycle event guard
    pub arriving_emitted: bool,        // Lifecycle event guard
    pub arrived_emitted: bool,         // Lifecycle event guard
    pub departed_emitted: bool,        // Lifecycle event guard
}
```

### Key Constants
- **THETA_ARRIVAL**: 191 (probability threshold for arrival confirmation)
- **ARRIVAL_DISTANCE_CM**: 5000 (50m arrival zone radius)
- **DEPARTURE_DISTANCE_CM**: 4000 (40m departure threshold)
- **CORRIDOR_START_OFFSET**: 8000 (80m before stop - configurable per stop)

### Announcement Logic (v8.4+)
The `should_announce()` method triggers voice announcements:
- **Condition**: In active FSM state (Approaching/Arriving/AtStop)
- **Trigger**: `s_cm >= corridor_start_cm` (corridor entry)
- **Guard**: `last_announced_stop != self.index` (once per stop)
- **State change**: Sets `last_announced_stop = self.index`

### One-Time Announcement Rule (v8.6+)
Critical for preventing duplicate arrivals:
- **Flag**: `announced` boolean set to `true` on arrival
- **Persistence**: Never resets within a trip
- **Reactivation**: `can_reactivate()` always returns `false`
- **Purpose**: Prevents GPS noise or route loops from triggering duplicate announcements

## Edge Cases

### GPS Noise Recovery
- **Backward transition**: Arriving → Idle if GPS drifts outside corridor
- **Dwell time reset**: Returns to 0 on corridor exit
- **Announcement preservation**: `announced` flag NOT reset on backward transitions

### Corridor Exit Before Arrival
- **Scenario**: Bus enters corridor but leaves before reaching Arriving state
- **Behavior**: Approaching → Idle transition, dwell_time reset to 0
- **Re-entry**: Normal corridor entry behavior resumes on next approach

### Direct Departure from Arriving State
- **Scenario**: Bus passes close to stop but doesn't trigger arrival
- **Behavior**: Arriving → Departed transition without AtStop state
- **Event**: Emits `Departed` lifecycle event and Departure event (skips arrival)

### Terminal State Behavior
- **TripComplete**: No further state transitions or dwell time accumulation
- **Departed**: Cannot reactivate - permanent terminal state per stop

## Related Files

- **Implementation**: `crates/pipeline/detection/src/state_machine.rs`
- **Type definitions**: `crates/shared/src/lib.rs` (FsmState enum)
- **Integration tests**: `crates/pipeline/detection/src/state_machine.rs` (test module)
- **Usage**: `crates/pipeline/detection/src/lib.rs` (StopState integration)
- **Android implementation**: `android/app/src/main/java/com/busarrival/app/data/pipeline/detection/statemachine/StateMachine.kt`
- **Android pipeline boundary**: `android/app/src/main/java/com/busarrival/app/service/DetectionPipeline.kt`
- **Android UI callback**: `android/app/src/main/java/com/busarrival/app/service/DetectionService.kt`

## Test Coverage

The implementation includes comprehensive unit tests covering:
- Normal state progression (Idle → Approaching → Arriving → AtStop → Departed)
- Corridor entry/exit transitions with dwell time tracking
- One-time announcement rule enforcement
- TripComplete terminal state behavior
- Backward transitions (GPS noise recovery)
- Departed state reactivation prevention
- Once-per-stop lifecycle event emission
- Android `PipelineResult.Success.stopEvents` boundary
- Departure lifecycle emission after leaving corridor end
- All FSM state handling without panics

See `state_machine.rs` test module for specific test cases and expected behaviors.

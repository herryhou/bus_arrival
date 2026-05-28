# Stop Lifecycle Events Design

Date: 2026-05-28

## Goal

Emit one-shot lifecycle events when a stop enters these FSM states:

- `Approaching`
- `Arriving`
- `AtStop`, exposed as `Arrived`
- `Departed`

Each event type must emit at most once per stop per pipeline session. UI code will use these events to show toast messages for now, but the detection pipeline must stay UI-neutral.

## Existing Behavior

The Rust stop FSM already has internal `detection::state_machine::StopEvent` values for confirmed arrival and departure:

- `StopEvent::Arrived`
- `StopEvent::Departed`
- `StopEvent::None`

Rust host code converts `Arrived` and `Departed` into existing domain outputs:

- `ArrivalEvent`
- `DepartureEvent`

Android currently creates `ArrivalEvent` and `DepartureEvent` directly from selected FSM transitions. `Idle -> Approaching` and `Approaching -> Arriving` update raw state but do not emit an event.

There are other `StopEvent` types in the repo for trace analysis:

- `tools/validate_trace.py`
- `crates/trace_validator/src/types.rs`

Those types record trace/state-change analysis data and are not part of this feature.

## Design

Extend the Rust internal stop event model to include lifecycle transitions:

```text
StopEvent.Approaching
StopEvent.Arriving
StopEvent.Arrived
StopEvent.Departed
StopEvent.None
```

State transition mapping:

```text
Idle -> Approaching         emits Approaching
Approaching -> Arriving     emits Arriving
Arriving -> AtStop          emits Arrived
Arriving -> Departed        emits Departed
AtStop -> Departed          emits Departed
```

The `Arriving -> Departed` transition covers the blow-past edge case where the bus passes the stop without satisfying the confirmed arrival threshold. It should still emit `Departed` once for that stop.

No event is emitted for transitions back to `Idle`, terminal no-op updates, or repeated states.

## Once-Per-Stop Rule

Each `StopState` tracks whether it has already emitted each lifecycle event. This applies to both implementations:

- Rust `detection::state_machine::StopState`
- Android `StopState`

```text
approaching_emitted
arriving_emitted
arrived_emitted
departed_emitted
```

If GPS jitter causes a stop to leave and re-enter a state, the event does not fire again. These flags reset when stop states are recreated:

- pipeline initialization
- route change
- explicit pipeline reset
- recovery logic that intentionally rebuilds stop states from a recovered stop index

They do not reset on ordinary `Idle` fallback, suspect/off-route ticks, or a clean GPS re-entry that preserves stop state.

## Domain Events

Keep existing domain events unchanged:

- `ArrivalEvent` still represents confirmed arrival.
- `DepartureEvent` still represents departure.
- Rust `detection::state_machine::StopEvent` represents internal lifecycle transition output from the Rust FSM.
- Android uses a lifecycle-specific enum, not the trace-analysis `StopEvent` names.

Do not extend `ArrivalEvent` or `ArrivalEventType` for `Approaching` or `Arriving`. Those states are lifecycle/progress signals, not confirmed arrivals.

## Android Boundary

This section applies to the Android Kotlin pipeline only: `android/app/src/main/java/com/busarrival/app/service/DetectionPipeline.kt`. It does not change Rust `crates/pipeline/src/lib.rs::PipelineResult`.

Add an Android lifecycle enum:

```kotlin
enum class StopLifecycleEvent {
    Approaching,
    Arriving,
    Arrived,
    Departed,
    None
}
```

Android `StateMachine.update(...)` should return a result object instead of only a pair:

```kotlin
data class StopMachineUpdate(
    val lifecycleEvent: StopLifecycleEvent,
    val arrivalEvent: ArrivalEvent?,
    val departureEvent: DepartureEvent?
)
```

`DetectionPipeline` aggregates lifecycle events into `PipelineResult.Success`:

```kotlin
data class StopUiEvent(
    val stopIndex: Int,
    val event: StopLifecycleEvent,
    val timestamp: TimestampMs
)
```

`PipelineResult.Success` includes:

```kotlin
val stopEvents: List<StopUiEvent>
```

The pipeline must not depend on Android `Context`, toast APIs, Compose, or UI classes.

## UI Callback

`DetectionService` or the ViewModel dispatches lifecycle events to a UI callback:

```kotlin
interface StopEventCallback {
    fun onStopEvent(event: StopUiEvent)
}
```

The callback implementation can show a toast now and can later be replaced with snackbar, sound, notification, logging, or other presentation behavior.

## Testing

Add focused tests for the state machine and pipeline boundary:

- `Idle -> Approaching` emits `Approaching` once.
- Re-entering `Approaching` after falling back to `Idle` does not emit `Approaching` again.
- `Approaching -> Arriving` emits `Arriving` once.
- `Arriving -> AtStop` emits `Arrived` once and still creates the existing `ArrivalEvent`.
- `Arriving -> Departed` emits `Departed` once when the bus passes the stop without entering `AtStop`.
- `AtStop -> Departed` emits `Departed` once and still creates the existing `DepartureEvent`.
- A backward GPS jump after `AtStop` does not re-emit earlier lifecycle events if the FSM regresses.
- Multiple active stops can each emit their own once-per-stop lifecycle event in the same pipeline session.
- `DetectionPipeline` exposes lifecycle events in `PipelineResult.Success` without UI dependencies.

## Non-Goals

- No toast logic inside `StateMachine` or `DetectionPipeline`.
- No persistence or history storage for lifecycle events.
- No changes to route matching, probability calculation, or arrival thresholds.
- No broad refactor of service or UI architecture.

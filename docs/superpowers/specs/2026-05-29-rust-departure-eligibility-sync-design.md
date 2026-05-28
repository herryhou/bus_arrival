# Rust Departure Eligibility Sync Design

## Context

Android was updated so stop lifecycle events can reach the UI, including `Departed`.
During that work, Android needed one extra orchestration rule: a stop that is already
`Arriving` or `AtStop` must remain eligible for FSM updates even after the vehicle
passes the stop corridor end. Without that rule, the strict corridor filter can stop
calling the FSM before it has a chance to emit `Departed`.

The Rust host pipeline and firmware still use strict active-stop selection before
calling the stop FSM. The stop FSM itself can emit `Departed`, but only if it is
called while the stop is in `Arriving` or `AtStop` and the vehicle has moved far
enough past the stop.

## Goal

Sync Rust host and firmware orchestration with Android so post-corridor departure
ticks still reach the stop FSM for stops already awaiting departure.

## Non-Goals

- Do not change the meaning of `pipeline_filter::active_stops()`.
- Do not add new lifecycle states or events.
- Do not change skip-on-reentry behavior.
- Do not refactor unrelated detection or firmware control flow.

## Design

Keep `pipeline_filter::active_stops()` as a pure corridor filter. It should continue
to mean "stops whose corridor contains the current progress value", independent of
FSM state.

Add lifecycle-aware eligibility only at runtime orchestration points that already
have access to each stop's FSM state:

- Host Rust: `crates/pipeline/src/detection_state.rs`
- Firmware: `crates/pico2-firmware/src/control/mod.rs`

A stop should be processed when either condition is true:

1. `s_cm` is within `[corridor_start_cm, corridor_end_cm]`.
2. The stop FSM state is `Arriving` or `AtStop`.

The second condition is intentionally state-based. It lets the FSM receive the
post-corridor sample needed to emit `Departed`, but it does not make idle stops
outside their corridors active.

## Data Flow

For each GPS update:

1. Runtime code calculates the matched route progress `s_cm`.
2. Runtime code selects stops to update.
3. Selection includes strict corridor matches plus stops already awaiting departure.
4. The stop FSM receives only those selected stops.
5. If departure conditions are met, the FSM emits `StopEvent::Departed`.

## Testing

Add focused tests before implementation:

- Host pipeline regression: a stop already in `Arriving` or `AtStop` is processed
  just after `corridor_end_cm` and emits `Departed`.
- Firmware predicate regression: post-corridor `Arriving` and `AtStop` stops are
  eligible, while a post-corridor `Idle` stop is not.

Run the targeted tests first, then the Rust regression suites:

```bash
rtk cargo test -p pipeline detection_state
rtk cargo test -p pico2-firmware
rtk cargo test -p detection
rtk cargo test -p pipeline --lib
```

## Documentation

Update state-machine documentation to clarify that the shared corridor filter remains
strict, while host and firmware orchestration keep `Arriving` and `AtStop` stops
eligible until departure can be observed.

## Success Criteria

- Rust host and firmware can emit `Departed` after the strict corridor end.
- Idle stops outside their corridors are not processed because of this change.
- `pipeline_filter::active_stops()` keeps its existing strict corridor semantics.
- Targeted and regression tests pass.

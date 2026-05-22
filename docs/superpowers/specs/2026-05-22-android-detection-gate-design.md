# Android Detection Gate Design

## Context

The Android pipeline currently lets `DetectionPipeline` decide when to run
arrival detection by inspecting `ModeState` details directly. This made an
edge case visible in the `tz_23_short` scenario: early GPS fixes were already
far from the route and were accumulating off-route suspicion, but the stop FSM
still advanced to `Approaching` before `OffRoute` was confirmed.

The desired behavior is stricter: when route trust is not established, detection
must not update stop FSM state and must not expose active stop state trace
entries.

## Goals

- Keep startup, suspect, off-route, and recovery semantics explicit.
- Keep route-trust policy owned by `ModeMachine`.
- Prevent stop FSM state from advancing during untrusted geometry.
- Avoid treating startup as confirmed off-route.
- Preserve off-route hysteresis: a single bad tick should not immediately enter
  `OffRoute`.

## Non-Goals

- Do not change map matching thresholds.
- Do not change arrival probability or stop FSM transition rules.
- Do not add new user-facing trace schema fields unless needed by tests.
- Do not refactor unrelated pipeline phases.

## Design

`ModeMachine.update()` should return both the updated mode state and a detection
gate. The gate is the single authoritative answer to: "May this tick update
arrival detection state?"

Suggested shape:

```kotlin
data class ModeUpdate(
    val state: ModeState,
    val detectionEnabled: Boolean,
)
```

The exact type name can follow local style, but the important boundary is that
`DetectionPipeline` receives an explicit decision instead of reading
`suspectTicks` and duplicating policy.

## Detection Gate Rules

Detection must be disabled for:

- The first GPS fix after initialization.
- `Mode.Normal` with `suspectTicks > 0`.
- `Mode.OffRoute`.
- `Mode.Recovering`.
- Transition ticks into or out of off-route/recovery when state is being frozen,
  cleared, or reset.

Detection may be enabled only when:

- The updated mode is `Mode.Normal`.
- The tick is not the first fix.
- No off-route suspicion is active after processing the current match.

## Pipeline Behavior

`DetectionPipeline.process()` should:

1. Run GPS conversion, map matching, Kalman update, and mode update.
2. Use the mode update result to choose one of two paths:
   - `detectionEnabled == false`: update time/position bookkeeping, write a
     trace tick with empty `corridor.active_stops` and empty `stop_states`, and
     return no arrivals/departures.
   - `detectionEnabled == true`: run probability and stop FSM updates, then
     write trace with active stop state entries.
3. Avoid direct detection-policy checks like `modeState.suspectTicks > 0` in
   pipeline code.

The trace should reflect the same gate as detection. If detection did not run,
trace output must not imply that a stop corridor or FSM state is active.

## Why Not Default To OffRoute

Startup is an untrusted state, not confirmed off-route. Defaulting to
`OffRoute` would require a fake frozen position before a trustworthy position
exists. That can distort recovery displacement checks and make traces claim an
off-route episode before the system has evidence.

The mode should still start in `Normal`, but detection should be disabled until
the current tick is trusted.

## Edge Cases

### Clean Startup On Route

First fix: detection disabled. The next trusted Normal tick may run detection.
This avoids first-fix stop FSM movement from a single unverified projection.

### Startup Far From Route

First fix: detection disabled. Subsequent bad geometry ticks accumulate suspect
state. Detection remains disabled until either off-route is confirmed or a good
match clears suspicion.

### Transient Bad Tick During Normal Operation

One or more suspect ticks pause detection FSM updates. If route fit recovers
before off-route confirmation, the next trusted Normal tick resumes detection.
This trades a short pause for preventing false state progression.

### Confirmed OffRoute

Detection remains disabled. Position is frozen according to existing off-route
logic. Trace has no active stop states.

### Recovery

Detection remains disabled while recovery is active. When recovery succeeds and
stop states are reset, detection resumes on the next trusted Normal tick.

### Trace Previous Fields

Previous probability and distance in stop-state trace entries should refer to
previous emitted stop-state entries, not hidden internal updates. Because hidden
updates should no longer occur while the gate is disabled, this rule becomes
simpler and remains safe for first visible entries.

## Testing

Add or update tests for:

- `ModeMachine`: first fix disables detection.
- `ModeMachine`: suspect ticks disable detection without immediately entering
  `OffRoute`.
- `ModeMachine`: trusted Normal tick enables detection.
- `ModeMachine`: `OffRoute` and `Recovering` disable detection.
- `DetectionPipeline`: when the gate is disabled, no stop FSM state advances and
  trace `stop_states` is empty.
- `Tz23ScenarioTest`: ticks before initial off-route confirmation have empty
  `stop_states`.
- Existing grouped trace tests continue to pass for trusted Normal ticks with
  active stops.

## Implementation Notes

- Keep the change surgical: introduce the gate result, update call sites, then
  remove pipeline-side policy checks.
- Keep `ModeState` fields available for diagnostics and transitions, but do not
  require consumers to infer detection policy from them.
- Prefer adding focused mode-machine unit tests before adjusting the pipeline.

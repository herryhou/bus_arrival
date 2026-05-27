# Android Acquiring Cold Boot Sync Design

## Goal

Sync Android localization with Rust cold-boot acquisition behavior. Android should not only label startup ticks as `acquiring`; it should mirror Rust by suppressing normal position, Kalman, and detection output until the route lock is acquired.

Success criteria:

- Android `KalmanState` carries an explicit cold-boot marker.
- Android emits `PipelineResult.Acquiring` while waiting for initial route lock.
- Trace v2 can emit `detection.status = "acquiring"` without schema changes.
- Stop detection and stop-state trace output are suppressed while acquiring.
- Cold boot clears only after 2 consecutive good route matches with heading eligibility.

## Current State

Rust has:

- `KalmanState.is_cold_boot`
- `ProcessResult::Acquiring`
- `is_cold_start()`
- Trace status `"acquiring"`
- Cold-boot acquisition that waits for 2 good matches with heading before snapping to route.

Android has:

- `KalmanState` without `isColdBoot`
- `DetectionPipeline.process()` that immediately proceeds through normal projection/Kalman flow
- `Hysteresis.update()` for off-route state but no `isColdStart()` helper
- Trace status as a string, so `"acquiring"` requires no schema change
- UI-level `GpsFixState.Acquiring`, which is separate from pipeline localization status

## Design

### State Model

Add `isColdBoot: Boolean = false` to `android/app/src/main/java/com/busarrival/app/domain/model/StateModels.kt`.

`KalmanState.init()` will set:

- `sCm = zCm`
- `vCms = vGpsCms`
- `lastSegIdx = segIdx`
- `isColdBoot = true`

The default remains `false` so tests or manually constructed states are warm unless they opt in.

`DetectionPipeline.initialize()` and `reset()` should create a cold-boot localization state with zero position and velocity:

- `sCm = 0`
- `vCms = 0`
- `lastSegIdx = 0`
- `isColdBoot = true`

This matches Rust's `LocalizationState::new()`, where cold boot exists before the first GPS fix is processed. The implementation should not seed emitted acquiring output from a projected first fix.

### Hysteresis Parity Helper

Add `Hysteresis.isColdStart(state: KalmanState): Boolean`.

It returns `state.isColdBoot`. This mirrors Rust's `is_cold_start()` helper and gives the pipeline a named parity point.

### Pipeline Result

Add a `PipelineResult.Acquiring` variant in `android/app/src/main/java/com/busarrival/app/service/DetectionPipeline.kt`.

The variant should carry the diagnostics needed by callers and tests:

- `segIdx: Int`
- `matchD2: Long`
- `headingConstraintMet: Boolean`

It should not carry arrivals or departures.

### Cold-Boot Flow

`DetectionPipeline.process()` should handle cold boot before normal hysteresis, projection, Kalman update, and detection.

The flow:

1. Convert the Android `Location` to `GpsPoint`.
2. Convert lat/lon to grid coordinates.
3. Ensure a `KalmanState` exists. During cold boot this should be a zeroed cold-boot state, not a projected first-fix seed.
4. If `Hysteresis.isColdStart(kalmanState)` is true:
   - run map matching with cold-start relaxed heading behavior, matching Rust's `find_best_segment_restricted(..., use_relaxed_heading = true)`
   - compute `headingConstraintMet`
   - if `matchResult.dist2 <= Hysteresis.OFF_ROUTE_D2_THRESHOLD` and `headingConstraintMet`, increment `kalmanState.offRouteClearTicks`
   - otherwise reset `kalmanState.offRouteClearTicks` to `0`
5. If `offRouteClearTicks < 2`:
   - set `lastGpsTime = gps.timestamp`
   - write a trace tick with:
     - `detection.status = "acquiring"`
     - `detection.off_route = false`
     - `kalman.s_cm = 0`
     - `kalman.v_cms = 0`
     - no active corridor or stop states
     - map-matching segment and heading diagnostics from the current match
   - return `PipelineResult.Acquiring`
6. If `offRouteClearTicks >= 2`:
   - project/snap to route at the matched segment
   - set `kalmanState.sCm` to the snapped position
   - clear `isColdBoot`
   - clear `frozenSCm`, `offRouteSuspectTicks`, and `offRouteClearTicks`
   - blend velocity with the Rust EMA formula: `v = v + 3 * (vGps - v) / 10`
   - update `lastSegIdx`, `lastGpsTime`, and `lastSCm`
   - return valid/success output for the snapped tick

Detection should remain disabled on the tick that first clears acquisition unless existing Rust trace comparison shows detection is expected on that same tick. The conservative default is no detection for that tick, matching Android's current first-fix guard.

### Trace Output

No trace schema change is needed. `DetectionTraceTick.status` is already a string.

`writeTraceCore()` should support acquisition output without requiring non-null Kalman velocity or active stop states. It can be reused by passing `detectionAllowed = false`, `positionSCm = 0`, and `PositionSignals(zGpsCm = 0, sCm = 0)`.

If necessary, introduce a small private helper such as `writeAcquiringTrace()` to avoid making `writeTraceCore()` harder to read.

### Tests

Add focused Android tests around cold boot acquisition. Prefer small pipeline-level tests over broad scenario tests unless fixtures already make the scenario easy to assert.

Required coverage:

- `KalmanState.init()` sets `isColdBoot = true`.
- `Hysteresis.isColdStart()` returns the state field.
- `DetectionPipeline.initialize()` starts with a zeroed cold-boot localization state.
- First cold-boot tick returns `PipelineResult.Acquiring`.
- Acquiring trace tick has `detection.status = "acquiring"`, `kalman.s_cm = 0`, `kalman.v_cms = 0`, and no stop states.
- Two consecutive good matches clear cold boot and produce success output.
- A bad match or failed heading resets the acquire counter.

Keep existing detour and trace schema tests passing.

## Non-Goals

- Do not refactor Android localization into a full Rust-style `ProcessResult` module.
- Do not change the trace schema.
- Do not change UI `GpsFixState.Acquiring`; it is separate from this pipeline sync.
- Do not broaden off-route recovery behavior beyond the cold-boot acquisition path.

## Risks

- Android `MapMatcher` may not expose exactly the same heading eligibility detail as Rust. If it does not, the implementation should add the smallest helper needed to compute the same boolean.
- `KalmanState.init()` currently needs a projected route position. The implementation must avoid emitting that seed position during acquiring.
- Existing scenario tests may assume immediate nonzero positions at startup. Those tests should be updated only if the new Rust-parity behavior changes their startup expectations.

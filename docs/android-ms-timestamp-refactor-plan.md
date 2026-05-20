# Android End-to-End Millisecond Timestamp Refactor Plan

## Summary

Make GPS and event timestamps millisecond-native across Android, shared Rust types,
pipeline timing, detection, and trace output. The canonical trace field is
`time_ms`; legacy `time` may be accepted by readers only for compatibility.

This is a focused semantic refactor. Do not change detection thresholds, map
matching behavior, route binary formats, or tick-based dwell behavior in this
pass.

## Public Interface Changes

- Rust shared types:
  - Add `TimestampMs = u64` and `DurationMs = u64` aliases in
    `crates/shared/src/lib.rs`.
  - Use `TimestampMs` for `GpsPoint.timestamp`, `ArrivalEvent.time`,
    `DepartureEvent.time`, `DrState.last_gps_time`,
    `KalmanState.off_route_freeze_time`, and related timestamp fields.
  - Use `DurationMs` where elapsed millisecond values are represented directly.
- Android semantic types:
  - Add `typealias TimestampMs = Long` and `typealias DurationMs = Long` in
    `SemanticTypes.kt`.
  - Use `TimestampMs` for Android `GpsPoint.timestamp`, arrival/departure event
    timestamps, `DrState.lastGpsTime`, and service/pipeline last GPS timestamp
    state.
- Trace JSON:
  - Android `TraceTick` must serialize `time_ms`, not `time`.
  - Rust trace output already uses `time_ms`; keep that canonical.
  - Visualizer parser may accept legacy `time` input, but parsed records should
    expose millisecond semantics as `time_ms`.

## Implementation Steps

1. Add semantic timestamp aliases.
   - Rust: add aliases near other semantic units in `crates/shared/src/lib.rs`.
   - Android: add aliases near other semantic units in
     `android/app/src/main/java/com/busarrival/app/data/pipeline/types/SemanticTypes.kt`.

2. Apply aliases to model and state boundaries.
   - Update Rust shared timestamp fields to use `TimestampMs`.
   - Update Android domain models and service state fields to use `TimestampMs`.
   - Keep database column names as `timestamp`; values remain milliseconds.

3. Preserve millisecond data flow.
   - Android `GpsPoint.fromLocation(location)` must continue preserving
     `location.time` exactly.
   - Remove Android trace down-conversion from `gps.timestamp / 1000`.
   - Keep recovery/localization conversions to seconds explicit at algorithm
     boundaries, e.g. `(gps.timestamp - lastGpsTime) / 1000`.

4. Make trace output canonical.
   - Rename Android `TraceTick.time` to `time_ms`.
   - Update Android trace schema tests and docs to expect `time_ms` and reject
     canonical output containing `time`.
   - Update generated Android detour trace fixture if tests regenerate it.

5. Update consumers and compatibility readers.
   - Android `TraceLoader` should decode `time_ms` traces.
   - Android golden tests should compare arrival times and trace times in
     milliseconds; remove `/ 1000` from arrival collection.
   - Visualizer parser should accept both:
     - `time_ms`: canonical milliseconds.
     - legacy `time`: treat as seconds only for compatibility, convert to
       `time_ms`.
   - Visualizer types and time range/filter helpers should use millisecond
     semantics.

## Test Plan

- Rust:
  - `rtk cargo test -p pipeline --test jsonl_reader`
  - `rtk cargo test -p pipeline --test jsonl_integration`
  - `rtk cargo test -p gps_processor --test bdd_localization`
  - `rtk cargo test -p detection --test trace_output`
  - `rtk cargo test -p pipeline --lib`
  - `rtk cargo test -p detection --lib`
- Android:
  - `rtk ./gradlew testDebugUnitTest --tests com.busarrival.app.scenarios.DetourScenarioGoldenTest`
  - `rtk ./gradlew testDebugUnitTest`
- Visualizer:
  - Run existing parser/type tests if present.
  - If no visualizer test suite exists, add a small parser test only if the
    project already has a test harness.

## Acceptance Criteria

- No production Android trace line emits a top-level `time` field.
- Android and Rust trace output both use `time_ms` for GPS tick timestamps.
- Android arrival/departure timestamps remain milliseconds and are compared to
  trace timestamps without unit conversion.
- Recovery and dead-reckoning behavior remains unchanged for whole-second test
  scenarios.
- The only unrelated dirty file left unstaged is
  `android/build/reports/problems/problems-report.html`.

## Follow-Up Refactor

Start a separate refactor after this one for cleanup:

- Rename remaining internal `time`/`timestamp` identifiers to `timestampMs`
  where API compatibility does not matter.
- Decide whether dwell time should be elapsed-time-based instead of tick-based.
- Remove legacy `time` parsing once old trace fixtures and visualizer samples
  are migrated.
- Regenerate static visualizer samples in a fixture-only commit.

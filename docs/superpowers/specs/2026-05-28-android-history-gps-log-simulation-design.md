# Android History GPS Log Simulation Design

Date: 2026-05-28

## Goal

Use GPS log files shown in the Android History tab as a raw GPS input source for simulation playback. A selected log should feed recorded fixes back through the normal Android detection pipeline, so arrivals, departures, map matching, stop states, and UI status are recomputed from the recorded GPS data.

Success criteria:

- A user can start simulation from a log in the History tab.
- Simulation runs through the same pipeline entry point as live GPS updates.
- Playback timing follows recorded GPS timestamps, scaled by playback speed.
- Replaying a log does not create another GPS log.
- Existing History share/delete behavior remains intact.

## Current Context

History already lists logs through `GpsLogStorageManager`, which supports file storage and SAF-backed storage. Logs are JSONL rows written by `GpsLogWriter` with fields:

- `t`: Android location timestamp in milliseconds.
- `lat`, `lon`: recorded coordinates.
- `a`: optional accuracy in meters.
- `s`: optional speed in meters per second.
- `b`: optional bearing in degrees.
- `p`: optional provider.
- `m`: optional mock-provider flag.

Detection currently has two related paths:

- Live detection in `DetectionService`, where `LocationManager` delivers Android `Location` updates into `processLocation`.
- Trace replay in `DetectionViewModel`, where saved `PipelineEvent` traces drive UI playback without recomputing the pipeline.

This feature should use the live detection path, not trace replay, because the requested behavior is to use the log as an input GPS source.

## User Flow

The History tab adds a play/simulate action to each GPS log row. Tapping it selects that log and opens the Detection tab in simulation mode.

On the Detection screen:

- The active route is loaded as usual.
- The selected GPS log is loaded as a simulation source.
- Playback controls are shown while the simulation source is loaded.
- Pressing play feeds recorded GPS fixes into the normal detection pipeline.
- Stop cancels simulation and resets pipeline state.

Starting live detection while simulation is active clears simulation first. Starting simulation while live detection is active stops live detection first. The selected History log is read-only during simulation.

The active recording log should not be simulatable while it may still be growing. Disable the row play action for the active recording log and let the existing delete protection remain unchanged.

## Architecture

### Recorded GPS Fix Parsing

Add a small parser for GPS log rows:

```kotlin
data class RecordedGpsFix(
    val timeMillis: Long,
    val latitude: Double,
    val longitude: Double,
    val accuracyM: Float?,
    val speedMps: Float?,
    val bearingDeg: Float?,
    val provider: String?,
    val isMock: Boolean
)
```

The parser should:

- Read JSONL lines from `GpsLogStorageManager.loadLog(context, reference)`.
- Parse only the fields written by `GpsLogWriter`.
- Skip malformed rows.
- Return an error only when no valid fixes remain.
- Preserve row order from the file.

Conversion to Android `Location` should set optional fields only when present. The provider should default to a simulation-specific provider name when the row omits `p`.

### Simulation Source in DetectionService

Extend `DetectionService` with an explicit simulation start action, for example:

- `ACTION_START_SIMULATION`
- `EXTRA_GPS_LOG_REFERENCE`

Simulation start should reuse the same route loading and pipeline initialization as live start:

1. Load active route from preferences.
2. Initialize stop state machines, Kalman state, and timing state.
3. Load and parse GPS fixes from the selected log.
4. Disable GPS logging for this run.
5. Start foreground service.
6. Launch a simulation coroutine that emits `Location` values to `processLocation`.

The simulation coroutine owns playback timing. The first fix is emitted immediately. Each following delay is:

```text
max(0, nextFix.timeMillis - previousFix.timeMillis) / playbackSpeed
```

Negative timestamp deltas are clamped to zero. Playback speed should use the same choices as the existing replay controls where possible.

Stop should cancel the simulation job, close any log writer, set running state false, reset the pipeline, and stop the foreground service just like live detection.

### UI and Navigation

Prefer passing the selected log reference as a Detection navigation argument:

```text
detection?simulateLog=<encoded reference>
```

This keeps the flow explicit and one-shot. If encoded content URIs make the route awkward in Compose Navigation, use a small pending-simulation preference as a fallback; the Detection screen must consume and clear it.

History UI changes:

- Add an icon button to each `GpsLogRow` for simulation.
- Keep the existing checkbox, bulk share, and delete behavior.
- Disable the simulation button for the active recording log.

Detection UI changes:

- Distinguish mode in status text, such as `Simulating gps-log-...jsonl`.
- Show playback controls when a GPS log simulation source is loaded.
- Preserve existing live Start/Stop behavior for normal GPS.
- Do not show simulation as a real GPS permission requirement beyond route availability. If current screen structure requires location permission before showing Detection content, relax that gate for simulation mode only.

### Seeking and Determinism

Seeking in a raw GPS simulation cannot simply jump UI state forward because pipeline state depends on all prior fixes. Seek should:

1. Pause playback.
2. Reset pipeline state.
3. Replay fixes from the start through the requested timestamp without real-time delays.
4. Set the current playback position.
5. Resume only if playback was active before seeking.

This is deterministic and avoids maintaining snapshot state.

## Error Handling

- Missing or unreadable log: show a snackbar and do not start simulation.
- Empty log or no valid rows: show a snackbar and do not start simulation.
- Malformed rows mixed with valid rows: skip malformed rows.
- Non-monotonic timestamps: clamp negative delays to zero.
- Active live detection: stop it before simulation starts.
- Active simulation: stop it before live detection starts.

## Testing

Add focused tests for:

- GPS JSONL parser handles required fields and optional fields.
- Malformed rows are skipped, and all-invalid input reports failure.
- Recorded fixes convert to `Location` with optional accuracy, speed, bearing, provider, and mock flag.
- Timestamp delay calculation clamps negative deltas and scales by speed.
- History view model or UI state exposes a simulation action and disables it for the active recording log.
- Detection simulation source feeds locations through the same processing entry point used by live GPS. If direct service testing is hard, isolate the source/timing loop behind a small testable class and keep service wiring thin.

Existing `GpsLogStorageManager` and History share/delete tests should remain valid.

## Out of Scope

- Android mock-provider injection.
- Importing arbitrary external files outside the active History storage backend.
- Editing or annotating GPS log files.
- Creating new trace files from simulated playback.
- Changing Rust pipeline behavior.

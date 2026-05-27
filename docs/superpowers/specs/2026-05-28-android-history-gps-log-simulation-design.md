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
- Simulation works within the current Detection screen and foreground-service permission model.

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
- Treat missing `m` as `false`; the mock flag is optional in JSON but non-null in memory.

Conversion to Android `Location` should set optional fields only when present. The provider should default to a simulation-specific provider name when the row omits `p`. Mock-provider conversion must use the Android API setter (`setIsFromMockProvider(true)` on the target SDK used by this app) rather than trying to assign to `Location.isFromMockProvider`, which is read-only.

### Simulation Source in DetectionService

Extend `DetectionService` with an explicit simulation start action, for example:

- `ACTION_START_SIMULATION`
- `EXTRA_GPS_LOG_REFERENCE`

Simulation start should reuse the same route loading and pipeline initialization as live start:

1. Load active route from preferences.
2. Initialize stop state machines, Kalman state, and timing state.
3. Load and parse GPS fixes from the selected log.
4. Force GPS logging off for this run.
5. Start foreground service.
6. Launch a simulation coroutine that emits `Location` values to `processLocation`.

The simulation coroutine owns playback timing. The first fix is emitted immediately. Each following delay is:

```text
max(0, nextFix.timeMillis - previousFix.timeMillis) / playbackSpeed
```

Negative timestamp deltas are clamped to zero. Playback speed should use the same choices as the existing replay controls where possible.

Playback speed must be clamped to the supported positive set. Zero is not a valid speed; pause is represented by stopping the playback coroutine while preserving the current fix index.

Stop should cancel the simulation job, close any log writer, set running state false, reset the pipeline, and stop the foreground service just like live detection. The service must guard `processLocation` with the current source generation or mode so a late emission after cancellation cannot update a stopped or newly restarted run.

The simulation path must never call `gpsLogWriter.open()`. It should install or keep a `NoopGpsLogStore` writer for the run and set `GpsLogStatus.Disabled("Simulation mode")`, regardless of the user's GPS logging preference.

### UI and Navigation

Use a pending-simulation preference as the primary handoff from History to Detection:

1. History writes the selected log reference and display filename into preferences.
2. History navigates to the existing Detection route.
3. Detection consumes and clears the pending simulation request on resume.

Do not pass the log reference through a navigation route. File paths and SAF content URIs can be long and heavily encoded; using preferences avoids fragile route encoding and keeps the existing navigation graph small.

History UI changes:

- Add an icon button to each `GpsLogRow` for simulation.
- Keep the existing checkbox, bulk share, and delete behavior.
- Disable the simulation button for the active recording log.

Detection UI changes:

- Distinguish mode in status text, such as `Simulating gps-log-...jsonl`.
- Show playback controls when a GPS log simulation source is loaded.
- Preserve existing live Start/Stop behavior for normal GPS.
- Keep the existing location permission gate for Detection. The current service and screen structure are location-service oriented, and relaxing this only for simulation would require broader refactoring outside this feature.
- Parse the selected log before showing duration-dependent controls. GPS log duration is known only after loading valid fixes.

### Seeking and Determinism

Seeking in a raw GPS simulation cannot simply jump UI state forward because pipeline state depends on all prior fixes. To keep the first implementation bounded, arbitrary scrub seeking is out of scope. The initial playback controls should support:

- Play.
- Pause.
- Stop/reset.
- Positive playback-speed changes from the supported set.
- Progress display based on parsed log timestamps.

If later work adds seeking, it must avoid main-thread fast-forwarding. Acceptable designs are checkpointed pipeline snapshots or a background fast-forward operation with cancellation and progress. A direct replay from the start on every scrubber drag is not acceptable for long logs.

## Error Handling

- Missing or unreadable log: show a snackbar and do not start simulation.
- Empty log or no valid rows: show a snackbar and do not start simulation.
- Malformed rows mixed with valid rows: skip malformed rows.
- Non-monotonic timestamps: clamp negative delays to zero.
- Active live detection: stop it before simulation starts.
- Active simulation: stop it before live detection starts.
- Stop during simulation: cancel the source job and ignore any late emission whose source generation no longer matches the active run.
- Active recording log: disable simulation while the log reference equals `lastGpsLogReference`. Once recording stops, normal file close semantics are enough; no cooldown is required unless testing shows SAF/file writes remain visible after close.

## Testing

Add focused tests for:

- GPS JSONL parser handles required fields and optional fields.
- Malformed rows are skipped, and all-invalid input reports failure.
- Recorded fixes convert to `Location` with optional accuracy, speed, bearing, provider, and mock flag.
- Timestamp delay calculation clamps negative deltas and scales by speed.
- Playback speed rejects or clamps non-positive values.
- History view model or UI state exposes a simulation action and disables it for the active recording log.
- Detection simulation source feeds locations through the same processing entry point used by live GPS. If direct service testing is hard, isolate the source/timing loop behind a small testable class and keep service wiring thin.
- Stop/cancel prevents late simulated fixes from updating a stopped run.

Existing `GpsLogStorageManager` and History share/delete tests should remain valid.

## Out of Scope

- Android mock-provider injection.
- Importing arbitrary external files outside the active History storage backend.
- Editing or annotating GPS log files.
- Creating new trace files from simulated playback.
- Changing Rust pipeline behavior.

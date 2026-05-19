# Android GPS Log Capture Design

**Date:** 2026-05-19  
**Status:** Draft  
**Goal:** Save human-readable raw GPS logs while detection is actively running, so sessions can be replayed later for debug, testing, and review.

## Problem

Today the Android detection flow processes GPS updates in memory, but there is no capture path for the raw GPS inputs that produced a session. That makes it hard to:
- replay a real run
- inspect the exact GPS stream that triggered a bug
- compare Android behavior against recorded sessions

The capture must follow the same lifecycle as detection. It should start when the user starts detection and stop when detection stops. It must not record pre-roll or post-roll data.

## Scope

### In scope
- Start a GPS log file when detection starts
- Append one human-readable JSONL record per GPS update while detection is running
- Close the log when detection stops
- Make the file easy to inspect and replay manually

### Out of scope
- Replay tooling
- Binary capture formats
- Pre-roll or post-roll buffering
- Logging when detection is not running
- Recording derived pipeline state unless it is already part of the raw location object

## Design

### File format

Use JSONL.

JSONL is the simplest human-readable format for this use case:
- easy to inspect in a text editor
- easy to parse later for replay
- one record per line keeps capture append-only
- keep each record as small as practical

### File lifecycle

- When `DetectionService.startDetection()` succeeds and immediately before location updates are requested, create a new JSONL file for that session.
- While detection is running, write one JSON object per received GPS location.
- When `DetectionService.stopDetection()` runs, flush and close the file.
- If detection never starts successfully, no log file is created.

### File naming

Each session writes to a distinct file so runs do not overwrite each other.

Recommended filename pattern:

`gps-log-<routeId>-<yyyyMMdd-HHmmss>.jsonl`

If route identity is unavailable, fall back to:

`gps-log-<yyyyMMdd-HHmmss>.jsonl`

### File storage

Store logs in shared document storage, because captured GPS logs are intended to be shared for debugging and review.

Preferred destination:

`<user-selected shared folder>/BusArrival/gps-logs/`

Use Android's Storage Access Framework for shared storage:
- ask the user to choose a log folder before capture is enabled
- persist the returned tree URI permission
- create each session JSONL file in that tree
- keep logs visible to file managers, cloud sync tools, and manual sharing flows
- preserve logs after app uninstall

Do not use direct raw filesystem writes to public paths such as `/sdcard/Download/...`. On modern Android, shared document writes should go through the Storage Access Framework or another scoped-storage API.

If no shared log folder has been configured, detection may either disable GPS logging for that session or fall back to app-specific external storage at `<externalFilesDir>/gps-logs/`. The service must expose this state internally so the UI can tell the user where the log was written, or that logging is disabled.

### JSONL schema

Write one JSON object per line. Each object should include only values available from the Android raw location object or directly associated session metadata.

Prefer compact field names and omit any optional field that is not present on the Android `Location`. Do not serialize missing numeric values as `0`, because `0` can be a valid speed, bearing, or accuracy value.

Required fields:
- `t` for `Location.time` in Unix epoch milliseconds
- `lat`
- `lon`

Optional fields:
- `a` for horizontal accuracy in meters, only when `Location.hasAccuracy()` is true
- `s` for speed in meters per second, only when `Location.hasSpeed()` is true
- `b` for bearing in degrees, only when `Location.hasBearing()` is true
- `p` for provider, only when non-null
- `m` for mock-provider flag, only when true

Do not log derived pipeline values such as route progress, map-matched segment, Kalman state, stop state, or arrival probability in this file. Those belong in trace output, not raw GPS capture.

### Capture semantics

- Log only while detection is actively running.
- Log the raw location update as received by the detection service, before map matching, Kalman filtering, or arrival detection.
- Preserve row order exactly as received.
- Use one JSONL record per GPS update event.
- Do not deduplicate, aggregate, or downsample.
- Write the raw GPS row at the location callback boundary before dispatching pipeline work to background coroutines.
- `stopDetection()` must prevent additional rows from being accepted, then flush and close the writer before pipeline state is reset.

## Architecture

The capture belongs inside the Android detection service lifecycle, not in the UI.

```
DetectionScreen -> DetectionViewModel -> DetectionService
                                        ├─ LocationManager
                                        └─ GpsLogWriter
```

`GpsLogWriter` is a small service-owned helper responsible only for:
- opening the session file
- writing JSONL records
- appending rows
- flushing and closing
- disabling itself after a logging failure

`LocationManager` remains unchanged.

## Error Handling

Logging failure must not stop detection.

If the file cannot be created or written:
- detection continues normally
- the service records the failure internally through a diagnostic field or log message
- the writer disables itself for the rest of the session
- later GPS updates are not blocked and do not retry logging

The feature should fail open because logging is a debug/review aid, not a core detection dependency.

## Testing

Add tests that verify:
- starting detection creates a JSONL log
- each active GPS update appends exactly one row
- stopping detection closes the file
- no file is created when detection does not start
- the JSONL records match the expected schema
- the log remains empty outside the active detection window
- missing optional `Location` values are omitted rather than serialized as zero
- write/open failure disables logging without stopping detection
- rows preserve callback order even when pipeline processing is asynchronous

Prefer service-level tests around `DetectionService` or its logging helper rather than UI tests.

## Success Criteria

- Starting detection produces a new JSONL file for that session
- Only active detection time is logged
- The file is human-readable and replay-friendly
- Detection behavior is unchanged if logging fails
- Stopping detection reliably closes the log file

## Notes

This design intentionally stops short of replay implementation. The first step is to make raw session capture reliable and easy to inspect. Replay can be added later against the same JSONL schema.

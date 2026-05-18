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
- Append one human-readable row per GPS update while detection is running
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

Use CSV.

CSV is the simplest human-readable format for this use case:
- easy to open in a spreadsheet
- easy to inspect in a text editor
- easy to parse later for replay

### File lifecycle

- When `DetectionService.startDetection()` succeeds and location updates begin, create a new CSV file for that session.
- While detection is running, write one row for each received GPS location.
- When `DetectionService.stopDetection()` runs, flush and close the file.
- If detection never starts successfully, no log file is created.

### File naming

Each session writes to a distinct file so runs do not overwrite each other.

Recommended filename pattern:

`gps-log-<routeId>-<yyyyMMdd-HHmmss>.csv`

If route identity is unavailable, fall back to:

`gps-log-<yyyyMMdd-HHmmss>.csv`

### CSV schema

Write a header row once per file. The file should include only values available from the Android raw location object or directly associated session metadata.

Proposed columns:
- `timestamp_ms`
- `elapsed_ms`
- `latitude`
- `longitude`
- `accuracy_m`
- `speed_mps`
- `bearing_deg`
- `provider`
- `is_mock`

Optional fields may be left blank when the location API does not provide them.

### Capture semantics

- Log only while detection is actively running.
- Log the raw location update as received by the detection service, before map matching, Kalman filtering, or arrival detection.
- Preserve row order exactly as received.
- Use one row per GPS update event.
- Do not deduplicate, aggregate, or downsample.

## Architecture

The capture belongs inside the Android detection service lifecycle, not in the UI.

```
DetectionScreen -> DetectionViewModel -> DetectionService
                                        ├─ LocationManager
                                        └─ GpsLogWriter
```

`GpsLogWriter` is a small service-owned helper responsible only for:
- opening the session file
- writing the CSV header
- appending rows
- flushing and closing

`LocationManager` remains unchanged.

## Error Handling

Logging failure must not stop detection.

If the file cannot be created or written:
- detection continues normally
- the service records the failure internally
- later GPS updates are not blocked

The feature should fail open because logging is a debug/review aid, not a core detection dependency.

## Testing

Add tests that verify:
- starting detection creates a CSV log
- each active GPS update appends exactly one row
- stopping detection closes the file
- no file is created when detection does not start
- the CSV header matches the expected schema
- the log remains empty outside the active detection window

Prefer service-level tests around `DetectionService` or its logging helper rather than UI tests.

## Success Criteria

- Starting detection produces a new CSV file for that session
- Only active detection time is logged
- The file is human-readable and replay-friendly
- Detection behavior is unchanged if logging fails
- Stopping detection reliably closes the log file

## Notes

This design intentionally stops short of replay implementation. The first step is to make raw session capture reliable and easy to inspect. Replay can be added later against the same CSV schema.

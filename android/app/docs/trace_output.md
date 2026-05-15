# Trace Output for Golden Tests

## Overview

The Android detection pipeline can output trace.jsonl files for golden test validation, matching the Rust trace format for tool compatibility.

## Usage

```kotlin
val traceFile = File.createTempFile("trace", ".jsonl")
pipeline.initialize(routeData, traceFile = traceFile)

// Process GPS data
for (location in locations) {
    pipeline.process(location)
}

// Close to flush trace
pipeline.close()
```

## Trace Format

Each line is a JSON object (JSONL format):

```json
{"time":1234567890,"s_cm":123456,"off_route":false,"stop_states":[{"stop_idx":0,"fsm_state":"AtStop"}]}
```

## Fields

- `time`: GPS timestamp (milliseconds)
- `s_cm`: Route position (centimeters)
- `off_route`: Off-route mode flag
- `stop_states`: List of per-stop FSM states (null if empty)
  - `stop_idx`: Stop index
  - `fsm_state`: FSM state name ("Approaching", "Arriving", "AtStop", "Departed", "Idle")
  - `skip_on_reentry`: Skip flag for recovery

## Zero Overhead

When `traceFile = null` (default), TraceWriter is not instantiated. Zero allocation, zero overhead.

## Compatibility

Trace format is compatible with Rust trace format for tool compatibility. See `DetourScenarioGoldenTest` for validation examples.

## Current Status

- ✅ Trace output infrastructure complete
- ✅ All 10 validation functions implemented
- ⚠️  Golden test fails: off-route detection not working in Android implementation yet
- ✅ Test correctly identifies missing functionality

## Validations

The `DetourScenarioGoldenTest` validates 10 requirements:
1. Arrival sequence (stops 2,3,4,5 skipped)
2. GPS monotonicity (no backward jumps)
3. Off-route duration ≥5s
4. Position freeze during off-route
5. Immediate snap on re-entry (>100m jump)
6. Skipped stops validation
7. No arrivals during off-route
8. Ground truth consistency
9. Announce events validation
10. FSM state transitions

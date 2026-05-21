# Trace Output for Golden Tests

## Overview

The Android detection pipeline can output grouped `trace_v2.jsonl` files for golden test validation, matching the Rust v2 trace structure for tool compatibility.

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

Each line is a grouped JSON object (JSONL format):

```json
{
  "gps": {"time_ms": 1234567890, "lat": 25.0, "lon": 121.0},
  "kalman": {"s_cm": 123456, "v_cms": 250, "variance_cm2": 400, "divergence_cm": 0},
  "map_matching": {"segment_idx": 12, "heading_constraint_met": true},
  "detection": {"status": "normal", "off_route": false, "gps_jump": false},
  "corridor": {"active_stops": [0], "corridor_start_cm": 120000, "corridor_end_cm": 130000},
  "stop_states": [
    {
      "stop_idx": 0,
      "gps_distance_cm": 1500,
      "progress_distance_cm": 900,
      "fsm_state": "AtStop",
      "dwell_time_s": 4,
      "probability": 220,
      "previous_probability": 200,
      "features": {"p1": 255, "p2": 180, "p3": 220, "p4": 255},
      "announced": true,
      "skip_on_reentry": false,
      "previous_distance_cm": 1200,
      "just_arrived": false
    }
  ]
}
```

## Fields

- Top-level groups:
  - `gps`: raw GPS/timestamp fields
  - `kalman`: filtered progress and variance fields
  - `map_matching`: segment and heading-constraint result
  - `detection`: mode/status flags such as `off_route` and `gps_jump`
  - `corridor`: active stop corridor window
  - `stop_states`: per-stop FSM state entries (empty list if none)
- `stop_states` entry fields:
  - `stop_idx`: Stop index
  - `gps_distance_cm`: Raw GPS distance to stop
  - `progress_distance_cm`: Route-progress distance to stop
  - `fsm_state`: FSM state name ("Approaching", "Arriving", "AtStop", "Departed", "Idle")
  - `dwell_time_s`: Time spent dwelling at stop
  - `probability`: Current arrival probability
  - `previous_probability`: Previous tick probability
  - `features`: Component feature scores (`p1`-`p4`)
  - `announced`: Whether announce has already fired
  - `skip_on_reentry`: Skip flag for recovery
  - `previous_distance_cm`: Previous tick progress distance
  - `just_arrived`: Arrival edge marker

## Zero Overhead

When `traceFile = null` (default), TraceWriter is not instantiated. Zero allocation, zero overhead.

## Compatibility

`trace_v2.jsonl` is compatible with the grouped Rust trace schema. See `DetourScenarioGoldenTest` and `Tz23ScenarioTest` for validation examples.

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

# Trace Output Enhancement Design

**Goal:** Replace the current flat trace schema with a grouped `trace_v2.jsonl`
schema that exposes internal GPS, Kalman, map-matching, detection, corridor, and
per-stop state for debugging.

**Decision:** This is an intentional breaking schema migration. Existing flat
`trace.jsonl` output and consumers must migrate to `trace_v2.jsonl`; no v1/v2
dual-write or compatibility adapter is required.

---

## Trace V2 Schema

Each line in `trace_v2.jsonl` is one grouped JSON object:

```json
{
  "gps": {
    "time_ms": 1234567890,
    "lat": 24.156562,
    "lon": 120.649046,
    "heading_cdeg": 1800,
    "hdop": 1.5,
    "accuracy_cm": 1500,
    "num_sats": 12,
    "fix_type": "3d"
  },
  "kalman": {
    "s_cm": 123456,
    "v_cms": 500,
    "variance_cm2": 2500,
    "divergence_cm": 100
  },
  "map_matching": {
    "segment_idx": 42,
    "heading_constraint_met": true
  },
  "detection": {
    "status": "valid",
    "off_route": false,
    "gps_jump": false,
    "recovery_idx": null,
    "off_route_last_s_cm": null
  },
  "corridor": {
    "active_stops": [5, 6],
    "corridor_start_cm": 120000,
    "corridor_end_cm": 132000,
    "next_stop": [7, 128]
  },
  "stop_states": [
    {
      "stop_idx": 5,
      "gps_distance_cm": -1200,
      "progress_distance_cm": -1000,
      "fsm_state": "Approaching",
      "dwell_time_s": 0,
      "probability": 96,
      "previous_probability": 72,
      "features": {"p1": 120, "p2": 255, "p3": 118, "p4": 0},
      "just_arrived": false,
      "announced": false,
      "skip_on_reentry": false,
      "previous_distance_cm": -1800
    }
  ]
}
```

### Groups

- `gps`: raw GPS input and quality fields.
- `kalman`: filtered route position, velocity, uncertainty, and divergence.
- `map_matching`: route segment matching diagnostics.
- `detection`: GPS processing status and high-level detection mode flags.
- `corridor`: corridor filter output and next-stop summary.
- `stop_states`: diagnostic per-stop state.

---

## Core Semantics

### Detection Status

`detection.status` is the source of truth for GPS processing state.

Allowed values:

- `"valid"`
- `"off_route"`
- `"dr_outage"`
- `"suspect_off_route"`

`detection.off_route` is derived only from:

```text
detection.status == "off_route"
```

`"suspect_off_route"` remains visible as a distinct status and must not be
collapsed into `off_route: true`.

`detection.gps_jump` and `detection.recovery_idx` stay in the grouped schema to
preserve the current trace vocabulary. Their current implementation is partial:
Rust currently emits placeholder values in some paths, and Android emits the
available mode/recovery state from `DetectionPipeline`.

### Stop State Emission

`corridor.active_stops` is the exact corridor-filter output.

`stop_states` must contain only the stop states for the current
`corridor.active_stops` on the same tick. Consumers can rely on the two lists
containing the same stop indices.

### Probability Fields

`probability` is the current tick probability.

`previous_probability` is the probability value captured before this tick's stop
state update. It is not the current stored `last_probability` after update.
Implementations must snapshot the previous value before calling the stop state
update function.

### Detour Fields

`detection.off_route_last_s_cm` is the last valid route position before
confirmed off-route mode. It supports detour jump threshold debugging.

`stop_states[].skip_on_reentry` shows whether an active stop is skipped after
re-entry.

`stop_states[].previous_distance_cm` shows the previous route-distance-to-stop
value used for re-acquisition and transition debugging.

### Announcement Field

`stop_states[].announced` exposes the per-stop one-time announcement flag for
currently active stops.

---

## Implementation Scope

### Rust

- `crates/pipeline/detection/src/trace.rs`
  - Replace the flat `TraceRecord` shape with grouped v2 structs.
  - Add `previous_probability`, `announced`, `skip_on_reentry`, and
    `previous_distance_cm` to stop-state trace items.
- `crates/pipeline/src/lib.rs`
  - Build grouped `TraceRecord` values.
  - Pass `GpsRecord.status` into `detection.status`.
  - Derive `detection.off_route` only from `status == "off_route"`.
  - Move corridor fields into the `corridor` group.
- `crates/pipeline/src/detection_state.rs`
  - Keep tracking `off_route_last_s_cm`.
  - Emit trace stop states only for current active stops.
  - Snapshot `previous_probability` before stop state update.

### Android

- `android/app/src/main/java/com/busarrival/app/service/TraceTick.kt`
  - Replace the flat trace data class with grouped v2 data classes.
  - Add stop-state diagnostic fields.
- `android/app/src/main/java/com/busarrival/app/service/DetectionPipeline.kt`
  - Build grouped `TraceTick` values.
  - Keep Android trace-specific state inside `DetectionPipeline.kt`; do not add a
    new Android `DetectionState` abstraction for this work.
  - Emit trace stop states only for current active stops.
  - Snapshot `previous_probability` before each stop state update.

---

## Consumer Migration

All consumers that currently read flat `trace.jsonl` must migrate to
`trace_v2.jsonl` and grouped paths.

### Files and Tools

- `crates/trace_validator`
  - Parse grouped `TraceRecord` structs.
  - Read fields from `gps.time_ms`, `kalman.s_cm`, `kalman.v_cms`,
    `corridor.active_stops`, `detection.off_route`, and `stop_states`.
- `tools/arrival_from_trace.sh`
  - Read grouped v2 fields and emit the same arrival JSONL output.
- `tools/announce_from_trace.sh`
  - Read `corridor.active_stops`, `gps.time_ms`, `kalman.s_cm`, and
    `kalman.v_cms`.
- Rust scenario tests
  - Update JSON paths from flat fields to grouped fields.
  - Use `detection.off_route` for confirmed off-route episodes.
  - Use `detection.status` when tests need suspect/off-route distinction.
- Android scenario tests
  - Update `TraceTick` loading and assertions for grouped v2.
  - Preserve validation behavior while reading grouped fields.

### Fixture Naming

Generated trace fixtures must use `_trace_v2.jsonl` suffixes.

Examples:

- `ty225_short_detour_android_trace_v2.jsonl`
- `tz_23_short_trace_v2.jsonl`

The runtime/default trace filename for the new schema is `trace_v2.jsonl`.

---

## Verification

Each implementation phase must include tests that prove:

- Rust and Android serialize grouped v2 records.
- `trace_validator` parses grouped v2 records.
- Shell tools read grouped v2 records.
- Scenario tests no longer depend on flat top-level trace fields.
- `detection.off_route` is false for `"suspect_off_route"`.
- `corridor.active_stops` is corridor-only.
- `stop_states` contains only the same stop indices as `corridor.active_stops`.
- `previous_probability` is the pre-update value, while `probability` is the
  current tick value.

Full verification commands:

```bash
rtk gradle -p android testDebugUnitTest
rtk cargo test
```

---

## Implementation Order

1. Define grouped v2 structs in Rust and Android.
2. Migrate Rust trace emission and `trace_validator`.
3. Migrate Android trace emission and scenario tests.
4. Migrate shell tools and Rust scenario tests.
5. Rename generated fixtures to `_trace_v2.jsonl`.
6. Run full Android and Rust verification.

Each step should remove reliance on flat trace fields instead of adding v1/v2
compatibility.

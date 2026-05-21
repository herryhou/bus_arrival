# Trace Output Enhancement Design

**Goal:** Add missing internal state fields to trace.jsonl for better debugging visibility.

**Approach:** Scenario-driven, phased implementation with grouped structure for better reasoning.

---

## New Grouped Structure

Current flat structure becomes grouped for logical organization:

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
    "recovery_idx": null
  },
  "corridor": {
    "active_stops": [5, 6],
    "next_stop": [7, 128]
  },
  "stop_states": [
    // Active stops + meaningful inactive stops
  ]
}
```

**Groups:**
- `gps` - Raw GPS input and quality
- `kalman` - Filter state and divergence
- `map_matching` - Segment matching results
- `detection` - State machine status
- `corridor` - Stop corridor filter state
- `stop_states` - Per-stop detailed state (expanded scope)

---

## Phase 1: Core State Visibility

### Fields

**`detection` group:**
- `status: String?` - GPS processing status (source of truth)
  - Values: `"valid"`, `"off_route"`, `"dr_outage"`, `"suspect_off_route"`
  - `off_route` field becomes derived: `status == "off_route"`

**`stop_states` items:**
- `last_probability: Int` - Previous tick's probability (0-255)

### Debugging Value

- `status` explains WHY detection behaved (4-state nuance vs binary off_route)
- `last_probability` shows probability trends (sudden drops = noise detection)

### Files

**Rust:**
- `crates/pipeline/detection/src/trace.rs` - Add fields, restructure into groups
- `crates/pipeline/src/lib.rs` - Pass `status` from `GpsRecord`, pass `last_probability` from `StopState`

**Android:**
- `TraceTick.kt` - Restructure into groups, add fields
- `DetectionPipeline.kt` - Populate new structure

---

## Phase 2: Detour Debugging

### Fields

**`detection` group:**
- `off_route_last_s_cm: Int?` - Last valid position before off-route (cm)

**`stop_states` items (expanded scope):**
Now includes:
- Active stops (in corridor)
- Departed stops where `announced == true`
- Any stop where `skip_on_reentry == true`

**Per-stop fields:**
- `skip_on_reentry: Boolean` - Whether stop is skipped on off-route re-entry
- `previous_distance_cm: Int?` - Previous distance to stop (for re-acquisition detection)

### Debugging Value

- `off_route_last_s_cm` enables detour jump threshold debugging
- `skip_on_reentry` visible even when stop is inactive (critical for detour debugging)
- `previous_distance_cm` helps debug re-acquisition logic

### Emission Logic

```rust
// Emit stop_state if ANY condition true:
- s_cm >= corridor_start_cm && s_cm <= corridor_end_cm  // Active
- announced == true                                      // Departed but announced
- skip_on_reentry == true                                // Skipped (detour case)
```

### Files

**Rust:**
- `crates/pipeline/detection/src/trace.rs` - Add fields
- `crates/pipeline/src/detection_state.rs` - Track `off_route_last_s_cm`
- `crates/pipeline/src/lib.rs` - Expanded emission logic

**Android:**
- `TraceTick.kt` - Add fields
- `DetectionState.kt` - Track `off_route_last_s_cm`
- `DetectionPipeline.kt` - Expanded emission logic

---

## Phase 3: Announcement Debugging

### Fields

**`stop_states` items:**
- `announced: Boolean` - One-time announcement flag

### Debugging Value

- Confirms whether stop was already announced (prevents duplicate arrivals)
- Visible in trace even after departure (due to expanded emission logic)

### Files

**Rust:**
- `crates/pipeline/detection/src/trace.rs` - Add field
- `crates/pipeline/src/lib.rs` - Pass from `StopState.announced`

**Android:**
- `TraceTick.kt` - Add field
- `DetectionPipeline.kt` - Populate from `StopState.announced`

---

## Implementation Order

1. **Phase 1** - Core state visibility + grouped structure (highest ROI)
2. **Phase 2** - Detour debugging + expanded stop emission
3. **Phase 3** - Announcement debugging

Each phase is independently testable and can be verified before proceeding to next.

---

## Data Flow Summary

```
GpsRecord.status → detection.status (source of truth)
GpsRecord.status → detection.off_route (derived: status == "off_route")

DetectionState.off_route_last_s_cm → detection.off_route_last_s_cm

StopState.last_probability → stop_states[].last_probability
StopState.skip_on_reentry → stop_states[].skip_on_reentry
StopState.previous_distance_cm → stop_states[].previous_distance_cm
StopState.announced → stop_states[].announced

StopState + expanded emission logic → stop_states[] (active + meaningful inactive)
```

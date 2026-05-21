# Trace Output Enhancement Design

**Goal:** Add missing internal state fields to trace.jsonl for better debugging visibility.

**Approach:** Scenario-driven, phased implementation. Each phase adds fields for specific debugging scenarios.

---

## Phase 1: Core State Visibility

### Fields

**TraceRecord level:**
- `status: String?` - GPS processing status (`"valid"`, `"off_route"`, `"dr_outage"`, `"suspect_off_route"`)

**StopTraceState level:**
- `last_probability: Int` - Previous tick's probability (0-255)

### Debugging Value

- `status` explains WHY detection behaved (e.g., why arrivals stopped)
- `last_probability` shows probability trends (sudden drops = noise detection)

### Files

**Rust:**
- `crates/pipeline/detection/src/trace.rs` - Add fields to structs
- `crates/pipeline/src/lib.rs` - Pass `status` from `GpsRecord`, pass `last_probability` from `StopState`

**Android:**
- `android/app/src/main/java/com/busarrival/app/service/TraceTick.kt` - Add fields to data classes
- `android/app/src/main/java/com/busarrival/app/service/DetectionPipeline.kt` - Populate fields in TraceTick construction

### Data Flow

```
GpsRecord.status → TraceRecord.status
StopState.last_probability → StopTraceState.last_probability
```

---

## Phase 2: Detour Debugging

### Fields

**TraceRecord level:**
- `off_route_last_s_cm: Int?` - Last valid position before off-route (cm)

**StopTraceState level:**
- `skip_on_reentry: Boolean` - Whether stop is skipped on off-route re-entry
- `previous_distance_cm: Int?` - Previous distance to stop (for re-acquisition detection)

### Debugging Value

- `off_route_last_s_cm` enables detour jump threshold debugging
- `skip_on_reentry` shows which stops are skipped after detour
- `previous_distance_cm` helps debug re-acquisition logic

### Files

**Rust:**
- `crates/pipeline/detection/src/trace.rs` - Add fields
- `crates/pipeline/src/detection_state.rs` - Track `off_route_last_s_cm` in DetectionState
- `crates/pipeline/src/lib.rs` - Pass through to trace

**Android:**
- `TraceTick.kt` - Add fields
- `DetectionState.kt` - Track `off_route_last_s_cm`
- `DetectionPipeline.kt` - Populate fields

### Data Flow

```
DetectionState.off_route_last_s_cm → TraceRecord.off_route_last_s_cm
StopState.skip_on_reentry → StopTraceState.skip_on_reentry
StopState.previous_distance_cm → StopTraceState.previous_distance_cm
```

---

## Phase 3: Announcement Debugging

### Fields

**StopTraceState level:**
- `announced: Boolean` - One-time announcement flag

### Debugging Value

- Confirms whether stop was already announced (prevents duplicate arrivals)
- Helps debug why arrivals aren't triggering for stops that should have arrived

### Files

**Rust:**
- `crates/pipeline/detection/src/trace.rs` - Add field
- `crates/pipeline/src/lib.rs` - Pass from StopState

**Android:**
- `TraceTick.kt` - Add field
- `DetectionPipeline.kt` - Populate from StopState.announced

### Data Flow

```
StopState.announced → StopTraceState.announced
```

---

## Implementation Order

1. Phase 1 (Core state visibility) - Highest ROI
2. Phase 2 (Detour debugging) - Edge case visibility
3. Phase 3 (Announcement debugging) - Duplicate arrival debugging

Each phase is independently testable and can be verified before proceeding to next.

# Trace Output V2 Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the existing flat trace schema with the grouped `trace_v2.jsonl` schema across Rust, Android, trace consumers, and generated fixtures.

**Architecture:** Keep one canonical v2 schema shared by Rust and Android. Migrate producers first, then migrate every reader and fixture to grouped paths and `_trace_v2.jsonl` naming. Do not add compatibility shims or dual-write behavior.

**Tech Stack:** Rust workspace crates (`pipeline`, `detection`, `trace_validator`), Kotlin/Android serialization, `jq` shell tools, JSONL fixtures, Robolectric tests.

---

## File Structure

- Modify: `crates/pipeline/detection/src/trace.rs`
  - Replace flat `TraceRecord` with grouped v2 structs and expanded stop-state diagnostics.
- Modify: `crates/pipeline/src/detection_state.rs`
  - Emit diagnostic-superset stop states and snapshot previous probability.
- Modify: `crates/pipeline/src/lib.rs`
  - Build grouped trace records, derive `detection.off_route`, and move corridor fields under `corridor`.
- Modify: `crates/pipeline/src/main.rs`
  - Default output path and help text move from `*_trace.jsonl` / `trace.jsonl` to `*_trace_v2.jsonl` / `trace_v2.jsonl`.
- Modify: `crates/pipeline/detection/tests/trace_output.rs`
  - Assert grouped Rust serialization.
- Modify: `crates/trace_validator/src/parser.rs`
  - Parse grouped v2 records.
- Modify: `crates/trace_validator/src/analyzer.rs`
  - Read grouped time, Kalman, detection, and stop-state fields.
- Modify: `tools/arrival_from_trace.sh`
  - Read grouped v2 fields.
- Modify: `tools/announce_from_trace.sh`
  - Read grouped v2 fields.
- Modify: `android/app/src/main/java/com/busarrival/app/service/TraceTick.kt`
  - Replace flat trace data classes with grouped Kotlin data classes.
- Modify: `android/app/src/main/java/com/busarrival/app/service/DetectionPipeline.kt`
  - Build grouped ticks, emit diagnostic stop states, and snapshot previous probability.
- Modify: `android/app/src/test/java/com/busarrival/app/scenarios/common/TraceLoader.kt`
  - Load grouped v2 only; remove legacy time normalization.
- Modify: `android/app/src/test/java/com/busarrival/app/scenarios/common/TraceLoaderTest.kt`
  - Assert grouped v2 parsing.
- Modify: `android/app/src/test/java/com/busarrival/app/scenarios/DetourScenarioGoldenTest.kt`
  - Read new file name and grouped fields.
- Modify: `android/app/src/test/java/com/busarrival/app/scenarios/Tz23ScenarioTest.kt`
  - Read new file name and grouped fields.
- Modify: `android/app/docs/trace_output.md`
  - Document grouped v2 schema and `trace_v2.jsonl`.
- Modify: `test_data/ty225_short_detour_android_trace.jsonl`
  - Replace with renamed v2 fixture.
- Modify: `test_data/tz_23_short_trace.jsonl`
  - Replace with renamed v2 fixture.
- Create: `test_data/ty225_short_detour_android_trace_v2.jsonl`
  - Canonical Android detour v2 fixture.
- Create: `test_data/tz_23_short_trace_v2.jsonl`
  - Canonical Android JSONL-route v2 fixture.
- Delete: `test_data/ty225_short_detour_android_trace.jsonl`
  - Remove v1 fixture name after migration.
- Delete: `test_data/tz_23_short_trace.jsonl`
  - Remove v1 fixture name after migration.

---

### Task 1: Rust Trace V2 Schema

**Files:**
- Modify: `crates/pipeline/detection/src/trace.rs`
- Test: `crates/pipeline/detection/tests/trace_output.rs`

- [ ] **Step 1: Write the failing Rust serialization test**

In `crates/pipeline/detection/tests/trace_output.rs`, replace the flat-JSON assertions with this grouped-shape test:

```rust
#[test]
fn test_trace_v2_serialization_valid_json() {
    use detection::trace::{
        CorridorTrace, DetectionTrace, FeatureScores, GpsTrace, KalmanTrace,
        MapMatchingTrace, StopTraceState, TraceRecord,
    };
    use shared::FsmState;

    let record = TraceRecord {
        gps: GpsTrace {
            time_ms: 1_234_567_890,
            lat: 25.00425,
            lon: 121.28645,
            heading_cdeg: Some(-950),
            hdop: Some(1.2),
            accuracy_cm: Some(150),
            num_sats: Some(12),
            fix_type: Some("3d".to_string()),
        },
        kalman: KalmanTrace {
            s_cm: 10_000,
            v_cms: 500,
            variance_cm2: 100,
            divergence_cm: 15,
        },
        map_matching: MapMatchingTrace {
            segment_idx: Some(5),
            heading_constraint_met: true,
        },
        detection: DetectionTrace {
            status: "valid".to_string(),
            off_route: false,
            gps_jump: false,
            recovery_idx: None,
            off_route_last_s_cm: None,
        },
        corridor: CorridorTrace {
            active_stops: vec![0, 1],
            corridor_start_cm: Some(9_500),
            corridor_end_cm: Some(10_500),
            next_stop: Some((2, 200)),
        },
        stop_states: vec![StopTraceState {
            stop_idx: 1,
            gps_distance_cm: -320,
            progress_distance_cm: -300,
            fsm_state: FsmState::AtStop,
            dwell_time_s: 10,
            probability: 230,
            previous_probability: 180,
            features: FeatureScores { p1: 250, p2: 200, p3: 240, p4: 255 },
            just_arrived: true,
            announced: true,
            skip_on_reentry: false,
            previous_distance_cm: Some(-420),
        }],
    };

    let json = serde_json::to_string(&record).expect("serialize TraceRecord");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse trace json");

    assert_eq!(parsed["gps"]["time_ms"], 1_234_567_890u64);
    assert_eq!(parsed["kalman"]["s_cm"], 10_000);
    assert_eq!(parsed["map_matching"]["segment_idx"], 5);
    assert_eq!(parsed["detection"]["status"], "valid");
    assert_eq!(parsed["corridor"]["active_stops"][0], 0);
    assert_eq!(parsed["stop_states"][0]["previous_probability"], 180);
    assert_eq!(parsed["stop_states"][0]["announced"], true);
    assert_eq!(parsed["stop_states"][0]["previous_distance_cm"], -420);
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
rtk cargo test -p detection --test trace_output
```

Expected: FAIL because `TraceRecord` is still flat and the grouped helper structs do not exist.

- [ ] **Step 3: Replace the flat trace structs with grouped v2 structs**

In `crates/pipeline/detection/src/trace.rs`, replace the current `TraceRecord` definition with:

```rust
#[derive(Serialize, Deserialize, Debug)]
pub struct TraceRecord {
    pub gps: GpsTrace,
    pub kalman: KalmanTrace,
    pub map_matching: MapMatchingTrace,
    pub detection: DetectionTrace,
    pub corridor: CorridorTrace,
    pub stop_states: Vec<StopTraceState>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GpsTrace {
    pub time_ms: TimestampMs,
    #[serde(serialize_with = "serialize_f64_6dec")]
    pub lat: f64,
    #[serde(serialize_with = "serialize_f64_6dec")]
    pub lon: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heading_cdeg: Option<HeadCdeg>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hdop: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accuracy_cm: Option<DistCm>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_sats: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix_type: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct KalmanTrace {
    pub s_cm: DistCm,
    pub v_cms: SpeedCms,
    pub variance_cm2: i32,
    pub divergence_cm: i32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MapMatchingTrace {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segment_idx: Option<u16>,
    pub heading_constraint_met: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DetectionTrace {
    pub status: String,
    pub off_route: bool,
    pub gps_jump: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery_idx: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub off_route_last_s_cm: Option<DistCm>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CorridorTrace {
    pub active_stops: Vec<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corridor_start_cm: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corridor_end_cm: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_stop: Option<(u8, Prob8)>,
}
```

Expand `StopTraceState` with:

```rust
pub previous_probability: Prob8,
pub announced: bool,
pub skip_on_reentry: bool,
#[serde(skip_serializing_if = "Option::is_none")]
pub previous_distance_cm: Option<DistCm>,
```

- [ ] **Step 4: Run the test to verify it passes**

Run:

```bash
rtk cargo test -p detection --test trace_output
```

Expected: PASS.

- [ ] **Step 5: Commit**

Run:

```bash
rtk git add crates/pipeline/detection/src/trace.rs crates/pipeline/detection/tests/trace_output.rs
rtk git commit -m "Define grouped trace v2 schema"
```

---

### Task 2: Rust Detection-State Semantics and Trace Emission

**Files:**
- Modify: `crates/pipeline/src/detection_state.rs`
- Modify: `crates/pipeline/src/lib.rs`

- [ ] **Step 1: Write a failing detection-state test for the new semantics**

Add this test near the existing trace tests in `crates/pipeline/src/detection_state.rs`:

```rust
#[test]
fn trace_info_includes_announced_and_skipped_stops_outside_active_corridor() {
    use shared::binfile::{RouteData, Stop, RouteNode};

    let route = RouteData::new_for_test(
        vec![RouteNode::new(0, 0, 0), RouteNode::new(1000, 0, 10000)],
        vec![
            Stop::new(0, 1000, -5000, 5000),
            Stop::new(1, 9000, 8000, 10000),
        ],
    );
    let mut state = DetectionState::new(&route);
    state.stop_states[0].announced = true;
    state.stop_states[1].skip_on_reentry = true;

    let record = crate::gps::GpsRecord::new(1, 25.0, 121.0, 8_500, 300, None, "valid");
    let (active, trace_states) = state.get_trace_info(&record, &route);

    assert!(active.is_empty());
    assert_eq!(trace_states.iter().map(|s| s.stop_idx).collect::<Vec<_>>(), vec![0, 1]);
    assert!(trace_states.iter().any(|s| s.announced));
    assert!(trace_states.iter().any(|s| s.skip_on_reentry));
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
rtk cargo test -p pipeline detection_state
```

Expected: FAIL because `get_trace_info()` currently emits active stops only and does not expose the new fields.

- [ ] **Step 3: Expand `DetectionState::get_trace_info()` to emit the diagnostic superset**

In `crates/pipeline/src/detection_state.rs`, replace the active-only stop-state collection with:

```rust
let trace_indices: Vec<usize> = self
    .stop_states
    .iter()
    .enumerate()
    .filter_map(|(idx, state)| {
        let is_active = self.active_indices.contains(&idx);
        if is_active || state.announced || state.skip_on_reentry {
            Some(idx)
        } else {
            None
        }
    })
    .collect();

let stop_states: Vec<StopTraceState> = trace_indices
    .into_iter()
    .map(|idx| {
        let stop = &stops[idx];
        let stop_state = &self.stop_states[idx];
        let signals = PositionSignals::new(record.s_cm, record.s_cm);
        let features = detection::probability::compute_feature_scores(
            signals,
            record.v_cms,
            stop,
            stop_state.dwell_time_s,
            detection::probability::gaussian_lut(),
            detection::probability::logistic_lut(),
        );

        StopTraceState {
            stop_idx: idx as u8,
            gps_distance_cm: z_gps_cm - stop.progress_cm,
            progress_distance_cm: record.s_cm - stop.progress_cm,
            fsm_state: stop_state.fsm_state,
            dwell_time_s: stop_state.dwell_time_s,
            probability: stop_state.last_probability,
            previous_probability: stop_state.previous_probability,
            features,
            just_arrived: self.arrived_this_frame.contains(&(idx as u8)),
            announced: stop_state.announced,
            skip_on_reentry: stop_state.skip_on_reentry,
            previous_distance_cm: stop_state.previous_distance_cm,
        }
    })
    .collect();
```

Also add `previous_probability: Prob8` to the Rust `StopState` model if it does not already exist, and snapshot it immediately before the stop-state update call:

```rust
stop_state.previous_probability = stop_state.last_probability;
let event = stop_state.update(
    s_cm,
    v_cms,
    stop.progress_cm,
    stop.corridor_start_cm,
    probability,
);
```

- [ ] **Step 4: Build grouped records in `PipelineResult::add_trace_record()`**

In `crates/pipeline/src/lib.rs`, replace the flat push with:

```rust
use detection::trace::{
    CorridorTrace, DetectionTrace, GpsTrace, KalmanTrace, MapMatchingTrace, TraceRecord,
};

self.trace_records.push(TraceRecordWrapper(TraceRecord {
    gps: GpsTrace {
        time_ms: record.time,
        lat: record.lat,
        lon: record.lon,
        heading_cdeg: record.heading_cdeg,
        hdop: record.hdop,
        accuracy_cm: record.accuracy_cm,
        num_sats: record.num_sats,
        fix_type: record.fix_type.clone(),
    },
    kalman: KalmanTrace {
        s_cm: record.s_cm,
        v_cms: record.v_cms,
        variance_cm2: record.variance_cm2,
        divergence_cm: record.divergence_cm,
    },
    map_matching: MapMatchingTrace {
        segment_idx: record.segment_idx,
        heading_constraint_met: record.heading_constraint_met,
    },
    detection: DetectionTrace {
        status: record.status.to_string(),
        off_route: record.status == "off_route",
        gps_jump: false,
        recovery_idx: None,
        off_route_last_s_cm: det_state.off_route_last_s_cm(),
    },
    corridor: CorridorTrace {
        active_stops,
        corridor_start_cm,
        corridor_end_cm,
        next_stop,
    },
    stop_states,
}));
```

Add a narrow accessor in `DetectionState`:

```rust
pub fn off_route_last_s_cm(&self) -> Option<DistCm> {
    self.off_route_last_s_cm
}
```

- [ ] **Step 5: Run focused Rust tests**

Run:

```bash
rtk cargo test -p pipeline detection_state
rtk cargo test -p pipeline --test bdd_arrival
```

Expected: PASS.

- [ ] **Step 6: Commit**

Run:

```bash
rtk git add crates/pipeline/src/detection_state.rs crates/pipeline/src/lib.rs
rtk git commit -m "Emit grouped trace v2 from Rust pipeline"
```

---

### Task 3: Rust Consumers and CLI Naming

**Files:**
- Modify: `crates/trace_validator/src/parser.rs`
- Modify: `crates/trace_validator/src/analyzer.rs`
- Modify: `tools/arrival_from_trace.sh`
- Modify: `tools/announce_from_trace.sh`
- Modify: `crates/pipeline/src/main.rs`

- [ ] **Step 1: Write failing parser and analyzer tests**

In `crates/trace_validator/src/parser.rs`, replace the flat sample line in `test_parse_trace_valid_record()` with:

```rust
let json_line = r#"{"gps":{"time_ms":1,"lat":25.0,"lon":121.0,"heading_cdeg":0,"hdop":1.5,"num_sats":12,"fix_type":"3d"},"kalman":{"s_cm":0,"v_cms":100,"variance_cm2":100,"divergence_cm":5},"map_matching":{"segment_idx":0,"heading_constraint_met":true},"detection":{"status":"valid","off_route":false,"gps_jump":false,"recovery_idx":null,"off_route_last_s_cm":null},"corridor":{"active_stops":[0],"corridor_start_cm":null,"corridor_end_cm":null,"next_stop":null},"stop_states":[{"stop_idx":0,"gps_distance_cm":-7000,"progress_distance_cm":-7000,"fsm_state":"Approaching","dwell_time_s":0,"probability":10,"previous_probability":0,"features":{"p1":5,"p2":3,"p3":2,"p4":0},"just_arrived":false,"announced":false,"skip_on_reentry":false,"previous_distance_cm":null}]}"#;
```

In `crates/trace_validator/src/analyzer.rs`, update `test_analyze_empty_records()` to construct grouped records. The key assertion stays the same.

- [ ] **Step 2: Run the validator tests to verify failure**

Run:

```bash
rtk cargo test -p trace_validator
```

Expected: FAIL because parser and analyzer still read flat fields.

- [ ] **Step 3: Migrate parser and analyzer to grouped paths**

In `crates/trace_validator/src/analyzer.rs`, replace:

```rust
time_range: (records[0].time_ms, records.last().unwrap().time_ms),
```

with:

```rust
time_range: (
    records[0].gps.time_ms,
    records.last().unwrap().gps.time_ms,
),
```

Replace the per-record event call with:

```rust
record_event(
    analysis,
    record.gps.time_ms,
    stop_state.fsm_state,
    stop_state.progress_distance_cm,
    record.kalman.s_cm,
    record.kalman.v_cms,
    stop_state.just_arrived,
);
```

In `crates/pipeline/src/main.rs`, change the default naming and help text:

```rust
return Err("Too many arguments. Usage: pipeline <input> <route_data> [--output <trace_v2.jsonl>]".into());
println!("  pipeline gps.jsonl route_data.bin --output custom_trace_v2.jsonl");
println!("  ./tools/arrival_from_trace.sh trace_v2.jsonl > arrivals.jsonl");
println!("  ./tools/announce_from_trace.sh trace_v2.jsonl > announce.jsonl");
let new_name = format!("{}_trace_v2.jsonl", base_name);
```

- [ ] **Step 4: Migrate the shell tools to grouped fields**

In `tools/arrival_from_trace.sh`, replace the jq program with:

```bash
jq -c 'select(.stop_states and (.stop_states | length > 0)) |
  . as $tick |
  .stop_states[] |
  select(.just_arrived == true) |
  {
    time: ($tick.gps.time_ms / 1000 | floor),
    stop_idx,
    s_cm: $tick.kalman.s_cm,
    v_cms: $tick.kalman.v_cms,
    probability
  }' \
  "$TRACE_FILE"
```

In `tools/announce_from_trace.sh`, replace the jq program with:

```bash
jq -c 'select(.corridor.active_stops and (.corridor.active_stops | length > 0)) |
  {
    time: (.gps.time_ms / 1000 | floor),
    stop_idx: .corridor.active_stops[0],
    s_cm: .kalman.s_cm,
    v_cms: .kalman.v_cms
  }' \
  "$TRACE_FILE"
```

- [ ] **Step 5: Run focused migration checks**

Run:

```bash
rtk cargo test -p trace_validator
rtk test bash tools/arrival_from_trace.sh test_data/ty225_short_detour_trace.jsonl
rtk test bash tools/announce_from_trace.sh test_data/ty225_short_detour_trace.jsonl
```

Expected: validator tests PASS; shell scripts emit JSON lines instead of jq path errors. The fixture path gets renamed later in Task 6.

- [ ] **Step 6: Commit**

Run:

```bash
rtk git add crates/trace_validator/src/parser.rs crates/trace_validator/src/analyzer.rs crates/pipeline/src/main.rs tools/arrival_from_trace.sh tools/announce_from_trace.sh
rtk git commit -m "Migrate Rust trace consumers to v2"
```

---

### Task 4: Android Trace V2 Schema and Loader

**Files:**
- Modify: `android/app/src/main/java/com/busarrival/app/service/TraceTick.kt`
- Modify: `android/app/src/test/java/com/busarrival/app/scenarios/common/TraceLoader.kt`
- Modify: `android/app/src/test/java/com/busarrival/app/scenarios/common/TraceLoaderTest.kt`

- [ ] **Step 1: Write failing Android loader tests**

In `android/app/src/test/java/com/busarrival/app/scenarios/common/TraceLoaderTest.kt`, replace the legacy/flat tests with:

```kotlin
@Test
fun load_readsGroupedTraceV2() {
    val file = writeTrace(
        """{"gps":{"time_ms":80001000,"lat":25.0,"lon":121.0},"kalman":{"s_cm":100,"v_cms":10,"variance_cm2":0,"divergence_cm":0},"map_matching":{"segment_idx":null,"heading_constraint_met":true},"detection":{"status":"valid","off_route":false,"gps_jump":false,"recovery_idx":null,"off_route_last_s_cm":null},"corridor":{"active_stops":[],"corridor_start_cm":null,"corridor_end_cm":null,"next_stop":null},"stop_states":[]}"""
    )

    try {
        val tick = TraceLoader.load(file).single()
        assertEquals(80_001_000L, tick.gps.time_ms)
        assertEquals(100L, tick.kalman.s_cm)
        assertEquals("valid", tick.detection.status)
    } finally {
        file.delete()
    }
}
```

- [ ] **Step 2: Run the Android loader tests to verify failure**

Run:

```bash
rtk gradle -p android testDebugUnitTest --tests com.busarrival.app.scenarios.common.TraceLoaderTest
```

Expected: FAIL because `TraceTick` is still flat and `TraceLoader` still contains v1 normalization logic.

- [ ] **Step 3: Replace flat Kotlin trace classes with grouped v2 classes**

In `android/app/src/main/java/com/busarrival/app/service/TraceTick.kt`, replace the flat data class with:

```kotlin
@Serializable
data class TraceTick(
    val gps: TraceGps,
    val kalman: TraceKalman,
    val map_matching: TraceMapMatching,
    val detection: TraceDetection,
    val corridor: TraceCorridor,
    val stop_states: List<StopStateEntry> = emptyList(),
)

@Serializable
data class TraceGps(
    val time_ms: TimestampMs,
    val lat: Double? = null,
    val lon: Double? = null,
    val heading_cdeg: Short? = null,
    val hdop: Float? = null,
    val accuracy_cm: Int? = null,
    val num_sats: Int? = null,
    val fix_type: String? = null,
)

@Serializable
data class TraceKalman(
    val s_cm: Long,
    val v_cms: Int = 0,
    val variance_cm2: Int = 0,
    val divergence_cm: Int = 0,
)

@Serializable
data class TraceMapMatching(
    val segment_idx: Int? = null,
    val heading_constraint_met: Boolean = false,
)

@Serializable
data class TraceDetection(
    val status: String,
    val off_route: Boolean,
    val gps_jump: Boolean = false,
    val recovery_idx: Int? = null,
    val off_route_last_s_cm: Long? = null,
)

@Serializable
data class TraceCorridor(
    val active_stops: List<Int> = emptyList(),
    val corridor_start_cm: Int? = null,
    val corridor_end_cm: Int? = null,
    val next_stop: List<Int>? = null,
)
```

Expand `StopStateEntry` with:

```kotlin
val previous_probability: Int = 0,
val announced: Boolean = false,
val skip_on_reentry: Boolean = false,
val previous_distance_cm: Int? = null,
```

- [ ] **Step 4: Simplify `TraceLoader` to v2 only**

In `android/app/src/test/java/com/busarrival/app/scenarios/common/TraceLoader.kt`, replace `load()` with:

```kotlin
fun load(file: File): List<TraceTick> {
    return file.readLines().mapNotNull { line ->
        if (line.isBlank()) return@mapNotNull null
        try {
            json.decodeFromString<TraceTick>(line)
        } catch (e: Exception) {
            println("Warning: Failed to parse trace line: $line")
            null
        }
    }
}
```

Delete `normalizeTraceLine()`. This migration is intentionally breaking and must not preserve flat/legacy shapes.

- [ ] **Step 5: Run the loader tests to verify they pass**

Run:

```bash
rtk gradle -p android testDebugUnitTest --tests com.busarrival.app.scenarios.common.TraceLoaderTest
```

Expected: PASS.

- [ ] **Step 6: Commit**

Run:

```bash
rtk git add android/app/src/main/java/com/busarrival/app/service/TraceTick.kt android/app/src/test/java/com/busarrival/app/scenarios/common/TraceLoader.kt android/app/src/test/java/com/busarrival/app/scenarios/common/TraceLoaderTest.kt
rtk git commit -m "Define Android trace v2 model"
```

---

### Task 5: Android Trace V2 Emission

**Files:**
- Modify: `android/app/src/main/java/com/busarrival/app/service/DetectionPipeline.kt`

- [ ] **Step 1: Write a failing Android emission test**

In `android/app/src/test/java/com/busarrival/app/scenarios/Tz23ScenarioTest.kt`, add:

```kotlin
@Test
fun test_tz23_trace_v2_contains_accuracy_and_status() {
    val run = processScenario()
    val firstTick = run.ticks.first()

    assertNotNull(firstTick.gps.accuracy_cm)
    assertEquals("valid", firstTick.detection.status)
    assertNotNull(firstTick.kalman.s_cm)
}
```

- [ ] **Step 2: Run the focused Android test to verify failure**

Run:

```bash
rtk gradle -p android testDebugUnitTest --tests com.busarrival.app.scenarios.Tz23ScenarioTest.test_tz23_trace_v2_contains_accuracy_and_status
```

Expected: FAIL because `DetectionPipeline` still writes flat ticks.

- [ ] **Step 3: Replace flat tick construction in `DetectionPipeline`**

In `android/app/src/main/java/com/busarrival/app/service/DetectionPipeline.kt`, replace the `traceWriter?.write(TraceTick(...))` call with:

```kotlin
traceWriter?.write(
    TraceTick(
        gps = TraceGps(
            time_ms = gps.timestamp,
            lat = gps.lat,
            lon = gps.lon,
            heading_cdeg = gps.headingCdeg,
            hdop = gps.hdop,
            accuracy_cm = gps.accuracyCm,
        ),
        kalman = TraceKalman(
            s_cm = positionSCm.toLong(),
            v_cms = kalmanState!!.vCms,
            variance_cm2 = 0,
            divergence_cm = sCm - positionSCm,
        ),
        map_matching = TraceMapMatching(
            segment_idx = matchResult.segIdx,
            heading_constraint_met = matchResult.dist2 != Long.MAX_VALUE,
        ),
        detection = TraceDetection(
            status = when (modeState.mode) {
                Mode.OffRoute -> "off_route"
                Mode.Recovering -> "suspect_off_route"
                else -> "valid"
            },
            off_route = modeState.mode == Mode.OffRoute,
            gps_jump = jumpDetected,
            recovery_idx = null,
            off_route_last_s_cm = modeState.frozenSCm?.toLong(),
        ),
        corridor = TraceCorridor(
            active_stops = activeEntries.keys.sorted(),
            corridor_start_cm = corridorStartCm,
            corridor_end_cm = corridorEndCm,
            next_stop = nextStop,
        ),
        stop_states = traceEntries,
    )
)
```

Build `traceEntries` from the diagnostic superset instead of active-only:

```kotlin
val traceEntries = stopStates
    .filterValues { state ->
        val isActive = state.fsmState != FsmState.Idle && state.fsmState != FsmState.Departed
        isActive || state.announced || state.skipOnReentry
    }
    .toSortedMap()
    .map { (idx, state) ->
        StopStateEntry(
            stop_idx = idx,
            gps_distance_cm = detectionSignals.zGpsCm - stop.progressCm,
            progress_distance_cm = detectionSignals.sCm - stop.progressCm,
            fsm_state = state.fsmState.name,
            dwell_time_s = state.dwellTimeS,
            probability = state.lastProbability.value,
            previous_probability = state.previousProbability.value,
            features = TraceFeatureScores(
                p1 = features.p1.value,
                p2 = features.p2.value,
                p3 = features.p3.value,
                p4 = features.p4.value,
            ),
            just_arrived = justArrivedStops.contains(idx),
            announced = state.announced,
            skip_on_reentry = state.skipOnReentry,
            previous_distance_cm = state.previousDistanceCm,
        )
    }
```

Also snapshot `previousProbability` immediately before `StateMachine.update(...)` by extending the Android `StopState` model with:

```kotlin
var previousProbability: Prob8 = UByteWrapper(0),
```

and then assigning:

```kotlin
state.previousProbability = state.lastProbability
```

before the update call.

- [ ] **Step 4: Run focused Android scenario tests**

Run:

```bash
rtk gradle -p android testDebugUnitTest --tests com.busarrival.app.scenarios.Tz23ScenarioTest
rtk gradle -p android testDebugUnitTest --tests com.busarrival.app.scenarios.DetourScenarioGoldenTest
```

Expected: PASS after the grouped field assertions are updated in Task 6.

- [ ] **Step 5: Commit**

Run:

```bash
rtk git add android/app/src/main/java/com/busarrival/app/service/DetectionPipeline.kt android/app/src/main/java/com/busarrival/app/domain/model/StateModels.kt
rtk git commit -m "Emit grouped trace v2 from Android pipeline"
```

---

### Task 6: Android Scenario Migration, Fixture Rename, and Docs

**Files:**
- Modify: `android/app/src/test/java/com/busarrival/app/scenarios/DetourScenarioGoldenTest.kt`
- Modify: `android/app/src/test/java/com/busarrival/app/scenarios/Tz23ScenarioTest.kt`
- Modify: `android/app/docs/trace_output.md`
- Create: `test_data/ty225_short_detour_android_trace_v2.jsonl`
- Create: `test_data/tz_23_short_trace_v2.jsonl`
- Delete: `test_data/ty225_short_detour_android_trace.jsonl`
- Delete: `test_data/tz_23_short_trace.jsonl`

- [ ] **Step 1: Update Android tests to use v2 fields and filenames**

In `DetourScenarioGoldenTest.kt`, replace:

```kotlin
const val ANDROID_TRACE_FILENAME = "ty225_short_detour_android_trace.jsonl"
```

with:

```kotlin
const val ANDROID_TRACE_FILENAME = "ty225_short_detour_android_trace_v2.jsonl"
```

Replace flat accesses such as:

```kotlin
tick.time_ms
tick.s_cm
tick.off_route
tick.gps_jump
```

with:

```kotlin
tick.gps.time_ms
tick.kalman.s_cm
tick.detection.off_route
tick.detection.gps_jump
```

Update the schema assertion to:

```kotlin
assertEquals(setOf("gps", "kalman", "map_matching", "detection", "corridor", "stop_states"), firstTrace.keys)
```

and stop-state keys to:

```kotlin
setOf(
    "stop_idx",
    "gps_distance_cm",
    "progress_distance_cm",
    "fsm_state",
    "dwell_time_s",
    "probability",
    "previous_probability",
    "features",
    "just_arrived",
    "announced",
    "skip_on_reentry",
    "previous_distance_cm",
)
```

In `Tz23ScenarioTest.kt`, replace:

```kotlin
const val TRACE_FILENAME = "tz_23_trace.jsonl"
val destFile = testDataFile("tz_23_short_trace.jsonl")
```

with:

```kotlin
const val TRACE_FILENAME = "tz_23_trace_v2.jsonl"
val destFile = testDataFile("tz_23_short_trace_v2.jsonl")
```

- [ ] **Step 2: Update docs to grouped v2 terminology**

In `android/app/docs/trace_output.md`, update:

```markdown
- Output file: `trace.jsonl`
- Top-level fields: `time_ms`, `s_cm`, `active_stops`, ...
```

to:

```markdown
- Output file: `trace_v2.jsonl`
- Top-level groups: `gps`, `kalman`, `map_matching`, `detection`, `corridor`, `stop_states`
```

Document that `stop_states` is a diagnostic superset and `corridor.active_stops` is the only active-stop source of truth.

- [ ] **Step 3: Regenerate the canonical Android fixtures**

Run:

```bash
rtk gradle -p android testDebugUnitTest --tests com.busarrival.app.scenarios.DetourScenarioGoldenTest.test_android_trace_file_is_generated_for_manual_review
rtk gradle -p android testDebugUnitTest --tests com.busarrival.app.scenarios.Tz23ScenarioTest.test_tz23_short_trace_output_written
```

Expected: generates grouped files in `test_data/` with v2 names.

- [ ] **Step 4: Rename and remove the old fixture names**

Run:

```bash
rtk git add test_data/ty225_short_detour_android_trace_v2.jsonl test_data/tz_23_short_trace_v2.jsonl
rtk git rm test_data/ty225_short_detour_android_trace.jsonl test_data/tz_23_short_trace.jsonl
```

If `git rm` refuses because those files were not tracked, delete them with a manual editor step instead of force-removing unrelated files.

- [ ] **Step 5: Run focused Android migration tests**

Run:

```bash
rtk gradle -p android testDebugUnitTest --tests com.busarrival.app.scenarios.DetourScenarioGoldenTest
rtk gradle -p android testDebugUnitTest --tests com.busarrival.app.scenarios.Tz23ScenarioTest
```

Expected: PASS.

- [ ] **Step 6: Commit**

Run:

```bash
rtk git add android/app/src/test/java/com/busarrival/app/scenarios/DetourScenarioGoldenTest.kt android/app/src/test/java/com/busarrival/app/scenarios/Tz23ScenarioTest.kt android/app/docs/trace_output.md test_data/ty225_short_detour_android_trace_v2.jsonl test_data/tz_23_short_trace_v2.jsonl
rtk git commit -m "Migrate Android trace tests and fixtures to v2"
```

---

### Task 7: Full Verification

**Files:**
- Modify only if verification exposes missed compile or assertion issues in files already changed by Tasks 1-6.

- [ ] **Step 1: Run Rust verification**

Run:

```bash
rtk cargo test
```

Expected: PASS.

- [ ] **Step 2: Run Android verification**

Run:

```bash
rtk gradle -p android testDebugUnitTest
```

Expected: PASS.

- [ ] **Step 3: Check for stale v1 trace names in source**

Run:

```bash
rtk grep "trace\\.jsonl|_trace\\.jsonl|time_ms\\\":|active_stops\\\":\\[" crates android tools docs test_data
```

Expected:
- source files should point to `trace_v2.jsonl` / `_trace_v2.jsonl`
- remaining `trace.jsonl` hits should be historical docs or intentionally untouched artifacts only

- [ ] **Step 4: Check changed files**

Run:

```bash
rtk git status --short
rtk git diff --stat
```

Expected: only files from this plan, plus any unrelated pre-existing dirty files already present in `master`.

- [ ] **Step 5: Commit any verification fixes**

If verification required fixes, run:

```bash
rtk git add crates android tools test_data docs/superpowers/plans
rtk git commit -m "Fix trace v2 verification issues"
```

If no fixes were needed, do not create an empty commit.

---

## Plan Self-Review

Spec coverage:

- Grouped v2 schema: Tasks 1, 2, and 4.
- `detection.off_route` derived only from `"off_route"`: Task 2.
- `stop_states` diagnostic superset: Tasks 2 and 5.
- `previous_probability`: Tasks 1, 2, and 5.
- Consumer migration in the same plan: Tasks 3 and 6.
- `trace_v2.jsonl` and `_trace_v2.jsonl` naming: Tasks 3 and 6.
- Full verification: Task 7.

Type consistency:

- Rust uses grouped `TraceRecord` structs in `detection::trace`.
- Kotlin uses grouped `TraceTick` sub-objects matching the same schema names.
- `accuracy_cm` remains centimeter-based on both platforms.
- `previous_probability` is a stored pre-update value, not a re-labeled current probability.

Scope:

- No v1/v2 compatibility adapter.
- No UI changes outside trace-reading tests.
- No unrelated trace visualizer migration.

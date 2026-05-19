# MS-Native Pipeline Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert the pipeline from second-based GPS timestamps to millisecond-native processing end-to-end.

**Architecture:** Keep JSONL ingestion as the source of raw millisecond timestamps, then propagate `timestamp_ms` through the shared GPS model, localization, detection, trace output, and tests. The refactor should preserve current behavior where possible, but every component that computes `dt`, caches by timestamp, or emits traces must stop assuming 1-second granularity.

**Tech Stack:** Rust, existing `pipeline`, `shared`, and `pico2-firmware` crates, current test suites

---

### Task 1: Promote the shared GPS timestamp to milliseconds

**Files:**
- Modify: `crates/shared/src/lib.rs`
- Modify: `crates/pipeline/src/jsonl_reader.rs`
- Modify: `crates/pipeline/gps_processor/src/accumulator.rs`
- Modify: `crates/pico2-firmware/src/parser.rs`
- Test: `crates/pipeline/tests/jsonl_reader.rs`
- Test: `crates/pipeline/gps_processor/src/accumulator.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn jsonl_record_keeps_raw_ms_timestamp_in_gps_point() {
    let mut reader = JsonReader::new();
    let record = reader.parse_line(r#"{"t":1779172271904,"lat":24.156562,"lon":120.649046}"#).unwrap();
    assert_eq!(record.gps.timestamp, 1779172271904);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `rtk cargo test -p pipeline --test jsonl_reader`
Expected: FAIL because `GpsPoint.timestamp` is still seconds-based.

- [ ] **Step 3: Write minimal implementation**

```rust
pub struct GpsPoint {
    pub timestamp: u64, // milliseconds since epoch
    pub lat: f64,
    pub lon: f64,
    pub heading_cdeg: Option<HeadCdeg>,
    pub speed_cms: Option<SpeedCms>,
    pub hdop_x10: Option<u16>,
    pub has_fix: bool,
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `rtk cargo test -p pipeline --test jsonl_reader`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/shared/src/lib.rs crates/pipeline/src/jsonl_reader.rs crates/pipeline/tests/jsonl_reader.rs crates/pipeline/gps_processor/src/accumulator.rs crates/pico2-firmware/src/parser.rs
git commit -m "refactor: make gps timestamps millisecond-native"
```

### Task 2: Update dt math in localization and detection

**Files:**
- Modify: `crates/pipeline/gps_processor/src/kalman/mod.rs`
- Modify: `crates/pipeline/gps_processor/src/kalman/hysteresis.rs`
- Modify: `crates/pipeline/src/detection_state.rs`
- Modify: `crates/pico2-firmware/src/estimation/mod.rs`
- Modify: `crates/pico2-firmware/src/control/mod.rs`
- Test: `crates/pipeline/gps_processor/tests/bdd_localization.rs`
- Test: `crates/pico2-firmware/tests/test_new_architecture_integration.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn kalman_dt_uses_millisecond_delta() {
    let mut state = KalmanState::new();
    let mut dr = DrState::new();
    let gps1 = GpsPoint { timestamp: 1000, ..GpsPoint::new() };
    let gps2 = GpsPoint { timestamp: 2500, ..GpsPoint::new() };
    let dt_ms = gps2.timestamp.saturating_sub(gps1.timestamp);
    assert_eq!(dt_ms, 1500);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `rtk cargo test -p pipeline --test bdd_localization`
Expected: FAIL until dt handling and state updates stop assuming 1-second deltas.

- [ ] **Step 3: Write minimal implementation**

```rust
let dt_ms = gps.timestamp.saturating_sub(dr.last_gps_time.unwrap_or(gps.timestamp));
let dt_s = (dt_ms / 1000) as i32;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `rtk cargo test -p pipeline --test bdd_localization`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/pipeline/gps_processor/src/kalman/mod.rs crates/pipeline/gps_processor/src/kalman/hysteresis.rs crates/pipeline/src/detection_state.rs crates/pico2-firmware/src/estimation/mod.rs crates/pico2-firmware/src/control/mod.rs
git commit -m "refactor: update pipeline timing to ms-native deltas"
```

### Task 3: Update trace and integration surfaces

**Files:**
- Modify: `crates/pipeline/src/lib.rs`
- Modify: `crates/pipeline/src/main.rs`
- Modify: `crates/pipeline/detection/src/trace.rs`
- Modify: `docs/superpowers/specs/2026-05-19-jsonl-gps-input-design.md`
- Modify: `docs/superpowers/plans/2026-05-20-jsonl-timestamp-api.md`
- Test: `crates/pipeline/tests/jsonl_integration.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn trace_records_preserve_ms_timestamps() {
    let result = Pipeline::process_file(
        "../../test_data/tz_23-gps-log-20260519-063111.jsonl",
        "../../test_data/ty225_normal.bin",
    ).unwrap();
    assert!(result.trace_records.iter().any(|r| r.time > 1_000_000));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `rtk cargo test -p pipeline --test jsonl_integration`
Expected: FAIL until trace and detection surfaces stop collapsing timestamps to seconds.

- [ ] **Step 3: Write minimal implementation**

```rust
pub struct TraceRecord {
    pub time_ms: u64,
    // existing fields unchanged
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `rtk cargo test -p pipeline --test jsonl_integration`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/pipeline/src/lib.rs crates/pipeline/src/main.rs crates/pipeline/detection/src/trace.rs docs/superpowers/specs/2026-05-19-jsonl-gps-input-design.md docs/superpowers/plans/2026-05-20-jsonl-timestamp-api.md crates/pipeline/tests/jsonl_integration.rs
git commit -m "refactor: emit millisecond-native trace timestamps"
```


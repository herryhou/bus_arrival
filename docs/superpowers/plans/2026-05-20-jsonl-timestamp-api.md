# JSONL Timestamp API Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Surface the raw JSONL millisecond timestamp in the reader API while keeping the existing seconds-based pipeline behavior unchanged.

**Architecture:** Add a small public wrapper type in the JSONL reader that carries both the parsed `GpsPoint` and the original `timestamp_ms`. Update the pipeline JSONL path to consume the wrapper explicitly, and update the spec/docs to name the field in the public API instead of only mentioning an internal side channel.

**Tech Stack:** Rust, `serde_json`, existing `pipeline` crate test suite

---

### Task 1: Define the public JSONL record type

**Files:**
- Modify: `crates/pipeline/src/jsonl_reader.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn jsonl_reader_returns_timestamp_ms() {
    let mut reader = JsonReader::new();
    let record = reader.parse_line(r#"{"t":1779172271904,"lat":24.156562,"lon":120.649046}"#).unwrap();
    assert_eq!(record.timestamp_ms, 1779172271904);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `rtk cargo test -p pipeline --test jsonl_reader`
Expected: fail because `parse_line` does not yet return a record with `timestamp_ms`.

- [ ] **Step 3: Write minimal implementation**

```rust
pub struct JsonlRecord {
    pub timestamp_ms: u64,
    pub gps: GpsPoint,
}

pub fn parse_line(&mut self, line: &str) -> Option<JsonlRecord> {
    // parse JSON, set timestamp_ms from sample.t, build GpsPoint, return wrapper
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `rtk cargo test -p pipeline --test jsonl_reader`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/pipeline/src/jsonl_reader.rs crates/pipeline/tests/jsonl_reader.rs
git commit -m "feat: surface jsonl timestamp in reader api"
```

### Task 2: Update pipeline consumption and docs

**Files:**
- Modify: `crates/pipeline/src/lib.rs`
- Modify: `docs/superpowers/specs/2026-05-19-jsonl-gps-input-design.md`

- [ ] **Step 1: Update the pipeline to consume the wrapper**

```rust
if let Some(record) = jsonl_reader.parse_line(&line) {
    let gps = record.gps;
    let _timestamp_ms = record.timestamp_ms;
    if let Some(gps_record) = loc_state.process_gps(&gps, route_data) {
        // unchanged downstream flow
    }
}
```

- [ ] **Step 2: Update the spec language**

```markdown
`JsonlRecord { timestamp_ms, gps }` is the public reader output.
The reader MUST retain the raw `t` value unchanged in `timestamp_ms`.
```

- [ ] **Step 3: Run the pipeline suite**

Run: `rtk cargo test -p pipeline`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/pipeline/src/lib.rs docs/superpowers/specs/2026-05-19-jsonl-gps-input-design.md
git commit -m "docs: surface jsonl timestamp in public reader api"
```


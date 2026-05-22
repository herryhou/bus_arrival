# Rust Trace V2 Transition Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove legacy trace.jsonl support from Rust pipeline, standardize on trace_v2.jsonl format matching Android.

**Architecture:** Update test code to use trace_v2 format, update Makefile targets, add regression test for output filename, delete old fixtures.

**Tech Stack:** Rust (pipeline crate), Makefile, bash

---

## Task 1: Update detour_reentry_integration.rs to use trace_v2 format

**Files:**
- Modify: `crates/pipeline/tests/scenarios/detour_reentry_integration.rs:29-31,75-77,176-178,237-239`

**Context:** The old trace format had flat structure (`time_ms`, `s_cm`, `off_route` at top level). The new trace_v2 format has nested structure (`gps.time_ms`, `kalman.s_cm`, `detection.off_route`). This test file references old format and needs updating.

- [ ] **Step 1: Update first trace file reference to use trace_v2**

Replace line 29:
```rust
let trace_file = std::fs::File::open(super::common::test_data_dir().join("ty225_short_detour_trace.jsonl"))
```

With:
```rust
let trace_file = std::fs::File::open(super::common::test_data_dir().join("ty225_short_detour_trace_v2.jsonl"))
```

- [ ] **Step 2: Update field access for nested trace_v2 structure**

Replace lines 44-46:
```rust
let time = trace["time_ms"].as_u64().unwrap();
let s_cm = trace["s_cm"].as_i64().unwrap();
let off_route = trace["off_route"].as_bool().unwrap();
```

With:
```rust
let time = trace["gps"]["time_ms"].as_u64().unwrap();
let s_cm = trace["kalman"]["s_cm"].as_i64().unwrap();
let off_route = trace["detection"]["off_route"].as_bool().unwrap();
```

- [ ] **Step 3: Update second trace file reference**

Replace line 75:
```rust
let trace_file = std::fs::File::open(super::common::test_data_dir().join("ty225_short_detour_trace.jsonl"))
```

With:
```rust
let trace_file = std::fs::File::open(super::common::test_data_dir().join("ty225_short_detour_trace_v2.jsonl"))
```

- [ ] **Step 4: Update field access in frozen position loop**

Replace line 83:
```rust
let off_route = trace["off_route"].as_bool().unwrap();
```

With:
```rust
let off_route = trace["detection"]["off_route"].as_bool().unwrap();
```

- [ ] **Step 5: Update third trace file reference**

Replace line 176:
```rust
let trace_file = std::fs::File::open(super::common::test_data_dir().join("ty225_short_detour_trace.jsonl"))
```

With:
```rust
let trace_file = std::fs::File::open(super::common::test_data_dir().join("ty225_short_detour_trace_v2.jsonl"))
```

- [ ] **Step 6: Update field access in no_arrivals_during_offroute test**

Replace lines 188-189:
```rust
let time = trace["time_ms"].as_u64().unwrap();
let off_route = trace["off_route"].as_bool().unwrap();
```

With:
```rust
let time = trace["gps"]["time_ms"].as_u64().unwrap();
let off_route = trace["detection"]["off_route"].as_bool().unwrap();
```

- [ ] **Step 7: Update fourth trace file reference**

Replace line 237:
```rust
let trace_file = std::fs::File::open(super::common::test_data_dir().join("ty225_short_detour_trace.jsonl"))
```

With:
```rust
let trace_file = std::fs::File::open(super::common::test_data_dir().join("ty225_short_detour_trace_v2.jsonl"))
```

- [ ] **Step 8: Update field access in reentry_immediate_snap test**

Replace lines 247-249:
```rust
let time = trace["time_ms"].as_u64().unwrap();
let s_cm = trace["s_cm"].as_i64().unwrap();
let off_route = trace["off_route"].as_bool().unwrap();
```

With:
```rust
let time = trace["gps"]["time_ms"].as_u64().unwrap();
let s_cm = trace["kalman"]["s_cm"].as_i64().unwrap();
let off_route = trace["detection"]["off_route"].as_bool().unwrap();
```

- [ ] **Step 9: Run test to verify changes**

Run: `cargo test -p pipeline test_detour_reentry_snap_behavior`
Expected: PASS

- [ ] **Step 10: Run all detour tests**

Run: `cargo test -p pipeline detour_reentry_integration`
Expected: All PASS

- [ ] **Step 11: Commit**

```bash
git add crates/pipeline/tests/scenarios/detour_reentry_integration.rs
git commit -m "test: update detour_reentry_integration.rs to use trace_v2 format

- Change trace file references from *_trace.jsonl to *_trace_v2.jsonl
- Update field access for nested structure (gps.time_ms, kalman.s_cm, detection.off_route)"
```

---

## Task 2: Remove commented old trace code from normal.rs

**Files:**
- Modify: `crates/pipeline/tests/scenarios/normal.rs:158-162`

- [ ] **Step 1: Remove commented TODO block**

Delete lines 158-162:
```rust
// TODO: Add trace file path validation when trace output is implemented
// let trace_path = test_data_dir().join("ty225_normal_trace.jsonl");
// let report = analyze_position_accuracy(&trace_path);
// report.print_report();
// report.assert_all_acceptable().unwrap();
```

- [ ] **Step 2: Verify test still passes**

Run: `cargo test -p pipeline test_normal_scenario`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/pipeline/tests/scenarios/normal.rs
git commit -m "test: remove obsolete TODO comment from normal.rs"
```

---

## Task 3: Add test for generate_trace_path() output filename contract

**Files:**
- Create: `crates/pipeline/tests/main_rs_trace_path_test.rs`

- [ ] **Step 1: Create new test file**

Create file `crates/pipeline/tests/main_rs_trace_path_test.rs`:
```rust
//! Test that main.rs generates correct trace_v2.jsonl output filenames

use std::path::Path;

/// Helper function from main.rs - duplicated here for testing
fn generate_trace_path(nmea_path: &Path) -> std::path::PathBuf {
    let mut trace_path = nmea_path.to_path_buf();
    let file_stem = trace_path.file_stem().unwrap_or_default();
    let parent = trace_path.parent();

    let stem_str = file_stem.to_string_lossy();
    let base_name = stem_str.strip_suffix("_nmea").unwrap_or(&stem_str);
    let new_name = format!("{}_trace_v2.jsonl", base_name);

    if let Some(p) = parent {
        trace_path = p.join(new_name);
    } else {
        trace_path = PathBuf::from(new_name);
    }

    trace_path
}

#[test]
fn test_generate_trace_path_uses_v2_suffix() {
    // Test case 1: _nmea suffix
    let input = Path::new("test_data/ty225_normal_nmea.txt");
    let output = generate_trace_path(input);
    assert_eq!(
        output,
        Path::new("test_data/ty225_normal_trace_v2.jsonl"),
        "Expected _trace_v2.jsonl suffix for input with _nmea suffix"
    );

    // Test case 2: without _nmea suffix
    let input = Path::new("test_data/gps_log.txt");
    let output = generate_trace_path(input);
    assert_eq!(
        output,
        Path::new("test_data/gps_log_trace_v2.jsonl"),
        "Expected _trace_v2.jsonl suffix for input without _nmea suffix"
    );

    // Test case 3: ty225_short_detour
    let input = Path::new("test_data/ty225_short_detour_nmea.txt");
    let output = generate_trace_path(input);
    assert_eq!(
        output,
        Path::new("test_data/ty225_short_detour_trace_v2.jsonl"),
        "Expected _trace_v2.jsonl suffix for detour scenario"
    );
}

#[test]
fn test_generate_trace_path_does_not_use_old_format() {
    let input = Path::new("test_data/ty225_normal_nmea.txt");
    let output = generate_trace_path(input);

    let output_str = output.to_string_lossy();
    assert!(
        !output_str.contains("_trace.jsonl"),
        "Output should NOT contain old _trace.jsonl format. Got: {}",
        output_str
    );
    assert!(
        output_str.contains("_trace_v2.jsonl"),
        "Output must contain _trace_v2.jsonl format. Got: {}",
        output_str
    );
}
```

- [ ] **Step 2: Run new tests**

Run: `cargo test -p pipeline main_rs_trace_path`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/pipeline/tests/main_rs_trace_path_test.rs
git commit -m "test: add regression test for trace_v2 output filename contract

Prevents drift back to old _trace.jsonl naming convention."
```

---

## Task 4: Update Makefile TRACE_OUT variable

**Files:**
- Modify: `Makefile:55`

- [ ] **Step 1: Update TRACE_OUT to use trace_v2.jsonl**

Replace line 55:
```makefile
TRACE_OUT := $(DATA_DIR)/$(ROUTE_NAME)_$(SCENARIO)_trace.jsonl
```

With:
```makefile
TRACE_OUT := $(DATA_DIR)/$(ROUTE_NAME)_$(SCENARIO)_trace_v2.jsonl
```

- [ ] **Step 2: Verify makefile syntax**

Run: `make -n run ROUTE_NAME=ty225 SCENARIO=normal`
Expected: No errors, shows commands that would run

- [ ] **Step 3: Commit**

```bash
git add Makefile
git commit -m "build: update TRACE_OUT to use trace_v2.jsonl format"
```

---

## Task 5: Update Makefile golden target

**Files:**
- Modify: `Makefile:193`

- [ ] **Step 1: Update golden target cp command**

Replace line 193:
```makefile
@cp $(TRACE_OUT) test_data/golden/$(ROUTE_NAME)_normal_trace.jsonl
```

With:
```makefile
@cp $(TRACE_OUT) test_data/golden/$(ROUTE_NAME)_normal_trace_v2.jsonl
```

- [ ] **Step 2: Update echo message**

Replace line 195:
```makefile
@echo "Generated: test_data/golden/$(ROUTE_NAME)_normal_trace.jsonl"
```

With:
```makefile
@echo "Generated: test_data/golden/$(ROUTE_NAME)_normal_trace_v2.jsonl"
```

- [ ] **Step 3: Commit**

```bash
git add Makefile
git commit -m "build: update golden target to use trace_v2.jsonl"
```

---

## Task 6: Update Makefile clean target

**Files:**
- Modify: `Makefile:215`

- [ ] **Step 1: Update clean target to remove both formats**

Replace line 215:
```makefile
rm -f $(DATA_DIR)/*_trace.jsonl
```

With:
```makefile
rm -f $(DATA_DIR)/*_trace.jsonl $(DATA_DIR)/*_trace_v2.jsonl
```

- [ ] **Step 2: Test clean target**

Run: `make clean`
Expected: No errors, removes trace files

- [ ] **Step 3: Commit**

```bash
git add Makefile
git commit -m "build: update clean target to remove both trace formats"
```

---

## Task 7: Update Makefile validate-ty225 target

**Files:**
- Modify: `Makefile:277`

- [ ] **Step 1: Update validate-ty225 fixture path**

Replace line 277:
```makefile
test_data/tpF805_normal_trace.jsonl \
```

With:
```makefile
test_data/tpF805_normal_trace_v2.jsonl \
```

- [ ] **Step 2: Commit**

```bash
git add Makefile
git commit -m "build: update validate-ty225 to use trace_v2.jsonl fixture"
```

---

## Task 8: Add Makefile update-fixtures target

**Files:**
- Modify: `Makefile:60` (add to .PHONY), `Makefile:260` (add target after golden)

- [ ] **Step 1: Add update-fixtures to .PHONY**

Replace line 60:
```makefile
.PHONY: all run gen_nmea preprocess simulate detect pipeline clean help validate-trace validate-ty225 validate-all build-firmware firmware-uf2 flash-firmware run-detour run-detour-no-gen regression-test regression-save
```

With:
```makefile
.PHONY: all run gen_nmea preprocess simulate detect pipeline clean help validate-trace validate-ty225 validate-all build-firmware firmware-uf2 flash-firmware run-detour run-detour-no-gen regression-test regression-save update-fixtures
```

- [ ] **Step 2: Add update-fixtures target after golden target**

Insert after line 195 (after golden target):
```makefile

# Update trace_v2 fixtures from NMEA source
update-fixtures:
	@echo "=== Updating trace_v2 fixtures ==="
	cargo run -p pipeline -- test_data/ty225_normal_nmea.txt test_data/ty225_normal.bin --output test_data/ty225_normal_trace_v2.jsonl
	cargo run -p pipeline -- test_data/ty225_short_detour_nmea.txt test_data/ty225_short_detour.bin --output test_data/ty225_short_detour_trace_v2.jsonl
	@echo "Fixture update complete"
```

- [ ] **Step 3: Test update-fixtures target**

Run: `make -n update-fixtures`
Expected: Shows commands that would run

- [ ] **Step 4: Commit**

```bash
git add Makefile
git commit -m "build: add update-fixtures target for manual trace regeneration"
```

---

## Task 9: Delete old trace fixture files

**Files:**
- Delete: `test_data/ty225_normal_trace.jsonl`
- Delete: `test_data/ty225_short_detour_trace.jsonl`

- [ ] **Step 1: Delete old trace files**

```bash
rm test_data/ty225_normal_trace.jsonl
rm test_data/ty225_short_detour_trace.jsonl
```

- [ ] **Step 2: Verify v2 files still exist**

```bash
ls -la test_data/*_trace_v2.jsonl
```

Expected: Shows ty225_normal_trace_v2.jsonl and ty225_short_detour_trace_v2.jsonl

- [ ] **Step 3: Run all tests to verify**

Run: `cargo test -p pipeline`
Expected: All PASS

- [ ] **Step 4: Commit**

```bash
git add test_data/
git commit -m "test: remove old trace.jsonl fixtures, keep trace_v2.jsonl"
```

---

## Task 10: Update CLAUDE.md documentation

**Files:**
- Modify: `CLAUDE.md` (trace output examples)

- [ ] **Step 1: Find and update trace.jsonl references**

Search for instances of `trace.jsonl` in CLAUDE.md and update to `trace_v2.jsonl` where referring to the output format.

- [ ] **Step 2: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: update examples to reference trace_v2.jsonl format"
```

---

## Task 11: Final verification

**Files:**
- None (verification only)

- [ ] **Step 1: Run all pipeline tests**

Run: `cargo test -p pipeline`
Expected: All PASS

- [ ] **Step 2: Run regression tests**

Run: `cargo test -p pipeline regression_tests`
Expected: All PASS

- [ ] **Step 3: Verify make run generates correct output**

Run: `make run ROUTE_NAME=ty225 SCENARIO=normal`
Expected: Output shows `trace_v2.jsonl` in TRACE_OUT message

- [ ] **Step 4: Verify clean removes both formats**

Run: `make clean`
Expected: Both `*_trace.jsonl` and `*_trace_v2.jsonl` removed

- [ ] **Step 5: Check for any remaining old format references**

```bash
grep -r "trace\.jsonl" --include="*.rs" --include="*.md" crates/pipeline/ Makefile CLAUDE.md
```

Expected: No results (or only in comments/history)

- [ ] **Step 6: Final commit**

```bash
git commit --allow-empty -m "test: verify trace_v2 transition complete

All tests pass, Makefile targets updated, old fixtures removed."
```

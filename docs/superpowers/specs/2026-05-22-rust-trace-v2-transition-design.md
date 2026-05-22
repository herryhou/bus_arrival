# Rust Trace V2 Transition Design

**Date:** 2026-05-22
**Status:** Approved
**Scope:** Rust Host (pipeline crate), test fixtures

## Overview

Remove legacy `trace.jsonl` support from Rust pipeline code. `trace_v2.jsonl` becomes the sole trace format. Embedded firmware unchanged (no trace output capability). Android already migrated.

## Motivation

- **Parity:** Rust Host should match Android's trace_v2.jsonl format
- **Cleanup:** Remove dual-format code paths, simplify maintenance
- **Clarity:** Single format eliminates confusion about which format to use

## Current State

**Android:** Fully migrated to `trace_v2.jsonl`
- `TraceTick` data class with nested structures
- `TraceLoader` reads v2 format
- Tests validate grouped structure

**Rust Host:** Partially migrated
- `TraceRecord` already matches Android format
- `main.rs` generates `_trace_v2.jsonl` output
- Test fixtures still reference old `trace.jsonl` format
- Legacy code paths remain

**Rust Embedded:** No trace output (no_std constraints)
- Debug via NMEA replay through host pipeline

## Design

### Format Differences

| Old (trace.jsonl) | New (trace_v2.jsonl) |
|-------------------|----------------------|
| Flattened per-stop fields | Grouped by category |
| Limited state tracking | Expanded fields (previous_probability, previous_distance_cm, just_arrived, skip_on_reentry) |
| Inconsistent with Android | Matches Android exactly |

### Changes

#### 1. Rust Host Code

**`crates/pipeline/src/main.rs`**
- No changes (already generates `_trace_v2.jsonl`)

**`crates/pipeline/tests/scenarios/common/mod.rs`**
- Update `load_trace_reader()` to use `trace_v2.jsonl` filenames
- Remove old format references

**`crates/pipeline/tests/scenarios/normal.rs`**
- Remove commented old trace code

**`crates/pipeline/tests/scenarios/detour_reentry_integration.rs`**
- Update trace file references to `trace_v2.jsonl`

#### 2. Test Fixtures

**Delete:**
- `test_data/ty225_normal_trace.jsonl`
- `test_data/ty225_short_detour_trace.jsonl`

**Keep:**
- All `*_trace_v2.jsonl` files

#### 3. Makefile

**Update existing targets (hard-coded `_trace.jsonl` references):**
- Line 55: `TRACE_OUT` variable → `$(DATA_DIR)/$(ROUTE_NAME)_$(SCENARIO)_trace_v2.jsonl`
- Line 193: `golden` target cp command → use `_trace_v2.jsonl`
- Line 215: `clean` target rm pattern → remove both `*_trace.jsonl` and `*_trace_v2.jsonl`
- Line 277: `validate-ty225` fixture path → `tpF805_normal_trace_v2.jsonl`

**Add new target:**
```makefile
update-fixtures:
	cargo run -p pipeline -- test_data/ty225_normal_nmea.txt test_data/ty225_normal.bin --output test_data/ty225_normal_trace_v2.jsonl
	cargo run -p pipeline -- test_data/ty225_short_detour_nmea.txt test_data/ty225_short_detour.bin --output test_data/ty225_short_detour_trace_v2.jsonl
```

#### 4. Documentation

Update `CLAUDE.md` examples to reference `trace_v2.jsonl`

## Testing

**Existing tests:**
- `test_tpF805_trace_v2_stop_states_match_active_stops()` validates trace_v2 schema (stop_states grouping, field expansion)
- Golden tests compare against trace_v2 fixtures

**New test needed:**
- Add test validating `generate_trace_path()` output filename contract
- Ensures auto-generated paths use `_trace_v2.jsonl` suffix
- Prevents regression if default output naming drifts back to `_trace.jsonl`

## Rollout

1. Update test code references (`mod.rs`, `normal.rs`, `detour_reentry_integration.rs`)
2. Update Makefile (TRACE_OUT, golden, clean, validate-ty225)
3. Add makefile `update-fixtures` target
4. Add test for `generate_trace_path()` output filename contract
5. Delete old trace fixtures
6. Update documentation
7. Verify all tests pass

## Alternatives Considered

**A)** Rename functions to explicitly say "trace_v2"
- Rejected: Unnecessary churn. "trace" naming fine when v2 is only format.

**B)** Add deprecation warnings first
- Rejected: Internal code, no external consumers. Direct removal cleaner.

**C)** Embedded trace output
- Rejected: no_std constraints, limited flash. Debug via host replay sufficient.

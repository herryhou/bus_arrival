# Output Consolidation Design: Single trace.jsonl

**Date:** 2026-05-05
**Status:** Approved
**Author:** Claude

## Overview

Consolidate pipeline output from 3 files (trace.jsonl, arrivals.jsonl, announce.jsonl) to 1 file (trace.jsonl). Consumers filter what they need using jq or helper scripts.

## Motivation

Currently the pipeline generates redundant output files:
- `trace.jsonl` (800+ lines) - contains ALL state machine info
- `arrivals.jsonl` (4 lines) - subset of trace (just_arrived=true)
- `announce.jsonl` (6 lines) - subset of trace (corridor entries)

This creates code complexity and file clutter. The trace already contains everything needed.

## Design

### CLI Changes

**Before:**
```bash
pipeline <nmea> <route_data> <output> [--trace <file>] [--announce <file>]
```

**After:**
```bash
pipeline <nmea> <route_data> [--output <trace.jsonl>]
```

**Auto-generated output path (when --output not specified):**
1. Take input NMEA filename
2. Remove directory path (if any)
3. Replace extension with `_trace.jsonl`
4. Preserve directory from input path

**Transformation examples:**
- `test_data/ty225_normal_nmea.txt` → `test_data/ty225_normal_trace.jsonl`
- `nmea.txt` → `nmea_trace.jsonl`
- `route.v2.nmea` → `route.v2_trace.jsonl`
- `/path/to/gps.nmea` → `/path/to/gps_trace.jsonl`

**Explicit output path:**
```bash
pipeline input.nmea route.bin --output custom_trace.jsonl
```

### API Changes (PipelineResult)

**Keep:**
- `arrivals: Vec<ArrivalEvent>` - primary output, used by tests
- `departures: Vec<DepartureEvent>` - primary output
- `trace_records: Vec<TraceRecord>` - **Changed from Option to Vec** (trace always enabled, std only)

**Remove:**
- `PipelineConfig` struct - no longer needed
- `announce_events: Option<Vec<AnnounceEvent>>` - derivable from trace
- `enable_trace`, `enable_announce` config fields

**Rationale for removing Option:**
Since trace is always enabled, `Option<Vec>` adds unnecessary `unwrap()` overhead. Change to `Vec` directly.

**Note on no_std/embedded:**
`trace_records` is gated by `#[cfg(feature = "std")]` and is NOT compiled for the embedded firmware. The firmware uses its own 2-layer architecture (Control/Estimation layers) and doesn't use `PipelineResult` at all. This change affects only the std (host) build.

### Component Changes

#### 1. crates/pipeline/src/main.rs

- Remove `output` positional argument
- Remove `--trace` and `--announce` options
- Auto-generate trace output path from input NMEA filename
- Remove separate arrivals/announce file writing logic
- Simplify summary print statements

#### 2. crates/pipeline/src/lib.rs

- Remove `PipelineConfig` struct
- Remove `enable_trace`, `enable_announce` from process_nmea_file signature
- Remove `announce_events` from `PipelineResult`
- Remove `write_output()` function (no longer writes merged arrivals/departures)
- `trace_records` is always `Some` (no conditional)

#### 3. New: tools/arrival_from_trace.sh

Helper script to extract arrivals from trace.jsonl:
```bash
./tools/arrival_from_trace.sh test_data/route_scenario_trace.jsonl > arrivals.jsonl
```

**jq query (with edge case handling):**
```bash
jq 'select(.stop_states and (.stop_states[] | select(.just_arrived == true))) |
    {time, stop_idx: (.stop_states[] | select(.just_arrived == true) | .stop_idx), s_cm, v_cms, probability: (.stop_states[] | select(.just_arrived == true) | .probability)}' \
  "$1"
```

**Edge cases handled:**
- Empty `stop_states` array - safely skipped
- Multiple stops with `just_arrived=true` - all extracted
- Missing fields - uses `?` operator where appropriate

#### 4. New: tools/announce_from_trace.sh

Helper script to extract announce events from trace.jsonl:
```bash
./tools/announce_from_trace.sh test_data/route_scenario_trace.jsonl > announce.jsonl
```

**jq query (with edge case handling):**
```bash
jq 'select(.active_stops and (.active_stops | length > 0)) |
    {time, stop_idx: .active_stops[0], s_cm, v_cms}' \
  "$1"
```

**Edge cases handled:**
- Missing `active_stops` field - uses `and` for null check
- Empty array - `length > 0` check
- Multiple active stops - takes first (corridor entry semantics)

#### 5. Makefile

- Update `pipeline` target to use 2-arg CLI
- Remove `ANNOUNCE_OUT` variable
- Update summary output messages
- Remove `--trace` and `--announce` flags from pipeline invocation

#### 6. Documentation (CLAUDE.md)

- Update build commands to use new CLI
- Add jq filtering examples
- Document helper script usage

## Trace Format

The `trace.jsonl` file contains one JSON object per line (JSONL format). Each line represents a single GPS update with complete state machine information.

### TraceRecord Structure

```json
{
  "time": 1234567890,
  "lat": 25.123456,
  "lon": 121.654321,
  "s_cm": 12345,
  "v_cms": 500,
  "heading_cdeg": 1234,
  "active_stops": [0, 1],
  "stop_states": [
    {
      "stop_idx": 0,
      "gps_distance_cm": 100,
      "progress_distance_cm": 50,
      "fsm_state": "Arriving",
      "dwell_time_s": 0,
      "probability": 200,
      "features": {...},
      "just_arrived": false,
      "skip_on_reentry": false
    }
  ],
  "gps_jump": false,
  "recovery_idx": null,
  "status": "valid",
  "off_route": false,
  "segment_idx": 42,
  "heading_constraint_met": true,
  "divergence_cm": 5,
  "hdop": 1.5,
  "num_sats": 12,
  "fix_type": "3D",
  "variance_cm2": 100,
  "corridor_start_cm": 12000,
  "corridor_end_cm": 13000,
  "next_stop": [2, 150]
}
```

### Key Fields for Filtering

- **`just_arrived`** (in `stop_states[]`) - true when arrival detected this frame
- **`active_stops`** - array of stop indices currently in corridor
- **`fsm_state`** (in `stop_states[]`) - current state: "Approaching", "Arriving", "AtStop", "Departed"
- **`off_route`** - true when bus is off-route (detouring)

## Data Flow

**Before:**
```
NMEA → Pipeline → {
  arrivals.jsonl    (4 lines - arrivals only)
  announce.jsonl    (6 lines - corridor entries)
  trace.jsonl       (800+ lines - everything)
}
```

**After:**
```
NMEA → Pipeline → trace.jsonl (800+ lines - everything)

Consumers filter:
  → tools/arrival_from_trace.sh → arrivals.jsonl
  → tools/announce_from_trace.sh → announce.jsonl
  → or use jq directly
```

## Migration Path for Consumers

### Option 1: Use helper scripts

```bash
# Before
pipeline input.nmea route.bin arrivals.jsonl --trace trace.jsonl --announce announce.jsonl

# After
pipeline input.nmea route.bin
./tools/arrival_from_trace.sh test_data/route_scenario_trace.jsonl > arrivals.jsonl
./tools/announce_from_trace.sh test_data/route_scenario_trace.jsonl > announce.jsonl
```

### Option 2: Use jq directly

```bash
# Extract arrivals
jq 'select(.stop_states[].just_arrived == true) |
    {time, stop_idx: .stop_states[0].stop_idx, s_cm, v_cms, probability}' \
  trace.jsonl > arrivals.jsonl

# Extract announce events (corridor entry)
jq 'select(.active_stops | length > 0) |
    {time, stop_idx: .active_stops[0], s_cm, v_cms}' \
  trace.jsonl > announce.jsonl

# Find all Approaching/Arriving states
jq '.stop_states[]? | select(.fsm_state == "Approaching" or .fsm_state == "Arriving")' \
  trace.jsonl
```

## Testing Strategy

### Unit Tests
- `regression_tests.rs` uses `result.arrivals` directly - **unaffected**
- Tests check in-memory arrivals, not file output

### Integration Tests
- `integration_test.rs` currently doesn't read output files - **unaffected**
- If any tests read files, update to read trace instead

### Golden Tests
- Current golden tests use `result.arrivals` in-memory - **unaffected**
- **Add**: Trace golden test verification
  - Verify trace.jsonl format is valid JSONL
  - Verify expected fields are present
  - Verify arrivals can be extracted from trace
  - Use existing test data (ty225_normal, ty225_short_detour, etc.)

### Helper Script Tests
- Test `arrival_from_trace.sh` produces valid output
- Test `announce_from_trace.sh` produces valid output
- Verify edge cases (empty trace, no arrivals, etc.)

## Error Handling

- **Trace write failure**: Return `PipelineError::IoError` immediately (fail-fast). Processing stops; partial trace file may exist but will be incomplete.
- **Invalid NMEA**: Continue processing, log to stderr (existing behavior)
- **Route data load failure**: Return `PipelineError::RouteDataError` (existing behavior)

**Trace write behavior:**
- Trace file is created at start of processing
- Each line is written as GPS updates are processed
- If write fails mid-processing, error is returned immediately
- Partial trace file may exist but should be considered invalid

## Implementation Files

1. `crates/pipeline/src/main.rs` - CLI simplification
2. `crates/pipeline/src/lib.rs` - Remove PipelineConfig, simplify API
3. `tools/arrival_from_trace.sh` - New helper script
4. `tools/announce_from_trace.sh` - New helper script
5. `Makefile` - Update pipeline target
6. `docs/CLAUDE.md` - Update documentation

## Backward Compatibility

This is a **breaking change** for consumers that depend on the old CLI and separate output files.

### Breaking Changes
- CLI: `pipeline <nmea> <route_data> <output>` → `pipeline <nmea> <route_data>`
- Removed: `--trace` and `--announce` flags
- Removed: `arrivals.jsonl` and `announce.jsonl` file generation
- API: `PipelineConfig` removed from library interface
- API: `PipelineResult.trace_records` changed from `Option<Vec>` to `Vec`
- API: `PipelineResult.announce_events` removed

### Migration Path

**For CLI users:**
```bash
# Old
make run ROUTE_NAME=ty225 SCENARIO=normal

# New (same command, different output)
make run ROUTE_NAME=ty225 SCENARIO=normal
# Output: test_data/ty225_normal_trace.jsonl (instead of 3 files)
```

**For script consumers:**
```bash
# Old
pipeline input.nmea route.bin arrivals.jsonl --trace trace.jsonl --announce announce.jsonl

# New
pipeline input.nmea route.bin
./tools/arrival_from_trace.sh input_trace.jsonl > arrivals.jsonl
./tools/announce_from_trace.sh input_trace.jsonl > announce.jsonl
```

**For library users:**
```rust
// Old
let config = PipelineConfig { enable_trace: true, enable_announce: true };
let result = Pipeline::process_nmea_file(nmea, route, output, &config)?;

// New
let result = Pipeline::process_nmea_file(nmea, route)?;
// result.trace_records is now Vec, not Option
// result.announce_events removed (filter trace_records instead)
```

### Mitigation
- Helper scripts provide a simple migration path
- jq examples allow custom filtering
- In-memory API (result.arrivals, result.departures) unchanged for library users
- This spec documents all changes clearly

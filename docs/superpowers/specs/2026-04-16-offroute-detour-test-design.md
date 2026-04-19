# Off-Route Detour Detection Test Case Design

**Date:** 2026-04-16
**Status:** Approved

## Overview

Create a comprehensive test case for off-route (detour) detection based on ty225 route. The test uses a self-contained route segment covering ty225 stops 5-14 (10 stops total) and simulates a 60-second off-road detour from stop 6 to stop 11, skipping stops 7-10.

**Purpose:** Validate the complete off-route detection lifecycle including detection trigger (5s), position freezing, skipped stops, and re-acquisition recovery.

**Key Requirements:**
- Route segment: ty225 stops 5-14 (self-contained)
- Detour: Stop 6 → Stop 11 (skipping 7, 8, 9, 10)
- Duration: 60 seconds off-route
- GPS: Consistent 1-second intervals
- Output files: `ty225_detour_*` naming convention

## Architecture

The test generation system consists of three main components:

### 1. Route Extractor (New)
**Responsibility:** Extract route segment from full ty225 route data.

**Inputs:**
- `test_data/ty225_route.json` - Full route points
- `test_data/ty225_stops.json` - Full stop definitions

**Outputs:**
- `test_data/ty225_detour_stops.json` - 10 stops (indices 5-14)
- `test_data/ty225_detour_route.json` - Route points for segment

**Operations:**
- Extract stops 5-14 (original indices) from ty225_stops.json
- Extract corresponding route segment from ty225_route.json
- Re-index stops to 0-9 for self-contained test
- Simplify route using Douglas-Peucker if needed

### 2. Detour Simulator (Modified gen_shortcut_sim.js)
**Responsibility:** Generate NMEA GPS trace with controlled detour scenario.

**Key Changes from gen_shortcut_sim.js:**
1. Fix timing: Ensure `emitGPS()` is called exactly once per loop iteration with `ts++` increment
2. No timestamp gaps: Eliminate issues like 80139→80145 jumps
3. Detour configuration: 60-second duration, stop 6→stop 11 path

**Inputs:**
- `test_data/ty225_detour_stops.json`
- `test_data/ty225_detour_route.json`

**Outputs:**
- `test_data/ty225_detour_detour_nmea.txt` - NMEA GPS trace
- `test_data/ty225_detour_detour_gt.json` - Ground truth annotations

**Operations:**
- Normal operation: Stop 5 → Stop 6
- Detour start: At stop 6 departure
- Off-route phase: 60s straight-line travel to stop 11
- Re-acquisition: At stop 11 arrival
- Normal operation: Stop 11 → Stop 14

### 3. Validation Pipeline
**Responsibility:** Run full detection pipeline and validate results.

**Inputs:**
- `test_data/ty225_detour_detour_nmea.txt`
- `test_data/ty225_detour_detour_gt.json`
- `test_data/ty225_detour_detour.bin` (from preprocessor)

**Outputs:**
- `test_data/ty225_detour_detour_arrivals.json` - Arrival detection results
- `test_data/ty225_detour_detour_trace.jsonl` - Pipeline trace
- `test_data/ty225_detour_detour_announce.jsonl` - Announce events
- `test_data/ty225_detour_detour_validation.html` - Visual validation
- `test_data/ty225_detour_detour_summary.md` - Test summary

## Data Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                    Phase 1: Route Extraction                    │
├─────────────────────────────────────────────────────────────────┤
│  ty225_route.json + ty225_stops.json                            │
│                          ↓                                       │
│  Route Extractor → ty225_detour_route.json + stops.json         │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                   Phase 2: NMEA Generation                      │
├─────────────────────────────────────────────────────────────────┤
│  ty225_detour_route.json + ty225_detour_stops.json              │
│                          ↓                                       │
│  Detour Simulator → ty225_detour_detour_nmea.txt                │
│                     + ty225_detour_detour_gt.json                │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                  Phase 3: Pipeline Processing                   │
├─────────────────────────────────────────────────────────────────┤
│  ty225_detour_detour_nmea.txt + route_data.bin                  │
│                          ↓                                       │
│  Unified Pipeline → arrivals.json + trace.jsonl + announce.jsonl│
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                      Phase 4: Validation                         │
├─────────────────────────────────────────────────────────────────┤
│  trace.jsonl + ground_truth → Validation Report                 │
│                          ↓                                       │
│  Visual + Text Summary (validation.html + summary.md)           │
└─────────────────────────────────────────────────────────────────┘
```

## Error Handling & Edge Cases

### Scenario Validation
**Pre-flight checks:**
- Verify route has at least 10 stops for segment extraction
- Verify stop indices 5-14 exist in source data
- Verify stop 6 → stop 11 distance allows 60s travel at cruise speed

**Failure handling:**
- Invalid stop indices: Clear error message indicating valid range
- Insufficient route points: Fallback to shorter segment or error
- Speed constraints: Adjust detour duration or show constraint violation

### Timing Validation
**GPS continuity checks:**
- Verify NMEA timestamps increment by exactly 1 second
- Detect gaps: Parse NMEA file, check `ts[i+1] - ts[i] == 1` for all i
- Detect duplicates: Check for repeated timestamps

**Failure handling:**
- Timing gaps found: Report gap locations and sizes, fail generation
- Fix: Ensure `emitGPS()` increments `ts` exactly once per call, no early returns

### Test Validation Checks

| Check | Description | Pass Criteria |
|-------|-------------|---------------|
| GPS Continuity | No timestamp gaps in NMEA file | All Δt = 1s |
| Off-Route Detection | Detection triggers at 5s after detour start | GT shows off-route at expected time |
| Position Freezing | DR position stops advancing during off-route | Trace shows same position during off-route |
| Skipped Stops | Stops 7-10 NOT announced | Announce file missing stops 7-10 |
| Re-Acquisition | Stop 11 detected after detour | Announce file includes stop 11 |
| Timing | 60-second detour duration | GT off_route_duration_s = 60 |

**Failure Modes:**
- GPS continuity failure: Fix gen_shortcut_sim.js timing logic
- Off-route not detected: Check detection thresholds, GPS noise levels
- Position not frozen: Check off-route state machine logic
- Skipped stops announced: Check stop filtering during off-route
- Re-acquisition fails: Check recovery::find_stop_index() logic
- Timing mismatch: Adjust detour distance or duration

### Edge Cases
**Short detours (< 5s):** Not applicable to this test (60s detour)
**Long detours (> 120s):** May trigger additional safeguards, test validates 60s case
**GPS noise:** AR(1) noise model ensures realistic but predictable behavior
**Boundary conditions:** Detour start/end at stop locations (well-defined)

## Testing Strategy

### Test Execution
```bash
# Step 1: Generate route segment
node tools/extract_route_segment.js

# Step 2: Generate NMEA and ground truth
make gen_nmea ROUTE_NAME=ty225_detour SCENARIO=detour

# Step 3: Generate binary route data
make preprocess ROUTE_NAME=ty225_detour SCENARIO=detour

# Step 4: Run full pipeline
make pipeline ROUTE_NAME=ty225_detour SCENARIO=detour

# Step 5: Validate results
node tools/validate_detour_test.js
```

### Validation Checks

1. **GPS Continuity:** Verify 1-second intervals throughout NMEA file (no gaps)
2. **Off-Route Detection:** Confirm detection triggers at 5s after detour start
3. **Position Freezing:** Verify DR position stops advancing during off-route
4. **Skipped Stops:** Confirm stops 7-10 are NOT announced
5. **Re-Acquisition:** Verify stop 11 is correctly detected after detour
6. **Timing:** Confirm 60-second detour duration in ground truth

### Success Criteria
- All 6 validation checks pass
- Ground truth timestamps match NMEA timestamps
- No false positives (off-route detection during normal segments)
- No false negatives (missed off-route detection)

### Regression Testing
- Run existing ty225 tests (normal, shortcut) to ensure no regressions
- Compare detour behavior against baseline expectations

## Ground Truth Format

```json
[
  {
    "stop_idx": 0,
    "seg_idx": 0,
    "timestamp": 1700000000,
    "dwell_s": 10
  },
  {
    "stop_idx": 1,
    "seg_idx": 6,
    "timestamp": 1700000100,
    "dwell_s": 7
  },
  {
    "stop_idx": 2,
    "lat": <stop_6_lat>,
    "lon": <stop_6_lon>,
    "timestamp": <departure_time>,
    "phase": "shortcut_start",
    "event": "departure_shortcut"
  },
  {
    "stop_idx": 2,
    "lat": <stop_11_lat>,
    "lon": <stop_11_lon>,
    "timestamp": <reacquisition_time>,
    "phase": "shortcut_end",
    "event": "re_acquisition",
    "off_route_duration_s": 60
  },
  {
    "stop_idx": 2,
    "seg_idx": <stop_11_seg>,
    "timestamp": <arrival_time>,
    "dwell_s": 10
  }
]
```

## Summary

This design creates a self-contained, validated test case for off-route detour detection. The route segment (stops 5-14 from ty225) provides a manageable scope while covering all critical scenarios. The 60-second straight-line detour from stop 6 to 11 tests the complete off-route detection lifecycle.

**Key Innovation:** Fix timing issues in `gen_shortcut_sim.js` to ensure consistent 1-second GPS intervals, addressing gaps observed in previous test runs (e.g., 80139→80145).

**Deliverables:**
1. Route extractor script (`tools/extract_route_segment.js`)
2. Modified detour simulator (`tools/gen_detour_sim.js`)
3. Validation script (`tools/validate_detour_test.js`)
4. Complete test data package (`test_data/ty225_detour_*`)
5. Test summary report (`test_data/ty225_detour_detour_summary.md`)

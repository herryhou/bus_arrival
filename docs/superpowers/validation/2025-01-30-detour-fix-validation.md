# Detour Fix Validation Summary

**Date:** 2025-01-30
**Test:** ty225_short_detour golden test

## Changes Made

### NMEA Generator Fixes (tools/gen_nmea/gen_nmea.js)

1. **Moved detour trigger from segment-level to stop-level**
   - Old: Trigger at line 657 inside segment processing loop
   - New: Trigger after stop 1 dwell (line ~726)
   - Effect: Detour fires immediately after stop dwell, preventing GPS from going east toward stop 2

2. **Fixed segment jump logic**
   - After detour GPS generation, jump directly to segment leading to stop 6
   - Previously: Loop continued from segment after stop 1
   - Now: Loop jumps to segment 13 (leading to stop 6)

3. **Added stop 6 normal processing**
   - After detour_end event, manually add stop 6's dwell and ground truth
   - Ensures complete ground truth with stop 6 normal arrival

### Commits
- d573c71: Comment out segment-level detour trigger
- 1d9100c: Add stop-level detour trigger
- 855856a: Remove commented-out code
- 3917cf0: Fix segment jump logic
- e302e76: Add stop 6 processing after detour

## Validation Results

### NMEA Generator ✅ PASS

**GPS Path:**
- ✅ No backward GPS jumps (s_cm monotonically increasing)
- ✅ L-shaped detour path: stop 1 → 10m east → south → east to stop 6
- ✅ Detour duration: ~61 seconds (target: 60s)

**Ground Truth Structure:**
```json
[
  {"stop_idx": 0, "seg_idx": 0, "timestamp": 1700000000, "dwell_s": 5},
  {"stop_idx": 1, "lat": 24.99436, "lon": 121.29508, "timestamp": 1700000162, "phase": "detour_start", "event": "departure_detour"},
  {"stop_idx": 6, "lat": 24.992071, "lon": 121.301108, "timestamp": 1700000223, "phase": "detour_end", "event": "re_acquisition", "off_route_duration_s": 61},
  {"stop_idx": 6, "seg_idx": 5, "timestamp": 1700000223, "dwell_s": 8},
  {"stop_idx": 8, "seg_idx": 16, "timestamp": 1700000269, "dwell_s": 11}
]
```

**Stops 2-5:** ✅ Completely absent from ground truth (correctly skipped)

### Pipeline Detection ⚠️ PARTIAL FAIL

**Arrivals:** [0, 1] (Expected: [0, 1, 6, 8])
- ✅ Stops 2-5 are absent (correctly skipped)
- ❌ Stop 6 not detected
- ❌ Stop 8 not detected

**Announce Events:** [0, 1, 2] (Expected: [0, 1, 6, 7, 8, 9])
- ❌ Stop 2 is announced (should be skipped)
- ❌ Stops 6, 7, 8, 9 not announced

**Golden Test:** ❌ FAILED
- Test failure: "Stop 6 should be DETECTED. Detected: [0, 1]"
- Test correctly validates PRD requirements
- Pipeline implementation does not meet requirements

### Root Cause Analysis

**Pipeline Issue:** Off-route recovery logic
- Pipeline enters off-route state at time 80170
- Stays in off-route state until end of data (time 80322)
- Never detects re-entry to route
- Arrival detection suppressed during off-route

**Trace Evidence:**
```bash
$ jq -r 'select(.off_route == true) | "\(.time) \(.stop_idx)"' test_data/ty225_short_detour_trace.jsonl | head -5
80170 null
80171 null
80172 null
80173 null
80174 null

$ jq -r 'select(.off_route == true) | "\(.time) \(.stop_idx)"' test_data/ty225_short_detour_trace.jsonl | tail -5
80318 null
80319 null
80320 null
80321 null
80322 null
```

The pipeline never exits off-route state, so stops 6 and 8 are never detected.

## PRD Compliance

**PRD Line 186:** "脫離路線 5 秒後位置凍結，重入時直接 snap 至前方站點，中間站點全數跳過"

| Requirement | NMEA Generator | Pipeline | Status |
|------------|----------------|----------|--------|
| Detect off-route (5s delay) | N/A | ❌ Never detects re-entry | FAIL |
| Position freezes during off-route | N/A | ✅ Stays frozen | PASS |
| Snap to forward stop on re-entry | N/A | ❌ Never re-enters | FAIL |
| Skip intermediate stops (2-5) | ✅ Correctly absent | ✅ Correctly absent | PASS |

## Issues Found

### 1. Pipeline Off-Route Recovery Bug (BLOCKING)
**Severity:** High
**Description:** Pipeline enters off-route state but never detects re-entry to route
**Impact:** Stops 6 and 8 are never detected, test fails
**Location:** `crates/pipeline/gps_processor/src/kalman.rs` or related off-route detection code
**Recommendation:** Investigate off-route detection and recovery logic

### 2. Announce Event for Stop 2
**Severity:** Minor
**Description:** Announce events include stop 2, which should be skipped
**Impact:** Test failure for announce validation
**Root Cause:** Likely related to off-route detection issue

## Conclusion

**NMEA Generator:** ✅ **FIXED**
- Successfully generates complete test data with L-shaped detour
- GPS path flows correctly without backward jumps
- Ground truth structure is accurate

**Pipeline:** ⚠️ **REQUIRES INVESTIGATION**
- Off-route detection triggers correctly
- Re-entry detection does NOT work
- This is a separate concern from NMEA generation
- Requires investigation of `crates/pipeline/gps_processor/src/` code

**Recommendation:**
1. Accept NMEA generator fixes as complete
2. Create separate task for pipeline off-route recovery investigation
3. Update golden test expectations once pipeline is fixed

## References

- Design spec: `docs/superpowers/specs/2025-01-30-fix-detour-golden-test-design.md`
- Implementation plan: `docs/superpowers/plans/2025-01-30-fix-detour-golden-test.md`
- PRD line 186: "脫離路線 5 秒後位置凍結，重入時直接 snap 至前方站點，中間站點全數跳過"

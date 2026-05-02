# Fix Detour Golden Test - Design Spec

**Date:** 2025-01-30

## Problem Statement

The `ty225_short_detour` golden test is failing because:

1. **Backward GPS jump:** The NMEA generator processes route segments after stop 1's dwell, emitting GPS points going east toward stop 2. Then it suddenly jumps 550m BACK to start the detour.

2. **Stop 2 incorrectly detected:** During the suspect phase (before off_route is confirmed), position advances via DR. This advancing position enters stop 2's corridor, triggering arrival detection.

3. **Ground truth mismatch:** Expected arrivals are [0, 1, 6, 8] but actual results are [0, 1, 2].

## Root Cause

The detour trigger check happens INSIDE the segment processing loop (line 657), AFTER stop processing has completed. This means:

1. Stop 1 dwell is processed (lines 779-809)
2. Segment loop continues, emitting GPS points going east toward stop 2
3. Multiple segments processed...
4. FINALLY detour triggered when condition `stopIndexMap[si] === detourFromStop` is met

The detour should trigger **immediately after stop 1's dwell**, not after processing additional route segments.

## Solution

**Trigger detour at the STOP level, not SEGMENT level.**

After emitting stop 1's dwell time, immediately check if this is the detour start stop. If so:
1. Trigger detour immediately
2. Generate detour GPS points (L-shaped path)
3. Skip all segment processing until detour end
4. Resume at detour end stop

## Architecture Changes

### Current Flow (Broken)
```
Process segment → Stop 1 dwell → Continue segments (east toward stop 2) → 
Detour trigger (late) → GPS jump west → Detour GPS → Resume at stop 6
```

### New Flow (Fixed)
```
Process segment → Stop 1 dwell → [Is detour?] → YES: Detour GPS → Skip segments → Resume at stop 6
                              → NO: Continue normal
```

## Implementation

### File: `tools/gen_nmea/gen_nmea.js`

#### Change 1: Move detour trigger to stop processing (after line 809)

```javascript
// After stop/light processing (line 809)
if (prevSeg && prevSeg.stopBefore) {
    const currentStopIdx = stopIndexMap[si];
    
    // NEW: Trigger detour immediately after stop 1 dwell
    if (detour && !detourActive && !detourCompleted && currentStopIdx === detourFromStop) {
        detourActive = true;
        console.log(`Detour triggered at stop ${detourFromStop}, going via waypoint (${detourWaypointLat}, ${detourWaypointLon}) to stop ${detourToStop}`);
        
        // Generate detour GPS points HERE (moved from segment loop)
        // [detour generation code - see below]
        
        // Skip to detour end segment
        while (si < route_points.length - 1 && stopIndexMap[si] !== detourToStop) {
            si++;
        }
        continue; // Skip normal segment processing
    }
    
    // Original stop processing logic continues...
}
```

#### Change 2: Remove old detour trigger (line 657)

Delete or comment out the segment-level detour trigger:
```javascript
// REMOVED: Detour trigger now happens at stop level, not segment level
// if (detour && !detourActive && !detourCompleted && seg.stopBefore && stopIndexMap[si] === detourFromStop) {
//   detourActive = true;
//   ...
// }
```

#### Change 3: Update segment skip logic (lines 760-771)

The existing segment skip logic for `detourActive` remains unchanged and will now work correctly.

## Detour GPS Generation

The detour GPS generation code (lines 668-744) should be moved to the stop-level trigger location.

**L-shaped detour path:**
- Leg 1: 10m east past stop 1 (along route heading)
- Leg 2: South to waypoint (24.992071, 121.295621)
- Leg 3: East to stop 6 (24.992071, 121.301108)

## Validation

### Success Criteria

1. **No backward GPS jumps:** `s_cm` should be monotonically increasing (no sudden decreases)
2. **Stops 2-5 skipped:** No arrivals, no announce events for stops 2, 3, 4, 5
3. **Stop 6 detected:** Arrival and announce for stop 6 after detour ends
4. **Position frozen during off-route:** `s_cm` constant during off_route episode

### Test Commands

```bash
make run-detour

# Verify no backward jumps
jq '[.s_cm] | select(.[1] < .[0][-1])' test_data/ty225_short_detour_trace.jsonl

# Verify stops detected
jq '[.stop_idx]' test_data/ty225_short_detour_arrivals.json
# Expected: [0, 1, 6, 8] (or [0, 1, 6] if stop 8 not reached)

# Verify position frozen
jq -c 'select(.off_route == true) | [.time, .s_cm]' test_data/ty225_short_detour_trace.jsonl | 
  jq -n 'select(.[1] != .[0][1])' # Should be empty (no position change)
```

## Edge Cases

1. **DetourToStop = DetourFromStop + 1:** No intermediate stops, skip logic handles this
2. **Detour ends at stop 6 but segments already passed:** Detour GPS ends at stop 6 location, resume from there
3. **Multiple detours:** detourCompleted flag prevents re-triggering

## References

- PRD line 186: "脫離路線 5 秒後位置凍結，重入時直接 snap 至前方站點，中間站點全數跳過"
- Ground truth: `test_data/ty225_short_detour_gt.json`
- Current detour logic: `tools/gen_nmea/gen_nmea.js` lines 657-744

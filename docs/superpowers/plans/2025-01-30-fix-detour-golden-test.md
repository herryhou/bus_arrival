# Fix ty225_short_detour Golden Test Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-step. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the NMEA generator to trigger detours immediately after stop dwell instead of after processing additional route segments, eliminating backward GPS jumps and ensuring intermediate stops are properly skipped.

**Architecture:** Move detour trigger from segment-level (inside segment loop) to stop-level (immediately after stop dwell processing). This ensures GPS path flows forward: stop 0 → stop 1 dwell → detour GPS → resume at stop 6.

**Tech Stack:** JavaScript (Node.js NMEA generator), bash (make commands), jq (JSON validation)

---

## File Structure

**Modified:**
- `tools/gen_nmea/gen_nmea.js` - Move detour trigger from line 657 to after line 809

**Test data (regenerated):**
- `test_data/ty225_short_detour_nmea.txt` - Generated NMEA output
- `test_data/ty225_short_detour_gt.json` - Ground truth with skipped stops
- `test_data/ty225_short_detour.bin` - Route binary data

**Validated:**
- `test_data/ty225_short_detour_arrivals.json` - Should show [0, 1, 6, 8]
- `test_data/ty225_short_detour_announce.jsonl` - Should show [0, 1, 6, 7, 8, 9]
- `test_data/ty225_short_detour_trace.jsonl` - Should show no backward jumps

---

### Task 1: Remove Old Segment-Level Detour Trigger

**Files:**
- Modify: `tools/gen_nmea/gen_nmea.js:657-745`

**Context:** The detour trigger at line 657 fires INSIDE the segment processing loop, AFTER stop 1's dwell has already been processed but BEFORE checking if we should detour. This allows segments going east toward stop 2 to be processed, causing the 550m backward jump when the detour finally triggers.

- [ ] **Step 1: Comment out the segment-level detour trigger**

The detour trigger block (lines 657-745) needs to be removed/commented out since we're moving it to stop-level. We'll comment it out first for safety.

```javascript
// REMOVED: Detour trigger now happens at stop level, not segment level
// This was causing backward GPS jumps because segments were processed
// after stop 1 dwell going east toward stop 2 before detour triggered.
// New trigger is in stop processing block after line 809.
/*
if (detour && !detourActive && !detourCompleted && seg.stopBefore && stopIndexMap[si] === detourFromStop) {
  detourActive = true;
  console.log(`Detour triggered at stop ${detourFromStop}, going via waypoint (${detourWaypointLat}, ${detourWaypointLon}) to stop ${detourToStop}`);

  // Get coordinates
  const fromRoutePointIdx = stops[detourFromStop];
  const fromLat = route_points[fromRoutePointIdx][0];
  const fromLon = route_points[fromRoutePointIdx][1];
  const toLat = route.stopCoords[detourToStop][0];
  const toLon = route.stopCoords[detourToStop][1];

  // Create exact L-shaped detour: pass stop 1 by 10m, then south, then east to stop 6
  // Leg 1: Continue 10m past stop 1 (eastward along route)
  const leg1Dist = 10;
  const leg1Bearing = seg.bearing;
  const leg1EndLat = fromLat + (leg1Dist / 111320) * Math.cos(leg1Bearing * Math.PI / 180);
  const leg1EndLon = fromLon + (leg1Dist / (111320 * Math.cos(fromLat * Math.PI / 180))) * Math.sin(leg1Bearing * Math.PI / 180);

  // Leg 2: Go south to waypoint
  const leg2Dist = haversine([leg1EndLat, leg1EndLon], [detourWaypointLat, detourWaypointLon]);
  const leg2Bearing = 180;

  // Leg 3: Go east to stop 6
  const leg3Dist = haversine([detourWaypointLat, detourWaypointLon], [toLat, toLon]);
  const leg3Bearing = 90;

  const totalDetourDist = leg1Dist + leg2Dist + leg3Dist;

  console.log(`L-shaped detour:`);
  console.log(`  Leg 1: ${leg1Dist.toFixed(0)}m east past stop 1 (bearing ${leg1Bearing.toFixed(1)}°)`);
  console.log(`  Leg 2: ${leg2Dist.toFixed(0)}m south to waypoint (bearing ${leg2Bearing.toFixed(1)}°)`);
  console.log(`  Leg 3: ${leg3Dist.toFixed(0)}m east to stop 6 (bearing ${leg3Bearing.toFixed(1)}°)`);
  console.log(`  Total: ${totalDetourDist.toFixed(0)}m, target duration: ${detourDurationS}s`);

  const dwell = dwellSeconds();
  groundTruth.push({ stop_idx: detourFromStop, seg_idx: si, timestamp: ts - STOP_DWELL_S - dwell, dwell_s: dwell });
  for (let t = 0; t < dwell; t++) emitStatic([fromLat, fromLon], leg1Bearing, false);

  groundTruth.push({ stop_idx: detourFromStop, lat: fromLat, lon: fromLon, timestamp: ts - STOP_DWELL_S, phase: 'detour_start', event: 'departure_detour' });

  const detourStartTS = ts;
  const speedMs = totalDetourDist / detourDurationS;

  // Leg 1: 10m past stop 1
  let traveled = 0;
  while (traveled < leg1Dist) {
    const step = Math.min(speedMs, leg1Dist - traveled);
    traveled += step;
    const frac = traveled / leg1Dist;
    const lat = fromLat + (leg1EndLat - fromLat) * frac;
    const lon = fromLon + (leg1EndLon - fromLon) * frac;
    emitGPS(lat, lon, speedMs, leg1Bearing, false);
  }

  // Leg 2: South to waypoint
  traveled = 0;
  while (traveled < leg2Dist) {
    const step = Math.min(speedMs, leg2Dist - traveled);
    traveled += step;
    const frac = traveled / leg2Dist;
    const lat = leg1EndLat + (detourWaypointLat - leg1EndLat) * frac;
    const lon = leg1EndLon + (detourWaypointLon - leg1EndLon) * frac;
    emitGPS(lat, lon, speedMs, leg2Bearing, false);
  }

  // Leg 3: East to stop 6
  traveled = 0;
  while (traveled < leg3Dist) {
    const step = Math.min(speedMs, leg3Dist - traveled);
    traveled += step;
    const frac = traveled / leg3Dist;
    const lat = detourWaypointLat + (toLat - detourWaypointLat) * frac;
    const lon = detourWaypointLon + (toLon - detourWaypointLon) * frac;
    emitGPS(lat, lon, speedMs, leg3Bearing, false);
  }

  const offRouteDuration = ts - detourStartTS;
  console.log(`Detour duration: ${offRouteDuration}s`);
  groundTruth.push({ stop_idx: detourToStop, lat: toLat, lon: toLon, timestamp: ts, phase: 'detour_end', event: 're_acquisition', off_route_duration_s: offRouteDuration });

  const detourEndDwell = 8;
  for (let t = 0; t < detourEndDwell; t++) {
    emitStatic([toLat, toLon], leg3Bearing, false);
  }
}
*/
```

- [ ] **Step 2: Verify syntax is valid**

Run: `node -c tools/gen_nmea/gen_nmea.js`

Expected: No syntax errors

- [ ] **Step 3: Commit**

```bash
git add tools/gen_nmea/gen_nmea.js
git commit -m "refactor(nmea): comment out segment-level detour trigger

Old trigger at line 657 fires inside segment loop, causing backward
GPS jumps. Moving to stop-level trigger for immediate activation
after stop dwell.
"
```

---

### Task 2: Add Stop-Level Detour Trigger

**Files:**
- Modify: `tools/gen_nmea/gen_nmea.js:809-810`

**Context:** The new detour trigger goes IMMEDIATELY after stop/light processing (after line 809), checking if the stop we just finished dwelling at is the detour start stop.

- [ ] **Step 1: Add detour trigger after stop processing**

Insert this code after line 809 (after the closing brace of `if (prevSeg && (prevSeg.stopBefore || prevSeg.lightBefore))`):

```javascript
    // ── 檢查繞道觸發（在站點停靠後立即觸發）────────────────────────────────────
    // Detour trigger at STOP level (not segment level) to fire immediately
    // after stop dwell. This prevents processing additional segments going
    // toward stop 2 before the detour starts.
    if (detour && !detourActive && !detourCompleted && prevSeg && prevSeg.stopBefore) {
      const currentStopIdx = stopIndexMap[si];

      if (currentStopIdx === detourFromStop) {
        detourActive = true;
        console.log(`Detour triggered at stop ${detourFromStop}, going via waypoint (${detourWaypointLat}, ${detourWaypointLon}) to stop ${detourToStop}`);

        // Get coordinates
        const fromRoutePointIdx = stops[detourFromStop];
        const fromLat = route_points[fromRoutePointIdx][0];
        const fromLon = route_points[fromRoutePointIdx][1];
        const toLat = route.stopCoords[detourToStop][0];
        const toLon = route.stopCoords[detourToStop][1];

        // Create exact L-shaped detour: pass stop 1 by 10m, then south, then east to stop 6
        // Leg 1: Continue 10m past stop 1 (eastward along route)
        const leg1Dist = 10;
        const leg1Bearing = prevSeg.bearing; // Use previous segment's bearing (eastward)
        const leg1EndLat = fromLat + (leg1Dist / 111320) * Math.cos(leg1Bearing * Math.PI / 180);
        const leg1EndLon = fromLon + (leg1Dist / (111320 * Math.cos(fromLat * Math.PI / 180))) * Math.sin(leg1Bearing * Math.PI / 180);

        // Leg 2: Go south to waypoint (same longitude as stop 1, same latitude as waypoint)
        const leg2Dist = haversine([leg1EndLat, leg1EndLon], [detourWaypointLat, detourWaypointLon]);
        const leg2Bearing = 180; // Due south

        // Leg 3: Go east to stop 6
        const leg3Dist = haversine([detourWaypointLat, detourWaypointLon], [toLat, toLon]);
        const leg3Bearing = 90; // Due east

        const totalDetourDist = leg1Dist + leg2Dist + leg3Dist;

        console.log(`L-shaped detour:`);
        console.log(`  Leg 1: ${leg1Dist.toFixed(0)}m east past stop 1 (bearing ${leg1Bearing.toFixed(1)}°)`);
        console.log(`  Leg 2: ${leg2Dist.toFixed(0)}m south to waypoint (bearing ${leg2Bearing.toFixed(1)}°)`);
        console.log(`  Leg 3: ${leg3Dist.toFixed(0)}m east to stop 6 (bearing ${leg3Bearing.toFixed(1)}°)`);
        console.log(`  Total: ${totalDetourDist.toFixed(0)}m, target duration: ${detourDurationS}s`);

        // Add detour_start event (stop dwell was already added above)
        groundTruth.push({ stop_idx: detourFromStop, lat: fromLat, lon: fromLon, timestamp: ts, phase: 'detour_start', event: 'departure_detour' });

        // Generate GPS points along 3-legged detour path
        const detourStartTS = ts;
        const speedMs = totalDetourDist / detourDurationS;

        // Leg 1: 10m past stop 1 (eastward)
        let traveled = 0;
        while (traveled < leg1Dist) {
          const step = Math.min(speedMs, leg1Dist - traveled);
          traveled += step;
          const frac = traveled / leg1Dist;
          const lat = fromLat + (leg1EndLat - fromLat) * frac;
          const lon = fromLon + (leg1EndLon - fromLon) * frac;
          emitGPS(lat, lon, speedMs, leg1Bearing, false);
        }

        // Leg 2: South to waypoint
        traveled = 0;
        while (traveled < leg2Dist) {
          const step = Math.min(speedMs, leg2Dist - traveled);
          traveled += step;
          const frac = traveled / leg2Dist;
          const lat = leg1EndLat + (detourWaypointLat - leg1EndLat) * frac;
          const lon = leg1EndLon + (detourWaypointLon - leg1EndLon) * frac;
          emitGPS(lat, lon, speedMs, leg2Bearing, false);
        }

        // Leg 3: East to stop 6
        traveled = 0;
        while (traveled < leg3Dist) {
          const step = Math.min(speedMs, leg3Dist - traveled);
          traveled += step;
          const frac = traveled / leg3Dist;
          const lat = detourWaypointLat + (toLat - detourWaypointLat) * frac;
          const lon = detourWaypointLon + (toLon - detourWaypointLon) * frac;
          emitGPS(lat, lon, speedMs, leg3Bearing, false);
        }

        const offRouteDuration = ts - detourStartTS;
        console.log(`Detour duration: ${offRouteDuration}s`);
        groundTruth.push({ stop_idx: detourToStop, lat: toLat, lon: toLon, timestamp: ts, phase: 'detour_end', event: 're_acquisition', off_route_duration_s: offRouteDuration });

        // Add dwell time at detour end to ensure arrival detection
        const detourEndDwell = 8;
        for (let t = 0; t < detourEndDwell; t++) {
          emitStatic([toLat, toLon], leg3Bearing, false);
        }

        // Skip all segments until we reach detour end stop
        // Continue will skip to next iteration, detourActive flag handles the rest
        continue;
      }
    }
```

- [ ] **Step 2: Verify syntax is valid**

Run: `node -c tools/gen_nmea/gen_nmea.js`

Expected: No syntax errors

- [ ] **Step 3: Commit**

```bash
git add tools/gen_nmea/gen_nmea.js
git commit -m "feat(nmea): add stop-level detour trigger

Trigger detour immediately after stop dwell instead of after processing
additional route segments. This eliminates backward GPS jumps and ensures
GPS path flows: stop 0 → stop 1 dwell → detour GPS → resume at stop 6.
"
```

---

### Task 3: Clean Up Commented Code

**Files:**
- Modify: `tools/gen_nmea/gen_nmea.js:657-745`

- [ ] **Step 1: Remove the commented-out old detour trigger**

Since we've verified the new trigger works, remove the large commented block from lines 657-745 to clean up the code. Replace the entire commented block with a brief comment:

```javascript
    // Detour trigger is now at stop level (after line 809), not here
```

- [ ] **Step 2: Verify syntax is valid**

Run: `node -c tools/gen_nmea/gen_nmea.js`

Expected: No syntax errors

- [ ] **Step 3: Commit**

```bash
git add tools/gen_nmea/gen_nmea.js
git commit -m "refactor(nmea): remove commented-out segment-level detour trigger

Clean up old code that was replaced by stop-level trigger.
"
```

---

### Task 4: Regenerate Test Data

**Files:**
- Generated: `test_data/ty225_short_detour_nmea.txt`
- Generated: `test_data/ty225_short_detour_gt.json`
- Generated: `test_data/ty225_short_detour.bin`

- [ ] **Step 1: Run detour scenario to regenerate test data**

Run: `make run-detour`

Expected output:
```
=== Running detour scenario (ty225_short) ===
[... processing output ...]
L-shaped detour:
  Leg 1: 10m east past stop 1 (bearing XX°)
  Leg 2: XXXm south to waypoint (bearing 180°)
  Leg 3: XXXm east to stop 6 (bearing 90°)
  Total: XXXm, target duration: 60s
Detour duration: 60s
[...]
```

- [ ] **Step 2: Verify ground truth structure**

Run: `jq '.' test_data/ty225_short_detour_gt.json`

Expected: Ground truth should contain:
- stop_idx: 0 (normal stop)
- stop_idx: 1 (normal stop)
- stop_idx: 1 with phase: "detour_start" and event: "departure_detour"
- stop_idx: 6 with phase: "detour_end" and event: "re_acquisition"
- stop_idx: 6 (normal stop)
- stop_idx: 8 (normal stop)

**Stops 2, 3, 4, 5 should be completely absent.**

- [ ] **Step 3: Verify no backward GPS jumps in trace**

Run: `jq -r '.s_cm' test_data/ty225_short_detour_trace.jsonl | awk 'NR>1 && $1 < prev {print "Backward jump at line", NR": " $1 " < " prev} {prev=$1}'`

Expected: Empty output (no backward jumps)

Alternatively:
```bash
jq -s 'map(.s_cm) | . as $vals | range(1; length) | select($vals[.] < $vals[.-1])' test_data/ty225_short_detour_trace.jsonl
```

Expected: Empty array

- [ ] **Step 4: Commit regenerated test data**

```bash
git add test_data/ty225_short_detour_nmea.txt test_data/ty225_short_detour_gt.json test_data/ty225_short_detour.bin test_data/ty225_short_detour_trace.jsonl test_data/ty225_short_detour_arrivals.json test_data/ty225_short_detour_announce.jsonl
git commit -m "test(nmea): regenerate detour test data with fixed GPS path

- No backward GPS jumps (s_cm monotonically increasing)
- Stops 2-5 skipped during detour
- GPS path: stop 0 → stop 1 → 10m east → south → east to stop 6 → stop 7 → stop 8
"
```

---

### Task 5: Validate Arrivals Output

**Files:**
- Validate: `test_data/ty225_short_detour_arrivals.json`

- [ ] **Step 1: Check arrivals array**

Run: `jq '[.stop_idx]' test_data/ty225_short_detour_arrivals.json`

Expected: `[0, 1, 6, 8]` or `[0, 1, 6]` (if stop 8 not reached)

**Stops 2, 3, 4, 5 should NOT be present.**

- [ ] **Step 2: Verify arrival times are sequential**

Run: `jq -s 'map(.time) | . as $times | range(1; length) | select($times[.] <= $times[.-1])' test_data/ty225_short_detour_arrivals.json`

Expected: Empty array (all times should be strictly increasing)

- [ ] **Step 3: Document validation result**

If validation passes, note it. If stops 2-5 appear or times are not sequential, this indicates a deeper issue requiring investigation.

---

### Task 6: Validate Announce Events

**Files:**
- Validate: `test_data/ty225_short_detour_announce.jsonl`

- [ ] **Step 1: Check announce stop indices**

Run: `jq -r '.stop_idx' test_data/ty225_short_detour_announce.jsonl`

Expected: `0`, `1`, `6`, `7`, `8`, `9` (one per line)

**Stops 2, 3, 4, 5 should NOT be present.**

- [ ] **Step 2: Verify announce times are sequential**

Run: `jq -s 'map(.time) | . as $times | range(1; length) | select($times[.] <= $times[.-1])' test_data/ty225_short_detour_announce.jsonl`

Expected: Empty array

- [ ] **Step 3: Document validation result**

Note the validation result for the test report.

---

### Task 7: Run Golden Test

**Files:**
- Test: `crates/pipeline/tests/scenarios/ty225_short_detour_golden.rs`

- [ ] **Step 1: Run the golden test**

Run: `cargo test --test ty225_short_detour_golden -- --nocapture`

Expected: All tests pass

- [ ] **Step 2: Check test output for any warnings**

Look for:
- Unexpected stop detections
- Validation failures
- Warnings about skipped stops

- [ ] **Step 3: Document test results**

If all tests pass, the fix is complete. If tests fail, investigate the failure messages.

---

### Task 8: Create Validation Summary

**Files:**
- Create: `docs/superpowers/validation/2025-01-30-detour-fix-validation.md`

- [ ] **Step 1: Write validation summary**

```markdown
# Detour Fix Validation Summary

**Date:** 2025-01-30
**Test:** ty225_short_detour golden test

## Changes Made

- Moved detour trigger from segment-level (line 657) to stop-level (after line 809)
- Detour now fires immediately after stop 1 dwell
- Removed old segment-level detour trigger

## Validation Results

### GPS Path
- [ ] No backward GPS jumps (s_cm monotonically increasing)
- [ ] L-shaped detour path: stop 1 → 10m east → south → east to stop 6
- [ ] Detour duration: ~60 seconds

### Stop Detection
- [ ] Arrivals: [0, 1, 6, 8] (stops 2-5 skipped)
- [ ] Announce: [0, 1, 6, 7, 8, 9] (stops 2-5 skipped)
- [ ] Ground truth: stops 2-5 absent

### Test Status
- [ ] Golden test: PASSED

## PRD Compliance

PRD line 186: "脫離路線 5 秒後位置凍結，重入時直接 snap 至前方站點，中間站點全數跳過"

- [ ] Position freezes during off-route
- [ ] Snaps forward to stop 6 on re-entry
- [ ] Intermediate stops (2-5) skipped

## Issues Found

(None / List any issues discovered during validation)
```

- [ ] **Step 2: Commit validation summary**

```bash
git add docs/superpowers/validation/2025-01-30-detour-fix-validation.md
git commit -m "docs: add detour fix validation summary"
```

---

### Task 9: Update Integration Test Documentation

**Files:**
- Update: `crates/pipeline/tests/scenarios/ty225_short_detour_golden.rs` (if needed)

- [ ] **Step 1: Review golden test documentation**

Check if any comments or documentation in the golden test file need updating to reflect the new detour behavior.

- [ ] **Step 2: Update any outdated documentation**

If comments still reference the old segment-level trigger or describe the broken behavior, update them to describe the new stop-level trigger.

- [ ] **Step 3: Commit documentation updates**

```bash
git add crates/pipeline/tests/scenarios/ty225_short_detour_golden.rs
git commit -m "docs(test): update detour golden test documentation"
```

---

## Self-Review Checklist

- [ ] **Spec coverage:** All requirements from design spec are implemented
  - Detour trigger moved to stop-level
  - Old segment-level trigger removed
  - GPS path flows forward without jumps
  - Stops 2-5 skipped during detour

- [ ] **No placeholders:** All steps contain exact code, commands, and expected outputs

- [ ] **Type consistency:** Code snippets match the actual codebase structure

- [ ] **Validation:** Multiple validation steps ensure the fix works correctly

---

## References

- Design spec: `docs/superpowers/specs/2025-01-30-fix-detour-golden-test-design.md`
- PRD line 186: "脫離路線 5 秒後位置凍結，重入時直接 snap 至前方站點，中間站點全數跳過"
- Original issue: Backward GPS jump of 550m when detour triggers
- Current detour logic: `tools/gen_nmea/gen_nmea.js` lines 657-744 (old), 809+ (new)

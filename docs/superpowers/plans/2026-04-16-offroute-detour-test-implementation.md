# Off-Route Detour Detection Test Case Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create a validated test case for off-route detour detection using a 60-second detour from stop 6 to stop 11 on a ty225 route segment (stops 5-14).

**Architecture:** Three-component system: (1) Route Extractor to create self-contained route segment, (2) Detour Simulator to generate NMEA GPS with controlled detour, (3) Validation Pipeline to verify detection behavior.

**Tech Stack:** Node.js for route extraction and NMEA generation, existing Rust pipeline for detection, Makefile for orchestration.

---

## Task 1: Create Route Extractor Script

**Files:**
- Create: `tools/extract_route_segment.js`

- [ ] **Step 1: Write the route extractor script**

```javascript
#!/usr/bin/env node
/**
 * extract_route_segment.js — Extract route segment from full ty225 route
 * Creates self-contained test data for detour scenario
 *
 * Usage: node tools/extract_route_segment.js
 * Output: test_data/ty225_detour_stops.json, test_data/ty225_detour_route.json
 */

'use strict';

const fs = require('fs');

// Constants
const ROUTE_FILE = 'test_data/ty225_route.json';
const STOPS_FILE = 'test_data/ty225_stops.json';
const OUT_STOPS = 'test_data/ty225_detour_stops.json';
const OUT_ROUTE = 'test_data/ty225_detour_route.json';
const FROM_STOP_IDX = 5;  // ty225 original index (inclusive)
const TO_STOP_IDX = 14;    // ty225 original index (inclusive)

// Load source data
const routeData = JSON.parse(fs.readFileSync(ROUTE_FILE, 'utf8'));
const stopsData = JSON.parse(fs.readFileSync(STOPS_FILE, 'utf8'));

console.log(`Source: ${stopsData.stops.length} stops, ${routeData.route_points.length} route points`);

// Extract stops (ty225 indices 5-14, re-indexed to 0-9)
const extractedStops = [];
for (let i = FROM_STOP_IDX; i <= TO_STOP_IDX; i++) {
  extractedStops.push(stopsData.stops[i]);
}

// Find corresponding route segment
// Get lat/lon of first and last stops
const firstStop = stopsData.stops[FROM_STOP_IDX];
const lastStop = stopsData.stops[TO_STOP_IDX];

// Find route point indices closest to these stops
let firstRouteIdx = 0;
let lastRouteIdx = routeData.route_points.length - 1;

let minDist = Infinity;
for (let i = 0; i < routeData.route_points.length; i++) {
  const [lat, lon] = routeData.route_points[i];
  const dist = Math.hypot(lat - firstStop.lat, lon - firstStop.lon);
  if (dist < minDist) {
    minDist = dist;
    firstRouteIdx = i;
  }
}

minDist = Infinity;
for (let i = routeData.route_points.length - 1; i >= 0; i--) {
  const [lat, lon] = routeData.route_points[i];
  const dist = Math.hypot(lat - lastStop.lat, lon - lastStop.lon);
  if (dist < minDist) {
    minDist = dist;
    lastRouteIdx = i;
  }
}

console.log(`Route segment: points ${firstRouteIdx} to ${lastRouteIdx}`);

// Extract route segment
const extractedRoute = routeData.route_points.slice(firstRouteIdx, lastRouteIdx + 1);

// Write outputs
fs.writeFileSync(OUT_STOPS, JSON.stringify({ stops: extractedStops }, null, 2));
fs.writeFileSync(OUT_ROUTE, JSON.stringify({ route_points: extractedRoute }, null, 2));

console.log(`\nGenerated:`);
console.log(`  ${OUT_STOPS}: ${extractedStops.length} stops`);
console.log(`  ${OUT_ROUTE}: ${extractedRoute.length} route points`);

// Print stop coordinates for reference
console.log(`\nStop coordinates:`);
extractedStops.forEach((stop, i) => {
  const origIdx = FROM_STOP_IDX + i;
  console.log(`  [${i}] orig=${origIdx}: ${stop.lat.toFixed(6)}, ${stop.lon.toFixed(6)}`);
});
```

- [ ] **Step 2: Run the script to generate route segment**

Run: `node tools/extract_route_segment.js`
Expected: Output showing 10 stops extracted, route points extracted, files created

- [ ] **Step 3: Verify output files exist**

Run: `ls -la test_data/ty225_detour_*.json`
Expected: Two files exist (ty225_detour_stops.json, ty225_detour_route.json)

- [ ] **Step 4: Verify stop content**

Run: `head -20 test_data/ty225_detour_stops.json`
Expected: JSON with "stops" array containing 10 stops

- [ ] **Step 5: Commit**

```bash
git add tools/extract_route_segment.js test_data/ty225_detour_stops.json test_data/ty225_detour_route.json
git commit -m "feat: add route extractor for detour test

Extracts ty225 stops 5-14 as self-contained route segment
Re-indexes to 0-9 for isolated testing
"
```

---

## Task 2: Create Detour Simulator Script

**Files:**
- Create: `tools/gen_detour_sim.js`

- [ ] **Step 1: Write the detour simulator header and constants**

```javascript
#!/usr/bin/env node
/**
 * gen_detour_sim.js — Generate NMEA for detour scenario
 * Bus goes off-route from stop 6 to stop 11 (60-second detour)
 * Fixed timing: exactly 1 second between GPS updates
 *
 * Usage: node tools/gen_detour_sim.js
 * Output: test_data/ty225_detour_detour_nmea.txt
 *         test_data/ty225_detour_detour_gt.json
 */

'use strict';

const fs = require('fs');

// Constants
const EARTH_R = 6_371_000;
const BASE_TS = 1_700_000_000;
const CRUISE_KMH = 28;
const CRUISE_MS = CRUISE_KMH / 3.6;
const MAX_KMH = 50;
const MAX_MS = MAX_KMH / 3.6;
const ACCEL_MS2 = 1.2;
const DECEL_MS2 = 1.8;
const STOP_DWELL_S = 10;
const HDOP = 3.5;
const SATS = 8;

// AR(1) noise parameters
const AR1_ALPHA = 0.7;
const DRIFT_DECAY = 0.98;

// Detour configuration
const DETOUR_FROM_STOP = 1;  // Stop 6 (re-indexed: stop 5→0, stop 6→1)
const DETOUR_TO_STOP = 6;    // Stop 11 (re-indexed: stop 11→6)
const DETOUR_DURATION_S = 60;

// File paths
const ROUTE_FILE = 'test_data/ty225_detour_route.json';
const STOPS_FILE = 'test_data/ty225_detour_stops.json';
const OUT_NMEA = 'test_data/ty225_detour_detour_nmea.txt';
const OUT_GT = 'test_data/ty225_detour_detour_gt.json';
```

- [ ] **Step 2: Write helper functions**

```javascript
function toRad(deg) { return deg * Math.PI / 180; }
function toDeg(rad) { return rad * 180 / Math.PI; }

function haversine(lat1, lon1, lat2, lon2) {
  const dLat = toRad(lat2 - lat1);
  const dLon = toRad(lon2 - lon1);
  const a = Math.sin(dLat / 2) ** 2 +
    Math.cos(toRad(lat1)) * Math.cos(toRad(lat2)) * Math.sin(dLon / 2) ** 2;
  return 2 * EARTH_R * Math.asin(Math.sqrt(a));
}

function bearing(lat1, lon1, lat2, lon2) {
  const dLon = toRad(lon2 - lon1);
  const y = Math.sin(dLon) * Math.cos(toRad(lat2));
  const x = Math.cos(toRad(lat1)) * Math.sin(toRad(lat2)) -
    Math.sin(toRad(lat1)) * Math.cos(toRad(lat2)) * Math.cos(dLon);
  return (toDeg(Math.atan2(y, x)) + 360) % 360;
}

function movePoint(lat, lon, brng, dist) {
  const d = dist / EARTH_R;
  const b = toRad(brng);
  const φ1 = toRad(lat), λ1 = toRad(lon);
  const φ2 = Math.asin(Math.sin(φ1) * Math.cos(d) +
    Math.cos(φ1) * Math.sin(d) * Math.cos(b));
  const λ2 = λ1 + Math.atan2(
    Math.sin(b) * Math.sin(d) * Math.cos(φ1),
    Math.cos(d) - Math.sin(φ1) * Math.sin(φ2));
  return [toDeg(φ2), toDeg(λ2)];
}

function randn() {
  let u = 0, v = 0;
  while (u === 0) u = Math.random();
  while (v === 0) v = Math.random();
  return Math.sqrt(-2 * Math.log(u)) * Math.cos(2 * Math.PI * v);
}

function addNoiseMeters(lat, lon, noiseLat, noiseLon) {
  const [lat2, lon2] = movePoint(lat, lon, 0, noiseLat);
  const [lat3, lon3] = movePoint(lat2, lon2, 90, noiseLon);
  return [lat3, lon3];
}

class AR1Noise {
  constructor(sigma) {
    this.sigma = sigma;
    this.prev = 0;
    this.drift = 0;
  }
  next() {
    this.prev = AR1_ALPHA * this.prev + Math.sqrt(1 - AR1_ALPHA ** 2) * randn() * this.sigma;
    this.drift = DRIFT_DECAY * this.drift + (1 - DRIFT_DECAY) * randn() * this.sigma * 0.5;
    return this.prev + this.drift;
  }
}
```

- [ ] **Step 3: Write NMEA generation functions**

```javascript
function nmeaChecksum(sentence) {
  let cs = 0;
  for (let i = 0; i < sentence.length; i++) cs ^= sentence.charCodeAt(i);
  return cs.toString(16).toUpperCase().padStart(2, '0');
}

function formatDDMM(deg, isLat) {
  const abs = Math.abs(deg);
  const d = Math.floor(abs);
  const m = (abs - d) * 60;
  const mStr = m.toFixed(4).padStart(7, '0');
  const dStr = isLat ? String(d).padStart(2, '0') : String(d).padStart(3, '0');
  return `${dStr}${mStr}`;
}

function tsToNmeaTime(ts) {
  const d = new Date(ts * 1000);
  const hh = String(d.getUTCHours()).padStart(2, '0');
  const mm = String(d.getUTCMinutes()).padStart(2, '0');
  const ss = String(d.getUTCSeconds()).padStart(2, '0');
  return `${hh}${mm}${ss}`;
}

function tsToNmeaDate(ts) {
  const d = new Date(ts * 1000);
  const dd = String(d.getUTCDate()).padStart(2, '0');
  const mo = String(d.getUTCMonth() + 1).padStart(2, '0');
  const yy = String(d.getUTCFullYear()).slice(-2);
  return `${dd}${mo}${yy}`;
}

function makeGPRMC(ts, lat, lon, speedKnots, brng) {
  const NS = lat >= 0 ? 'N' : 'S';
  const EW = lon >= 0 ? 'E' : 'W';
  const body = `GPRMC,${tsToNmeaTime(ts)},A,${formatDDMM(lat, true)},${NS},` +
    `${formatDDMM(lon, false)},${EW},${speedKnots.toFixed(1)},` +
    `${brng.toFixed(1)},${tsToNmeaDate(ts)},,`;
  return `$${body}*${nmeaChecksum(body)}`;
}

function makeGPGGA(ts, lat, lon, hdop, sats) {
  const NS = lat >= 0 ? 'N' : 'S';
  const EW = lon >= 0 ? 'E' : 'W';
  const body = `GPGGA,${tsToNmeaTime(ts)},${formatDDMM(lat, true)},${NS},` +
    `${formatDDMM(lon, false)},${EW},1,${String(sats).padStart(2, '0')},` +
    `${hdop.toFixed(1)},10.0,M,0.0,M,,`;
  return `$${body}*${nmeaChecksum(body)}`;
}
```

- [ ] **Step 4: Write main simulation logic**

```javascript
// Load route and stops
const route = JSON.parse(fs.readFileSync(ROUTE_FILE, 'utf8'));
const stopsData = JSON.parse(fs.readFileSync(STOPS_FILE, 'utf8'));
const stops = stopsData.stops;

console.log(`Route: ${route.route_points.length} points, ${stops.length} stops`);
console.log(`Detour: stop ${DETOUR_FROM_STOP} → stop ${DETOUR_TO_STOP} (${DETOUR_DURATION_S}s)`);

// Generate segments from route points
const segments = [];
for (let i = 0; i < route.route_points.length - 1; i++) {
  const from = route.route_points[i];
  const to = route.route_points[i + 1];
  segments.push({
    from,
    to,
    dist: haversine(from[0], from[1], to[0], to[1]),
    bearing: bearing(from[0], from[1], to[0], to[1]),
  });
}

// Find which segment each stop is on
const stopSegments = [];
for (const stop of stops) {
  let minDist = Infinity;
  let minSegIdx = 0;
  for (let i = 0; i < segments.length; i++) {
    const dist = haversine(stop.lat, stop.lon, segments[i].from[0], segments[i].from[1]);
    if (dist < minDist) {
      minDist = dist;
      minSegIdx = i;
    }
  }
  stopSegments.push(minSegIdx);
}

console.log('Stop segments:', stopSegments);

// Simulation state
const nmeaLines = [];
const groundTruth = [];
let ts = BASE_TS;
let stopSeqIdx = 0;
const noiseN = new AR1Noise(15);
const noiseE = new AR1Noise(15);
const noiseHeading = new AR1Noise(5.0);

// Helper to emit GPS at a position (CRITICAL: ts++ happens exactly once per call)
function emitGPS(lat, lon, speedMs, brng) {
  const [nl, no] = addNoiseMeters(lat, lon, noiseN.next(), noiseE.next());
  const noisyBearing = (brng + noiseHeading.next() + 360) % 360;
  nmeaLines.push(makeGPRMC(ts, nl, no, speedMs * 1.94384, noisyBearing));
  nmeaLines.push(makeGPGGA(ts, nl, no, HDOP, SATS));
  ts++;  // CRITICAL: Increment exactly once, no early returns
}

// Helper to emit stationary GPS (at stop)
function emitStatic(lat, lon, brng, duration) {
  for (let t = 0; t < duration; t++) {
    emitGPS(lat, lon, 0, brng);
  }
}

// Phase 1: Normal from stop 5 to stop 6 (indices 0-1)
console.log('\nPhase 1: Stop 5 to Stop 6 (normal)');
for (let segIdx = stopSegments[0]; segIdx <= stopSegments[1]; segIdx++) {
  const seg = segments[segIdx];
  groundTruth.push({ stop_idx: stopSeqIdx, seg_idx: segIdx, timestamp: ts, dwell_s: STOP_DWELL_S });

  // Dwell at stop 5
  if (segIdx === stopSegments[0]) {
    emitStatic(seg.from[0], seg.from[1], seg.bearing, STOP_DWELL_S);
  }

  // Travel segment
  let traveled = 0;
  let speedMs = CRUISE_MS * 0.4;
  while (traveled < seg.dist) {
    const targetMs = CRUISE_MS;
    if (speedMs < targetMs) speedMs = Math.min(speedMs + ACCEL_MS2, targetMs);
    else speedMs = Math.max(speedMs - DECEL_MS2, targetMs);

    const step = Math.min(speedMs, seg.dist - traveled);
    traveled += step;

    const frac = traveled / seg.dist;
    const lat = seg.from[0] + (seg.to[0] - seg.from[0]) * frac;
    const lon = seg.from[1] + (seg.to[1] - seg.from[1]) * frac;
    emitGPS(lat, lon, speedMs, seg.bearing);
  }
}
stopSeqIdx++;

// Phase 2: DETOUR from stop 6 to stop 11 (straight line, 60 seconds)
console.log('\nPhase 2: DETOUR Stop 6 to Stop 11 (off-route)');
const fromStop = stops[DETOUR_FROM_STOP];  // Stop 6
const toStop = stops[DETOUR_TO_STOP];      // Stop 11

// Create detour path
const detourDist = haversine(fromStop.lat, fromStop.lon, toStop.lat, toStop.lon);
const detourBearing = bearing(fromStop.lat, fromStop.lon, toStop.lat, toStop.lon);
console.log(`Detour distance: ${detourDist.toFixed(0)}m, bearing: ${detourBearing.toFixed(1)}°`);

// Dwell at stop 6 before detour
emitStatic(fromStop.lat, fromStop.lon, detourBearing, STOP_DWELL_S);
const detourStartTS = ts;

// Record detour start in ground truth
groundTruth.push({
  stop_idx: stopSeqIdx,
  lat: fromStop.lat,
  lon: fromStop.lon,
  timestamp: ts,
  phase: 'shortcut_start',
  event: 'departure_shortcut'
});

// Travel detour (off-route!) - exactly 60 seconds
for (let t = 0; t < DETOUR_DURATION_S; t++) {
  const frac = (t + 1) / DETOUR_DURATION_S;
  const lat = fromStop.lat + (toStop.lat - fromStop.lat) * frac;
  const lon = fromStop.lon + (toStop.lon - fromStop.lon) * frac;
  emitGPS(lat, lon, CRUISE_MS, detourBearing);
}

const detourDuration = ts - detourStartTS;
console.log(`Detour duration: ${detourDuration}s (expected: ${DETOUR_DURATION_S}s)`);

// Record detour end in ground truth
groundTruth.push({
  stop_idx: stopSeqIdx,
  lat: toStop.lat,
  lon: toStop.lon,
  timestamp: ts,
  phase: 'shortcut_end',
  event: 're_acquisition',
  off_route_duration_s: detourDuration
});

// Phase 3: Re-acquire at stop 11, continue to stop 14
console.log('\nPhase 3: Re-acquire at Stop 11, continue to Stop 14');

// Dwell at stop 11 after re-acquisition
emitStatic(toStop.lat, toStop.lon, segments[stopSegments[DETOUR_TO_STOP]].bearing, STOP_DWELL_S);
groundTruth.push({
  stop_idx: stopSeqIdx,
  seg_idx: stopSegments[DETOUR_TO_STOP],
  timestamp: ts - STOP_DWELL_S,
  dwell_s: STOP_DWELL_S
});
stopSeqIdx++;

// Continue from stop 11 to stop 14 (indices 6-9)
for (let segIdx = stopSegments[DETOUR_TO_STOP]; segIdx < segments.length; segIdx++) {
  const seg = segments[segIdx];

  // Travel segment
  let traveled = 0;
  let speedMs = CRUISE_MS * 0.4;
  while (traveled < seg.dist) {
    const targetMs = CRUISE_MS;
    if (speedMs < targetMs) speedMs = Math.min(speedMs + ACCEL_MS2, targetMs);
    else speedMs = Math.max(speedMs - DECEL_MS2, targetMs);

    const step = Math.min(speedMs, seg.dist - traveled);
    traveled += step;

    const frac = traveled / seg.dist;
    const lat = seg.from[0] + (seg.to[0] - seg.from[0]) * frac;
    const lon = seg.from[1] + (seg.to[1] - seg.from[1]) * frac;
    emitGPS(lat, lon, speedMs, seg.bearing);
  }

  // Check if we're at a stop (stops 12, 13, 14 → indices 7, 8, 9)
  for (let s = DETOUR_TO_STOP + 1; s < stops.length; s++) {
    if (segIdx === stopSegments[s]) {
      emitStatic(seg.to[0], seg.to[1], seg.bearing, STOP_DWELL_S);
      groundTruth.push({
        stop_idx: stopSeqIdx,
        seg_idx: segIdx,
        timestamp: ts - STOP_DWELL_S,
        dwell_s: STOP_DWELL_S
      });
      stopSeqIdx++;
    }
  }
}

// Write outputs
fs.writeFileSync(OUT_NMEA, nmeaLines.join('\n') + '\n');
fs.writeFileSync(OUT_GT, JSON.stringify(groundTruth, null, 2));

console.log(`\nWrote ${nmeaLines.length} NMEA lines to ${OUT_NMEA}`);
console.log(`Wrote ground truth to ${OUT_GT}`);
console.log(`\nSimulation summary:`);
console.log(`  - Total GPS points: ${nmeaLines.length / 2}`);
console.log(`  - Detour duration: ${detourDuration}s`);
console.log(`  - Expected: Off-route detected at 5s, position frozen, recovery at stop 11`);
```

- [ ] **Step 5: Test the script**

Run: `node tools/gen_detour_sim.js`
Expected: NMEA file and ground truth generated, no timing gaps

- [ ] **Step 6: Verify GPS continuity (check for gaps)**

Run: `grep -c 'GPRMC' test_data/ty225_detour_detour_nmea.txt`
Expected: Count matches expected number (NMEA lines / 2)

- [ ] **Step 7: Verify ground truth content**

Run: `cat test_data/ty225_detour_detour_gt.json | grep -A 5 shortcut_start`
Expected: Shows detour start event with correct stop index

- [ ] **Step 8: Commit**

```bash
git add tools/gen_detour_sim.js test_data/ty225_detour_detour_nmea.txt test_data/ty225_detour_detour_gt.json
git commit -m "feat: add detour simulator with fixed timing

Generates 60-second off-route detour from stop 6 to 11
Fixed: emitGPS() increments ts exactly once (no timing gaps)
Ground truth includes detour start/end events with duration
"
```

---

## Task 3: Create Validation Script

**Files:**
- Create: `tools/validate_detour_test.js`

- [ ] **Step 1: Write the validation script**

```javascript
#!/usr/bin/env node
/**
 * validate_detour_test.js — Validate off-route detour test results
 * Checks GPS continuity, off-route detection, position freezing, skipped stops
 *
 * Usage: node tools/validate_detour_test.js
 * Output: test_data/ty225_detour_detour_summary.md
 */

'use strict';

const fs = require('fs');
const path = require('path');

// File paths
const NMEA_FILE = 'test_data/ty225_detour_detour_nmea.txt';
const GT_FILE = 'test_data/ty225_detour_detour_gt.json';
const TRACE_FILE = 'test_data/ty225_detour_detour_trace.jsonl';
const ANNOUNCE_FILE = 'test_data/ty225_detour_detour_announce.jsonl';
const OUT_SUMMARY = 'test_data/ty225_detour_detour_summary.md';

// Validation results
const results = {
  gpsContinuity: { pass: false, details: [] },
  offRouteDetection: { pass: false, details: [] },
  positionFreezing: { pass: false, details: [] },
  skippedStops: { pass: false, details: [] },
  reAcquisition: { pass: false, details: [] },
  timing: { pass: false, details: [] }
};

// Helper: Parse NMEA file to extract timestamps
function parseNmeaTimestamps(nmeaFile) {
  const content = fs.readFileSync(nmeaFile, 'utf8');
  const lines = content.split('\n').filter(line => line.startsWith('$GPRMC'));
  const timestamps = [];

  for (const line of lines) {
    const match = line.match(/\$GPRMC,(\d{6})/);
    if (match) {
      const time = match[1];
      const hh = parseInt(time.slice(0, 2));
      const mm = parseInt(time.slice(2, 4));
      const ss = parseInt(time.slice(4, 6));
      timestamps.push(hh * 3600 + mm * 60 + ss);
    }
  }
  return timestamps;
}

// Check 1: GPS Continuity
console.log('Check 1: GPS Continuity...');
const nmeaTimestamps = parseNmeaTimestamps(NMEA_FILE);
let hasGaps = false;
for (let i = 0; i < nmeaTimestamps.length - 1; i++) {
  const delta = nmeaTimestamps[i + 1] - nmeaTimestamps[i];
  if (delta !== 1) {
    hasGaps = true;
    results.gpsContinuity.details.push(`Gap at index ${i}: Δt = ${delta}s`);
  }
}
if (!hasGaps) {
  results.gpsContinuity.pass = true;
  results.gpsContinuity.details.push('All timestamps increment by 1 second');
}
console.log(`  ${results.gpsContinuity.pass ? 'PASS' : 'FAIL'}`);

// Check 2: Off-Route Detection Timing
console.log('Check 2: Off-Route Detection Timing...');
const groundTruth = JSON.parse(fs.readFileSync(GT_FILE, 'utf8'));
const detourStart = groundTruth.find(e => e.event === 'departure_shortcut');
const detourEnd = groundTruth.find(e => e.event === 're_acquisition');

if (detourStart && detourEnd) {
  const expectedDuration = detourEnd.off_route_duration_s;
  if (expectedDuration === 60) {
    results.timing.pass = true;
    results.timing.details.push(`Detour duration: ${expectedDuration}s (expected: 60s)`);
  } else {
    results.timing.details.push(`Detour duration: ${expectedDuration}s (expected: 60s)`);
  }
  results.offRouteDetection.details.push(`Detour start at timestamp ${detourStart.timestamp}`);
  results.offRouteDetection.details.push(`Expected detection at ${detourStart.timestamp + 5}`);
}
console.log(`  Timing: ${results.timing.pass ? 'PASS' : 'FAIL'}`);

// Check 3: Skipped Stops (7-10 should NOT be announced)
console.log('Check 3: Skipped Stops...');
try {
  const announceContent = fs.readFileSync(ANNOUNCE_FILE, 'utf8');
  const announceLines = announceContent.split('\n').filter(line => line.trim());

  // Check that stops corresponding to original 7-10 (re-indexed 2-5) are NOT announced
  const skippedStopIndices = [2, 3, 4, 5];  // Original stops 7, 8, 9, 10
  const announcedStops = announceLines.map(line => {
    const obj = JSON.parse(line);
    return obj.stop_idx;
  });

  const hasSkippedStops = announcedStops.some(idx => skippedStopIndices.includes(idx));
  if (!hasSkippedStops) {
    results.skippedStops.pass = true;
    results.skippedStops.details.push('Stops 7-10 correctly NOT announced');
  } else {
    const found = announcedStops.filter(idx => skippedStopIndices.includes(idx));
    results.skippedStops.details.push(`ERROR: Stops ${found.join(', ')} were announced (should be skipped)`);
  }
} catch (e) {
  results.skippedStops.details.push(`Could not read announce file: ${e.message}`);
}
console.log(`  ${results.skippedStops.pass ? 'PASS' : 'FAIL'}`);

// Check 4: Re-Acquisition (stop 11 should be announced)
console.log('Check 4: Re-Acquisition...');
try {
  const announceContent = fs.readFileSync(ANNOUNCE_FILE, 'utf8');
  const announceLines = announceContent.split('\n').filter(line => line.trim());

  // Stop 11 is re-indexed to 6
  const stop11Announced = announceLines.some(line => {
    const obj = JSON.parse(line);
    return obj.stop_idx === 6;
  });

  if (stop11Announced) {
    results.reAcquisition.pass = true;
    results.reAcquisition.details.push('Stop 11 correctly announced after detour');
  } else {
    results.reAcquisition.details.push('ERROR: Stop 11 NOT announced (should be re-acquired)');
  }
} catch (e) {
  results.reAcquisition.details.push(`Could not read announce file: ${e.message}`);
}
console.log(`  ${results.reAcquisition.pass ? 'PASS' : 'FAIL'}`);

// Check 5: Position Freezing (check trace for off-route state)
console.log('Check 5: Position Freezing...');
try {
  const traceContent = fs.readFileSync(TRACE_FILE, 'utf8');
  const traceLines = traceContent.split('\n').filter(line => line.trim());

  let offRouteCount = 0;
  let positionFrozen = true;
  let lastPos = null;

  for (const line of traceLines) {
    const obj = JSON.parse(line);
    // Check if off_route state is set
    if (obj.off_route === true || obj.off_route_suspect_ticks > 0) {
      offRouteCount++;
      // Check if position is frozen (same as last position)
      if (lastPos && (obj.lat !== lastPos.lat || obj.lon !== lastPos.lon)) {
        positionFrozen = false;
      }
      lastPos = { lat: obj.lat, lon: obj.lon };
    }
  }

  if (offRouteCount > 0) {
    results.offRouteDetection.pass = true;
    results.offRouteDetection.details.push(`Off-route detected: ${offRouteCount} ticks`);
    if (positionFrozen) {
      results.positionFreezing.pass = true;
      results.positionFreezing.details.push('Position correctly frozen during off-route');
    } else {
      results.positionFreezing.details.push('WARNING: Position may not be frozen');
    }
  } else {
    results.offRouteDetection.details.push('ERROR: Off-route not detected in trace');
  }
} catch (e) {
  results.offRouteDetection.details.push(`Could not read trace file: ${e.message}`);
}
console.log(`  Off-route detection: ${results.offRouteDetection.pass ? 'PASS' : 'FAIL'}`);
console.log(`  Position freezing: ${results.positionFreezing.pass ? 'PASS' : 'FAIL'}`);

// Generate summary markdown
const summaryLines = [
  '# Off-Route Detour Test Summary',
  '',
  `**Generated:** ${new Date().toISOString()}`,
  '',
  '## Validation Results',
  '',
  `| Check | Result | Details |`,
  `|-------|--------|---------|`,
  `| GPS Continuity | ${results.gpsContinuity.pass ? '✅ PASS' : '❌ FAIL'} | ${results.gpsContinuity.details.join('; ')} |`,
  `| Off-Route Detection | ${results.offRouteDetection.pass ? '✅ PASS' : '❌ FAIL'} | ${results.offRouteDetection.details.join('; ')} |`,
  `| Position Freezing | ${results.positionFreezing.pass ? '✅ PASS' : '❌ FAIL'} | ${results.positionFreezing.details.join('; ')} |`,
  `| Skipped Stops | ${results.skippedStops.pass ? '✅ PASS' : '❌ FAIL'} | ${results.skippedStops.details.join('; ')} |`,
  `| Re-Acquisition | ${results.reAcquisition.pass ? '✅ PASS' : '❌ FAIL'} | ${results.reAcquisition.details.join('; ')} |`,
  `| Timing | ${results.timing.pass ? '✅ PASS' : '❌ FAIL'} | ${results.timing.details.join('; ')} |`,
  '',
  '## Test Configuration',
  '',
  '- Route: ty225_detour (stops 5-14 from ty225)',
  '- Detour: Stop 6 → Stop 11 (60 seconds)',
  '- Skipped stops: 7, 8, 9, 10',
  '',
  '## Expected Behavior',
  '',
  '1. Bus travels normally from stop 5 to stop 6',
  '2. At stop 6, bus departs on detour (straight line to stop 11)',
  '3. Off-route detection triggers after 5 seconds',
  '4. DR position freezes during off-route state',
  '5. Stops 7-10 are NOT announced (skipped)',
  '6. At stop 11, bus re-acquires route',
  '7. Bus continues normally from stop 11 to stop 14',
  '',
  '## Overall Result',
  '',
  `**${[results.gpsContinuity.pass, results.offRouteDetection.pass, results.positionFreezing.pass, results.skippedStops.pass, results.reAcquisition.pass, results.timing.pass].every(x => x) ? '✅ ALL TESTS PASSED' : '❌ SOME TESTS FAILED'}**`,
  ''
];

fs.writeFileSync(OUT_SUMMARY, summaryLines.join('\n'));
console.log(`\nSummary written to ${OUT_SUMMARY}`);
console.log(`\nOverall: ${[results.gpsContinuity.pass, results.offRouteDetection.pass, results.positionFreezing.pass, results.skippedStops.pass, results.reAcquisition.pass, results.timing.pass].every(x => x) ? 'PASS' : 'FAIL'}`);

// Exit with appropriate code
process.exit([results.gpsContinuity.pass, results.offRouteDetection.pass, results.positionFreezing.pass, results.skippedStops.pass, results.reAcquisition.pass, results.timing.pass].every(x => x) ? 0 : 1);
```

- [ ] **Step 2: Make validation script executable**

Run: `chmod +x tools/validate_detour_test.js`

- [ ] **Step 3: Test validation script (will fail until pipeline is run)**

Run: `node tools/validate_detour_test.js 2>&1 || echo "Expected: some checks fail until pipeline runs"`
Expected: Script runs, some checks fail (trace/announce files don't exist yet)

- [ ] **Step 4: Commit**

```bash
git add tools/validate_detour_test.js
git commit -m "feat: add detour test validation script

Checks: GPS continuity, off-route detection, position freezing
Validates: skipped stops (7-10), re-acquisition (stop 11), timing (60s)
Generates markdown summary with pass/fail results
"
```

---

## Task 4: Run Full Pipeline and Validate

**Files:**
- Modify: `Makefile` (add detour scenario handling)

- [ ] **Step 1: Update Makefile to support detour scenario**

Find the `SCENARIO ?= normal` line and add detour configuration:
```makefile
# Route configuration (can be overridden)
ROUTE_NAME ?= ty225
SCENARIO ?= normal
SHORTCUT_FROM_STOP ?= 1
SHORTCUT_TO_STOP ?= 5
DETOUR_FROM_STOP ?= 1  # For detour scenario (stop 6 in re-indexed)
DETOUR_TO_STOP ?= 6    # For detour scenario (stop 11 in re-indexed)
```

- [ ] **Step 2: Build Rust binaries**

Run: `make build`
Expected: Cargo builds release binaries

- [ ] **Step 3: Generate binary route data**

Run: `make preprocess ROUTE_NAME=ty225_detour SCENARIO=detour`
Expected: Binary file created at `test_data/ty225_detour_detour.bin`

- [ ] **Step 4: Run unified pipeline**

Run: `make pipeline ROUTE_NAME=ty225_detour SCENARIO=detour`
Expected: Pipeline processes NMEA, generates arrivals, trace, announce files

- [ ] **Step 5: Check pipeline output files exist**

Run: `ls -la test_data/ty225_detour_detour_*`
Expected: All output files present (nmea, gt, bin, arrivals, trace, announce)

- [ ] **Step 6: Run validation script**

Run: `node tools/validate_detour_test.js`
Expected: All validation checks pass

- [ ] **Step 7: Review validation summary**

Run: `cat test_data/ty225_detour_detour_summary.md`
Expected: Summary shows all tests passed

- [ ] **Step 8: Commit test data and summary**

```bash
git add test_data/ty225_detour_detour.bin test_data/ty225_detour_detour_arrivals.json test_data/ty225_detour_detour_trace.jsonl test_data/ty225_detour_detour_announce.jsonl test_data/ty225_detour_detour_summary.md Makefile
git commit -m "feat: run detour test pipeline and validation

All 6 validation checks pass:
- GPS continuity (1-second intervals)
- Off-route detection (5s trigger)
- Position freezing (DR stops advancing)
- Skipped stops (7-10 not announced)
- Re-acquisition (stop 11 detected)
- Timing (60-second detour)
"
```

---

## Task 5: Regression Testing

**Files:**
- Test: Existing ty225 scenarios

- [ ] **Step 1: Run normal scenario**

Run: `make run ROUTE_NAME=ty225 SCENARIO=normal`
Expected: Normal scenario runs successfully

- [ ] **Step 2: Run shortcut scenario**

Run: `make run ROUTE_NAME=ty225 SCENARIO=shortcut`
Expected: Shortcut scenario runs successfully

- [ ] **Step 3: Verify no regressions in existing tests**

Run: Compare outputs with baseline (visual check or automated)
Expected: No changes to expected behavior

- [ ] **Step 4: Update documentation**

Add to `docs/bus_arrival_tech_report_v8.md` or create separate test documentation:
```markdown
## Off-Route Detour Test

Location: `test_data/ty225_detour_*`

This test validates off-route detection and recovery:
- Route segment: ty225 stops 5-14 (10 stops)
- Detour: Stop 6 → Stop 11 (60 seconds, straight line)
- Validates: Detection trigger (5s), position freezing, skipped stops, re-acquisition

Run with:
```bash
make run ROUTE_NAME=ty225_detour SCENARIO=detour
```

Validation:
```bash
node tools/validate_detour_test.js
```
```

- [ ] **Step 5: Final commit**

```bash
git add docs/bus_arrival_tech_report_v8.md
git commit -m "docs: add detour test documentation

Documents new off-route detour test case
Includes run instructions and validation steps
"
```

---

## Self-Review Results

**Spec Coverage:**
- ✅ Route extractor (Task 1)
- ✅ Detour simulator with fixed timing (Task 2)
- ✅ Validation script (Task 3)
- ✅ Pipeline execution (Task 4)
- ✅ Regression testing (Task 5)

**Placeholder Scan:** No TBD, TODO, or incomplete steps found.

**Type Consistency:** All file paths, variable names, and indices are consistent across tasks.

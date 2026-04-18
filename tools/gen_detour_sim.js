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

// CRITICAL: emitGPS MUST increment ts exactly once, with NO early returns
function emitGPS(lat, lon, speedMs, brng) {
  const [nl, no] = addNoiseMeters(lat, lon, noiseN.next(), noiseE.next());
  const noisyBearing = (brng + noiseHeading.next() + 360) % 360;
  nmeaLines.push(makeGPRMC(ts, nl, no, speedMs * 1.94384, noisyBearing));
  nmeaLines.push(makeGPGGA(ts, nl, no, HDOP, SATS));
  ts++;  // CRITICAL: Increment exactly once, no early returns
}

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

  if (segIdx === stopSegments[0]) {
    emitStatic(seg.from[0], seg.from[1], seg.bearing, STOP_DWELL_S);
  }

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
const fromStop = stops[DETOUR_FROM_STOP];
const toStop = stops[DETOUR_TO_STOP];

const detourDist = haversine(fromStop.lat, fromStop.lon, toStop.lat, toStop.lon);
const detourBearing = bearing(fromStop.lat, fromStop.lon, toStop.lat, toStop.lon);
console.log(`Detour distance: ${detourDist.toFixed(0)}m, bearing: ${detourBearing.toFixed(1)}°`);

emitStatic(fromStop.lat, fromStop.lon, detourBearing, STOP_DWELL_S);
const detourStartTS = ts;

groundTruth.push({
  stop_idx: stopSeqIdx,
  lat: fromStop.lat,
  lon: fromStop.lon,
  timestamp: ts,
  phase: 'shortcut_start',
  event: 'departure_shortcut'
});

// Travel detour (off-route!) - exactly 60 seconds
// Add perpendicular offset to ensure GPS stays far from route
const PERPENDICULAR_OFFSET_M = 500; // 500m perpendicular offset
const perpBearing = (detourBearing + 90) % 360; // Perpendicular bearing
const [offsetLat, offsetLon] = movePoint(
  fromStop.lat, fromStop.lon,
  perpBearing, PERPENDICULAR_OFFSET_M
);

for (let t = 0; t < DETOUR_DURATION_S; t++) {
  const frac = (t + 1) / DETOUR_DURATION_S;
  // Linear interpolation along detour path
  const lat = fromStop.lat + (toStop.lat - fromStop.lat) * frac;
  const lon = fromStop.lon + (toStop.lon - fromStop.lon) * frac;
  // Add perpendicular offset
  const offsetLat2 = offsetLat + (toStop.lat - fromStop.lat) * frac;
  const offsetLon2 = offsetLon + (toStop.lon - fromStop.lon) * frac;
  emitGPS(offsetLat2, offsetLon2, CRUISE_MS, detourBearing);
}

const detourDuration = ts - detourStartTS;
console.log(`Detour duration: ${detourDuration}s (expected: ${DETOUR_DURATION_S}s)`);

groundTruth.push({
  stop_idx: DETOUR_TO_STOP,  // Use correct stop index (6 = stop 11)
  lat: toStop.lat,
  lon: toStop.lon,
  timestamp: ts,
  phase: 'shortcut_end',
  event: 're_acquisition',
  off_route_duration_s: detourDuration
});

// Phase 3: Re-acquire at stop 11, continue to stop 14
console.log('\nPhase 3: Re-acquire at Stop 11, continue to Stop 14');

emitStatic(toStop.lat, toStop.lon, segments[stopSegments[DETOUR_TO_STOP]].bearing, STOP_DWELL_S);
groundTruth.push({
  stop_idx: DETOUR_TO_STOP,  // Use correct stop index (6 = stop 11)
  seg_idx: stopSegments[DETOUR_TO_STOP],
  timestamp: ts - STOP_DWELL_S,
  dwell_s: STOP_DWELL_S
});
stopSeqIdx = DETOUR_TO_STOP + 1;  // Skip to stop after re-acquisition

// Continue from stop 11 to stop 14 (indices 6-9)
for (let segIdx = stopSegments[DETOUR_TO_STOP]; segIdx < segments.length; segIdx++) {
  const seg = segments[segIdx];

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

#!/usr/bin/env node
/**
 * Generate NMEA for shortcut scenario: bus goes off-route from stop 6 to stop 10
 * This tests the off-route detection system.
 *
 * Route: ty225_short (stops 5-13 from ty225)
 * Shortcut: Bus deviates from route at stop 6, goes straight to stop 10
 * Expected behavior: Off-route detected after 5+ seconds, position frozen
 */

const fs = require('fs');

// Constants from gen_nmea.js
const EARTH_R = 6_371_000;
const BASE_TS = 1_700_000_000;
const CRUISE_KMH = 28;
const CRUISE_MS = CRUISE_KMH / 3.6;
const MAX_KMH = 50;
const MAX_MS = MAX_KMH / 3.6;
const ACCEL_MS2 = 1.2;
const DECEL_MS2 = 1.8;
const STOP_DWELL_S = 8;
const HDOP = 3.5;
const SATS = 8;

// AR(1) noise parameters
const AR1_ALPHA = 0.7;
const DRIFT_DECAY = 0.98;

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

// NMEA generation
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

// Load route
const routeFile = process.argv[2] || '../test_data/ty225_short_route.json';
const route = JSON.parse(fs.readFileSync(routeFile, 'utf8'));
const stops = JSON.parse(fs.readFileSync('../test_data/ty225_short_stops.json', 'utf8')).stops;

console.log(`Route has ${route.route_points.length} points, ${stops.length} stops`);

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

// Simulation
const nmeaLines = [];
const groundTruth = [];
const trace = []; // For trace.jsonl

let ts = BASE_TS;
let stopSeqIdx = 0;
const noiseN = new AR1Noise(15);
const noiseE = new AR1Noise(15);
const noiseHeading = new AR1Noise(5.0);

// Helper to emit GPS at a position
function emitGPS(lat, lon, speedMs, brng) {
  const [nl, no] = addNoiseMeters(lat, lon, noiseN.next(), noiseE.next());
  const noisyBearing = (brng + noiseHeading.next() + 360) % 360;
  nmeaLines.push(makeGPRMC(ts, nl, no, speedMs * 1.94384, noisyBearing));
  nmeaLines.push(makeGPGGA(ts, nl, no, HDOP, SATS));
  trace.push({ ts, lat: nl, lon: no, speed_cms: Math.round(speedMs * 100), heading_cdeg: Math.round(noisyBearing * 100) });
  ts++;
}

// Helper to emit stationary GPS (at stop)
function emitStatic(lat, lon, brng, duration) {
  for (let t = 0; t < duration; t++) {
    emitGPS(lat, lon, 0, brng);
  }
}

// Phase 1: Normal from stop 5 to stop 6
console.log('\\nPhase 1: Stop 5 to Stop 6 (normal)');
for (let segIdx = stopSegments[0]; segIdx <= stopSegments[1]; segIdx++) {
  const seg = segments[segIdx];
  groundTruth.push({ stop_idx: stopSeqIdx, seg_idx: segIdx, phase: 'normal_to_stop6' });

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

// Phase 2: OFF-ROUTE - straight from stop 6 to stop 10 (shortcut)
console.log('\\nPhase 2: OFF-ROUTE Stop 6 to Stop 10 (shortcut)');
const stop6 = stops[1]; // Stop 6 (index 1 in our array)
const stop10 = stops[5]; // Stop 10 (index 5 in our array)

// Create shortcut path
const shortcutDist = haversine(stop6.lat, stop6.lon, stop10.lat, stop10.lon);
const shortcutBearing = bearing(stop6.lat, stop6.lon, stop10.lat, stop10.lon);
console.log(`Shortcut distance: ${shortcutDist.toFixed(0)}m, bearing: ${shortcutBearing.toFixed(1)}°`);

// Dwell at stop 6
emitStatic(stop6.lat, stop6.lon, shortcutBearing, STOP_DWELL_S);
groundTruth.push({ stop_idx: stopSeqIdx, lat: stop6.lat, lon: stop6.lon, timestamp: ts - STOP_DWELL_S, phase: 'stop_6', event: 'departure_shortcut' });

// Travel shortcut (off-route!)
let traveled = 0;
let speedMs = CRUISE_MS * 0.4;
const offRouteStartTS = ts;

while (traveled < shortcutDist) {
  const targetMs = CRUISE_MS;
  if (speedMs < targetMs) speedMs = Math.min(speedMs + ACCEL_MS2, targetMs);
  else speedMs = Math.max(speedMs - DECEL_MS2, targetMs);

  const step = Math.min(speedMs, shortcutDist - traveled);
  traveled += step;

  const frac = traveled / shortcutDist;
  const lat = stop6.lat + (stop10.lat - stop6.lat) * frac;
  const lon = stop6.lon + (stop10.lon - stop6.lon) * frac;
  emitGPS(lat, lon, speedMs, shortcutBearing);
}

const offRouteDuration = ts - offRouteStartTS;
console.log(`Off-route duration: ${offRouteDuration}s (expected to trigger off-route detection)`);

// Phase 3: Re-acquire at stop 10, continue to stop 13
console.log('\\nPhase 3: Re-acquire at Stop 10, continue to Stop 13');
groundTruth.push({ stop_idx: stopSeqIdx, lat: stop10.lat, lon: stop10.lon, timestamp: ts, phase: 'stop_10', event: 're_acquisition', off_route_duration_s: offRouteDuration });

// Dwell at stop 10
emitStatic(stop10.lat, stop10.lon, segments[stopSegments[5]].bearing, STOP_DWELL_S);
stopSeqIdx++;

// Continue from stop 10 to stop 13
for (let segIdx = stopSegments[5]; segIdx < segments.length; segIdx++) {
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

  // Check if we're at a stop
  for (let s = 6; s < stops.length; s++) {
    if (segIdx === stopSegments[s]) {
      emitStatic(seg.to[0], seg.to[1], seg.bearing, STOP_DWELL_S);
      groundTruth.push({ stop_idx: stopSeqIdx, seg_idx: segIdx, phase: 'normal_after_stop10' });
      stopSeqIdx++;
    }
  }
}

// Write outputs
const outNmea = process.argv[3] || '../test_data/ty225_shortcut.nmea';
const outGt = process.argv[4] || '../test_data/ty225_shortcut_gt.json';
const outTrace = process.argv[5] || '../test_data/ty225_shortcut_trace.jsonl';

fs.writeFileSync(outNmea, nmeaLines.join('\n') + '\n');
console.log(`\\nWrote ${nmeaLines.length} NMEA lines to ${outNmea}`);

fs.writeFileSync(outGt, JSON.stringify(groundTruth, null, 2));
console.log(`Wrote ground truth to ${outGt}`);

fs.writeFileSync(outTrace, trace.map(line => JSON.stringify(line)).join('\n') + '\n');
console.log(`Wrote trace to ${outTrace}`);

console.log(`\\nSimulation summary:`);
console.log(`  - Total GPS points: ${nmeaLines.length / 2}`);
console.log(`  - Off-route duration: ${offRouteDuration}s`);
console.log(`  - Expected behavior: Off-route detected at 5s, position frozen, recovery on re-acquisition`);

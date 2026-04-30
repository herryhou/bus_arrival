#!/usr/bin/env node
/**
 * gen_nmea.js — GPS Bus Route NMEA Simulator
 * Generates test data (test.nmea + ground_truth.json) for arrival detection pipeline
 * Pure Node.js, no external dependencies.
 *
 * AGENT USAGE:
 *   # Step 1: Discover parameter shapes
 *   ./gen_nmea.js schema generate
 *
 *   # Step 2: Execute with discovered schema
 *   ./gen_nmea.js generate --json '{"route":"route.json","scenario":"normal"}'
 *
 *   # Step 3: Check exit code + parse result
 *   # exit 0 = ok, exit 1 = validation, exit 2 = execution error
 */

'use strict';

const fs = require('fs');

// ─── JSON Schema for Agent Discovery ──────────────────────────────────────────

const GENERATE_SCHEMA = {
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "GenerateNMEA",
  "description": "Generate NMEA sentences and ground truth for bus route simulation",
  "type": "object",
  "properties": {
    "route": {
      "type": "string",
      "description": "Path to route JSON file containing route_points",
      "default": "route.json"
    },
    "stops": {
      "type": "string",
      "description": "Path to stops JSON file with lat/lon coordinates (optional, uses route.stops if not provided)",
      "default": null
    },
    "scenario": {
      "type": "string",
      "description": "Simulation scenario preset",
      "enum": ["normal", "drift", "jump", "outage", "shortcut", "detour"],
      "default": "normal"
    },
    "shortcut_from_stop": {
      "type": "number",
      "description": "Starting stop index for shortcut (0-based, only used when scenario=shortcut)",
      "default": 1
    },
    "shortcut_to_stop": {
      "type": "number",
      "description": "Ending stop index for shortcut (0-based, only used when scenario=shortcut)",
      "default": 5
    },
    "detour_from_stop": {
      "type": "number",
      "description": "Starting stop index for detour (0-based, only used when scenario=detour)",
      "default": 1
    },
    "detour_to_stop": {
      "type": "number",
      "description": "Ending stop index for detour (0-based, only used when scenario=detour)",
      "default": 6
    },
    "detour_waypoint_lat": {
      "type": "number",
      "description": "Waypoint latitude for detour (only used when scenario=detour)",
      "default": 24.99207
    },
    "detour_waypoint_lon": {
      "type": "number",
      "description": "Waypoint longitude for detour (only used when scenario=detour)",
      "default": 121.29562
    },
    "detour_duration_s": {
      "type": "number",
      "description": "Duration of detour in seconds (only used when scenario=detour)",
      "default": 60
    },
    "outage_start_seg": {
      "type": "number",
      "description": "Starting segment index for GPS outage (only used when scenario= outage)",
      "default": 15
    },
    "outage_end_seg": {
      "type": "number",
      "description": "Ending segment index for GPS outage (only used when scenario=outage)",
      "default": 17
    },
    "out_nmea": {
      "type": "string",
      "description": "Output path for NMEA sentences file",
      "default": "test.nmea"
    },
    "out_gt": {
      "type": "string",
      "description": "Output path for ground truth JSON file",
      "default": "ground_truth.json"
    }
  }
};

const CLI_SCHEMA = {
  "version": "1.0.0",
  "name": "gen_nmea",
  "description": "GPS bus route NMEA simulator for arrival detection testing",
  "commands": ["schema", "generate", "help"]
};

// ─── Output Helpers ───────────────────────────────────────────────────────────

let globalFlags = {
  json: false,
  nonInteractive: false,
  dryRun: false,
  verbose: false,
  force: false
};

function output(data) {
  if (globalFlags.json) {
    console.log(JSON.stringify(data, null, 2));
  } else {
    if (data.status === 'ok') {
      console.log(`✓ ${data.message || 'Success'}`);
      if (data.logs) data.logs.forEach(log => console.log(`  ${log}`));
    } else {
      console.error(`✗ Error ${data.code}: ${data.message}`);
      if (data.validation_errors) {
        data.validation_errors.forEach(e => console.error(`  - ${e}`));
      }
    }
  }
}

function exitSuccess(data) {
  output({ status: 'ok', ...data });
  process.exit(0);
}

function exitError(code, message, validationErrors = null) {
  const data = {
    status: 'error',
    code: code,
    message: message
  };
  if (validationErrors) data.validation_errors = validationErrors;
  output(data);
  process.exit(code);
}

// ─── CLI 解析 ────────────────────────────────────────────────────────────────

function parseArgs(argv) {
  const args = {
    command: 'generate',
    commandArgs: [],
    route: 'route.json',
    scenario: 'normal',
    outage_start_seg: 15,
    outage_end_seg: 17,
    shortcut_from_stop: 1,
    shortcut_to_stop: 5,
    detour_from_stop: 1,
    detour_to_stop: 6,
    detour_waypoint_lat: 24.99207,
    detour_waypoint_lon: 121.29562,
    detour_duration_s: 60,
    out_nmea: 'test.nmea',
    out_gt: 'ground_truth.json',
  };
  const commands = ['schema', 'generate', 'help'];
  let commandFound = false;

  for (let i = 2; i < argv.length; i++) {
    const arg = argv[i];

    // Help is special - always show help and exit
    if (arg === '--help' || arg === '-h') {
      args.command = 'help';
      return args;
    }

    if (arg === '--route') {
      args.route = argv[++i];
      continue;
    }

    if (arg === '--stops') {
      args.stops = argv[++i];
      continue;
    }

    if (arg === '--outage-start-seg') {
      args.outage_start_seg = parseInt(argv[++i]);
      continue;
    }

    if (arg === '--outage-end-seg') {
      args.outage_end_seg = parseInt(argv[++i]);
      continue;
    }

    if (arg === '--out-nmea' || arg === '--out_nmea') {
      args.out_nmea = argv[++i];
      continue;
    }

    if (arg === '--out-gt' || arg === '--out_gt') {
      args.out_gt = argv[++i];
      continue;
    }

    if (arg === '--scenario') {
      args.scenario = argv[++i];
      continue;
    }

    if (arg === '--shortcut-from-stop') {
      args.shortcut_from_stop = parseInt(argv[++i]);
      continue;
    }

    if (arg === '--shortcut-to-stop') {
      args.shortcut_to_stop = parseInt(argv[++i]);
      continue;
    }
    if (arg === '--detour-from-stop') {
      args.detour_from_stop = parseInt(argv[++i]);
      continue;
    }
    if (arg === '--detour-to-stop') {
      args.detour_to_stop = parseInt(argv[++i]);
      continue;
    }
    if (arg === '--detour-waypoint-lat') {
      args.detour_waypoint_lat = parseFloat(argv[++i]);
      continue;
    }
    if (arg === '--detour-waypoint-lon') {
      args.detour_waypoint_lon = parseFloat(argv[++i]);
      continue;
    }
    if (arg === '--detour-duration-s') {
      args.detour_duration_s = parseInt(argv[++i]);
      continue;
    }

    // Check for global flags first
    if (arg === '--json') {
      globalFlags.json = true;
      continue;
    }
    if (arg === '--non-interactive') {
      globalFlags.nonInteractive = true;
      continue;
    }
    if (arg === '--dry-run') {
      globalFlags.dryRun = true;
      continue;
    }
    if (arg === '--verbose') {
      globalFlags.verbose = true;
      continue;
    }
    if (arg === '--force') {
      globalFlags.force = true;
      continue;
    }

    // Check for --json payload
    if (arg === '--json-payload') {
      const payload = argv[++i];
      try {
        const parsed = JSON.parse(payload);
        Object.assign(args, parsed);
      } catch (e) {
        exitError(1, `Invalid JSON payload: ${e.message}`);
      }
      continue;
    }

    // Check for command (only first command is used, rest go to commandArgs)
    if (commands.includes(arg)) {
      if (!commandFound) {
        args.command = arg;
        commandFound = true;
      } else {
        args.commandArgs.push(arg);
      }
      continue;
    }

    // After a command, collect remaining args for the command
    if (commandFound) {
      args.commandArgs.push(arg);
      continue;
    }

    // Legacy flag support for backwards compatibility (before any command)
    // Note: Most flags are now handled above, this is just for unknown flags
    if (arg.startsWith('--')) {
      // Convert hyphenated to snake_case for known options
      const knownOptions = ['route', 'stops', 'scenario', 'outage-start-seg', 'outage-end-seg', 'out-nmea', 'out-gt',
                            'outage_start_seg', 'outage_end_seg', 'out_nmea', 'out_gt', 'json-payload',
                            'shortcut-from-stop', 'shortcut-to-stop', 'shortcut_from_stop', 'shortcut_to_stop',
                            'detour-from-stop', 'detour-to-stop', 'detour-waypoint-lat', 'detour-waypoint-lon', 'detour-duration-s',
                            'detour_from_stop', 'detour_to_stop', 'detour_waypoint_lat', 'detour_waypoint_lon', 'detour_duration_s'];
      if (!knownOptions.includes(arg.slice(2)) && !knownOptions.includes(arg.replace(/-/g, '_').slice(2))) {
        exitError(1, `Unknown option: ${arg}`, [`Use --help for usage information`]);
      }
    }
  }
  return args;
}

// ─── 常數 / 場景設定 ─────────────────────────────────────────────────────────

const SCENARIOS = {
  normal: { hdop: 3.5, sigmaM: 15, sigmaHeading: 5.0, sats: 8, jump: false, outage: false, canyon: false, shortcut: false, detour: false },
  drift: { hdop: 7.0, sigmaM: 35, sigmaHeading: 15.0, sats: 5, jump: false, outage: false, canyon: true, shortcut: false, detour: false },
  jump: { hdop: 3.5, sigmaM: 18, sigmaHeading: 5.0, sats: 8, jump: true, outage: false, canyon: false, shortcut: false, detour: false },
  outage: { hdop: 3.5, sigmaM: 18, sigmaHeading: 5.0, sats: 8, jump: false, outage: true, canyon: false, shortcut: false, detour: false },
  shortcut: { hdop: 3.5, sigmaM: 15, sigmaHeading: 5.0, sats: 8, jump: false, outage: false, canyon: false, shortcut: true, detour: false },
  detour: { hdop: 3.5, sigmaM: 15, sigmaHeading: 5.0, sats: 8, jump: false, outage: false, canyon: false, shortcut: false, detour: true },
};

const CRUISE_KMH = 28;
const MAX_KMH = 50;
const ACCEL_MS2 = 1.2;
const DECEL_MS2 = 1.8;
const STOP_DWELL_S = 8;
const SIGNAL_CYCLE = 30;      // 號誌週期（秒）
const SLOW_START = 0.60;    // 接近站點的路段中，從 60% 開始降速
const SLOW_RATIO = 0.30;    // 降速目標為巡航速度的 30%

const AR1_ALPHA = 0.7;     // AR(1) 自相關係數
const DRIFT_DECAY = 0.98;    // 漂移均值回歸係數
const EARTH_R = 6_371_000;

const BASE_TS = 1_700_000_000;

// ─── 數學 / 地理工具 ─────────────────────────────────────────────────────────

function toRad(deg) { return deg * Math.PI / 180; }
function toDeg(rad) { return rad * 180 / Math.PI; }

function haversine([lat1, lon1], [lat2, lon2]) {
  const dLat = toRad(lat2 - lat1);
  const dLon = toRad(lon2 - lon1);
  const a = Math.sin(dLat / 2) ** 2 +
    Math.cos(toRad(lat1)) * Math.cos(toRad(lat2)) * Math.sin(dLon / 2) ** 2;
  return 2 * EARTH_R * Math.asin(Math.sqrt(a));
}

function bearing([lat1, lon1], [lat2, lon2]) {
  const dLon = toRad(lon2 - lon1);
  const y = Math.sin(dLon) * Math.cos(toRad(lat2));
  const x = Math.cos(toRad(lat1)) * Math.sin(toRad(lat2)) -
    Math.sin(toRad(lat1)) * Math.cos(toRad(lat2)) * Math.cos(dLon);
  return (toDeg(Math.atan2(y, x)) + 360) % 360;
}

function movePoint([lat, lon], brng, dist) {
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

/** 以公尺為單位施加南北 + 東西偏移 */
function addNoiseMeters([lat, lon], noiseLat, noiseLon) {
  const [lat2, lon2] = movePoint([lat, lon], 0, noiseLat);
  const [lat3, lon3] = movePoint([lat2, lon2], 90, noiseLon);
  return [lat3, lon3];
}

// ─── NMEA 工具 ───────────────────────────────────────────────────────────────

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

// ─── AR(1) 雜訊產生器 ────────────────────────────────────────────────────────

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

// ─── 路線前處理 ──────────────────────────────────────────────────────────────

/**
 * Find the closest route point index for each stop, respecting traveling order
 * @param {Array} stops - Array of {lat, lon} stop objects
 * @param {Array} routePoints - Array of [lat, lon] route points
 * @returns {Array} - Array of route point indices closest to each stop in traveling order
 */
function findStopIndices(stops, routePoints) {
  const indices = [];
  let searchStartIdx = 0;

  for (const stop of stops) {
    let minDist = Infinity;
    let minIdx = searchStartIdx;

    // Search from searchStartIdx onwards to preserve traveling order
    for (let i = searchStartIdx; i < routePoints.length; i++) {
      const dist = haversine([stop.lat, stop.lon], routePoints[i]);
      if (dist < minDist) {
        minDist = dist;
        minIdx = i;
      }
    }

    // For loop routes, also check from start if we're near the end
    if (searchStartIdx > routePoints.length * 0.8) {
      for (let i = 0; i < searchStartIdx; i++) {
        const dist = haversine([stop.lat, stop.lon], routePoints[i]);
        if (dist < minDist) {
          minDist = dist;
          minIdx = i;
        }
      }
    }

    indices.push(minIdx);
    searchStartIdx = minIdx + 1;

    // If we've reached the end, allow wrapping to start for loop routes
    if (searchStartIdx >= routePoints.length) {
      searchStartIdx = 0;
    }
  }

  return indices;
}

function buildSegments(routePoints, stops, lights) {
  const stopSet = new Set(stops);
  const lightSet = new Set(lights || []);
  return routePoints.slice(0, -1).map((from, i) => {
    const to = routePoints[i + 1];
    return {
      from,
      to,
      dist: haversine(from, to),
      bearing: bearing(from, to),
      stopBefore: stopSet.has(i + 1),
      lightBefore: lightSet.has(i + 1),
    };
  });
}

// ─── 速度工具 ────────────────────────────────────────────────────────────────

const CRUISE_MS = CRUISE_KMH / 3.6;
const MAX_MS = MAX_KMH / 3.6;

function dwellSeconds() {
  return Math.max(2, Math.min(Math.round(STOP_DWELL_S + randn() * 3), STOP_DWELL_S + 10));
}

// ─── 主模擬 ──────────────────────────────────────────────────────────────────

function simulate(route, cfg) {
  const { route_points, stops, traffic_lights } = route;
  const segs = buildSegments(route_points, stops, traffic_lights || []);
  const totalDist = segs.reduce((s, g) => s + g.dist, 0);

  const { hdop, sigmaM, sigmaHeading, sats, outageStartSeg, outageEndSeg, shortcut, shortcutFromStop, shortcutToStop, detour, detourFromStop, detourToStop, detourWaypointLat, detourWaypointLon, detourDurationS } = cfg;
  const noiseN = new AR1Noise(sigmaM);
  const noiseE = new AR1Noise(sigmaM);
  const noiseHeading = new AR1Noise(sigmaHeading);

  const jumpDistTarget = totalDist * 0.30;
  let jumpInjected = false;

  const nmeaLines = [];
  const groundTruth = [];

  let ts = BASE_TS;
  let travelledDist = 0;
  let stopSeqIdx = 0;

  // Build stop index mapping for shortcut scenario
  const stopIndexMap = [];
  let stopCount = 0;
  for (let i = 0; i < segs.length; i++) {
    if (segs[i].stopBefore) {
      stopIndexMap[i] = stopCount++;
    }
  }

  // Helper to emit GPS at a position
  const emitGPS = (lat, lon, speedMs, brng, outage = false) => {
    if (!outage) {
      const [nl, no] = addNoiseMeters([lat, lon], noiseN.next(), noiseE.next());
      const noisyBearing = (brng + noiseHeading.next() + 360) % 360;
      nmeaLines.push(makeGPRMC(ts, nl, no, speedMs * 1.94384, noisyBearing));
      nmeaLines.push(makeGPGGA(ts, nl, no, hdop, sats));
    }
    ts++;
  };

  // 輸出一秒靜止訊號的輔助函式
  const emitStatic = (pos, brng, outage) => {
    if (!outage) {
      const [nl, no] = addNoiseMeters(pos, noiseN.next(), noiseE.next());
      // Add heading noise even when stationary (GPS receivers still report heading)
      const noisyBearing = (brng + noiseHeading.next() + 360) % 360;
      nmeaLines.push(makeGPRMC(ts, nl, no, 0, noisyBearing));
      nmeaLines.push(makeGPGGA(ts, nl, no, hdop, sats));
    }
    ts++;
  };

  // ── Special handling: stop at route point index 0 ─────────────────────────────
  // Stops at index 0 cannot be processed by the stopBefore model since there's
  // no "previous segment" to trigger the dwell time. Handle them explicitly here.
  if (stops.includes(0) && segs.length > 0) {
    const firstSeg = segs[0];
    const isOutage = cfg.outage && 0 >= outageStartSeg && 0 <= outageEndSeg;
    const dwell = dwellSeconds();
    groundTruth.push({ stop_idx: stopSeqIdx++, seg_idx: 0, timestamp: ts, dwell_s: dwell });
    for (let t = 0; t < dwell; t++) emitStatic(firstSeg.from, firstSeg.bearing, isOutage);
  }

  // Shortcut state
  let shortcutActive = false;
  let shortcutCompleted = false;

  // Detour state
  let detourActive = false;
  let detourCompleted = false;

  for (let si = 0; si < segs.length; si++) {
    const seg = segs[si];
    const isOutage = cfg.outage && si >= outageStartSeg && si <= outageEndSeg;
    const prevSeg = si > 0 ? segs[si - 1] : null;

    // Check for shortcut trigger
    if (shortcut && !shortcutActive && !shortcutCompleted && seg.stopBefore && stopIndexMap[si] === shortcutFromStop) {
      shortcutActive = true;
      console.log(`Shortcut triggered at stop ${shortcutFromStop} (route point ${stops[shortcutFromStop]}), going to stop ${shortcutToStop} (route point ${stops[shortcutToStop]})`);

      // Get coordinates from route points
      // stops array contains route point indices for each stop
      const fromRoutePointIdx = stops[shortcutFromStop];
      const fromLat = route_points[fromRoutePointIdx][0];
      const fromLon = route_points[fromRoutePointIdx][1];
      // Use actual stop coordinates for shortcut end
      const toLat = route.stopCoords[shortcutToStop][0];
      const toLon = route.stopCoords[shortcutToStop][1];

      // Calculate shortcut distance and bearing
      const shortcutDist = haversine([fromLat, fromLon], [toLat, toLon]);
      const shortcutBearing = bearing([fromLat, fromLon], [toLat, toLon]);

      console.log(`Shortcut: ${shortcutDist.toFixed(0)}m, bearing ${shortcutBearing.toFixed(1)}°`);

      // Dwell at from_stop before shortcut
      emitStatic([fromLat, fromLon], shortcutBearing, false);
      groundTruth.push({ stop_idx: shortcutFromStop, lat: fromLat, lon: fromLon, timestamp: ts - STOP_DWELL_S, phase: 'shortcut_start', event: 'departure_shortcut' });

      // Generate GPS points along shortcut
      const shortcutStartTS = ts;
      let traveled = 0;
      let speedMs = CRUISE_MS * 0.4;

      while (traveled < shortcutDist) {
        const targetMs = CRUISE_MS;
        if (speedMs < targetMs) speedMs = Math.min(speedMs + ACCEL_MS2, targetMs);
        else speedMs = Math.max(speedMs - DECEL_MS2, targetMs);

        const step = Math.min(speedMs, shortcutDist - traveled);
        traveled += step;

        const frac = traveled / shortcutDist;
        const lat = fromLat + (toLat - fromLat) * frac;
        const lon = fromLon + (toLon - fromLon) * frac;
        emitGPS(lat, lon, speedMs, shortcutBearing, false);
      }

      const offRouteDuration = ts - shortcutStartTS;
      console.log(`Shortcut duration: ${offRouteDuration}s`);
      groundTruth.push({ stop_idx: shortcutToStop, lat: toLat, lon: toLon, timestamp: ts, phase: 'shortcut_end', event: 're_acquisition', off_route_duration_s: offRouteDuration });
    }

    // Detour trigger is now at stop level (after stop processing block), not here

    // Skip segments during shortcut (we've already injected GPS)
    if (shortcutActive) {
      if (seg.stopBefore && stopIndexMap[si] === shortcutToStop) {
        shortcutActive = false;
        shortcutCompleted = true;
        console.log(`Shortcut completed at stop ${shortcutToStop}, resuming normal route`);
        // Continue processing this segment normally
      } else {
        // Skip this segment during shortcut
        continue;
      }
    }

    // Skip segments during detour (we've already injected GPS)
    if (detourActive) {
      if (seg.stopBefore && stopIndexMap[si] === detourToStop) {
        detourActive = false;
        detourCompleted = true;
        console.log(`Detour completed at stop ${detourToStop}, resuming normal route`);
        // Continue processing this segment normally
      } else {
        // Skip this segment during detour
        continue;
      }
    }

    // DEBUG: Log every 20th segment
    if (si % 20 === 0) {
      console.log('Processing segment', si, 'dist', seg.dist.toFixed(2) + 'm');
    }

    // ── 在本段起點處理停靠 / 紅燈 ───────────────────────────────────────────
    if (prevSeg && (prevSeg.stopBefore || prevSeg.lightBefore)) {
      if (prevSeg.stopBefore) {
        // Skip intermediate stops during detour (per PRD requirement)
        // Use stopIndexMap to find which stop this segment leads to
        const currentStopIdx = stopIndexMap[si];
        // Check if this stop should be skipped (between detour start and end)
        // Also skip segments that don't lead to actual stops (currentStopIdx === undefined)
        if (currentStopIdx === undefined) {
          console.log(`Skipping seg_idx=${si} (does not lead to a stop)`);
        } else if (detourCompleted && currentStopIdx > detourFromStop && currentStopIdx < detourToStop) {
          console.log(`Skipping stop ${currentStopIdx} (intermediate stop during detour, seg_idx=${si}), detourFromStop=${detourFromStop}, detourToStop=${detourToStop}`);
          // Don't increment stopSeqIdx or add to ground truth
        } else {
          if (detourCompleted) {
            console.log(`Processing stop ${currentStopIdx} at seg_idx=${si}, detourFromStop=${detourFromStop}, detourToStop=${detourToStop}`);
          }
          const dwell = dwellSeconds();
          // For detour case, use actual stop index; for normal case, use sequential index
          const gtStopIdx = detourCompleted ? currentStopIdx : stopSeqIdx;
          groundTruth.push({ stop_idx: gtStopIdx, seg_idx: si, timestamp: ts, dwell_s: dwell });
          if (!detourCompleted) {
            stopSeqIdx++; // Only increment for normal stops
          }
          for (let t = 0; t < dwell; t++) emitStatic(seg.from, seg.bearing, isOutage);
        }
      } else {
        // 紅燈
        const wait = 5 + Math.floor(Math.random() * SIGNAL_CYCLE);
        for (let t = 0; t < wait; t++) emitStatic(seg.from, seg.bearing, isOutage);
      }
    }

    // ── 檢查繞道觸發（在站點停靠後立即觸發）────────────────────────────────────
    // Detour trigger at STOP level (not segment level) to fire immediately
    // after stop dwell. This prevents processing additional segments going
    // toward stop 2 before the detour starts.
    if (detour && !detourActive && !detourCompleted && prevSeg && prevSeg.stopBefore) {
      // Check the previous stop (that we just finished processing), not the current one
      const prevStopIdx = stopIndexMap[si - 1] ?? 0;

      if (prevStopIdx === detourFromStop) {
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

        // Add normal stop entry for detourToStop (stop 6 dwell)
        // This ensures stop 6 gets its normal arrival/dwell processing after detour_end
        const detourEndDwell = 8;
        const dwell = detourEndDwell; // Use fixed dwell for detour end
        groundTruth.push({ stop_idx: detourToStop, seg_idx: si, timestamp: ts, dwell_s: dwell });
        for (let t = 0; t < detourEndDwell; t++) {
          emitStatic([toLat, toLon], leg3Bearing, false);
        }

        // Jump to the segment that leads to detour end stop
        // Since we've already generated GPS to detourToStop's location,
        // we need to find the segment index where stopIndexMap[si] === detourToStop
        // and continue processing from there.
        while (si < segs.length - 1 && stopIndexMap[si] !== detourToStop) {
          si++;
        }
        // Clear detourActive flag since we've jumped to the target segment
        detourActive = false;
        detourCompleted = true;
        console.log(`Detour GPS completed, jumped to segment ${si} leading to stop ${detourToStop}`);
        // Continue processing from the segment that leads to detour end stop
        continue;
      }
    }

    // ── 行駛本段 ─────────────────────────────────────────────────────────────
    const willSlow = seg.stopBefore || seg.lightBefore;
    let traveled = 0;
    let speedMs = CRUISE_MS * 0.4;

    while (traveled < seg.dist) {
      const remaining = seg.dist - traveled;
      const progress = traveled / seg.dist;

      // 目標速度
      let targetMs;
      if (willSlow && progress >= SLOW_START) {
        const blend = (progress - SLOW_START) / (1 - SLOW_START);
        targetMs = CRUISE_MS * (1 - blend) + CRUISE_MS * SLOW_RATIO * blend;
      } else {
        targetMs = Math.min(CRUISE_MS + randn() * 1.5, MAX_MS);
      }
      targetMs = Math.max(0.5, targetMs);

      if (speedMs < targetMs) speedMs = Math.min(speedMs + ACCEL_MS2, targetMs);
      else speedMs = Math.max(speedMs - DECEL_MS2, targetMs);

      const step = Math.min(speedMs, remaining);
      traveled += step;
      travelledDist += step;

      // 插值座標
      const frac = traveled / seg.dist;
      let lat = seg.from[0] + (seg.to[0] - seg.from[0]) * frac;
      let lon = seg.from[1] + (seg.to[1] - seg.from[1]) * frac;

      // 跳點注入
      if (cfg.jump && !jumpInjected && travelledDist >= jumpDistTarget) {
        [lat, lon] = movePoint([lat, lon], Math.random() * 360, 100 + Math.random() * 30);
        jumpInjected = true;
      }

      if (!isOutage) {
        const [nl, no] = addNoiseMeters([lat, lon], noiseN.next(), noiseE.next());
        // Add heading noise to the segment bearing, normalize to 0-360 range
        const noisyBearing = (seg.bearing + noiseHeading.next() + 360) % 360;
        nmeaLines.push(makeGPRMC(ts, nl, no, speedMs * 1.94384, noisyBearing));
        nmeaLines.push(makeGPGGA(ts, nl, no, hdop, sats));

      }
      ts++;
    }
  }

  return { nmeaLines, groundTruth };
}

// ─── Commands ─────────────────────────────────────────────────────────────────

function cmdSchema(args) {
  // Schema command always outputs JSON (agent-friendly)
  const target = args[0]; // schema subcommand like 'generate'

  if (!target) {
    console.log(JSON.stringify(CLI_SCHEMA, null, 2));
    process.exit(0);
  }

  if (target === 'generate') {
    console.log(JSON.stringify(GENERATE_SCHEMA, null, 2));
    process.exit(0);
  }

  // Error for schema - still JSON
  console.log(JSON.stringify({
    status: 'error',
    code: 1,
    message: `Unknown command for schema: ${target}`,
    available_commands: CLI_SCHEMA.commands
  }, null, 2));
  process.exit(1);
}

function cmdHelp() {
  const helpText = `
gen_nmea.js — GPS Bus Route NMEA Simulator

USAGE:
  ./gen_nmea.js <command> [options]

COMMANDS:
  generate    Generate NMEA sentences and ground truth (default)
  schema      Display JSON Schema for parameter discovery
  help        Show this help message

GENERATE OPTIONS:
  --json-payload '{"route":"path","scenario":"normal"}'    Structured JSON input (recommended for agents)
  --route <path>                                           Route JSON file (default: route.json)
  --stops <path>                                           Stops JSON file with lat/lon (optional, uses route.stops if not provided)
  --scenario <name>                                        Scenario: normal, drift, jump, outage, shortcut (default: normal)
  --outage-start-seg <num>                                 Starting segment index for GPS outage (default: 15)
  --outage-end-seg <num>                                   Ending segment index for GPS outage (default: 17)
  --shortcut-from-stop <num>                               Starting stop index for shortcut (default: 1)
  --shortcut-to-stop <num>                                 Ending stop index for shortcut (default: 5)
  --out-nmea <path>                                        NMEA output file (default: test.nmea)
  --out-gt <path>                                          Ground truth output file (default: ground_truth.json)

GLOBAL FLAGS:
  --json                    Output structured JSON instead of human-readable text
  --non-interactive         Fail instead of prompting (currently: no prompts anyway)
  --dry-run                 Simulate without writing output files
  --verbose                 Include debug logs in JSON output
  --force                   Skip confirmation prompts (currently: none)

SCENARIOS:
  normal    HDOP=3.5, σ(pos)=15m, σ(heading)=5°, 8 sats, no anomalies (GPS output: 1 Hz)
  drift     HDOP=7.0, σ(pos)=35m, σ(heading)=15°, 5 sats, urban canyon drift
  jump      Normal GPS, but with 100m+ position jump at 30% distance
  outage    GPS signal outage on segments 15-17
  shortcut  Bus takes shortcut from --shortcut-from-stop to --shortcut-to-stop (off-route)

AGENT USAGE EXAMPLES:
  # Step 1: Discover parameter shapes
  ./gen_nmea.js schema generate

  # Step 2: Execute with discovered schema
  ./gen_nmea.js generate --json-payload '{"route":"route.json","scenario":"drift"}' --json

  # Step 3: Check exit code + parse result
  # exit 0 = ok, exit 1 = validation error, exit 2 = execution error

EXIT CODES:
  0  Success
  1  Validation error (missing/invalid parameters)
  2  Execution error (file not found, write failed, etc.)
  3  (reserved for future auth errors)
  4  Cancelled by user

EXAMPLES:
  # Human-readable output (default)
  ./gen_nmea.js generate --route myroute.json --scenario drift

  # Agent-friendly JSON output
  ./gen_nmea.js generate --json-payload '{"route":"myroute.json","scenario":"drift"}' --json

  # Dry run simulation
  ./gen_nmea.js generate --scenario jump --dry-run --json
`;

  if (globalFlags.json) {
    exitSuccess({
      command: 'help',
      data: { help_text: helpText.trim() },
      message: 'Help documentation'
    });
  } else {
    console.log(helpText);
    process.exit(0);
  }
}

function cmdGenerate(args) {
  const logs = [];
  const startTime = Date.now();

  if (globalFlags.verbose) logs.push('Starting NMEA generation...');

  // Validate scenario
  let cfg = { ...SCENARIOS[args.scenario] };
  if (!cfg) {
    exitError(1, `Unknown scenario: ${args.scenario}`,
      [`Available scenarios: ${Object.keys(SCENARIOS).join(', ')}`]);
  }

  // Add custom outage parameters for outage scenario
  if (args.scenario === 'outage') {
    cfg.outageStartSeg = args.outage_start_seg;
    cfg.outageEndSeg = args.outage_end_seg;
  }

  // Add custom shortcut parameters for shortcut scenario
  if (args.scenario === 'shortcut') {
    cfg.shortcutFromStop = args.shortcut_from_stop;
    cfg.shortcutToStop = args.shortcut_to_stop;
  }

  // Add custom detour parameters for detour scenario
  if (args.scenario === 'detour') {
    cfg.detourFromStop = args.detour_from_stop;
    cfg.detourToStop = args.detour_to_stop;
    cfg.detourWaypointLat = args.detour_waypoint_lat;
    cfg.detourWaypointLon = args.detour_waypoint_lon;
    cfg.detourDurationS = args.detour_duration_s;
  }

  // Load route file
  let route;
  try {
    const routeContent = fs.readFileSync(args.route, 'utf8');
    route = JSON.parse(routeContent);
    if (globalFlags.verbose) logs.push(`Loaded route from ${args.route}`);
  } catch (e) {
    exitError(2, `Cannot read route file "${args.route}": ${e.message}`);
  }

  // Load stops from separate file if provided
  if (args.stops) {
    let stopsData;
    try {
      const stopsContent = fs.readFileSync(args.stops, 'utf8');
      stopsData = JSON.parse(stopsContent);
      if (globalFlags.verbose) logs.push(`Loaded stops from ${args.stops}`);
    } catch (e) {
      exitError(2, `Cannot read stops file "${args.stops}": ${e.message}`);
    }

    // Find closest route points for each stop
    if (!Array.isArray(stopsData.stops)) {
      exitError(1, 'stops file must contain a "stops" array');
    }

    route.stops = findStopIndices(stopsData.stops, route.route_points);
    // Also store actual stop coordinates for detour generation
    route.stopCoords = stopsData.stops.map(s => [s.lat, s.lon]);
    console.log(route.stops);
    if (globalFlags.verbose) logs.push(`Mapped ${stopsData.stops.length} stops to route point indices: ${route.stops.slice(0, 5).join(', ')}...`);
  }

  // Validate route structure
  const validationErrors = [];
  if (!Array.isArray(route.route_points) || route.route_points.length < 2) {
    validationErrors.push('route_points must be an array with at least 2 points');
  }
  if (!Array.isArray(route.stops)) {
    validationErrors.push('Missing required field: stops (use --stops file or include stops array in route.json)');
  }
  if (validationErrors.length > 0) {
    exitError(1, 'Route validation failed', validationErrors);
  }

  if (globalFlags.verbose) {
    logs.push(`Scenario: ${args.scenario}`);
    logs.push(`Route points: ${route.route_points.length}`);
    logs.push(`Stops: ${route.stops.length}`);
  }

  // Run simulation
  const { nmeaLines, groundTruth } = simulate(route, cfg);

  // Write outputs (unless dry-run)
  if (!globalFlags.dryRun) {
    try {
      fs.writeFileSync(args.out_nmea, nmeaLines.join('\n') + '\n', 'utf8');
      if (globalFlags.verbose) logs.push(`Wrote ${nmeaLines.length} NMEA lines to ${args.out_nmea}`);
    } catch (e) {
      exitError(2, `Failed to write NMEA file "${args.out_nmea}": ${e.message}`);
    }

    try {
      fs.writeFileSync(args.out_gt, JSON.stringify(groundTruth, null, 2), 'utf8');
      if (globalFlags.verbose) logs.push(`Wrote ${groundTruth.length} stop events to ${args.out_gt}`);
    } catch (e) {
      exitError(2, `Failed to write ground truth file "${args.out_gt}": ${e.message}`);
    }
  } else {
    logs.push('[DRY RUN] Skipped writing output files');
  }

  const duration = Date.now() - startTime;

  exitSuccess({
    command: 'generate',
    data: {
      scenario: args.scenario,
      route_points: route.route_points.length,
      stops: route.stops.length,
      nmea_lines: nmeaLines.length,
      ground_truth_entries: groundTruth.length,
      output_nmea: args.out_nmea,
      output_gt: args.out_gt,
      dry_run: globalFlags.dryRun
    },
    logs: globalFlags.verbose ? logs : undefined,
    duration_ms: duration,
    message: `Generated ${nmeaLines.length} NMEA lines, ${groundTruth.length} stop events`
  });
}

// ─── 入口 ────────────────────────────────────────────────────────────────────

function main() {
  const args = parseArgs(process.argv);

  switch (args.command) {
    case 'schema':
      cmdSchema(args.commandArgs);
      break;
    case 'help':
      cmdHelp();
      break;
    case 'generate':
    default:
      cmdGenerate(args);
      break;
  }
}

main();
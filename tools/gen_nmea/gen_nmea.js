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
      "enum": ["normal", "drift", "jump", "outage"],
      "default": "normal"
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

    if (arg === '--out_nmea') {
      args.out_nmea = argv[++i];
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
    switch (arg) {
      case '--route':
      case '--stops':
      case '--scenario':
      case '--out-nmea':
      case '--out-gt':
        const key = arg.slice(2).replace(/-([a-z])/g, (_, c) => c.toUpperCase());
        args[key === 'outNmea' ? 'out_nmea' : key === 'outGt' ? 'out_gt' : key] = argv[++i];
        break;
      default:
        exitError(1, `Unknown option: ${arg}`, [`Use --help for usage information`]);
    }
  }
  return args;
}

// ─── 常數 / 場景設定 ─────────────────────────────────────────────────────────

const SCENARIOS = {
  normal: { hdop: 3.5, sigmaM: 18, sats: 8, jump: false, outage: false, canyon: false },
  drift: { hdop: 7.0, sigmaM: 35, sats: 5, jump: false, outage: false, canyon: true },
  jump: { hdop: 3.5, sigmaM: 18, sats: 8, jump: true, outage: false, canyon: false },
  outage: { hdop: 3.5, sigmaM: 18, sats: 8, jump: false, outage: true, canyon: false },
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
 * Find the closest route point index for each stop
 * @param {Array} stops - Array of {lat, lon} stop objects
 * @param {Array} routePoints - Array of [lat, lon] route points
 * @returns {Array} - Array of route point indices closest to each stop
 */
function findStopIndices(stops, routePoints) {
  return stops.map(stop => {
    let minDist = Infinity;
    let minIdx = 0;
    for (let i = 0; i < routePoints.length; i++) {
      const dist = haversine([stop.lat, stop.lon], routePoints[i]);
      if (dist < minDist) {
        minDist = dist;
        minIdx = i;
      }
    }
    return minIdx;
  });
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

  const { hdop, sigmaM, sats } = cfg;
  const noiseN = new AR1Noise(sigmaM);
  const noiseE = new AR1Noise(sigmaM);

  const jumpDistTarget = totalDist * 0.30;
  let jumpInjected = false;

  const OUTAGE_SEG_START = 15;
  const OUTAGE_SEG_END = 17;

  const nmeaLines = [];
  const groundTruth = [];

  let ts = BASE_TS;
  let travelledDist = 0;
  let stopSeqIdx = 0;

  // 輸出一秒靜止訊號的輔助函式
  const emitStatic = (pos, brng, outage) => {
    if (!outage) {
      const [nl, no] = addNoiseMeters(pos, noiseN.next(), noiseE.next());
      nmeaLines.push(makeGPRMC(ts, nl, no, 0, brng));
      nmeaLines.push(makeGPGGA(ts, nl, no, hdop, sats));
    }
    ts++;
  };

  for (let si = 0; si < segs.length; si++) {
    const seg = segs[si];
    const isOutage = cfg.outage && si >= OUTAGE_SEG_START && si <= OUTAGE_SEG_END;
    const prevSeg = si > 0 ? segs[si - 1] : null;

    // ── 在本段起點處理停靠 / 紅燈 ───────────────────────────────────────────
    if (prevSeg && (prevSeg.stopBefore || prevSeg.lightBefore)) {
      if (prevSeg.stopBefore) {
        const dwell = dwellSeconds();
        groundTruth.push({ stop_idx: stopSeqIdx++, seg_idx: si, timestamp: ts, dwell_s: dwell });
        for (let t = 0; t < dwell; t++) emitStatic(seg.from, seg.bearing, isOutage);
      } else {
        // 紅燈
        const wait = 5 + Math.floor(Math.random() * SIGNAL_CYCLE);
        for (let t = 0; t < wait; t++) emitStatic(seg.from, seg.bearing, isOutage);
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
        nmeaLines.push(makeGPRMC(ts, nl, no, speedMs * 1.94384, seg.bearing));
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
  --scenario <name>                                        Scenario: normal, drift, jump, outage (default: normal)
  --out-nmea <path>                                        NMEA output file (default: test.nmea)
  --out-gt <path>                                          Ground truth output file (default: ground_truth.json)

GLOBAL FLAGS:
  --json                    Output structured JSON instead of human-readable text
  --non-interactive         Fail instead of prompting (currently: no prompts anyway)
  --dry-run                 Simulate without writing output files
  --verbose                 Include debug logs in JSON output
  --force                   Skip confirmation prompts (currently: none)

SCENARIOS:
  normal    HDOP=3.5, σ=18m, 8 sats, no anomalies
  drift     HDOP=7.0, σ=35m, 5 sats, urban canyon drift
  jump      Normal GPS, but with 100m+ position jump at 30% distance
  outage    GPS signal outage on segments 15-17

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
  const cfg = SCENARIOS[args.scenario];
  if (!cfg) {
    exitError(1, `Unknown scenario: ${args.scenario}`,
      [`Available scenarios: ${Object.keys(SCENARIOS).join(', ')}`]);
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
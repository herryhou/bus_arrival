#!/usr/bin/env node
/**
 * gen_nmea.js — GPS 公車路線 NMEA 模擬器
 * 為到站偵測 Pipeline 產生測試資料（test.nmea + ground_truth.json）
 * 純 Node.js，無需任何外部套件。
 */

'use strict';

const fs   = require('fs');

// ─── CLI 解析 ────────────────────────────────────────────────────────────────

function parseArgs(argv) {
  const args = {
    route:    'route.json',
    scenario: 'normal',
    outNmea:  'test.nmea',
    outGt:    'ground_truth.json',
  };
  for (let i = 2; i < argv.length; i++) {
    switch (argv[i]) {
      case '--route':    args.route    = argv[++i]; break;
      case '--scenario': args.scenario = argv[++i]; break;
      case '--out-nmea': args.outNmea  = argv[++i]; break;
      case '--out-gt':   args.outGt    = argv[++i]; break;
      default:
        console.error(`未知選項：${argv[i]}`);
        process.exit(1);
    }
  }
  return args;
}

// ─── 常數 / 場景設定 ─────────────────────────────────────────────────────────

const SCENARIOS = {
  normal:  { hdop: 3.5, sigmaM: 18, sats: 8,  jump: false, outage: false, canyon: false },
  drift:   { hdop: 7.0, sigmaM: 35, sats: 5,  jump: false, outage: false, canyon: true  },
  jump:    { hdop: 3.5, sigmaM: 18, sats: 8,  jump: true,  outage: false, canyon: false },
  outage:  { hdop: 3.5, sigmaM: 18, sats: 8,  jump: false, outage: true,  canyon: false },
};

const CRUISE_KMH   = 28;
const MAX_KMH      = 50;
const ACCEL_MS2    = 1.2;
const DECEL_MS2    = 1.8;
const STOP_DWELL_S = 8;
const SIGNAL_CYCLE = 30;      // 號誌週期（秒）
const SLOW_START   = 0.60;    // 接近站點的路段中，從 60% 開始降速
const SLOW_RATIO   = 0.30;    // 降速目標為巡航速度的 30%

const AR1_ALPHA    = 0.7;     // AR(1) 自相關係數
const DRIFT_DECAY  = 0.98;    // 漂移均值回歸係數
const EARTH_R      = 6_371_000;

const BASE_TS      = 1_700_000_000;

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
  const d  = dist / EARTH_R;
  const b  = toRad(brng);
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
  const [lat2, lon2] = movePoint([lat, lon], 0,   noiseLat);
  const [lat3, lon3] = movePoint([lat2, lon2], 90, noiseLon);
  return [lat3, lon3];
}

// ─── NMEA 工具 ───────────────────────────────────────────────────────────────

function nmeaChecksum(sentence) {
  let cs = 0;
  for (let i = 1; i < sentence.length; i++) cs ^= sentence.charCodeAt(i);
  return cs.toString(16).toUpperCase().padStart(2, '0');
}

function formatDDMM(deg, isLat) {
  const abs  = Math.abs(deg);
  const d    = Math.floor(abs);
  const m    = (abs - d) * 60;
  const mStr = m.toFixed(4).padStart(7, '0');
  const dStr = isLat ? String(d).padStart(2, '0') : String(d).padStart(3, '0');
  return `${dStr}${mStr}`;
}

function tsToNmeaTime(ts) {
  const d  = new Date(ts * 1000);
  const hh = String(d.getUTCHours()).padStart(2, '0');
  const mm = String(d.getUTCMinutes()).padStart(2, '0');
  const ss = String(d.getUTCSeconds()).padStart(2, '0');
  return `${hh}${mm}${ss}`;
}

function tsToNmeaDate(ts) {
  const d  = new Date(ts * 1000);
  const dd = String(d.getUTCDate()).padStart(2, '0');
  const mo = String(d.getUTCMonth() + 1).padStart(2, '0');
  const yy = String(d.getUTCFullYear()).slice(-2);
  return `${dd}${mo}${yy}`;
}

function makeGPRMC(ts, lat, lon, speedKnots, brng) {
  const NS   = lat >= 0 ? 'N' : 'S';
  const EW   = lon >= 0 ? 'E' : 'W';
  const body = `GPRMC,${tsToNmeaTime(ts)},A,${formatDDMM(lat, true)},${NS},` +
               `${formatDDMM(lon, false)},${EW},${speedKnots.toFixed(1)},` +
               `${brng.toFixed(1)},${tsToNmeaDate(ts)},,`;
  return `$${body}*${nmeaChecksum(body)}`;
}

function makeGPGGA(ts, lat, lon, hdop, sats) {
  const NS   = lat >= 0 ? 'N' : 'S';
  const EW   = lon >= 0 ? 'E' : 'W';
  const body = `GPGGA,${tsToNmeaTime(ts)},${formatDDMM(lat, true)},${NS},` +
               `${formatDDMM(lon, false)},${EW},1,${String(sats).padStart(2, '0')},` +
               `${hdop.toFixed(1)},10.0,M,0.0,M,,`;
  return `$${body}*${nmeaChecksum(body)}`;
}

// ─── AR(1) 雜訊產生器 ────────────────────────────────────────────────────────

class AR1Noise {
  constructor(sigma) {
    this.sigma = sigma;
    this.prev  = 0;
    this.drift = 0;
  }
  next() {
    this.prev  = AR1_ALPHA * this.prev + Math.sqrt(1 - AR1_ALPHA ** 2) * randn() * this.sigma;
    this.drift = DRIFT_DECAY * this.drift + (1 - DRIFT_DECAY) * randn() * this.sigma * 0.5;
    return this.prev + this.drift;
  }
}

// ─── 路線前處理 ──────────────────────────────────────────────────────────────

function buildSegments(routePoints, stops, lights) {
  const stopSet  = new Set(stops);
  const lightSet = new Set(lights || []);
  return routePoints.slice(0, -1).map((from, i) => {
    const to = routePoints[i + 1];
    return {
      from,
      to,
      dist:        haversine(from, to),
      bearing:     bearing(from, to),
      stopBefore:  stopSet.has(i + 1),
      lightBefore: lightSet.has(i + 1),
    };
  });
}

// ─── 速度工具 ────────────────────────────────────────────────────────────────

const CRUISE_MS = CRUISE_KMH / 3.6;
const MAX_MS    = MAX_KMH / 3.6;

function dwellSeconds() {
  return Math.max(2, Math.min(Math.round(STOP_DWELL_S + randn() * 3), STOP_DWELL_S + 10));
}

// ─── 主模擬 ──────────────────────────────────────────────────────────────────

function simulate(route, cfg) {
  const { route_points, stops, traffic_lights } = route;
  const segs      = buildSegments(route_points, stops, traffic_lights || []);
  const totalDist = segs.reduce((s, g) => s + g.dist, 0);

  const { hdop, sigmaM, sats } = cfg;
  const noiseN = new AR1Noise(sigmaM);
  const noiseE = new AR1Noise(sigmaM);

  const jumpDistTarget = totalDist * 0.30;
  let   jumpInjected   = false;

  const OUTAGE_SEG_START = 15;
  const OUTAGE_SEG_END   = 17;

  const nmeaLines   = [];
  const groundTruth = [];

  let ts            = BASE_TS;
  let travelledDist = 0;
  let stopSeqIdx    = 0;

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
    const seg         = segs[si];
    const isOutage    = cfg.outage && si >= OUTAGE_SEG_START && si <= OUTAGE_SEG_END;
    const prevSeg     = si > 0 ? segs[si - 1] : null;

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
    let traveled   = 0;
    let speedMs    = CRUISE_MS * 0.4;

    while (traveled < seg.dist) {
      const remaining = seg.dist - traveled;
      const progress  = traveled / seg.dist;

      // 目標速度
      let targetMs;
      if (willSlow && progress >= SLOW_START) {
        const blend = (progress - SLOW_START) / (1 - SLOW_START);
        targetMs    = CRUISE_MS * (1 - blend) + CRUISE_MS * SLOW_RATIO * blend;
      } else {
        targetMs = Math.min(CRUISE_MS + randn() * 1.5, MAX_MS);
      }
      targetMs = Math.max(0.5, targetMs);

      if (speedMs < targetMs) speedMs = Math.min(speedMs + ACCEL_MS2, targetMs);
      else                    speedMs = Math.max(speedMs - DECEL_MS2, targetMs);

      const step = Math.min(speedMs, remaining);
      traveled      += step;
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

// ─── 入口 ────────────────────────────────────────────────────────────────────

function main() {
  const args = parseArgs(process.argv);

  const cfg = SCENARIOS[args.scenario];
  if (!cfg) {
    console.error(`未知場景：${args.scenario}。可用：${Object.keys(SCENARIOS).join(', ')}`);
    process.exit(1);
  }

  let route;
  try {
    route = JSON.parse(fs.readFileSync(args.route, 'utf8'));
  } catch (e) {
    console.error(`無法讀取路線檔案 "${args.route}"：${e.message}`);
    process.exit(1);
  }

  if (!Array.isArray(route.route_points) || route.route_points.length < 2) {
    console.error('route.json：route_points 至少需要 2 個節點');
    process.exit(1);
  }
  if (!Array.isArray(route.stops)) {
    console.error('route.json：缺少 stops 欄位');
    process.exit(1);
  }

  console.log(`場景：${args.scenario} | 節點：${route.route_points.length} | 站點：${route.stops.length}`);

  const { nmeaLines, groundTruth } = simulate(route, cfg);

  fs.writeFileSync(args.outNmea, nmeaLines.join('\n') + '\n', 'utf8');
  console.log(`✓ NMEA  → ${args.outNmea}  (${nmeaLines.length} 行)`);

  fs.writeFileSync(args.outGt, JSON.stringify(groundTruth, null, 2), 'utf8');
  console.log(`✓ GT    → ${args.outGt}  (${groundTruth.length} 次停靠)`);
}

main();
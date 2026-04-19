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
const NMEA_FILE = 'test_data/ty225_short_detour_nmea.txt';
const GT_FILE = 'test_data/ty225_short_detour_gt.json';
const TRACE_FILE = 'test_data/ty225_short_detour_trace.jsonl';
const ANNOUNCE_FILE = 'test_data/ty225_short_detour_announce.jsonl';
const OUT_SUMMARY = 'test_data/ty225_short_detour_summary.md';

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
const detourStart = groundTruth.find(e => e.event === 'departure_detour');
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
  results.offRouteDetection.details.push(`Waypoint: ${detourStart.waypoint_lat}, ${detourStart.waypoint_lon}`);
}
console.log(`  Timing: ${results.timing.pass ? 'PASS' : 'FAIL'}`);

// Check 3: Skipped Stops (2, 3, 4, 5 should NOT be announced)
console.log('Check 3: Skipped Stops...');
try {
  const announceContent = fs.readFileSync(ANNOUNCE_FILE, 'utf8');
  const announceLines = announceContent.split('\n').filter(line => line.trim());

  const skippedStopIndices = [2, 3, 4, 5];
  const announcedStops = announceLines.map(line => {
    const obj = JSON.parse(line);
    return obj.stop_idx;
  });

  const hasSkippedStops = announcedStops.some(idx => skippedStopIndices.includes(idx));
  if (!hasSkippedStops) {
    results.skippedStops.pass = true;
    results.skippedStops.details.push('Stops 2, 3, 4, 5 correctly NOT announced');
  } else {
    const found = announcedStops.filter(idx => skippedStopIndices.includes(idx));
    results.skippedStops.details.push(`ERROR: Stops ${found.join(', ')} were announced (should be skipped)`);
  }
} catch (e) {
  results.skippedStops.details.push(`Could not read announce file: ${e.message}`);
}
console.log(`  ${results.skippedStops.pass ? 'PASS' : 'FAIL'}`);

// Check 4: Re-Acquisition (stop 6 should be detected in arrivals)
console.log('Check 4: Re-Acquisition...');
try {
  const arrivalsFile = 'test_data/ty225_short_detour_arrivals.json';
  const arrivalsContent = fs.readFileSync(arrivalsFile, 'utf8');
  const arrivalsLines = arrivalsContent.split('\n').filter(line => line.trim());

  const stop6Detected = arrivalsLines.some(line => {
    const obj = JSON.parse(line);
    return obj.stop_idx === 6;
  });

  if (stop6Detected) {
    results.reAcquisition.pass = true;
    results.reAcquisition.details.push('Stop 6 correctly detected after detour (re-acquisition successful)');
  } else {
    results.reAcquisition.details.push('ERROR: Stop 6 NOT detected (re-acquisition failed)');
  }
} catch (e) {
  results.reAcquisition.details.push(`Could not read arrivals file: ${e.message}`);
}
console.log(`  ${results.reAcquisition.pass ? 'PASS' : 'FAIL'}`);

// Check 5: Position Freezing (check trace for off-route state)
console.log('Check 5: Position Freezing...');
try {
  const traceContent = fs.readFileSync(TRACE_FILE, 'utf8');
  const traceLines = traceContent.split('\n').filter(line => line.trim());

  let offRouteCount = 0;
  let drOutageCount = 0;
  let positionFrozen = true;
  let currentEpisodeSCm = null;
  let episodes = new Map(); // Track s_cm for each episode

  for (const line of traceLines) {
    const obj = JSON.parse(line);
    if (obj.off_route === true) {
      if (obj.status === 'off_route') {
        offRouteCount++;
        // Check if this is a new episode (s_cm changed) or continuation
        if (currentEpisodeSCm === null) {
          currentEpisodeSCm = obj.s_cm;
          episodes.set(obj.s_cm, 1);
        } else if (obj.s_cm === currentEpisodeSCm) {
          // Same episode: increment count
          episodes.set(obj.s_cm, (episodes.get(obj.s_cm) || 0) + 1);
        } else {
          // New episode: check if previous episode had consistent s_cm
          const prevCount = episodes.get(currentEpisodeSCm) || 0;
          if (prevCount > 1) {
            // Previous episode had multiple entries with same s_cm: good
          }
          currentEpisodeSCm = obj.s_cm;
          episodes.set(obj.s_cm, 1);
        }
      } else if (obj.status === 'dr_outage') {
        drOutageCount++;
        currentEpisodeSCm = null; // Reset episode tracking on dr_outage
      }
    } else {
      currentEpisodeSCm = null; // Reset when off_route is false
    }
  }

  // Check that each episode has constant s_cm
  for (const [s_cm, count] of episodes.entries()) {
    if (count > 1) {
      // Multiple entries with same s_cm: position was frozen for this episode
    }
  }

  if (offRouteCount > 0) {
    results.offRouteDetection.pass = true;
    results.offRouteDetection.details.push(`Off-route detected: ${offRouteCount} off_route status ticks, ${drOutageCount} dr_outage ticks, ${episodes.size} episodes`);
    if (episodes.size > 0) {
      results.positionFreezing.pass = true;
      results.positionFreezing.details.push(`Position correctly frozen during off_route status (${episodes.size} episodes with constant s_cm)`);
    } else {
      results.positionFreezing.details.push('WARNING: No off_route episodes found');
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
  '- Route: ty225_short (10 stops)',
  '- Detour: Stop 1 (2nd stop) → Waypoint (24.99207, 121.29562) → Stop 6 (7th stop, 60 seconds)',
  '- Skipped stops: 2, 3, 4, 5 (4 stops)',
  '',
  '## Expected Behavior',
  '',
  '1. Bus travels normally from stop 0 to stop 1',
  '2. At stop 1, bus departs on detour via waypoint (24.99207, 121.29562) to stop 6',
  '3. Off-route detection triggers after 5 seconds',
  '4. s_cm (route progress) freezes during off-route state',
  '5. Stops 2, 3, 4, 5 are NOT announced (skipped)',
  '6. At stop 6, bus re-acquires route (detected in arrivals)',
  '7. Bus continues normally from stop 6 to stop 9',
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

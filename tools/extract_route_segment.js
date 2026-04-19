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
let lastRouteIdx = 0;

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
for (let i = 0; i < routeData.route_points.length; i++) {
  const [lat, lon] = routeData.route_points[i];
  const dist = Math.hypot(lat - lastStop.lat, lon - lastStop.lon);
  if (dist < minDist) {
    minDist = dist;
    lastRouteIdx = i;
  }
}

console.log(`Route segment: points ${firstRouteIdx} to ${lastRouteIdx}`);

// Extract route segment (handle circular route - if last < first, route wraps around)
let extractedRoute;
if (lastRouteIdx >= firstRouteIdx) {
  // Normal case: segment is contiguous
  extractedRoute = routeData.route_points.slice(firstRouteIdx, lastRouteIdx + 1);
} else {
  // Circular case: concatenate from first to end, then start to last
  const firstPart = routeData.route_points.slice(firstRouteIdx);
  const secondPart = routeData.route_points.slice(0, lastRouteIdx + 1);
  extractedRoute = firstPart.concat(secondPart);
}

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

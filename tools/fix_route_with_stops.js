#!/usr/bin/env node
/**
 * fix_route_with_stops.js — Insert physical stop locations into route
 * Ensures route geometry matches stop locations for accurate M12 recovery
 */

'use strict';

const fs = require('fs');

// Input files
const ROUTE_FILE = 'test_data/ty225_short_route.json';
const STOPS_FILE = 'test_data/ty225_short_stops.json';

// Output file
const OUT_ROUTE = 'test_data/ty225_short_fixed_route.json';

function toRad(deg) { return deg * Math.PI / 180; }
function toDeg(rad) { return rad * 180 / Math.PI; }

function haversine(lat1, lon1, lat2, lon2) {
  const EARTH_R = 6_371_000;
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

// Load route and stops
const route = JSON.parse(fs.readFileSync(ROUTE_FILE, 'utf8'));
const stopsData = JSON.parse(fs.readFileSync(STOPS_FILE, 'utf8'));
const stops = stopsData.stops;

console.log(`Original route: ${route.route_points.length} points`);
console.log(`Stops: ${stops.length}`);

// Build new route with stops inserted
const newRoutePoints = [];
const processedStops = new Set();

// Add each original route point
for (let i = 0; i < route.route_points.length; i++) {
  const rp = route.route_points[i];
  newRoutePoints.push(rp);

  // Check if any stop should be inserted after this route point
  for (let s = 0; s < stops.length; s++) {
    if (processedStops.has(s)) continue;

    const stop = stops[s];
    const distToRP = haversine(rp[0], rp[1], stop.lat, stop.lon);

    // If stop is close to this route point (< 50m), replace it
    if (distToRP < 50) {
      console.log(`Stop ${s + 1} (${stop.lat}, ${stop.lon}) is ${distToRP.toFixed(0)}m from route point ${i} - replacing`);
      newRoutePoints[newRoutePoints.length - 1] = [stop.lat, stop.lon];
      processedStops.add(s);
    }
  }

  // Check if any stop should be inserted between this route point and the next
  if (i < route.route_points.length - 1) {
    const nextRP = route.route_points[i + 1];
    const segBearing = bearing(rp[0], rp[1], nextRP[0], nextRP[1]);
    const segDist = haversine(rp[0], rp[1], nextRP[0], nextRP[1]);

    for (let s = 0; s < stops.length; s++) {
      if (processedStops.has(s)) continue;

      const stop = stops[s];
      const distFromRP = haversine(rp[0], rp[1], stop.lat, stop.lon);
      const distToNext = haversine(stop.lat, stop.lon, nextRP[0], nextRP[1]);
      const bearingToStop = bearing(rp[0], rp[1], stop.lat, stop.lon);

      // Check if stop is roughly aligned with segment
      const bearingDiff = Math.abs(bearingToStop - segBearing);
      const bearingDiffComplement = Math.abs(bearingToStop - (segBearing + 180) % 360);

      // If stop is on the path between route points (within 30 degrees and distance adds up)
      if ((bearingDiff < 30 || bearingDiffComplement < 30) &&
          Math.abs(distFromRP + distToNext - segDist) < 50) {
        console.log(`Stop ${s + 1} (${stop.lat}, ${stop.lon}) is on segment ${i}-${i + 1} - inserting`);
        newRoutePoints.push([stop.lat, stop.lon]);
        processedStops.add(s);
      }
    }
  }
}

// Add any remaining stops that weren't inserted
for (let s = 0; s < stops.length; s++) {
  if (!processedStops.has(s)) {
    const stop = stops[s];
    console.log(`Stop ${s + 1} (${stop.lat}, ${stop.lon}) not matched - appending`);
    newRoutePoints.push([stop.lat, stop.lon]);
  }
}

console.log(`\nNew route: ${newRoutePoints.length} points`);

// Find stop indices in new route
const stopIndices = [];
let searchStartIdx = 0;

for (const stop of stops) {
  let minDist = Infinity;
  let minIdx = searchStartIdx;

  for (let i = searchStartIdx; i < newRoutePoints.length; i++) {
    const dist = haversine(stop.lat, stop.lon, newRoutePoints[i][0], newRoutePoints[i][1]);
    if (dist < minDist) {
      minDist = dist;
      minIdx = i;
    }
  }

  stopIndices.push(minIdx);
  searchStartIdx = minIdx + 1;
}

console.log(`Stop indices: ${stopIndices}`);

// Write output
const output = {
  route_points: newRoutePoints,
  stops: stopIndices
};

fs.writeFileSync(OUT_ROUTE, JSON.stringify(output, null, 2));
console.log(`\nWrote fixed route to ${OUT_ROUTE}`);

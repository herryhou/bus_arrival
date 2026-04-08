#!/usr/bin/env node
/**
 * Regenerate route points from stops in traveling order
 * This ensures the route follows the actual bus path
 */

const fs = require('fs');

function haversine(lat1, lon1, lat2, lon2) {
  const R = 6371000;
  const dLat = (lat2 - lat1) * Math.PI / 180;
  const dLon = (lon2 - lon1) * Math.PI / 180;
  const a = Math.sin(dLat/2) * Math.sin(dLat/2) +
            Math.cos(lat1 * Math.PI / 180) * Math.cos(lat2 * Math.PI / 180) *
            Math.sin(dLon/2) * Math.sin(dLon/2);
  return R * 2 * Math.atan2(Math.sqrt(a), Math.sqrt(1-a));
}

function bearing(lat1, lon1, lat2, lon2) {
  const toRad = deg => deg * Math.PI / 180;
  const dLon = toRad(lon2 - lon1);
  const y = Math.sin(dLon) * Math.cos(toRad(lat2));
  const x = Math.cos(toRad(lat1)) * Math.sin(toRad(lat2)) -
            Math.sin(toRad(lat1)) * Math.cos(toRad(lat2)) * Math.cos(dLon);
  return (Math.atan2(y, x) * 180 / Math.PI + 360) % 360;
}

function addIntermediatePoints(from, to, maxSpacing) {
  const points = [[from[0], from[1]]];
  const dist = haversine(from[0], from[1], to[0], to[1]);
  const numSegments = Math.max(1, Math.ceil(dist / maxSpacing));

  for (let i = 1; i < numSegments; i++) {
    const frac = i / numSegments;
    const lat = from[0] + (to[0] - from[0]) * frac;
    const lon = from[1] + (to[1] - from[1]) * frac;
    points.push([lat, lon]);
  }

  points.push([to[0], to[1]]);
  return points;
}

// Main
const stopsFile = process.argv[2];
const outFile = process.argv[3];
const maxSpacing = 100; // meters between route points

if (!stopsFile || !outFile) {
  console.error('Usage: node regenerate_route_from_stops.js <stops.json> <output_route.json>');
  process.exit(1);
}

const stopsData = JSON.parse(fs.readFileSync(stopsFile, 'utf8'));
const stops = stopsData.stops;

console.log(`Processing ${stops.length} stops...`);

// Check if this is a loop route (last stop close to first stop)
const distToStart = haversine(
  stops[stops.length - 1].lat, stops[stops.length - 1].lon,
  stops[0].lat, stops[0].lon
);
const isLoop = distToStart < 200;
console.log(`Loop route: ${isLoop ? 'YES' : 'NO'} (distance back to start: ${distToStart.toFixed(0)}m)`);

// Generate route points following stops in order
const routePoints = [];

for (let i = 0; i < stops.length; i++) {
  const from = [stops[i].lat, stops[i].lon];

  // Determine next stop
  let to;
  if (i === stops.length - 1 && isLoop) {
    // For loop routes, connect back to first stop
    to = [stops[0].lat, stops[0].lon];
  } else if (i < stops.length - 1) {
    to = [stops[i + 1].lat, stops[i + 1].lon];
  } else {
    // Not a loop and we're at the last stop - skip
    continue;
  }

  // Add intermediate points between stops
  const segmentPoints = addIntermediatePoints(from, to, maxSpacing);

  // Add all points except the first one (to avoid duplication)
  const startIdx = routePoints.length === 0 ? 0 : 1;
  for (let j = startIdx; j < segmentPoints.length; j++) {
    routePoints.push(segmentPoints[j]);
  }
}

console.log(`Generated ${routePoints.length} route points`);

// Calculate total route distance
let totalDist = 0;
for (let i = 0; i < routePoints.length - 1; i++) {
  totalDist += haversine(routePoints[i][0], routePoints[i][1],
                         routePoints[i+1][0], routePoints[i+1][1]);
}
console.log(`Total route distance: ${(totalDist/1000).toFixed(2)}km`);

// Verify stops are now in order along the route
console.log('\\nVerifying stop order...');
const stopPositions = stops.map(stop => {
  let minDist = Infinity;
  let minIdx = 0;
  for (let i = 0; i < routePoints.length; i++) {
    const dist = haversine(stop.lat, stop.lon, routePoints[i][0], routePoints[i][1]);
    if (dist < minDist) {
      minDist = dist;
      minIdx = i;
    }
  }
  return { idx: minIdx, dist: minDist };
});

let violations = 0;
for (let i = 1; i < stopPositions.length; i++) {
  if (stopPositions[i].idx < stopPositions[i-1].idx) {
    violations++;
    console.error(`  VIOLATION: Stop ${i} (route point ${stopPositions[i].idx}) comes before Stop ${i-1} (route point ${stopPositions[i-1].idx})`);
  }
}

if (violations === 0) {
  console.log('✓ All stops are in correct order along route!');
} else {
  console.error(`✗ Found ${violations} ordering violations`);
  process.exit(1);
}

// Write output
const output = {
  route_points: routePoints,
  stops: stops.map((_, i) => i) // Stop indices will be rematched
};

fs.writeFileSync(outFile, JSON.stringify(output, null, 2));
console.log(`\\nRoute written to ${outFile}`);

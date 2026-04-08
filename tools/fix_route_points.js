#!/usr/bin/env node
/**
 * Fix route points by generating them from stops
 * This ensures the route passes through all stop locations in traveling order
 */

const fs = require('fs');

// Read stops file
const stopsData = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const routeData = JSON.parse(fs.readFileSync(process.argv[3], 'utf8'));

function haversine(lat1, lon1, lat2, lon2) {
  const R = 6371000;
  const dLat = (lat2 - lat1) * Math.PI / 180;
  const dLon = (lon2 - lon1) * Math.PI / 180;
  const a = Math.sin(dLat/2) * Math.sin(dLat/2) +
            Math.cos(lat1 * Math.PI / 180) * Math.cos(lat2 * Math.PI / 180) *
            Math.sin(dLon/2) * Math.sin(dLon/2);
  return R * 2 * Math.atan2(Math.sqrt(a), Math.sqrt(1-a));
}

// Generate route points from stops with interpolation
const fixedRoutePoints = [];
const stops = stopsData.stops;

// Add intermediate points between stops for smoother route
for (let i = 0; i < stops.length; i++) {
  const stop = stops[i];

  // For loop routes, connect last stop back to first
  let nextStop;
  if (i === stops.length - 1) {
    // Check if this is a loop route (last stop close to first stop)
    const distToStart = haversine(stop.lat, stop.lon, stops[0].lat, stops[0].lon);
    if (distToStart < 100) {
      // This is a loop - add closing segment back to first stop
      nextStop = stops[0];
    } else {
      // Not a loop - skip
      continue;
    }
  } else {
    nextStop = stops[i + 1];
  }

  const dist = haversine(stop.lat, stop.lon, nextStop.lat, nextStop.lon);

  // Add intermediate points BEFORE the stop (approaching the stop)
  // The stop itself is added AFTER the intermediate points
  const numPoints = Math.min(Math.floor(dist / 100), 5);
  for (let j = 1; j <= numPoints; j++) {
    const frac = j / (numPoints + 2);  // Stop the interpolation before reaching the stop
    const midLat = stop.lat + (nextStop.lat - stop.lat) * frac;
    const midLon = stop.lon + (nextStop.lon - stop.lon) * frac;
    fixedRoutePoints.push([midLat, midLon]);
  }

  // Add the exact stop location as a route point
  fixedRoutePoints.push([stop.lat, stop.lon]);
}

// Add the final stop location if not already added (for loop closure)
const lastStop = stops[stops.length - 1];
const firstStop = stops[0];
const distToStart = haversine(lastStop.lat, lastStop.lon, firstStop.lat, firstStop.lon);
if (distToStart < 100) {
  // This is a loop - add the first stop again to close the loop
  fixedRoutePoints.push([firstStop.lat, firstStop.lon]);
}

// Update route data
routeData.route_points = fixedRoutePoints;

// Write fixed route
const outFile = process.argv[4];
fs.writeFileSync(outFile, JSON.stringify(routeData, null, 2));
console.log('Fixed route written to', outFile);
console.log('Original route points:', routeData.route_points.length);
console.log('Fixed route points:', fixedRoutePoints.length);

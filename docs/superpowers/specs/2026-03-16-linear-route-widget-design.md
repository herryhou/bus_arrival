# Linear Route Widget and Map Enhancements Design

**Date:** 2026-03-16
**Status:** Approved
**Author:** Claude (with user input)
**Version:** 2.0 (Reviewed and Approved)

## Overview

This document describes the design for adding a linear route visualization widget and map enhancements to the bus arrival visualizer. The goal is to provide better spatial context for the bus position, stop locations, and event visualization.

## Requirements

1. **Linear Route Widget:** A 1D visualization showing all stops, distance ticks, and current bus position
2. **Bus Position Indicator:** Show current snap point with bus icon on the linear widget
3. **Current Segment Highlight:** Visual indication of which route segment the bus is currently on
4. **Event Click Integration:** When clicking an event, pan to stop and show event point with FSM-state-specific colors
5. **50m Circles:** Show a 50m radius circle around every stop on the map

## Important Conventions

### Stop Indexing
- **Stop indices are 0-indexed** and correspond to the array position in `routeData.stops[]`
- The `stop_idx` field in `StopTraceState` matches the array index
- When rendering, display the array index as the stop number

### Coordinate System
- **Progress (cm):** Linear distance along route from start, stored in `cum_dist_cm` (nodes) and `progress_cm` (stops)
- **Coordinates (lat, lon):** Geographic positions, computed from `x_cm, y_cm` via projection
- **Bus position:** Use `currentRecord.s_cm` directly (already interpolated by arrival detector)

### Segment Definition
- **Segments are intervals between route nodes**, not stops
- A segment spans from `nodes[i].cum_dist_cm` to `nodes[i+1].cum_dist_cm`
- The "current segment" is the one containing `busProgress`

## Architecture

### New Component

- **LinearRouteWidget.svelte** - Canvas-based 1D route visualization

### Enhanced Components

- **MapView.svelte** - Add 50m circles, event markers, expose pan-to-stop method
- **EventLog.svelte** - Emit detailed event data (stopIdx, state) on click
- **+page.svelte** - Coordinate interactions and manage highlighted event state

### Data Flow

```
EventLog.click → +page.svelte → {
  MapView.panToStop(stopIdx)
  MapView.showEventMarker(stopIdx, state, color)
  LinearRouteWidget.highlightEvent(stopIdx, state)
}
```

## Component: LinearRouteWidget

### Props

```typescript
interface Props {
  routeData: RouteData;           // For stop positions and total length
  busProgress: number;            // Current s_cm from TraceRecord
  highlightedEvent?: {            // Optional event to highlight
    stopIdx: number;
    state: FsmState;
  } | null;
}
```

### Visual Elements

1. **Base Line** - Horizontal line from 0 to route length
   - Height: 4px, color: #333
   - Y-position: Middle of canvas

2. **Distance Ticks** - Every 100m (10,000 cm)
   - Minor tick: 4px height, every 100m
   - Major tick: 8px height + label, every 1000m (1km)
   - Labels show distance in km

3. **Stop Indicators** - Vertical lines at each stop's progress_cm
   - Height: 12px (6px above/below base line)
   - Color: #ef4444 (red)
   - Stop number label above each stop

4. **Bus Position** - Snap point on the line
   - 🚌 emoji icon
   - Positioned at current busProgress
   - Updates in real-time

5. **Current Segment** - Highlighted region between route nodes
   - Semi-transparent overlay (rgba(59, 130, 246, 0.2))
   - Shows active segment the bus is traveling on

6. **Event Highlight** - When event is clicked
   - Vertical marker line at stop's position
   - Color-coded dot based on FsmState

### Canvas Rendering

```typescript
// Scaling with edge case handling
const padding = { left: 20, right: 20 };
const drawWidth = canvas.width - padding.left - padding.right;

// Guard against edge cases
const maxProgress = routeData.nodes.length > 0
  ? routeData.nodes[routeData.nodes.length - 1].cum_dist_cm
  : 100000; // fallback: 1km

const scale = maxProgress > 0 ? drawWidth / maxProgress : 1;

function progressToX(progressCm: number): number {
  return padding.left + Math.min(progressCm, maxProgress) * scale;
}
```

### Helper Function - Find Current Segment

Segments are defined as intervals between route nodes. This function finds which segment contains a given progress value.

```typescript
function findSegment(progressCm: number): { start: number; end: number } | null {
  // Handle edge cases
  if (routeData.nodes.length < 2) return null;
  if (progressCm < routeData.nodes[0].cum_dist_cm) return null;
  if (progressCm >= routeData.nodes[routeData.nodes.length - 1].cum_dist_cm) {
    // Bus is at or past the last node - return the last segment
    const lastIdx = routeData.nodes.length - 2;
    return {
      start: routeData.nodes[lastIdx].cum_dist_cm,
      end: routeData.nodes[lastIdx + 1].cum_dist_cm
    };
  }

  // Find the segment containing progressCm
  for (let i = 0; i < routeData.nodes.length - 1; i++) {
    const node = routeData.nodes[i];
    const nextNode = routeData.nodes[i + 1];
    if (progressCm >= node.cum_dist_cm && progressCm < nextNode.cum_dist_cm) {
      return { start: node.cum_dist_cm, end: nextNode.cum_dist_cm };
    }
  }
  return null;
}
```

## Component: MapView Enhancements

### Utility Functions

#### metersToPixels
Convert meters to pixels at a given latitude and zoom level for MapLibre GL circle radius.

```typescript
const EARTH_RADIUS = 6378137; // meters

function metersToPixels(meters: number, lat: number, zoom: number): number {
  const latRad = lat * Math.PI / 180;
  const metersPerPixel = EARTH_RADIUS * Math.cos(latRad) / (256 * Math.pow(2, zoom));
  return meters / metersPerPixel;
}
```

#### interpolateStopPosition
Get lat/lon for a stop by interpolating its progress_cm along the route nodes.

```typescript
import { getRouteGeometry, projectCmToLatLon } from '$lib/parsers/routeData';

function getStopLatLon(stopProgressCm: number, routeData: RouteData): [number, number] | null {
  // Find the segment containing this stop
  for (let i = 0; i < routeData.nodes.length - 1; i++) {
    const node = routeData.nodes[i];
    const nextNode = routeData.nodes[i + 1];

    if (stopProgressCm >= node.cum_dist_cm && stopProgressCm <= nextNode.cum_dist_cm) {
      // Interpolate between nodes
      const segmentProgress = (stopProgressCm - node.cum_dist_cm) / (nextNode.cum_dist_cm - node.cum_dist_cm);

      // Interpolate x, y coordinates
      const x = node.x_cm + segmentProgress * (nextNode.x_cm - node.x_cm);
      const y = node.y_cm + segmentProgress * (nextNode.y_cm - node.y_cm);

      // Project to lat/lon
      return projectCmToLatLon(x, y, routeData.grid_origin, routeData.lat_avg_deg);
    }
  }
  return null;
}
```

### 50m Circles Layer

Using MapLibre GL's circle layer with zoom-based radius interpolation:

```typescript
map.addLayer({
  id: 'stops-50m-circles',
  type: 'circle',
  source: stopsSourceId,
  paint: {
    'circle-radius': [
      'interpolate',
      ['linear'],
      ['zoom'],
      12, metersToPixels(50, routeData.lat_avg_deg, 12),
      16, metersToPixels(50, routeData.lat_avg_deg, 16),
      18, metersToPixels(50, routeData.lat_avg_deg, 18)
    ],
    'circle-color': '#3b82f6',
    'circle-opacity': 0.15,
    'circle-stroke-width': 1,
    'circle-stroke-color': '#3b82f6',
    'circle-stroke-opacity': 0.3
  }
});
```

**Layer ordering:** Route line → 50m circles → Event marker (when shown) → Stop circles → Bus marker (topmost)

### Reactive Event Marker Updates

When `highlightedEvent` prop changes, update the event marker position and color:

```typescript
$effect(() => {
  if (!map || !mapLoaded) return;

  if (!highlightedEvent) {
    // Clear event marker
    (map.getSource('event-marker') as maplibregl.GeoJSONSource).setData({
      type: 'FeatureCollection',
      features: []
    });
    return;
  }

  // Get stop data
  const stop = routeData.stops[highlightedEvent.stopIdx];
  if (!stop) return;

  // Interpolate lat/lon for this stop
  const latLon = getStopLatLon(stop.progress_cm, routeData);
  if (!latLon) return;

  const color = FSM_STATE_COLORS[highlightedEvent.state];

  // Update event marker
  (map.getSource('event-marker') as maplibregl.GeoJSONSource).setData({
    type: 'FeatureCollection',
    features: [{
      type: 'Feature',
      properties: { color },
      geometry: {
        type: 'Point',
        coordinates: [latLon[1], latLon[0]] // [lon, lat] for MapLibre
      }
    }]
  });
});
```

### Event Marker System

**New Props:**
```typescript
interface Props {
  // ... existing
  highlightedEvent?: {
    stopIdx: number;
    state: FsmState;
    time: number;
  } | null;
}
```

**Dynamic event marker source:**
```typescript
map.addSource('event-marker', {
  type: 'geojson',
  data: { type: 'FeatureCollection', features: [] }
});

map.addLayer({
  id: 'event-marker-pulse',
  type: 'circle',
  source: 'event-marker',
  paint: {
    'circle-radius': 20,
    'circle-color': ['get', 'color'],
    'circle-opacity': 0.3
  },
  // Place below the main event marker
  'before': 'stops-circle'
});

map.addLayer({
  id: 'event-marker',
  type: 'circle',
  source: 'event-marker',
  paint: {
    'circle-radius': 12,
    'circle-color': ['get', 'color'],
    'circle-stroke-width': 3,
    'circle-stroke-color': '#ffffff',
    'circle-opacity': 1
  },
  // Place above stops but below bus marker
  'before': 'bus-marker'
});
```

### Exported panToStop Function

Using Svelte's `$state` with `$effect` for external function calls. Note that stops need coordinate interpolation since they only store progress_cm.

```typescript
let currentPanTarget = $state<number | null>(null);

export function panToStop(stopIdx: number) {
  currentPanTarget = stopIdx;
}

$effect(() => {
  if (!map || !mapLoaded || currentPanTarget === null) return;

  const stop = routeData.stops[currentPanTarget];
  if (!stop) return;

  // Interpolate lat/lon for this stop
  const latLon = getStopLatLon(stop.progress_cm, routeData);
  if (!latLon) return;

  map.easeTo({
    center: [latLon[1], latLon[0]], // [lon, lat] for MapLibre
    zoom: 16,
    duration: 500
  });

  currentPanTarget = null;
});
```

## Component: EventLog Enhancements

### Updated Props

```typescript
interface Props {
  traceData: TraceData;
  onSeek: (time: number) => void;
  onEventClick?: (event: ExtendedEventInfo) => void;
}

interface ExtendedEventInfo {
  time: number;
  stopIdx?: number;
  state?: FsmState;
}
```

### Updated Click Handler

```typescript
<button onclick={() => {
  onSeek(event.time);
  if (event.stopIdx !== undefined && event.state) {
    onEventClick?.({
      time: event.time,
      stopIdx: event.stopIdx,
      state: event.state
    });
  }
}}>
```

## Page Layout Changes

### Dashboard Grid Structure

```css
.dashboard-grid {
  flex: 1;
  display: grid;
  grid-template-columns: 1.5fr 1.5fr 1fr;
  grid-template-rows: 1fr auto;
  gap: 1px;
  background-color: #333;
}

.linear-route-panel {
  grid-column: 1 / -1;
  height: 80px;
}
```

### New State in +page.svelte

```typescript
let highlightedEvent = $state<{
  stopIdx: number;
  time: number;
  state: FsmState;
} | null>(null);

let mapViewRef: { panToStop: (idx: number) => void } | null = null;
```

### Event Handler

```typescript
function handleEventClick(info: { time: number; stopIdx?: number; state?: FsmState }) {
  if (info.stopIdx !== undefined && info.state) {
    highlightedEvent = {
      stopIdx: info.stopIdx,
      time: info.time,
      state: info.state
    };
    mapViewRef?.panToStop(info.stopIdx);
  }
}

function clearHighlight() {
  highlightedEvent = null;
}
```

### Clear Highlight Mechanism

Users can clear the event highlight by:
1. Clicking on the map (outside of stop markers)
2. Pressing the Escape key
3. Selecting a different stop

Add to MapView.svelte:
```typescript
map.on('click', (e) => {
  // Check if click was on a stop
  const features = map.queryRenderedFeatures(e.point, { layers: ['stops-circle'] });
  if (features.length === 0) {
    // Click was not on a stop - clear highlight
    clearHighlight();
  }
});

// Keyboard handler
document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') {
    clearHighlight();
  }
});
```

## Constants

### FSM State Colors

**File:** `visualizer/src/lib/constants/fsmColors.ts`

```typescript
import type { FsmState } from '$lib/types';

export const FSM_STATE_COLORS: Record<FsmState, string> = {
  'Approaching': '#eab308',  // yellow
  'Arriving': '#f97316',     // orange
  'AtStop': '#22c55e',       // green
  'Departed': '#6b7280'      // gray
};

export const FSM_STATE_LABELS: Record<FsmState, string> = {
  'Approaching': 'Approaching',
  'Arriving': 'Arriving',
  'AtStop': 'At Stop',
  'Departed': 'Departed'
};
```

## Implementation Order

1. **Create fsmColors.ts** - Shared constants
2. **LinearRouteWidget.svelte** - New component
3. **MapView.svelte** - Enhancements
4. **EventLog.svelte** - Enhancements
5. **+page.svelte** - Integration and layout
6. **Testing** - Verify with ty225_v2 test data

## File Structure

```
visualizer/src/lib/
├── components/
│   ├── LinearRouteWidget.svelte  (NEW)
│   ├── MapView.svelte            (MODIFIED)
│   ├── EventLog.svelte           (MODIFIED)
│   └── ...
├── constants/
│   └── fsmColors.ts              (NEW)
└── ...

visualizer/src/routes/
└── +page.svelte                  (MODIFIED)
```

## Acceptance Criteria

### Linear Route Widget
- ✓ Canvas renders horizontal line representing full route length
- ✓ Distance ticks appear every 100m, major ticks with labels every 1km
- ✓ All stops shown as vertical red lines with index numbers
- ✓ Bus position (🚌) updates in real-time as playback progresses
- ✓ Current segment between route nodes is highlighted in blue
- ✓ Clicking an event shows a color-coded marker at the corresponding stop

### Map 50m Circles
- ✓ Semi-transparent blue circles appear around all stops
- ✓ Circles scale correctly at different zoom levels
- ✓ Circles are visible but don't obscure route line or stop markers

### Event Click Integration
- ✓ Clicking an event in EventLog:
  - Seeks playback to that time
  - Pans map to the stop location
  - Shows color-coded marker (yellow/orange/green/gray) on map
  - Shows matching highlight on linear route widget
- ✓ Highlight persists until another event is clicked or cleared
- ✓ Pressing Escape or clicking map clears the highlight

### FSM State Colors
- ✓ Approaching: Yellow (#eab308)
- ✓ Arriving: Orange (#f97316)
- ✓ AtStop: Green (#22c55e)
- ✓ Departed: Gray (#6b7280)

### Edge Cases
- ✓ Widget handles routes with only 1-2 nodes gracefully
- ✓ No crashes when stopIdx is out of bounds
- ✓ Works correctly when bus is at start (progress_cm = 0) or end of route

## Testing Checklist

Use `test_data/ty225_v2_trace.jsonl` and `test_data/ty225_v2.bin` for verification:

1. Load both files and verify linear widget renders
2. Play through the route and verify bus position tracks correctly
3. Click a TRANSITION event - verify map pans and marker appears
4. Click an ARRIVAL event - verify green marker shows
5. Verify segment highlight updates as bus moves between nodes
6. Check that 50m circles are visible on map
7. Test Escape key clears highlight
8. Test clicking map clears highlight
9. Verify zoom levels don't break 50m circle sizing

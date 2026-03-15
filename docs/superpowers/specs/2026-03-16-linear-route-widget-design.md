# Linear Route Widget and Map Enhancements Design

**Date:** 2026-03-16
**Status:** Approved
**Author:** Claude (with user input)

## Overview

This document describes the design for adding a linear route visualization widget and map enhancements to the bus arrival visualizer. The goal is to provide better spatial context for the bus position, stop locations, and event visualization.

## Requirements

1. **Linear Route Widget:** A 1D visualization showing all stops, distance ticks, and current bus position
2. **Bus Position Indicator:** Show current snap point with bus icon on the linear widget
3. **Current Segment Highlight:** Visual indication of which route segment the bus is currently on
4. **Event Click Integration:** When clicking an event, pan to stop and show event point with FSM-state-specific colors
5. **50m Circles:** Show a 50m radius circle around every stop on the map

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
// Scaling
const padding = { left: 20, right: 20 };
const drawWidth = canvas.width - padding.left - padding.right;
const scale = drawWidth / routeData.nodes[routeData.nodes.length - 1].cum_dist_cm;

function progressToX(progressCm: number): number {
  return padding.left + progressCm * scale;
}
```

### Helper Function

```typescript
function findSegment(progressCm: number): { start: number; end: number } | null {
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

**Layer ordering:** Route line → 50m circles → Stop circles → Bus marker

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
  id: 'event-marker',
  type: 'circle',
  source: 'event-marker',
  paint: {
    'circle-radius': 12,
    'circle-color': ['get', 'color'],
    'circle-stroke-width': 3,
    'circle-stroke-color': '#ffffff'
  }
});
```

### Exported panToStop Function

Using Svelte's `$state` with `$effect` for external function calls:

```typescript
let currentPanTarget = $state<number | null>(null);

export function panToStop(stopIdx: number) {
  currentPanTarget = stopIdx;
}

$effect(() => {
  if (!map || !mapLoaded || currentPanTarget === null) return;

  const stop = stops.find(s => s.index === currentPanTarget);
  if (!stop) return;

  map.easeTo({
    center: [stop.lon, stop.lat],
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

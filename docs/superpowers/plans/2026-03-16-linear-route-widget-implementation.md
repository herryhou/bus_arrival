# Linear Route Widget and Map Enhancements Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a 1D linear route visualization widget and enhance the map with 50m circles, event markers, and pan-to-stop functionality for the bus arrival visualizer.

**Architecture:** Svelte 5 with MapLibre GL JS. New LinearRouteWidget component uses Canvas API for performance. MapView gets new layers and exported panToStop function using Svelte's $state/$effect reactivity. EventLog emits additional event data on click.

**Tech Stack:** Svelte 5, TypeScript, Canvas API, MapLibre GL JS

---

## File Structure

```
visualizer/src/lib/
├── components/
│   ├── LinearRouteWidget.svelte  (NEW) - Canvas-based 1D route visualization
│   ├── MapView.svelte            (MODIFY) - Add 50m circles, event markers, panToStop export
│   ├── EventLog.svelte           (MODIFY) - Add onEventClick callback
│   └── ...
├── constants/
│   └── fsmColors.ts              (NEW) - FSM state color constants
├── parsers/
│   └── routeData.ts              (MODIFY) - Add getStopLatLon interpolation function
└── types.ts                      (existing, no changes)

visualizer/src/routes/
└── +page.svelte                  (MODIFY) - Add LinearRouteWidget, state, handlers
```

---

## Chunk 1: FSM State Colors Constants

### Task 1: Create fsmColors.ts constants file

**Files:**
- Create: `visualizer/src/lib/constants/fsmColors.ts`

- [ ] **Step 1: Create the constants file**

```typescript
// visualizer/src/lib/constants/fsmColors.ts
import type { FsmState } from '$lib/types';

/**
 * Color constants for FSM states
 * Used across LinearRouteWidget, MapView, and EventLog for consistency
 */
export const FSM_STATE_COLORS: Record<FsmState, string> = {
	'Approaching': '#eab308',  // yellow
	'Arriving': '#f97316',     // orange
	'AtStop': '#22c55e',       // green
	'Departed': '#6b7280'      // gray
};

/**
 * Human-readable labels for FSM states
 */
export const FSM_STATE_LABELS: Record<FsmState, string> = {
	'Approaching': 'Approaching',
	'Arriving': 'Arriving',
	'AtStop': 'At Stop',
	'Departed': 'Departed'
};
```

- [ ] **Step 2: Commit**

```bash
git add visualizer/src/lib/constants/fsmColors.ts
git commit -m "feat: add FSM state color constants

Add centralized color constants for FSM states to ensure
consistency across LinearRouteWidget, MapView, and EventLog.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Chunk 2: Stop Position Interpolation Utility

### Task 2: Add getStopLatLon function to routeData.ts

**Files:**
- Modify: `visualizer/src/lib/parsers/routeData.ts` (add after line 304)

- [ ] **Step 1: Add getStopLatLon function**

Add this function after `getStopPositions` (after line 304):

```typescript
/**
 * Get interpolated lat/lon for a stop by progress_cm
 *
 * Finds the route segment containing the stop and interpolates
 * the position along that segment.
 *
 * @param stopProgressCm - Stop's progress_cm value
 * @param routeData - Parsed route data
 * @returns [lat, lon] or null if stop cannot be interpolated
 */
export function getStopLatLon(
	stopProgressCm: number,
	routeData: RouteData
): [number, number] | null {
	const nodes = routeData.nodes;
	if (nodes.length < 2) return null;

	// Find the segment containing this stop
	for (let i = 0; i < nodes.length - 1; i++) {
		const node = nodes[i];
		const nextNode = nodes[i + 1];

		if (stopProgressCm >= node.cum_dist_cm && stopProgressCm <= nextNode.cum_dist_cm) {
			// Interpolate between nodes
			const seg_len = nextNode.cum_dist_cm - node.cum_dist_cm;
			if (seg_len <= 0) {
				// Fallback to start node
				const [lat, lon] = projectCmToLatLonFromOrigin(node.x_cm, node.y_cm, routeData);
				return [lat, lon];
			}

			const t = (stopProgressCm - node.cum_dist_cm) / seg_len;
			const x_cm = node.x_cm + t * (nextNode.x_cm - node.x_cm);
			const y_cm = node.y_cm + t * (nextNode.y_cm - node.y_cm);

			// Project to lat/lon
			return projectCmToLatLonFromOrigin(x_cm, y_cm, routeData);
		}
	}

	return null;
}

/**
 * Helper to project cm to lat/lon using route data's grid_origin and lat_avg_deg
 * This is needed because projectCmToLatLon doesn't take grid_origin parameter
 */
function projectCmToLatLonFromOrigin(
	x_cm: number,
	y_cm: number,
	routeData: RouteData
): [number, number] {
	// projectCmToLatLon expects coordinates relative to fixed origin (120E, 20N)
	// Nodes are already stored as absolute coordinates, so we pass them directly
	return projectCmToLatLon(x_cm, y_cm, routeData.lat_avg_deg);
}
```

Note: You'll need to add `projectCmToLatLon` to the imports if it's not already imported. Check the import section at the top of the file.

- [ ] **Step 2: Verify file compiles**

Run: `cd visualizer && npm run check`
Expected: No TypeScript errors

- [ ] **Step 3: Commit**

```bash
git add visualizer/src/lib/parsers/routeData.ts
git commit -m "feat: add stop position interpolation utility

Add getStopLatLon() function to interpolate precise lat/lon for stops
along route segments. This replaces the nearest-node approximation in
getStopPositions with true linear interpolation.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Chunk 3: Linear Route Widget Component

### Task 3: Create LinearRouteWidget.svelte component

**Files:**
- Create: `visualizer/src/lib/components/LinearRouteWidget.svelte`

- [ ] **Step 1: Create component structure**

```svelte
<!-- visualizer/src/lib/components/LinearRouteWidget.svelte -->
<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import type { RouteData, FsmState } from '$lib/types';
	import { FSM_STATE_COLORS } from '$lib/constants/fsmColors';

	interface Props {
		routeData: RouteData;
		busProgress: number; // Current s_cm from TraceRecord
		highlightedEvent?: {
			stopIdx: number;
			state: FsmState;
		} | null;
	}

	let { routeData, busProgress, highlightedEvent = null }: Props = $props();

	let canvas: HTMLCanvasElement;
	let ctx: CanvasRenderingContext2D | null = null;
	let resizeObserver: ResizeObserver | null = null;

	// Canvas dimensions
	let width = $state(0);
	let height = $state(80);

	// Constants for rendering
	const PADDING = { left: 20, right: 20 };
	const TICK_INTERVAL_M = 100; // 100m
	const MAJOR_TICK_INTERVAL_M = 1000; // 1km
	const TICK_INTERVAL_CM = TICK_INTERVAL_M * 100; // 10,000 cm
	const MAJOR_TICK_INTERVAL_CM = MAJOR_TICK_INTERVAL_M * 100; // 100,000 cm

	// Calculate scale and max progress
	const maxProgress = $derived.by(() => {
		if (routeData.nodes.length === 0) return 100000; // 1km fallback
		return routeData.nodes[routeData.nodes.length - 1].cum_dist_cm;
	});

	const scale = $derived.by(() => {
		const drawWidth = width - PADDING.left - PADDING.right;
		return maxProgress > 0 ? drawWidth / maxProgress : 1;
	});

	// Convert progress (cm) to canvas X coordinate
	function progressToX(progressCm: number): number {
		return PADDING.left + Math.min(progressCm, maxProgress) * scale;
	}

	// Find current segment based on bus progress
	function findSegment(progressCm: number): { start: number; end: number } | null {
		if (routeData.nodes.length < 2) return null;
		if (progressCm < routeData.nodes[0].cum_dist_cm) return null;
		if (progressCm >= routeData.nodes[routeData.nodes.length - 1].cum_dist_cm) {
			const lastIdx = routeData.nodes.length - 2;
			return {
				start: routeData.nodes[lastIdx].cum_dist_cm,
				end: routeData.nodes[lastIdx + 1].cum_dist_cm
			};
		}

		for (let i = 0; i < routeData.nodes.length - 1; i++) {
			const node = routeData.nodes[i];
			const nextNode = routeData.nodes[i + 1];
			if (progressCm >= node.cum_dist_cm && progressCm < nextNode.cum_dist_cm) {
				return { start: node.cum_dist_cm, end: nextNode.cum_dist_cm };
			}
		}
		return null;
	}

	// Render function
	function render() {
		if (!ctx || width === 0) return;

		const centerY = height / 2;

		// Clear canvas
		ctx.clearRect(0, 0, width, height);

		// 1. Draw base line
		ctx.strokeStyle = '#333';
		ctx.lineWidth = 4;
		ctx.beginPath();
		ctx.moveTo(PADDING.left, centerY);
		ctx.lineTo(width - PADDING.right, centerY);
		ctx.stroke();

		// 2. Draw distance ticks
		for (let d = 0; d <= maxProgress; d += TICK_INTERVAL_CM) {
			const x = progressToX(d);
			const isMajor = d % MAJOR_TICK_INTERVAL_CM === 0;
			const tickHeight = isMajor ? 8 : 4;

			ctx.strokeStyle = '#555';
			ctx.lineWidth = 1;
			ctx.beginPath();
			ctx.moveTo(x, centerY - tickHeight / 2);
			ctx.lineTo(x, centerY + tickHeight / 2);
			ctx.stroke();

			if (isMajor) {
				ctx.fillStyle = '#666';
				ctx.font = '10px JetBrains Mono, Monaco, monospace';
				ctx.textAlign = 'center';
				ctx.fillText(`${(d / 100000).toFixed(1)}km`, x, centerY - 12);
			}
		}

		// 3. Draw stop indicators
		routeData.stops.forEach((stop, index) => {
			const x = progressToX(stop.progress_cm);

			// Vertical line
			ctx.strokeStyle = '#ef4444';
			ctx.lineWidth = 2;
			ctx.beginPath();
			ctx.moveTo(x, centerY - 6);
			ctx.lineTo(x, centerY + 6);
			ctx.stroke();

			// Stop number label
			ctx.fillStyle = '#ef4444';
			ctx.font = '11px JetBrains Mono, Monaco, monospace';
			ctx.textAlign = 'center';
			ctx.fillText(index.toString(), x, centerY - 10);
		});

		// 4. Draw current segment highlight
		if (busProgress > 0) {
			const segment = findSegment(busProgress);
			if (segment) {
				const x1 = progressToX(segment.start);
				const x2 = progressToX(segment.end);

				ctx.fillStyle = 'rgba(59, 130, 246, 0.2)';
				ctx.fillRect(x1, centerY - 10, x2 - x1, 20);
			}
		}

		// 5. Draw bus position
		const busX = progressToX(busProgress);
		ctx.font = '16px serif';
		ctx.textAlign = 'center';
		ctx.textBaseline = 'middle';
		ctx.fillText('🚌', busX, centerY);

		// 6. Draw event highlight if present
		if (highlightedEvent) {
			const stop = routeData.stops[highlightedEvent.stopIdx];
			if (stop) {
				const x = progressToX(stop.progress_cm);
				const color = FSM_STATE_COLORS[highlightedEvent.state];

				// Vertical marker
				ctx.strokeStyle = color;
				ctx.lineWidth = 3;
				ctx.beginPath();
				ctx.moveTo(x, centerY - 15);
				ctx.lineTo(x, centerY + 15);
				ctx.stroke();

				// Color dot
				ctx.fillStyle = color;
				ctx.beginPath();
				ctx.arc(x, centerY - 20, 5, 0, Math.PI * 2);
				ctx.fill();
			}
		}
	}

	// Re-render when dependencies change
	$effect(() => {
		render();
	});

	// Handle canvas resize
	onMount(() => {
		ctx = canvas.getContext('2d');
		if (!ctx) return;

		// Initial size
		updateSize();

		// Observe resize
		resizeObserver = new ResizeObserver(() => {
			updateSize();
		});
		resizeObserver.observe(canvas);

		return () => {
			resizeObserver?.disconnect();
		};
	});

	function updateSize() {
		const rect = canvas.getBoundingClientRect();
		width = rect.width;
		height = rect.height;

		// Update canvas resolution for high-DPI displays
		const dpr = window.devicePixelRatio || 1;
		canvas.width = rect.width * dpr;
		canvas.height = rect.height * dpr;

		if (ctx) {
			ctx.scale(dpr, dpr);
		}

		render();
	}

	onDestroy(() => {
		resizeObserver?.disconnect();
	});
</script>

<div class="linear-route-widget">
	<canvas bind:this={canvas}></canvas>
</div>

<style>
	.linear-route-widget {
		width: 100%;
		height: 100%;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	canvas {
		width: 100%;
		height: 100%;
		display: block;
	}
</style>
```

- [ ] **Step 2: Verify file compiles**

Run: `cd visualizer && npm run check`
Expected: No TypeScript errors

- [ ] **Step 3: Commit**

```bash
git add visualizer/src/lib/components/LinearRouteWidget.svelte
git commit -m "feat: add LinearRouteWidget component

Add canvas-based 1D route visualization showing:
- Base line with distance ticks (100m minor, 1km major)
- Stop indicators with index numbers
- Bus position (🚌) with real-time updates
- Current segment highlight (blue overlay)
- Event highlight with FSM state colors

Uses ResizeObserver for responsive canvas sizing.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Chunk 4: MapView Enhancements - Utility Functions and State

### Task 4: Add utility functions and state to MapView.svelte

**Files:**
- Modify: `visualizer/src/lib/components/MapView.svelte`

- [ ] **Step 1: Add imports and utility functions**

Find the import section (around lines 1-8) and add:

```typescript
import { FSM_STATE_COLORS } from '$lib/constants/fsmColors';
import { getStopLatLon } from '$lib/parsers/routeData';
import { projectCmToLatLon } from '$lib/parsers/projection';
import type { FsmState } from '$lib/types';
```

- [ ] **Step 2: Add utility functions after imports (before Props interface)**

Add after line 8 (after imports, before `interface Props`):

```typescript
// Constants for 50m circles
const EARTH_RADIUS = 6378137; // meters
const STOP_RADIUS_M = 50; // 50 meters

/**
 * Convert meters to pixels at a given latitude and zoom level
 * Used for MapLibre GL circle radius calculations
 */
function metersToPixels(meters: number, lat: number, zoom: number): number {
	const latRad = lat * Math.PI / 180;
	const metersPerPixel = EARTH_RADIUS * Math.cos(latRad) / (256 * Math.pow(2, zoom));
	return meters / metersPerPixel;
}

/**
 * Get stop lat/lon by interpolating along route nodes
 * Wrapper for getStopLatLon that handles the grid_origin parameter
 */
function getStopPosition(stopProgressCm: number, routeData: RouteData): [number, number] | null {
	return getStopLatLon(stopProgressCm, routeData);
}
```

- [ ] **Step 3: Add highlightedEvent prop**

Find the Props interface (around line 9) and add the highlightedEvent prop:

```typescript
interface Props {
	routeData: RouteData;
	busPosition?: { lat: number; lon: number; heading?: number } | null;
	selectedStop?: number | null;
	onStopClick?: (stopIndex: number) => void;
	highlightedEvent?: {
		stopIdx: number;
		state: FsmState;
		time: number;
	} | null;
}
```

Note: We use `import('$lib/types').FsmState` because we can't import types at the top level in Svelte components.

- [ ] **Step 4: Update props destructuring**

Find the destructuring line (around line 16-20) and update it:

```typescript
let {
	routeData,
	busPosition = null,
	selectedStop = null,
	onStopClick = () => {},
	highlightedEvent = null
}: Props = $props();
```

- [ ] **Step 5: Add currentPanTarget state**

Find the state variables section (after `let mapLoaded = false;`, around line 26) and add:

```typescript
let currentPanTarget = $state<number | null>(null);
```

- [ ] **Step 6: Verify file compiles**

Run: `cd visualizer && npm run check`
Expected: No TypeScript errors

- [ ] **Step 7: Commit**

```bash
git add visualizer/src/lib/components/MapView.svelte
git commit -m "feat(map): add utility functions and state for enhancements

Add metersToPixels() for circle radius calculation and getStopPosition()
for stop coordinate interpolation. Add highlightedEvent prop and
currentPanTarget state for event marker and pan-to-stop features.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Chunk 5: MapView - 50m Circles Layer

### Task 5: Add 50m circles layer to MapView

**Files:**
- Modify: `visualizer/src/lib/components/MapView.svelte`

- [ ] **Step 1: Add 50m circles layer in map.on('load') callback**

Find the map.on('load') callback (around line 60) and add the 50m circles layer after the stop circles layer (after line 142, before the click handlers):

```typescript
// Add 50m circles around stops
map.addLayer({
	id: 'stops-50m-circles',
	type: 'circle',
	source: stopsSourceId,
	paint: {
		'circle-radius': [
			'interpolate',
			['linear'],
			['zoom'],
			12, metersToPixels(STOP_RADIUS_M, routeData.lat_avg_deg, 12),
			14, metersToPixels(STOP_RADIUS_M, routeData.lat_avg_deg, 14),
			16, metersToPixels(STOP_RADIUS_M, routeData.lat_avg_deg, 16),
			18, metersToPixels(STOP_RADIUS_M, routeData.lat_avg_deg, 18)
		],
		'circle-color': '#3b82f6',
		'circle-opacity': 0.15,
		'circle-stroke-width': 1,
		'circle-stroke-color': '#3b82f6',
		'circle-stroke-opacity': 0.3
	},
	// Place below stop circles
	'before': 'stops-circle'
});
```

- [ ] **Step 2: Verify file compiles**

Run: `cd visualizer && npm run check`
Expected: No TypeScript errors

- [ ] **Step 3: Commit**

```bash
git add visualizer/src/lib/components/MapView.svelte
git commit -m "feat(map): add 50m circles around stops

Add semi-transparent blue circles with 50m radius around each stop.
Circles scale correctly at different zoom levels using interpolate
expression. Rendered below stop markers for better visibility.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Chunk 6: MapView - Event Marker System

### Task 6: Add event marker source and layers

**Files:**
- Modify: `visualizer/src/lib/components/MapView.svelte`

- [ ] **Step 1: Add event marker source and layers in map.on('load')**

Add after the 50m circles layer (after the code from Task 5):

```typescript
// Add event marker source (dynamic)
map.addSource('event-marker', {
	type: 'geojson',
	data: { type: 'FeatureCollection', features: [] }
});

// Add event marker pulse ring
map.addLayer({
	id: 'event-marker-pulse',
	type: 'circle',
	source: 'event-marker',
	paint: {
		'circle-radius': 20,
		'circle-color': ['get', 'color'],
		'circle-opacity': 0.3
	},
	'before': 'stops-circle'
});

// Add event marker main circle
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
	'before': 'bus-marker'
});
```

- [ ] **Step 2: Verify file compiles**

Run: `cd visualizer && npm run check`
Expected: No TypeScript errors

- [ ] **Step 3: Commit**

```bash
git add visualizer/src/lib/components/MapView.svelte
git commit -m "feat(map): add event marker source and layers

Add dynamic event marker with pulse ring effect. Marker position
and color are controlled by highlightedEvent prop. Layers are
positioned above stops but below bus marker.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Chunk 7: MapView - Event Marker Reactivity

### Task 7: Add reactive update for event marker

**Files:**
- Modify: `visualizer/src/lib/components/MapView.svelte`

- [ ] **Step 1: Add $effect for highlightedEvent**

Find the $effect blocks section (after the bus position $effect, around line 243) and add:

```typescript
// Update event marker when highlightedEvent changes
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
	const latLon = getStopPosition(stop.progress_cm, routeData);
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

- [ ] **Step 2: Verify file compiles**

Run: `cd visualizer && npm run check`
Expected: No TypeScript errors

- [ ] **Step 3: Commit**

```bash
git add visualizer/src/lib/components/MapView.svelte
git commit -m "feat(map): add reactive event marker updates

Add $effect that updates event marker position and color when
highlightedEvent prop changes. Clears marker when prop is null.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Chunk 8: MapView - Exported panToStop Function

### Task 8: Add exported panToStop function

**Files:**
- Modify: `visualizer/src/lib/components/MapView.svelte`

- [ ] **Step 1: Add exported panToStop function and its $effect**

Add after the event marker $effect from Task 7:

```typescript
// Export panToStop function for external calls
export function panToStop(stopIdx: number) {
	currentPanTarget = stopIdx;
}

// Reactive: pan when currentPanTarget changes
$effect(() => {
	if (!map || !mapLoaded || currentPanTarget === null) return;

	const stop = routeData.stops[currentPanTarget];
	if (!stop) return;

	// Interpolate lat/lon for this stop
	const latLon = getStopPosition(stop.progress_cm, routeData);
	if (!latLon) return;

	map.easeTo({
		center: [latLon[1], latLon[0]], // [lon, lat] for MapLibre
		zoom: 16,
		duration: 500
	});

	// Reset after triggering
	currentPanTarget = null;
});
```

- [ ] **Step 2: Verify file compiles**

Run: `cd visualizer && npm run check`
Expected: No TypeScript errors

- [ ] **Step 3: Commit**

```bash
git add visualizer/src/lib/components/MapView.svelte
git commit -m "feat(map): add exported panToStop function

Export panToStop() function that pans map to a stop location.
Uses Svelte's $state/$effect pattern for external function calls.
Interpolates stop position along route segments.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Chunk 9: MapView - Clear Highlight Mechanism

### Task 9: Add map click and Escape key handlers

**Files:**
- Modify: `visualizer/src/lib/components/MapView.svelte`

- [ ] **Step 1: Add clearHighlight callback to Props interface**

Update the Props interface to include the callback:

```typescript
interface Props {
	routeData: RouteData;
	busPosition?: { lat: number; lon: number; heading?: number } | null;
	selectedStop?: number | null;
	onStopClick?: (stopIndex: number) => void;
	highlightedEvent?: {
		stopIdx: number;
		state: import('$lib/types').FsmState;
		time: number;
	} | null;
	onClearHighlight?: () => void; // NEW
}
```

- [ ] **Step 2: Update props destructuring**

Update the destructuring:

```typescript
let {
	routeData,
	busPosition = null,
	selectedStop = null,
	onStopClick = () => {},
	highlightedEvent = null,
	onClearHighlight = () => {} // NEW
}: Props = $props();
```

- [ ] **Step 3: Add map click handler in map.on('load')**

Find the existing click handler for stops (around line 164) and add a new map-wide click handler after it:

```typescript
// Add click handler for clearing highlight on map click
map.on('click', (e) => {
	// Check if click was on a stop
	const features = map.queryRenderedFeatures(e.point, { layers: ['stops-circle'] });
	if (features.length === 0 && onClearHighlight) {
		// Click was not on a stop - clear highlight
		onClearHighlight();
	}
});
```

- [ ] **Step 4: Add Escape key handler**

First, add a module-level variable to store the handler (find the variable declarations section around line 23, after `let mapLoaded`):

```typescript
let handleKeyDownRef: ((e: KeyboardEvent) => void) | null = null;
```

Then, in the onMount callback (around line 30), add the keyboard handler at the end before the return statement:

```typescript
// Add Escape key handler for clearing highlight
handleKeyDownRef = (e: KeyboardEvent) => {
	if (e.key === 'Escape' && onClearHighlight) {
		onClearHighlight();
	}
};
document.addEventListener('keydown', handleKeyDownRef);
```

Finally, update the onDestroy callback (around line 270) to also remove the event listener:

Find:
```typescript
onDestroy(() => {
	if (map) {
		map.remove();
		map = null;
	}
});
```

Change to:
```typescript
onDestroy(() => {
	if (map) {
		map.remove();
		map = null;
	}
	if (handleKeyDownRef) {
		document.removeEventListener('keydown', handleKeyDownRef);
		handleKeyDownRef = null;
	}
});
```

- [ ] **Step 5: Verify file compiles**

Run: `cd visualizer && npm run check`
Expected: No TypeScript errors

- [ ] **Step 6: Commit**

```bash
git add visualizer/src/lib/components/MapView.svelte
git commit -m "feat(map): add clear highlight mechanism

Add onClearHighlight callback prop and trigger it when:
- User clicks map outside of stop markers
- User presses Escape key

Allows users to dismiss event highlights.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Chunk 10: EventLog Enhancements

### Task 10: Add onEventClick callback to EventLog

**Files:**
- Modify: `visualizer/src/lib/components/EventLog.svelte`

- [ ] **Step 1: Update Props interface**

Find the Props interface (around line 4) and update it:

```typescript
interface Props {
	traceData: TraceData;
	onSeek: (time: number) => void;
	onEventClick?: (info: { time: number; stopIdx?: number; state?: FsmState }) => void;
}

let { traceData, onSeek, onEventClick }: Props = $props();
```

- [ ] **Step 2: Update button click handler**

Find the button element (around line 88) and update the onclick handler:

```typescript
<button
	class="event-item {event.type.toLowerCase()}"
	onclick={() => {
		onSeek(event.time);
		// For ARRIVAL events, state is AtStop; for TRANSITION, use the recorded state
		const eventState = event.state || (event.type === 'ARRIVAL' ? 'AtStop' : undefined);
		if (event.stopIdx !== undefined && eventState) {
			onEventClick?.({
				time: event.time,
				stopIdx: event.stopIdx,
				state: eventState
			});
		}
	}}
>
```

- [ ] **Step 3: Verify file compiles**

Run: `cd visualizer && npm run check`
Expected: No TypeScript errors

- [ ] **Step 4: Commit**

```bash
git add visualizer/src/lib/components/EventLog.svelte
git commit -m "feat(eventlog): add onEventClick callback

Emit extended event info (time, stopIdx, state) when user clicks
an event. Only emits stop-related events (TRANSITION, ARRIVAL)
that have valid stopIdx and state.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Chunk 11: Page Integration - Imports and State

### Task 11: Update +page.svelte imports and add state

**Files:**
- Modify: `visualizer/src/routes/+page.svelte`

- [ ] **Step 1: Add LinearRouteWidget import**

Find the imports section (around line 2) and add:

```typescript
import LinearRouteWidget from '$lib/components/LinearRouteWidget.svelte';
```

- [ ] **Step 2: Add highlightedEvent state**

Find the state declarations section (around line 13, after `let selectedStop`) and add:

```typescript
let highlightedEvent = $state<{
	stopIdx: number;
	time: number;
	state: FsmState;
} | null>(null);

let mapViewRef: { panToStop: (idx: number) => void } | null = null;
```

- [ ] **Step 3: Verify file compiles**

Run: `cd visualizer && npm run check`
Expected: No TypeScript errors

- [ ] **Step 4: Commit**

```bash
git add visualizer/src/routes/+page.svelte
git commit -m "feat(page): add LinearRouteWidget import and state

Import LinearRouteWidget component and add highlightedEvent state
to track which event is currently highlighted. Add mapViewRef for
pan-to-stop function access.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Chunk 12: Page Integration - Event Handlers

### Task 12: Add event handlers to +page.svelte

**Files:**
- Modify: `visualizer/src/routes/+page.svelte`

- [ ] **Step 1: Add handleEventClick function**

Find the resetUpload function (around line 131) and add after it:

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

- [ ] **Step 2: Verify file compiles**

Run: `cd visualizer && npm run check`
Expected: No TypeScript errors

- [ ] **Step 3: Commit**

```bash
git add visualizer/src/routes/+page.svelte
git commit -m "feat(page): add event handlers

Add handleEventClick() to set highlighted event and trigger map pan.
Add clearHighlight() to reset highlighted event state.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Chunk 13: Page Integration - Update MapView Props

### Task 13: Update MapView component usage in +page.svelte

**Files:**
- Modify: `visualizer/src/routes/+page.svelte`

- [ ] **Step 1: Update MapView component**

Find the MapView component (around line 190) and update it:

```svelte
<MapView
	{routeData}
	{busPosition}
	{selectedStop}
	{highlightedEvent}
	onStopClick={(idx) => selectedStop = idx}
	onClearHighlight={clearHighlight}
	bind:this={mapViewRef}
/>
```

- [ ] **Step 2: Verify file compiles**

Run: `cd visualizer && npm run check`
Expected: No TypeScript errors

- [ ] **Step 3: Commit**

```bash
git add visualizer/src/routes/+page.svelte
git commit -m "feat(page): update MapView with new props

Pass highlightedEvent, onClearHighlight, and bind mapViewRef
to enable event marker and pan-to-stop functionality.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Chunk 14: Page Integration - Add LinearRouteWidget and Update EventLog

### Task 14: Add LinearRouteWidget panel and update EventLog

**Files:**
- Modify: `visualizer/src/routes/+page.svelte`

- [ ] **Step 1: Add LinearRouteWidget panel**

Find the dashboard-grid main element (around line 186) and add the new panel after the feed-panel section (after line 221, before `</main>`):

```svelte
<!-- Linear Route Panel -->
<section class="panel linear-route-panel">
	{#if routeData && currentRecord}
		<LinearRouteWidget
			{routeData}
			busProgress={currentRecord.s_cm}
			{highlightedEvent}
		/>
	{/if}
</section>
```

- [ ] **Step 2: Update EventLog component**

Find the EventLog component (around line 218) and update it:

```svelte
<EventLog
	{traceData}
	onSeek={handleSeek}
	onEventClick={handleEventClick}
/>
```

- [ ] **Step 3: Update CSS for linear-route-panel**

Find the `.panel` style (around line 368) and add the new style after it (after line 369):

```css
.linear-route-panel {
	height: 80px;
	min-height: 80px;
}
```

Also update the `.dashboard-grid` style to add the second row:

```css
.dashboard-grid {
	flex: 1;
	display: grid;
	grid-template-columns: 1.5fr 1.5fr 1fr;
	grid-template-rows: 1fr auto;
	gap: 1px;
	background-color: #333;
	min-height: 0;
}
```

- [ ] **Step 4: Verify file compiles**

Run: `cd visualizer && npm run check`
Expected: No TypeScript errors

- [ ] **Step 5: Commit**

```bash
git add visualizer/src/routes/+page.svelte
git commit -m "feat(page): add LinearRouteWidget panel and update EventLog

Add new 80px tall panel for LinearRouteWidget below the three main
panels. Update EventLog to emit event click data. Update grid
layout to support two rows.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Chunk 15: Testing and Verification

### Task 15: Manual testing with test data

**Files:**
- No file changes

- [ ] **Step 1: Start the dev server**

Run: `cd visualizer && npm run dev`

- [ ] **Step 2: Load test data**

1. Open http://localhost:5173
2. Upload `test_data/ty225_v2.bin` as Route Data
3. Upload `test_data/ty225_v2_trace.jsonl` as Trace Data

- [ ] **Step 3: Verify LinearRouteWidget**

Check the following:
- ✓ Horizontal line renders from 0 to route end
- ✓ Distance ticks every 100m, major ticks with labels every 1km
- ✓ All stops shown as red vertical lines with numbers
- ✓ Bus emoji (🚌) appears and moves during playback
- ✓ Blue highlight shows current segment between nodes
- ✓ Clicking an event shows color-coded marker on linear widget

- [ ] **Step 4: Verify 50m Circles**

Check the following:
- ✓ Semi-transparent blue circles around all stops
- ✓ Circles are visible but don't obscure route line
- ✓ Circles scale correctly at different zoom levels

- [ ] **Step 5: Verify Event Click Integration**

Check the following:
- ✓ Clicking a TRANSITION event pans map to stop
- ✓ Clicking an ARRIVAL event shows green marker
- ✓ Marker appears at correct stop position
- ✓ Linear widget shows corresponding highlight
- ✓ Pressing Escape clears highlight
- ✓ Clicking map (outside stops) clears highlight

- [ ] **Step 6: Verify FSM State Colors**

Check the following:
- ✓ Approaching events show yellow (#eab308)
- ✓ Arriving events show orange (#f97316)
- ✓ AtStop events show green (#22c55e)
- ✓ Departed events show gray (#6b7280)

- [ ] **Step 7: Verify Edge Cases**

Check the following:
- ✓ Widget works with small routes (1-2 nodes)
- ✓ No crashes when clicking events
- ✓ Bus position at start (0cm) renders correctly
- ✓ Bus position at end of route renders correctly

- [ ] **Step 8: Stop dev server**

Press Ctrl+C in the terminal

- [ ] **Step 9: Commit implementation completion**

```bash
git add visualizer/src/routes/+page.svelte
git commit -m "feat: complete linear route widget and map enhancements

All features implemented and tested:
- LinearRouteWidget with canvas rendering
- 50m circles on map
- Event markers with FSM state colors
- Pan-to-stop on event click
- Clear highlight mechanism

Testing completed with ty225_v2 test data.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Summary

This implementation plan adds:

1. **LinearRouteWidget** - Canvas-based 1D route visualization
2. **50m circles** - Semi-transparent circles around all stops
3. **Event markers** - Color-coded dots based on FSM state
4. **Pan-to-stop** - Map pans to stop when event is clicked
5. **Clear highlight** - Escape key and map click to clear highlights

Total tasks: 15
Total files: 5 new/modified
Estimated time: 2-3 hours

**Testing data:** `test_data/ty225_v2.bin` and `test_data/ty225_v2_trace.jsonl`

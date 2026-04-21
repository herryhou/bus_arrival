# Bus Tooltip Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a hover tooltip to the bus emoji (🚌) in LinearRouteWidget showing full diagnostic data from the current TraceRecord.

**Architecture:**
- Create new `BusTooltip.svelte` component for displaying diagnostic data in a 3-column grid
- Modify `LinearRouteWidget.svelte` to track bus hover state and render tooltip
- Use existing tooltip positioning pattern from `StopTooltip.svelte`

**Tech Stack:**
- Svelte 5 with `$state`, `$derived` runes
- TypeScript with existing `TraceRecord` type
- CSS Grid for 3-column layout

---

## Task 1: Create BusTooltip.svelte Component

**Files:**
- Create: `visualizer/src/lib/components/BusTooltip.svelte`

- [ ] **Step 1: Write the BusTooltip component skeleton**

```svelte
<!-- visualizer/src/lib/components/BusTooltip.svelte -->
<script lang="ts">
  import type { TraceRecord } from '$lib/types';

  interface Props {
    record: TraceRecord;
    x: number;
    y: number;
  }

  let { record, x, y }: Props = $props();

  // Format helpers - Position
  const lat = $derived(record.lat.toFixed(4));
  const lon = $derived(record.lon.toFixed(4));
  const progressKm = $derived((record.s_cm / 100000).toFixed(2));

  // Format helpers - Motion
  const speedKmh = $derived((record.v_cms * 3600 / 100000).toFixed(1));
  const heading = $derived(record.heading_cdeg ? Math.round(record.heading_cdeg / 100) : null);

  // Format helpers - Map matching
  const divergenceM = $derived((record.divergence_cm / 100).toFixed(1));
  const segmentDisplay = $derived(record.segment_idx ?? 'Off-route');

  // Fix type badge color
  const fixBadgeClass = $derived(record.fix_type === '3d' ? 'fix-3d' : record.fix_type === '2d' ? 'fix-2d' : 'fix-none');
</script>

{#if record}
  <div class="bus-tooltip" style="left: {x}px; top: {y}px;">
    <!-- Header -->
    <div class="tooltip-header">
      <span class="title">BUS DIAGNOSTIC</span>
      {#if record.fix_type}
        <span class="fix-badge {fixBadgeClass}">{record.fix_type}</span>
      {/if}
    </div>

    <!-- 3-column grid -->
    <div class="tooltip-grid">
      <!-- Col 1: Position & Motion -->
      <div class="col">
        <div class="section-label">POSITION & MOTION</div>
        <div class="field-row">
          <span class="field-name">Lat</span>
          <span class="field-value">{lat}°N</span>
        </div>
        <div class="field-row">
          <span class="field-name">Lon</span>
          <span class="field-value">{lon}°E</span>
        </div>
        <div class="field-row">
          <span class="field-name">Route</span>
          <span class="field-value">{progressKm} km</span>
        </div>
        <div class="field-row">
          <span class="field-name">Speed</span>
          <span class="field-value">{speedKmh} km/h</span>
        </div>
        {#if heading !== null}
          <div class="field-row">
            <span class="field-name">Head</span>
            <span class="field-value">{heading}°</span>
          </div>
        {/if}
      </div>

      <!-- Col 2: GPS & Map Matching -->
      <div class="col">
        <div class="section-label">GPS & MAP MATCH</div>
        <div class="field-row">
          <span class="field-name">HDOP</span>
          <span class="field-value">{record.hdop?.toFixed(1) ?? '-'}</span>
        </div>
        <div class="field-row">
          <span class="field-name">Sats</span>
          <span class="field-value">{record.num_sats ?? '-'}</span>
        </div>
        <div class="field-row">
          <span class="field-name">Seg</span>
          <span class="field-value">{segmentDisplay}</span>
        </div>
        <div class="field-row">
          <span class="field-name">Heading</span>
          <span class="field-value icon">{record.heading_constraint_met ? '✓' : '✗'}</span>
        </div>
        <div class="field-row">
          <span class="field-name">Div</span>
          <span class="field-value">{divergenceM} m</span>
        </div>
      </div>

      <!-- Col 3: Kalman & Status -->
      <div class="col">
        <div class="section-label">KALMAN & STATUS</div>
        <div class="field-row">
          <span class="field-name">σ²</span>
          <span class="field-value">{record.variance_cm2}</span>
        </div>
        <div class="field-row">
          <span class="field-name">Jump</span>
          <span class="field-value">{record.gps_jump ? 'Yes' : 'No'}</span>
        </div>
        <div class="field-row">
          <span class="field-name">Recov</span>
          <span class="field-value">{record.recovery_idx ?? '-'}</span>
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .bus-tooltip {
    position: fixed;
    background-color: #1a1a1a;
    border: 1px solid #333;
    border-radius: 8px;
    padding: 10px;
    min-width: 320px;
    max-width: 400px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.5);
    z-index: 1000;
    pointer-events: none;
    font-family: 'JetBrains Mono', monospace;
  }

  .tooltip-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 8px;
    padding-bottom: 6px;
    border-bottom: 1px solid #333;
  }

  .title {
    font-size: 11px;
    font-weight: bold;
    color: #22c55e;
    letter-spacing: 0.05em;
  }

  .fix-badge {
    font-size: 9px;
    padding: 2px 6px;
    border-radius: 3px;
    background-color: #ef4444;
    color: #fff;
  }

  .fix-badge.fix-3d {
    background-color: #22c55e;
  }

  .fix-badge.fix-2d {
    background-color: #f59e0b;
  }

  .fix-badge.fix-none {
    background-color: #ef4444;
  }

  .tooltip-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
  }

  .col {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .section-label {
    font-size: 8px;
    color: #666;
    margin-bottom: 2px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .field-row {
    display: flex;
    justify-content: space-between;
    font-size: 9px;
  }

  .field-name {
    color: #888;
  }

  .field-value {
    color: #e0e0e0;
  }

  .field-value.icon {
    color: {record.heading_constraint_met ? '#22c55e' : '#ef4444'};
  }
</style>
```

- [ ] **Step 2: Verify the component compiles**

Run: `cd visualizer && npm run build`
Expected: SUCCESS (or check with `npm run dev`)

- [ ] **Step 3: Commit**

```bash
git add visualizer/src/lib/components/BusTooltip.svelte
git commit -m "feat(visualizer): add BusTooltip component for diagnostic display"
```

---

## Task 2: Add Bus Hover State to LinearRouteWidget

**Files:**
- Modify: `visualizer/src/lib/components/LinearRouteWidget.svelte`

- [ ] **Step 1: Add hover state variables**

Find the existing state declarations (around line 31-33):
```svelte
  let busProgress = $derived.by(() => busProgressProp);
  let hoveredStop = $state<number | null>(null);
  let tooltipPos = $state<{ x: number; y: number } | null>(null);
```

Add after them:
```svelte
  let hoveredBus = $state(false);
  let busTooltipPos = $state<{ x: number; y: number } | null>(null);
```

- [ ] **Step 2: Add bus hover event handlers**

Add after the `handleStopHoverEnd` function (around line 73):
```svelte
  function handleBusHover(event: MouseEvent) {
    hoveredBus = true;
    const target = event.target as HTMLElement;
    const rect = target.getBoundingClientRect();
    busTooltipPos = { x: rect.left + rect.width / 2, y: rect.top - 8 };
  }

  function handleBusHoverEnd() {
    hoveredBus = false;
    busTooltipPos = null;
  }
```

- [ ] **Step 3: Update bus marker to trigger hover**

Find the bus marker div (around line 136):
```svelte
      <!-- Bus emoji -->
      <div class="bus-marker" style="left: {progressToPercent(busProgress)}%">🚌</div>
```

Replace with:
```svelte
      <!-- Bus emoji -->
      <div
        class="bus-marker"
        style="left: {progressToPercent(busProgress)}%"
        onmouseenter={handleBusHover}
        onmouseleave={handleBusHoverEnd}
      >🚌</div>
```

- [ ] **Step 4: Add cursor style to bus marker**

Find `.bus-marker` in the `<style>` section (around line 246):
```css
  .bus-marker {
    position: absolute;
    top: 22px;
    transform: translateX(-50%);
    font-size: 20px;
    z-index: 10;
  }
```

Add cursor style:
```css
  .bus-marker {
    position: absolute;
    top: 22px;
    transform: translateX(-50%);
    font-size: 20px;
    z-index: 10;
    cursor: help;
  }
```

- [ ] **Step 5: Verify changes compile**

Run: `cd visualizer && npm run build`
Expected: SUCCESS

- [ ] **Step 6: Commit**

```bash
git add visualizer/src/lib/components/LinearRouteWidget.svelte
git commit -m "feat(visualizer): add bus hover state to LinearRouteWidget"
```

---

## Task 3: Import and Render BusTooltip

**Files:**
- Modify: `visualizer/src/lib/components/LinearRouteWidget.svelte`

- [ ] **Step 1: Add BusTooltip import**

Find the imports at the top of the script section (around line 6):
```svelte
  import StopTooltip from './StopTooltip.svelte';
```

Add after it:
```svelte
  import BusTooltip from './BusTooltip.svelte';
```

- [ ] **Step 2: Add BusTooltip rendering**

Find the existing StopTooltip rendering block (around line 177):
```svelte
    <StopTooltip
      stopIndex={hoveredStop}
      stopState={stopState}
      vCms={currentRecord?.v_cms ?? 0}
      x={tooltipPos.x}
      y={tooltipPos.y}
    />
  {/if}
```

Add after it:
```svelte
  {#if hoveredBus && busTooltipPos && currentRecord}
    <BusTooltip
      record={currentRecord}
      x={busTooltipPos.x}
      y={busTooltipPos.y}
    />
  {/if}
```

- [ ] **Step 3: Verify the complete file compiles**

Run: `cd visualizer && npm run build`
Expected: SUCCESS

- [ ] **Step 4: Commit**

```bash
git add visualizer/src/lib/components/LinearRouteWidget.svelte
git commit -m "feat(visualizer): render BusTooltip on bus hover"
```

---

## Task 4: Manual Testing

**Files:**
- Test: `visualizer/` (dev server)

- [ ] **Step 1: Start dev server**

Run: `cd visualizer && npm run dev`
Expected: Server running at http://localhost:5173

- [ ] **Step 2: Load test data**

1. Open http://localhost:5173
2. Upload a route `.bin` file (e.g., `ty225.bin`)
3. Upload matching trace file (e.g., `ty225_trace.jsonl`)

- [ ] **Step 3: Test bus tooltip appears**

1. Hover over the 🚌 emoji in the LinearRouteWidget
2. Verify tooltip appears above the bus marker
3. Verify all fields display correctly

- [ ] **Step 4: Test tooltip positioning**

1. Scrub timeline to move bus to different positions
2. Hover over bus at various positions
3. Verify tooltip stays positioned correctly near the bus marker

- [ ] **Step 5: Test null/missing field handling**

1. Scrub through the timeline
2. Look for records with missing optional fields (hdop, num_sats, fix_type, heading_cdeg)
3. Verify tooltip shows "-" for missing values without errors

- [ ] **Step 6: Test tooltip dismissal**

1. Move mouse away from bus emoji
2. Verify tooltip disappears immediately
3. Move mouse back and forth
4. Verify tooltip appears/disappears smoothly

- [ ] **Step 7: Test with different trace files**

1. Reset session
2. Load a different route/trace pair
3. Verify tooltip works with different data

---

## Task 5: Optional Polish - Dynamic Icon Color

**Files:**
- Modify: `visualizer/src/lib/components/BusTooltip.svelte`

- [ ] **Step 1: Make icon color dynamic**

Find the icon color style:
```svelte
  .field-value.icon {
    color: {record.heading_constraint_met ? '#22c55e' : '#ef4444'};
  }
```

This won't work as-is because CSS can't use Svelte expressions directly. Replace with:

In the template, change the icon span:
```svelte
        <div class="field-row">
          <span class="field-name">Heading</span>
          <span class="field-value icon" class:icon-pass={record.heading_constraint_met} class:icon-fail={!record.heading_constraint_met}>
            {record.heading_constraint_met ? '✓' : '✗'}
          </span>
        </div>
```

In the `<style>` section, update:
```css
  .field-value.icon {
    color: #ef4444;
  }

  .field-value.icon-pass {
    color: #22c55e;
  }

  .field-value.icon-fail {
    color: #ef4444;
  }
```

- [ ] **Step 2: Verify and commit**

Run: `cd visualizer && npm run build`
Expected: SUCCESS

```bash
git add visualizer/src/lib/components/BusTooltip.svelte
git commit -m "fix(visualizer): make heading constraint icon color dynamic"
```

---

## Task 6: Optional Enhancement - Closest Stop Distance

**Files:**
- Modify: `visualizer/src/lib/components/LinearRouteWidget.svelte`

- [ ] **Step 1: Add closest stop calculation to BusTooltip**

This is an optional enhancement to show which stop is nearest. Add to BusTooltip props and display.

First, update LinearRouteWidget to pass closest stop info:

Find the BusTooltip rendering (from Task 3) and update:
```svelte
  {#if hoveredBus && busTooltipPos && currentRecord}
    {@const closestStop = getClosestStop(busProgress)}
    <BusTooltip
      record={currentRecord}
      closestStopIndex={closestStop?.index ?? null}
      closestStopDistance={closestStop?.distance ?? null}
      x={busTooltipPos.x}
      y={busTooltipPos.y}
    />
  {/if}
```

Add the helper function before `handleStopHoverEnd`:
```svelte
  function getClosestStop(progress: number): { index: number; distance: number } | null {
    if (routeData.stops.length === 0) return null;
    let closest = { index: 0, distance: Math.abs(routeData.stops[0].progress_cm - progress) };
    for (let i = 1; i < routeData.stops.length; i++) {
      const dist = Math.abs(routeData.stops[i].progress_cm - progress);
      if (dist < closest.distance) closest = { index: i, distance: dist };
    }
    return closest;
  }
```

Then update BusTooltip to accept and display these props.

This is OPTIONAL - only implement if requested.

---

## Summary

This plan adds a hover tooltip to the bus emoji in the LinearRouteWidget that displays:
- Position (lat, lon, route progress)
- Motion (speed, heading)
- GPS quality (HDOP, satellites, fix type)
- Map matching (segment, heading constraint, divergence)
- Kalman state (variance)
- Status (GPS jump, recovery)

The implementation follows existing patterns from `StopTooltip.svelte` and integrates cleanly with the current `LinearRouteWidget.svelte` architecture.

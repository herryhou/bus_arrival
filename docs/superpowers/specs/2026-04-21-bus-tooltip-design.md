# Bus Tooltip Design for LinearRouteWidget

**Date:** 2026-04-21
**Status:** Approved
**Related:** Visualizer enhancement - bus icon hover tooltips

## Overview

Add a hover tooltip to the bus emoji (🚌) in the LinearRouteWidget showing full diagnostic data from the current `TraceRecord`.

## Component Structure

```
LinearRouteWidget.svelte (enhanced)
└── BusTooltip.svelte (NEW - bus diagnostic popup)
```

## Visual Design

**Trigger:** Hover over the 🚌 emoji
**Position:** Floating popup positioned near the bus marker (using client coordinates)
**Style:** Dark theme matching existing StopTooltip

### Layout

3-column grid layout with grouped sections:

```
┌─────────────────────────────────────────┐
│ BUS DIAGNOSTIC                   [3D]   │
├──────────────────┬──────────────────────┤
│ Position & Motion│ GPS & Map Matching   │
│ 25.1234°N       │ HDOP: 1.2  Sats: 12  │
│ 121.5678°E      │ Fix: 3D   Seg: 42    │
│ 12.34 km        │ Heading: ✓ Div: 15m  │
│ 12.5 km/h 315°  │                      │
├──────────────────┴──────────────────────┤
│ Kalman & Status                         │
│ σ²: 4500  Jump: No  Recovery: -        │
└─────────────────────────────────────────┘
```

## Data Fields

| Category | Field | Format | Source |
|----------|-------|--------|--------|
| **Position** | lat | Decimal degrees (4 places) | currentRecord.lat |
| | lon | Decimal degrees (4 places) | currentRecord.lon |
| | s_cm | km along route | currentRecord.s_cm |
| **Motion** | v_cms | km/h (converted) | currentRecord.v_cms |
| | heading_cdeg | degrees (0-359) | currentRecord.heading_cdeg |
| **GPS Quality** | hdop | 1 decimal | currentRecord.hdop |
| | num_sats | integer | currentRecord.num_sats |
| | fix_type | badge (none/2d/3d) | currentRecord.fix_type |
| **Map Matching** | segment_idx | integer or "Off-route" | currentRecord.segment_idx |
| | heading_constraint_met | ✓/✗ icon | currentRecord.heading_constraint_met |
| | divergence_cm | meters | currentRecord.divergence_cm |
| **Kalman** | variance_cm2 | integer (σ²) | currentRecord.variance_cm2 |
| **Status** | gps_jump | Yes/No | currentRecord.gps_jump |
| | recovery_idx | integer or "-" | currentRecord.recovery_idx |

## Implementation

### LinearRouteWidget Changes

```typescript
// Add state
let hoveredBus = $state(false);
let busTooltipPos = $state<{ x: number; y: number } | null>(null);

// Add handlers
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

```svelte
<!-- Update bus marker -->
<div
  class="bus-marker"
  style="left: {progressToPercent(busProgress)}%"
  onmouseenter={handleBusHover}
  onmouseleave={handleBusHoverEnd}
>🚌</div>

<!-- Add tooltip -->
{#if hoveredBus && busTooltipPos && currentRecord}
  <BusTooltip
    record={currentRecord}
    x={busTooltipPos.x}
    y={busTooltipPos.y}
  />
{/if}
```

### BusTooltip.svelte Component

```svelte
<script lang="ts">
  import type { TraceRecord } from '$lib/types';

  interface Props {
    record: TraceRecord;
    x: number;
    y: number;
  }

  let { record, x, y }: Props = $props();

  // Format helpers
  const lat = $derived(record.lat.toFixed(4));
  const lon = $derived(record.lon.toFixed(4));
  const progressKm = $derived((record.s_cm / 100000).toFixed(2));
  const speedKmh = $derived((record.v_cms * 3600 / 100000).toFixed(1));
  const heading = $derived(record.heading_cdeg ? Math.round(record.heading_cdeg / 100) : null);
  const divergenceM = $derived((record.divergence_cm / 100).toFixed(1));
</script>

<div class="bus-tooltip" style="left: {x}px; top: {y}px;">
  <!-- Header -->
  <div class="tooltip-header">
    <span class="title">BUS DIAGNOSTIC</span>
    {#if record.fix_type}
      <span class="fix-badge" class:fix-3d={record.fix_type === '3d'}>{record.fix_type}</span>
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
      {#if heading}
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
        <span class="field-value">{record.segment_idx ?? 'Off-route'}</span>
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
```

### Styling

```css
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
  color: #22c55e;
}
```

## Out of Scope

- Map view changes (no modifications to MapView)
- Timeline or other panel changes
- Backend/Rust changes (visualizer-only work)

## Success Criteria

1. Hovering over 🚌 shows diagnostic tooltip
2. All 12+ diagnostic fields displayed in compact 3-column layout
3. Tooltip positioned correctly relative to bus marker
4. Styling matches existing StopTooltip design
5. Works with all trace records (handles null/missing fields gracefully)

<!-- visualizer/src/lib/components/BusTooltip.svelte -->
<script lang="ts">
  import type { TraceRecord } from '$lib/types';

  interface Props {
    record: TraceRecord | null;
    x: number | null;
    y: number | null;
  }

  let { record, x, y }: Props = $props();

  // Safe number formatting with NaN/null checks
  const safeToFixed = (value: number | null | undefined, digits: number): string => {
    if (value === null || value === undefined || Number.isNaN(value)) return '-';
    return value.toFixed(digits);
  };

  // Format helpers - Position
  const lat = $derived(safeToFixed(record?.lat ?? null, 4));
  const lon = $derived(safeToFixed(record?.lon ?? null, 4));
  const progressKm = $derived(safeToFixed((record?.s_cm ?? 0) / 100000, 2));

  // Format helpers - Motion
  const speedKmh = $derived(safeToFixed((record?.v_cms ?? 0) * 3600 / 100000, 1));
  const heading = $derived(record?.heading_cdeg ? Math.round(record.heading_cdeg / 100) : null);

  // Format helpers - Map matching
  const divergenceM = $derived(safeToFixed((record?.divergence_cm ?? 0) / 100, 1));
  const segmentDisplay = $derived(record?.segment_idx ?? 'Off-route');

  // Fix type badge color
  const fixBadgeClass = $derived(record?.fix_type === '3d' ? 'fix-3d' : record?.fix_type === '2d' ? 'fix-2d' : 'fix-none');
</script>

{#if record && x !== null && y !== null}
  <!-- Fixed positioning ensures tooltip stays in viewport -->
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
          <span class="field-value icon" class:icon-pass={record.heading_constraint_met} class:icon-fail={!record.heading_constraint_met}>
            {record.heading_constraint_met ? '✓' : '✗'}
          </span>
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
    color: #ef4444;
  }

  .field-value.icon-pass {
    color: #22c55e;
  }

  .field-value.icon-fail {
    color: #ef4444;
  }
</style>

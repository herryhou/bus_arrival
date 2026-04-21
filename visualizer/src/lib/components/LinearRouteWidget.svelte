<script lang="ts">
  import type { RouteData, FsmState, Stop, TraceRecord, StopTraceState } from '$lib/types';
  import { FSM_STATE_COLORS, FSM_STATE_ABBREVS } from '$lib/constants/fsmColors';
  import { getProbabilityColor } from '$lib/utils/probabilityColors';
  import StopMarker from './StopMarker.svelte';
  import StopTooltip from './StopTooltip.svelte';

  interface Props {
    routeData: RouteData;
    busProgress: number;
    busSpeed?: number;
    highlightedEvent?: {
      stopIdx: number;
      state: FsmState;
    } | null;
    traceData?: TraceRecord[] | null;
    currentTime?: number;
    onStopClick?: (idx: number) => void;
  }

  let {
    routeData,
    busProgress: busProgressProp,
    busSpeed = 0,
    highlightedEvent = null,
    traceData = null,
    currentTime = 0,
    onStopClick
  } = $props();

  let busProgress = $derived.by(() => busProgressProp);
  let hoveredStop = $state<number | null>(null);
  let tooltipPos = $state<{ x: number; y: number } | null>(null);

  const maxProgress = $derived.by(() => {
    if (routeData.nodes.length === 0) return 100000;
    return routeData.nodes[routeData.nodes.length - 1].cum_dist_cm;
  });

  // Find current trace record
  const currentRecord = $derived.by(() => {
    if (!traceData || traceData.length === 0) return null;
    return traceData.reduce((prev: TraceRecord, curr: TraceRecord) =>
      Math.abs(curr.time - currentTime) < Math.abs(prev.time - currentTime) ? curr : prev
    );
  });

  // Get stop state for a stop at current time
  function getStopState(stopIndex: number): StopTraceState | null {
    if (!currentRecord) return null;
    return currentRecord.stop_states.find((s: StopTraceState) => s.stop_idx === stopIndex) || null;
  }

  function progressToPercent(progressCm: number): number {
    return maxProgress > 0 ? (Math.min(progressCm, maxProgress) / maxProgress) * 100 : 0;
  }

  function handleStopClick(idx: number) {
    onStopClick?.(idx);
  }

  function handleStopHover(idx: number, event: CustomEvent) {
    hoveredStop = idx;
    // Get the target element from the event to calculate position
    const target = event.target as HTMLElement;
    const rect = target.getBoundingClientRect();
    tooltipPos = { x: rect.left + rect.width / 2, y: rect.bottom + 8 };
  }

  function handleStopHoverEnd() {
    hoveredStop = null;
    tooltipPos = null;
  }

  const speedKmh = $derived.by(() => {
    return (busSpeed * 3600) / 100000;
  });

  const progressKm = $derived.by(() => (busProgress / 100000).toFixed(2));
  const totalKm = $derived.by(() => (maxProgress / 100000).toFixed(1));
  const progressPercent = $derived.by(() => ((busProgress / maxProgress) * 100).toFixed(0));

  // Nearby stops for detail panel
  const nearbyStops = $derived.by(() => {
    const SEARCH_RADIUS_CM = 100000;
    return routeData.stops
      .map((stop: Stop, index: number) => ({
        stop,
        index,
        distance: Math.abs(stop.progress_cm - busProgress)
      }))
      .filter(({ distance }: { distance: number }) => distance <= SEARCH_RADIUS_CM)
      .sort((a: { distance: number }, b: { distance: number }) => a.distance - b.distance)
      .slice(0, 3);
  });
</script>

<div class="linear-route-widget">
  <!-- Main route view -->
  <div class="route-container">
    <!-- Route line -->
    <div class="route-line">
      <div class="route-base-line"></div>
      <div
        class="route-progress-line"
        style="left: {progressToPercent(busProgress)}%"
      ></div>
    </div>

    <!-- Stops -->
    <div class="stops-row">
      {#each routeData.stops as stop, stopIndex (stopIndex)}
        {@const stopState = getStopState(stopIndex)}
        {@const isHighlighted = highlightedEvent?.stopIdx === stopIndex}
        {@const isHovered = hoveredStop === stopIndex}
        {@const isSelected = false}

        <div
          class="stop-wrapper"
          style="left: {progressToPercent(stop.progress_cm)}%"
        >
          <StopMarker
            {stopIndex}
            probability={stopState?.probability ?? 0}
            fsmState={stopState?.fsm_state ?? null}
            isSelected={isHighlighted}
            isHovered={isHovered}
            onstopclick={(e) => handleStopClick(e.detail.stopIndex)}
            onstophover={(e) => handleStopHover(e.detail.stopIndex, e)}
            onstophoverend={handleStopHoverEnd}
          />
        </div>
      {/each}

      <!-- Bus emoji -->
      <div
        class="bus-marker"
        style="left: {progressToPercent(busProgress)}%"
      >🚌</div>
    </div>
  </div>

  <!-- Detail panel -->
  <div class="detail-panel">
    <!-- Speed -->
    <div class="detail-section">
      <span class="detail-label">SPEED</span>
      <span class="detail-value" style="color: {speedKmh > 0 ? '#22c55e' : '#64748b'}">
        {speedKmh.toFixed(1)} km/h
      </span>
    </div>

    <!-- Progress -->
    <div class="detail-section center">
      <span class="detail-label">PROGRESS</span>
      <span class="detail-value">{progressKm} / {totalKm} km</span>
      <span class="detail-percent" style="color: #f59e0b">{progressPercent}%</span>
    </div>

    <!-- Nearby stops -->
    {#if nearbyStops.length > 0}
      <div class="detail-section right">
        <span class="detail-label">NEARBY STOPS</span>
        <div class="nearby-list">
          {#each nearbyStops as { stop, index, distance }}
            {@const distanceKm = (distance / 100000).toFixed(2)}
            {@const direction = stop.progress_cm > busProgress ? '→' : '←'}
            <span
              class="nearby-item"
              style="color: {distance < 50000 ? '#22c55e' : '#cbd5e0'}"
            >
              #{index + 1} {direction} {distanceKm}km
            </span>
          {/each}
        </div>
      </div>
    {/if}
  </div>

  <!-- Tooltip -->
  {#if hoveredStop !== null && tooltipPos}
    {@const stopState = getStopState(hoveredStop)}
    <StopTooltip
      stopIndex={hoveredStop}
      stopState={stopState}
      vCms={currentRecord?.v_cms ?? 0}
      x={tooltipPos.x}
      y={tooltipPos.y}
    />
  {/if}
</div>

<style>
  .linear-route-widget {
    width: 100%;
    height: 100%;
    padding: 1rem;
    display: flex;
    flex-direction: column;
    gap: 1rem;
    position: relative;
  }

  .route-container {
    position: relative;
    height: 80px;
  }

  .route-line {
    position: absolute;
    top: 35px;
    left: 0;
    right: 0;
    height: 6px;
  }

  .route-base-line {
    position: absolute;
    width: 100%;
    height: 100%;
    background-color: #4a5568;
    border-radius: 3px;
  }

  .route-progress-line {
    position: absolute;
    top: 0;
    height: 100%;
    width: 6px;
    background-color: #3b82f6;
    border-radius: 3px;
    box-shadow: 0 0 10px rgba(59, 130, 246, 0.8);
    transform: translateX(-50%);
  }

  .stops-row {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 100%;
  }

  .stop-wrapper {
    position: absolute;
    transform: translateX(-50%);
  }

  .bus-marker {
    position: absolute;
    top: 22px;
    transform: translateX(-50%);
    font-size: 20px;
    z-index: 10;
  }

  .detail-panel {
    display: grid;
    grid-template-columns: 1fr 1fr 1fr;
    gap: 1rem;
    background-color: rgba(30, 41, 59, 0.8);
    border-radius: 8px;
    padding: 1rem;
  }

  .detail-section {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
  }

  .detail-section.center {
    align-items: center;
  }

  .detail-section.right {
    align-items: flex-end;
  }

  .detail-label {
    font-size: 10px;
    font-family: 'JetBrains Mono', monospace;
    color: #94a3b8;
    margin-bottom: 0.25rem;
  }

  .detail-value {
    font-size: 18px;
    font-family: 'JetBrains Mono', monospace;
    font-weight: bold;
    color: #e2e8f0;
  }

  .detail-percent {
    font-size: 12px;
    font-family: 'JetBrains Mono', monospace;
    margin-top: 0.25rem;
  }

  .nearby-list {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .nearby-item {
    font-size: 11px;
    font-family: 'JetBrains Mono', monospace;
  }
</style>

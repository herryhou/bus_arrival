<script lang="ts">
  import type {
    RouteData,
    FsmState,
    Stop,
    TraceRecord,
    StopTraceState,
  } from "$lib/types";
  import {
    FSM_STATE_COLORS,
    FSM_STATE_ABBREVS,
  } from "$lib/constants/fsmColors";
  import { getProbabilityColor } from "$lib/utils/probabilityColors";
  import StopMarker from "./StopMarker.svelte";
  import StopTooltip from "./StopTooltip.svelte";

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
    onStopClick,
  } = $props();

  let busProgress = $derived.by(() => busProgressProp);
  let hoveredStop = $state<number | null>(null);
  let tooltipPos = $state<{ x: number; y: number } | null>(null);
  let scrollContainer = $state<HTMLDivElement | null>(null);

  // Zoom controls
  let zoomLevel = $state(1.5); // 1.5 = default scale (more spacing)
  const MIN_ZOOM = 0.5; // Zoom out to see more (increased from 0.3)
  const MAX_ZOOM = 4.0; // Zoom in for detail (increased from 3.0)
  const MIN_STOP_WIDTH_PX = 50; // Increased from 40 for better spacing

  // Auto-scroll to bus when progress changes
  $effect(() => {
    // Trigger scroll when busProgress or scrollContainer changes
    busProgress;
    scrollContainer;
    if (
      scrollContainer &&
      scrollInfo.contentWidth > scrollInfo.containerWidth
    ) {
      scrollToBus();
    }
  });

  // Calculate content width based on zoom level and stop count
  const scrollInfo = $derived.by(() => {
    const stopCount = routeData.stops.length;
    const containerWidth = scrollContainer?.clientWidth || 800;

    // Calculate minimum width to prevent crowding at current zoom
    const minContentWidth = stopCount * MIN_STOP_WIDTH_PX * zoomLevel;

    // Also ensure full route is visible at minimum zoom
    const baseWidth = containerWidth * zoomLevel;

    const contentWidth = Math.max(baseWidth, minContentWidth);

    return { contentWidth, containerWidth };
  });

  const maxProgress = $derived.by(() => {
    if (routeData.nodes.length === 0) return 100000;
    return routeData.nodes[routeData.nodes.length - 1].cum_dist_cm;
  });

  // Find current trace record
  const currentRecord = $derived.by(() => {
    if (!traceData || traceData.length === 0) return null;
    return traceData.reduce((prev: TraceRecord, curr: TraceRecord) =>
      Math.abs(curr.time - currentTime) < Math.abs(prev.time - currentTime)
        ? curr
        : prev,
    );
  });

  // Get stop state for a stop at current time
  function getStopState(stopIndex: number): StopTraceState | null {
    if (!currentRecord) return null;
    return (
      currentRecord.stop_states.find(
        (s: StopTraceState) => s.stop_idx === stopIndex,
      ) || null
    );
  }

  function progressToPercent(progressCm: number): number {
    return maxProgress > 0
      ? (Math.min(progressCm, maxProgress) / maxProgress) * 100
      : 0;
  }

  // Convert progress to pixel position within the scrollable content
  function progressToPixel(progressCm: number): number {
    return maxProgress > 0
      ? (Math.min(progressCm, maxProgress) / maxProgress) *
          scrollInfo.contentWidth
      : 0;
  }

  // Auto-scroll to keep bus in view
  function scrollToBus() {
    if (!scrollContainer) return;
    const busX = progressToPixel(busProgress);
    const containerWidth = scrollContainer.clientWidth;
    const targetScroll = Math.max(0, busX - containerWidth / 2);
    scrollContainer.scrollTo({ left: targetScroll, behavior: "smooth" });
  }

  // Zoom controls
  function setZoom(newZoom: number) {
    zoomLevel = Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, newZoom));
  }

  function zoomIn() {
    setZoom(zoomLevel + 0.2);
  }

  function zoomOut() {
    setZoom(zoomLevel - 0.2);
  }

  function resetZoom() {
    zoomLevel = 1.0;
  }

  function handleWheel(event: WheelEvent) {
    if (!scrollContainer) return;
    // Only handle horizontal scroll or ctrl+wheel for zoom
    if (event.ctrlKey || Math.abs(event.deltaX) > Math.abs(event.deltaY)) {
      if (event.ctrlKey) {
        event.preventDefault();
        const delta = event.deltaY > 0 ? -0.1 : 0.1;
        setZoom(zoomLevel + delta);
      }
    }
  }

  // Format zoom as percentage
  const zoomPercent = $derived.by(() => Math.round(zoomLevel * 100));

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
  const progressPercent = $derived.by(() =>
    ((busProgress / maxProgress) * 100).toFixed(0),
  );

  // Nearby stops for detail panel
  const nearbyStops = $derived.by(() => {
    const SEARCH_RADIUS_CM = 100000;
    return routeData.stops
      .map((stop: Stop, index: number) => ({
        stop,
        index,
        distance: Math.abs(stop.progress_cm - busProgress),
      }))
      .filter(
        ({ distance }: { distance: number }) => distance <= SEARCH_RADIUS_CM,
      )
      .sort(
        (a: { distance: number }, b: { distance: number }) =>
          a.distance - b.distance,
      )
      .slice(0, 3);
  });

  // Find nearest stop to bus for displaying next to bus marker
  const nearestStop = $derived.by(() => {
    if (routeData.stops.length === 0) return null;
    return routeData.stops
      .map((stop: Stop, index: number) => ({
        stop,
        index,
        distance: Math.abs(stop.progress_cm - busProgress),
      }))
      .reduce(
        (
          nearest: { stop: Stop; index: number; distance: number } | null,
          current: { stop: Stop; index: number; distance: number },
        ) =>
          !nearest || current.distance < nearest.distance ? current : nearest,
      );
  });

  const nearestStopState = $derived.by(() => {
    if (!nearestStop) return null;
    return getStopState(nearestStop.index);
  });
</script>

<div class="linear-route-widget">
  <!-- Compact control bar (zoom + details) -->
  <div class="control-bar">
    <!-- Zoom controls -->
    <div class="zoom-compact">
      <button
        class="icon-btn"
        onclick={zoomOut}
        disabled={zoomLevel <= MIN_ZOOM}
        title="Zoom out">−</button
      >
      <input
        type="range"
        min={MIN_ZOOM}
        max={MAX_ZOOM}
        step={0.1}
        value={zoomLevel}
        oninput={(e) => {
          const target = e.target as HTMLInputElement;
          setZoom(parseFloat(target.value));
        }}
        class="zoom-slider-compact"
        title="Adjust zoom"
      />
      <span class="zoom-label">{zoomPercent}%</span>
      <button
        class="icon-btn"
        onclick={zoomIn}
        disabled={zoomLevel >= MAX_ZOOM}
        title="Zoom in">+</button
      >
    </div>

    <!-- Speed -->
    <div class="stat-item">
      <span
        class="stat-value"
        style="color: {speedKmh > 0 ? '#22c55e' : '#64748b'}"
      >
        {speedKmh.toFixed(1)}
      </span>
      <span class="stat-unit">km/h</span>
    </div>

    <!-- Progress -->
    <div class="stat-item">
      <span class="stat-value">{progressKm}</span>
      <span class="stat-unit">/ {totalKm} km</span>
    </div>

    <!-- Center on bus -->
    <button class="icon-btn accent" onclick={scrollToBus} title="Center on bus"
      >🚌</button
    >
  </div>

  <!-- Main route view -->
  <div
    class="route-container"
    bind:this={scrollContainer}
    onwheel={handleWheel}
  >
    <!-- Scrollable content wrapper -->
    <div class="route-content" style="width: {scrollInfo.contentWidth}px">
      <!-- Route line -->
      <div class="route-line">
        <div class="route-base-line"></div>
        <div
          class="route-progress-line"
          style="left: {progressToPixel(busProgress)}px"
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
            style="left: {progressToPixel(stop.progress_cm)}px"
          >
            <StopMarker
              {stopIndex}
              probability={stopState?.probability ?? 0}
              fsmState={stopState?.fsm_state ?? null}
              isSelected={isHighlighted}
              {isHovered}
              onstopclick={(e) => handleStopClick(e.detail.stopIndex)}
              onstophover={(e) => handleStopHover(e.detail.stopIndex, e)}
              onstophoverend={handleStopHoverEnd}
            />
          </div>
        {/each}

        <!-- Bus emoji with nearest stop info -->
        <span class="bus-emoji" style="left: {progressToPixel(busProgress)}px"
          >🚌</span
        >
        {#if nearestStop && nearestStopState}
          <div class="bus-info" style="left: {progressToPixel(busProgress)}px">
            <span class="nearest-stop">#{nearestStop.index + 1}</span>
            <span
              class="nearest-prob"
              style="color: {getProbabilityColor(nearestStopState.probability)}"
            >
              {nearestStopState.probability}
            </span>
            {#if nearestStopState.fsm_state}
              <span
                class="nearest-state"
                style="background-color: {FSM_STATE_COLORS[
                  nearestStopState.fsm_state
                ]}"
              >
                {FSM_STATE_ABBREVS[nearestStopState.fsm_state]}
              </span>
            {/if}
          </div>
        {/if}
      </div>
    </div>
  </div>

  <!-- Tooltip -->
  {#if hoveredStop !== null && tooltipPos}
    {@const stopState = getStopState(hoveredStop)}
    <StopTooltip
      stopIndex={hoveredStop}
      {stopState}
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
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0;
    position: relative;
  }

  .control-bar {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.2rem 0.4rem;
    background: linear-gradient(
      135deg,
      rgba(30, 41, 59, 0.7) 0%,
      rgba(15, 23, 42, 0.9) 100%
    );
    border-radius: 6px;
    border: 1px solid rgba(148, 163, 184, 0.15);
  }

  .zoom-compact {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    padding-right: 0.5rem;
    margin-right: 0.5rem;
    border-right: 1px solid rgba(148, 163, 184, 0.2);
  }

  .icon-btn {
    width: 24px;
    height: 24px;
    border: none;
    border-radius: 4px;
    background: rgba(59, 130, 246, 0.2);
    color: #93c5fd;
    font-size: 14px;
    font-weight: bold;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.15s ease;
    padding: 0;
  }

  .icon-btn:hover:not(:disabled) {
    background: rgba(59, 130, 246, 0.4);
    color: #fff;
  }

  .icon-btn:disabled {
    opacity: 0.3;
    cursor: not-allowed;
  }

  .icon-btn.accent {
    background: rgba(245, 158, 11, 0.2);
    color: #fbbf24;
  }

  .icon-btn.accent:hover {
    background: rgba(245, 158, 11, 0.4);
  }

  .zoom-slider-compact {
    width: 80px;
    height: 3px;
    -webkit-appearance: none;
    appearance: none;
    background: rgba(148, 163, 184, 0.2);
    border-radius: 2px;
    outline: none;
    cursor: pointer;
  }

  .zoom-slider-compact::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: linear-gradient(135deg, #60a5fa 0%, #3b82f6 100%);
    cursor: pointer;
    box-shadow: 0 1px 4px rgba(59, 130, 246, 0.4);
  }

  .zoom-slider-compact::-moz-range-thumb {
    width: 12px;
    height: 12px;
    border: none;
    border-radius: 50%;
    background: linear-gradient(135deg, #60a5fa 0%, #3b82f6 100%);
    cursor: pointer;
    box-shadow: 0 1px 4px rgba(59, 130, 246, 0.4);
  }

  .zoom-label {
    font-size: 10px;
    font-family: "JetBrains Mono", monospace;
    color: #94a3b8;
    min-width: 32px;
    text-align: center;
  }

  .stat-item {
    display: flex;
    align-items: baseline;
    gap: 0.15rem;
    padding: 0 0.25rem;
  }

  .stat-value {
    font-size: 13px;
    font-family: "JetBrains Mono", monospace;
    font-weight: 600;
    color: #e2e8f0;
  }

  .stat-unit {
    font-size: 9px;
    font-family: "JetBrains Mono", monospace;
    color: #64748b;
    text-transform: uppercase;
  }

  .route-container {
    position: relative;
    padding: 0 10px;
    height: 95px;
    overflow-x: auto;
    overflow-y: hidden;
    border-radius: 6px;
    background: linear-gradient(
      135deg,
      rgba(30, 41, 59, 0.5) 0%,
      rgba(15, 23, 42, 0.8) 100%
    );
    border: 1px solid rgba(148, 163, 184, 0.1);
    /* Custom scrollbar styling */
    scrollbar-width: thin;
    scrollbar-color: rgba(148, 163, 184, 0.3) transparent;
  }

  .route-container::-webkit-scrollbar {
    height: 6px;
  }

  .route-container::-webkit-scrollbar-track {
    background: transparent;
  }

  .route-container::-webkit-scrollbar-thumb {
    background: rgba(148, 163, 184, 0.3);
    border-radius: 3px;
  }

  .route-container::-webkit-scrollbar-thumb:hover {
    background: rgba(148, 163, 184, 0.5);
  }

  .route-content {
    position: relative;
    height: 100%;
    min-width: 100%;
  }

  .route-line {
    position: absolute;
    top: 45px;
    left: 0;
    right: 0;
    height: 6px;
    z-index: 1;
  }

  .route-base-line {
    position: absolute;
    width: 100%;
    height: 20%;
    background: linear-gradient(90deg, #4a5568 0%, #374151 100%);
    border-radius: 3px;
  }

  .route-progress-line {
    position: absolute;
    top: 0;
    height: 100%;
    width: 6px;
    background: linear-gradient(180deg, #60a5fa 0%, #3b82f6 100%);
    border-radius: 3px;
    box-shadow:
      0 0 12px rgba(59, 130, 246, 0.9),
      0 0 24px rgba(59, 130, 246, 0.4);
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
    top: 15px;
    transform: translateX(-50%);
  }

  .bus-emoji {
    position: absolute;
    top: 48px;
    transform: translateX(-50%);
    font-size: 18px;
    z-index: 50;
    filter: drop-shadow(0 2px 6px rgba(0, 0, 0, 0.6));
  }

  .bus-info {
    position: absolute;
    top: 48px;
    transform: translateX(10px);
    display: flex;
    align-items: center;
    gap: 3px;
    padding: 2px 5px;
    background: rgba(15, 23, 42, 0.95);
    border-radius: 4px;
    border: 1px solid rgba(148, 163, 184, 0.2);
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.4);
    z-index: 49;
  }

  .nearest-stop {
    font-size: 11px;
    font-family: "JetBrains Mono", monospace;
    font-weight: 600;
    color: #fbbf24;
  }

  .nearest-prob {
    font-size: 10px;
    font-family: "JetBrains Mono", monospace;
    font-weight: bold;
  }

  .nearest-state {
    font-size: 7px;
    font-family: "JetBrains Mono", monospace;
    padding: 1px 3px;
    border-radius: 2px;
    color: #000;
    font-weight: 600;
    text-transform: uppercase;
  }
</style>

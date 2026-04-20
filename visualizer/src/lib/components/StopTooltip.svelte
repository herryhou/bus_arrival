<!-- visualizer/src/lib/components/StopTooltip.svelte -->
<script lang="ts">
  import type { StopTraceState } from '$lib/types';
  import { getProbabilityColor, getProbabilityLabel } from '$lib/utils/probabilityColors';

  interface Props {
    stopIndex: number;
    stopState: StopTraceState | null;
    vCms: number;
    x: number;
    y: number;
  }

  let { stopIndex, stopState, vCms, x, y }: Props = $props();

  const probColor = $derived(stopState ? getProbabilityColor(stopState.probability) : '#666');
  const probLabel = $derived(stopState ? getProbabilityLabel(stopState.probability) : 'No Data');

  // Bayesian weights
  const weights = { p1: 13, p2: 6, p3: 10, p4: 3 };
  const weightSum = 32;

  function calculateContribution(value: number, weight: number): number {
    return Math.round((value * weight) / weightSum);
  }
</script>

{#if stopState}
  <div class="stop-tooltip" style="left: {x}px; top: {y}px;">
    <div class="tooltip-header">
      <span class="stop-name">Stop #{stopIndex + 1}</span>
      <span class="fsm-state">{stopState.fsm_state}</span>
    </div>

    <div class="probability-section">
      <div class="prob-main">
        <span class="prob-label">P</span>
        <span class="prob-value" style="color: {probColor}">
          {stopState.probability}
        </span>
        <span class="prob-max">/255</span>
        <span class="prob-label-text" style="color: {probColor}">{probLabel}</span>
      </div>

      <div class="features-grid">
        <div class="feature-item">
          <span class="feat-name">p₁</span>
          <span class="feat-val">{stopState.features.p1}</span>
          <span class="feat-contrib">+{calculateContribution(stopState.features.p1, weights.p1)}</span>
        </div>
        <div class="feature-item">
          <span class="feat-name">p₂</span>
          <span class="feat-val">{stopState.features.p2}</span>
          <span class="feat-contrib">+{calculateContribution(stopState.features.p2, weights.p2)}</span>
        </div>
        <div class="feature-item">
          <span class="feat-name">p₃</span>
          <span class="feat-val">{stopState.features.p3}</span>
          <span class="feat-contrib">+{calculateContribution(stopState.features.p3, weights.p3)}</span>
        </div>
        <div class="feature-item">
          <span class="feat-name">p₄</span>
          <span class="feat-val">{stopState.features.p4}</span>
          <span class="feat-contrib">+{calculateContribution(stopState.features.p4, weights.p4)}</span>
        </div>
      </div>
    </div>

    <div class="metrics-section">
      <div class="metric">
        <span class="metric-label">Dist</span>
        <span class="metric-value">{Math.abs(stopState.distance_cm)} cm</span>
      </div>
      <div class="metric">
        <span class="metric-label">Speed</span>
        <span class="metric-value">{vCms} cm/s</span>
      </div>
      <div class="metric">
        <span class="metric-label">Dwell</span>
        <span class="metric-value">{stopState.dwell_time_s} s</span>
      </div>
    </div>
  </div>
{:else}
  <div class="stop-tooltip no-data" style="left: {x}px; top: {y}px;">
    <span class="stop-name">Stop #{stopIndex + 1}</span>
    <span class="no-data-text">No data for current time</span>
  </div>
{/if}

<style>
  .stop-tooltip {
    position: absolute;
    background-color: #1a1a1a;
    border: 1px solid #333;
    border-radius: 8px;
    padding: 12px;
    min-width: 200px;
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
    padding-bottom: 8px;
    border-bottom: 1px solid #333;
  }

  .stop-name {
    font-size: 12px;
    font-weight: bold;
    color: #fff;
  }

  .fsm-state {
    font-size: 10px;
    padding: 2px 6px;
    border-radius: 3px;
    background-color: #3b82f6;
    color: #fff;
  }

  .probability-section {
    margin-bottom: 8px;
  }

  .prob-main {
    display: flex;
    align-items: baseline;
    gap: 4px;
    margin-bottom: 8px;
  }

  .prob-label {
    font-size: 10px;
    color: #888;
  }

  .prob-value {
    font-size: 18px;
    font-weight: bold;
  }

  .prob-max {
    font-size: 10px;
    color: #555;
  }

  .prob-label-text {
    font-size: 10px;
    font-weight: 500;
  }

  .features-grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 4px;
    background-color: #111;
    padding: 6px;
    border-radius: 4px;
  }

  .feature-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 9px;
  }

  .feat-name {
    color: #3b82f6;
    font-weight: bold;
  }

  .feat-val {
    color: #00ff00;
    background-color: #000;
    padding: 1px 4px;
    border-radius: 2px;
  }

  .feat-contrib {
    color: #888;
  }

  .metrics-section {
    display: flex;
    justify-content: space-between;
    gap: 8px;
  }

  .metric {
    display: flex;
    flex-direction: column;
    align-items: center;
  }

  .metric-label {
    font-size: 8px;
    color: #666;
  }

  .metric-value {
    font-size: 10px;
    color: #e0e0e0;
  }

  .no-data {
    padding: 8px 12px;
  }

  .no-data-text {
    font-size: 10px;
    color: #666;
  }
</style>

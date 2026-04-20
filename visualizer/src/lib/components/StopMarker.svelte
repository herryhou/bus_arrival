<script lang="ts">
  import type { FsmState } from '$lib/types';
  import { getProbabilityColor } from '$lib/utils/probabilityColors';
  import { FSM_STATE_COLORS, FSM_STATE_ABBREVS } from '$lib/constants/fsmColors';

  interface Props {
    stopIndex: number;
    probability: number;
    fsmState: FsmState | null;
    isSelected: boolean;
    isHovered: boolean;
  }

  let { stopIndex, probability, fsmState, isSelected, isHovered }: Props = $props();

  const color = $derived(getProbabilityColor(probability));
  const stateColor = $derived(fsmState ? FSM_STATE_COLORS[fsmState] : '#666');
  const stateAbbrev = $derived(fsmState ? FSM_STATE_ABBREVS[fsmState] : '');

  function dispatchClick() {
    const event = new CustomEvent('stopclick', {
      bubbles: true,
      detail: { stopIndex }
    });
    dispatchEvent(event);
  }

  function dispatchHover() {
    const event = new CustomEvent('stophover', {
      bubbles: true,
      detail: { stopIndex }
    });
    dispatchEvent(event);
  }

  function dispatchHoverEnd() {
    const event = new CustomEvent('stophoverend', {
      bubbles: true,
      detail: { stopIndex }
    });
    dispatchEvent(event);
  }
</script>

<div
  class="stop-marker"
  class:selected={isSelected}
  class:hovered={isHovered}
  onmouseenter={dispatchHover}
  onmouseleave={dispatchHoverEnd}
  onclick={dispatchClick}
  onkeydown={(e) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      dispatchClick();
    }
  }}
  role="button"
  tabindex="0"
>
  <span class="stop-number">{stopIndex + 1}</span>
  <span class="probability-value" style="color: {color}">{probability}</span>
  {#if fsmState && stateAbbrev}
    <span class="state-badge" style="background-color: {stateColor}">{stateAbbrev}</span>
  {/if}
</div>

<style>
  .stop-marker {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    cursor: pointer;
    padding: 4px;
    border-radius: 4px;
    transition: background-color 0.15s ease;
    min-width: 40px;
  }
  .stop-marker:hover { background-color: rgba(255, 255, 255, 0.1); }
  .stop-marker.selected { background-color: rgba(255, 255, 255, 0.15); }
  .stop-marker.selected .stop-number { font-weight: bold; color: #f59e0b; }
  .stop-number { font-size: 11px; font-family: 'JetBrains Mono', monospace; color: #ffffff; }
  .probability-value { font-size: 10px; font-family: 'JetBrains Mono', monospace; font-weight: bold; }
  .state-badge { font-size: 8px; font-family: 'JetBrains Mono', monospace; padding: 1px 4px; border-radius: 2px; color: #000; font-weight: 500; text-transform: uppercase; }
</style>

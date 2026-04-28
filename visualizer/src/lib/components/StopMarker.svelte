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
    onstopclick?: (e: CustomEvent<{ stopIndex: number }>) => void;
    onstophover?: (e: CustomEvent<{ stopIndex: number }>) => void;
    onstophoverend?: (e: CustomEvent<{ stopIndex: number }>) => void;
  }

  let { stopIndex, probability, fsmState, isSelected, isHovered, onstopclick, onstophover, onstophoverend }: Props = $props();

  const color = $derived(getProbabilityColor(probability));
  const stateColor = $derived(fsmState ? FSM_STATE_COLORS[fsmState] : '#666');
  const stateAbbrev = $derived(fsmState ? FSM_STATE_ABBREVS[fsmState] : '');
  const isOdd = $derived(stopIndex % 2 === 0); // 0-indexed, so index 0 = stop #1 (odd)

  function dispatchClick() {
    const event = new CustomEvent('stopclick', {
      bubbles: true,
      detail: { stopIndex }
    });
    dispatchEvent(event);
    onstopclick?.(event);
  }

  function dispatchHover() {
    const event = new CustomEvent('stophover', {
      bubbles: true,
      detail: { stopIndex }
    });
    dispatchEvent(event);
    onstophover?.(event);
  }

  function dispatchHoverEnd() {
    const event = new CustomEvent('stophoverend', {
      bubbles: true,
      detail: { stopIndex }
    });
    dispatchEvent(event);
    onstophoverend?.(event);
  }
</script>

<div
  class="stop-marker"
  class:selected={isSelected}
  class:hovered={isHovered}
  class:odd={isOdd}
  class:even={!isOdd}
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
</div>

<style>
  .stop-marker {
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    padding: 2px 6px;
    border-radius: 4px;
    transition: all 0.15s ease;
    min-width: 20px;
  }
  .stop-marker:hover { background-color: rgba(255, 255, 255, 0.1); transform: scale(1.1); }
  .stop-marker.selected { background-color: rgba(255, 255, 255, 0.15); }
  .stop-marker.selected .stop-number { font-weight: bold; color: #f59e0b; }

  .stop-number {
    font-size: 13px;
    font-family: 'JetBrains Mono', monospace;
    font-weight: 600;
  }

  /* Odd stops (1, 3, 5, ...) - cyan/blue */
  .stop-marker:where(.odd) .stop-number {
    color: #22d3ee; /* cyan-400 */
    text-shadow: 0 0 4px rgba(34, 211, 238, 0.5);
  }

  /* Even stops (2, 4, 6, ...) - orange/amber */
  .stop-marker:where(.even) .stop-number {
    color: #fbbf24; /* amber-400 */
    text-shadow: 0 0 4px rgba(251, 191, 36, 0.5);
  }

  /* Default color fallback */
  .stop-marker:not(.odd):not(.even) .stop-number {
    color: #ffffff;
  }
</style>

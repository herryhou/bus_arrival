<!-- visualizer/src/lib/components/BusTooltip.svelte -->
<script lang="ts">
  import type { TraceRecord } from "$lib/types";

  interface Props {
    record: TraceRecord | null;
    x: number | null;
    y: number | null;
  }

  let { record, x, y }: Props = $props();

  // Format record as readable JSON
  const jsonDisplay = $derived(
    record ? JSON.stringify(record, null, 2) : ""
  );
</script>

{#if record && x !== null && y !== null}
  <div class="bus-tooltip" style="left: {x}px; top: {y}px;">
    <div class="tooltip-header">
      <span class="title">TRACE RECORD</span>
    </div>
    <pre class="json-content">{jsonDisplay}</pre>
  </div>
{/if}

<style>
  .bus-tooltip {
    position: fixed;
    background-color: #1a1a1a;
    border: 1px solid #333;
    border-radius: 8px;
    padding: 0;
    min-width: 300px;
    max-width: 500px;
    max-height: 80vh;
    overflow: hidden;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.5);
    z-index: 1000;
    pointer-events: none;
    font-family: "JetBrains Mono", monospace;
  }

  .tooltip-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 8px 12px;
    border-bottom: 1px solid #333;
    background-color: #0f0f0f;
    border-radius: 8px 8px 0 0;
  }

  .title {
    font-size: 11px;
    font-weight: bold;
    color: #22c55e;
    letter-spacing: 0.05em;
  }

  .json-content {
    margin: 0;
    padding: 12px;
    font-size: 10px;
    line-height: 1.4;
    color: #e0e0e0;
    overflow: auto;
    max-height: calc(80vh - 40px);
    white-space: pre;
  }

  /* JSON syntax highlighting */
  .json-content :global(string) {
    color: #a5d6ff;
  }

  .json-content :global(number) {
    color: #79c0ff;
  }

  .json-content :global(boolean) {
    color: #ff7b72;
  }

  .json-content :global(null) {
    color: #ffa657;
  }
</style>

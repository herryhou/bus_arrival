<!-- visualizer/src/lib/components/BusDiagnostic.svelte -->
<script lang="ts">
  import type { TraceRecord } from "$lib/types";

  interface Props {
    record: TraceRecord | null;
  }

  let { record }: Props = $props();

  // Format record as readable JSON
  const jsonDisplay = $derived(
    record ? JSON.stringify(record, null, 2) : "No trace data available"
  );
</script>

<div class="bus-diagnostic">
  <div class="diagnostic-header">
    <span class="title">BUS DIAGNOSTIC</span>
    {#if record}
      <span class="timestamp">
        {new Date((record.time ?? 0) * 1000).toLocaleTimeString()}
      </span>
    {/if}
  </div>
  <pre class="json-content">{jsonDisplay}</pre>
</div>

<style>
  .bus-diagnostic {
    height: 100%;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .diagnostic-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.75rem 1rem;
    border-bottom: 1px solid #333;
    background-color: #0f0f0f;
  }

  .title {
    font-size: 11px;
    font-weight: bold;
    color: #22c55e;
    letter-spacing: 0.05em;
  }

  .timestamp {
    font-size: 10px;
    font-family: "JetBrains Mono", monospace;
    color: #666;
  }

  .json-content {
    margin: 0;
    padding: 1rem;
    font-size: 11px;
    line-height: 1.5;
    color: #e0e0e0;
    overflow: auto;
    flex: 1;
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

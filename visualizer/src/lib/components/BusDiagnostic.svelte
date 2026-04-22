<!-- visualizer/src/lib/components/BusDiagnostic.svelte -->
<script lang="ts">
  import type { TraceRecord } from "$lib/types";

  interface Props {
    record: TraceRecord | null;
  }

  let { record }: Props = $props();

  // Simple JSON syntax highlighter
  function highlightJson(json: string): string {
    return json.replace(
      /("(?:[^"\\]|\\.)*")\s*:|("(?:[^"\\]|\\.)*")|(\b\d+\.?\d*\b)|(\btrue\b|\bfalse\b)|(\bnull\b)/g,
      (match, key, string, number, boolean, nullVal) => {
        if (key) return `<span class="json-key">${key}</span>:`;
        if (string) return `<span class="json-string">${string}</span>`;
        if (number) return `<span class="json-number">${number}</span>`;
        if (boolean) return `<span class="json-boolean">${boolean}</span>`;
        if (nullVal) return `<span class="json-null">${nullVal}</span>`;
        return match;
      },
    );
  }

  // Format record as readable JSON with syntax highlighting
  const jsonDisplay = $derived(
    record
      ? highlightJson(JSON.stringify(record, null, 2))
      : "No trace data available",
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
  <pre class="json-content">{@html jsonDisplay}</pre>
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
  .json-content :global(.json-key) {
    color: #ffa657;
    font-weight: bold;
  }

  .json-content :global(.json-string) {
    color: #a5d6ff;
  }

  .json-content :global(.json-number) {
    color: #9ed2b0;
  }

  .json-content :global(.json-boolean) {
    color: #8cff72;
  }

  .json-content :global(.json-null) {
    color: #ffa657;
  }
</style>

<!-- visualizer/src/lib/components/UploadScreen.svelte -->
<script lang="ts">
  import type { RouteData, TraceData } from '$lib/types';
  import { loadRouteData } from '$lib/parsers/routeData';
  import { loadTraceFile, getTraceTimeRange } from '$lib/parsers/trace';
  import {
    pickRouteFile,
    pickTraceFile,
    isFileSystemAccessAPISupported
  } from '$lib/utils/filePicker';

  interface Props {
    onLoad: (data: { routeData: RouteData; traceData: TraceData }) => void;
  }

  let { onLoad }: Props = $props();

  let routeData = $state<RouteData | null>(null);
  let traceData = $state<TraceData | null>(null);
  let discoveredTraceFile = $state<File | null>(null);
  let useDiscoveredTrace = $state(true);

  let loading = $state(false);
  let error = $state<string | null>(null);
  let useFileSystemAPI = $state(isFileSystemAccessAPISupported());

  // Traditional file input fallbacks
  let routeFileInput = $state<HTMLInputElement | null>(null);
  let traceFileInput = $state<HTMLInputElement | null>(null);

  async function handleSmartRoutePick() {
    loading = true;
    error = null;

    try {
      const result = await pickRouteFile();
      if (!result) return;

      routeData = await loadRouteData(result.routeFile);

      if (result.traceFile && result.autoDiscovered) {
        discoveredTraceFile = result.traceFile;
        useDiscoveredTrace = true;
      }

      checkAndLoad();
    } catch (e) {
      error = `Failed to load route: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      loading = false;
    }
  }

  async function handleSmartTracePick() {
    loading = true;
    error = null;

    try {
      const file = await pickTraceFile();
      if (!file) return;

      traceData = await loadTraceFile(file);
      checkAndLoad();
    } catch (e) {
      error = `Failed to load trace: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      loading = false;
    }
  }

  async function handleRouteUpload() {
    const file = routeFileInput?.files?.[0];
    if (!file) return;

    loading = true;
    error = null;

    try {
      routeData = await loadRouteData(file);
      checkAndLoad();
    } catch (e) {
      error = `Failed to load route: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      loading = false;
    }
  }

  async function handleTraceUpload() {
    const file = traceFileInput?.files?.[0];
    if (!file) return;

    loading = true;
    error = null;

    try {
      traceData = await loadTraceFile(file);
      checkAndLoad();
    } catch (e) {
      error = `Failed to load trace: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      loading = false;
    }
  }

  async function checkAndLoad() {
    // Load discovered trace if available and enabled
    if (discoveredTraceFile && useDiscoveredTrace && !traceData) {
      try {
        traceData = await loadTraceFile(discoveredTraceFile);
      } catch (e) {
        error = `Failed to load discovered trace: ${e instanceof Error ? e.message : String(e)}`;
        return;
      }
    }

    // Check if both loaded
    if (routeData && traceData) {
      onLoad({ routeData, traceData });
    }
  }

  function reset() {
    routeData = null;
    traceData = null;
    discoveredTraceFile = null;
    useDiscoveredTrace = true;
    error = null;
  }
</script>

<div class="upload-screen">
  <div class="upload-card">
    <h1 class="title">Bus Arrival Lab</h1>
    <p class="subtitle">Scientific Arrival Detection Visualization</p>

    {#if error}
      <div class="error-banner">{error}</div>
    {/if}

    {#if useFileSystemAPI}
      <!-- File System Access API mode -->
      <div class="upload-section">
        <button onclick={handleSmartRoutePick} class="btn-primary" disabled={loading}>
          {loading ? 'Loading...' : 'Select Route Data (.bin)'}
        </button>

        {#if discoveredTraceFile}
          <div class="discovered-trace">
            <label class="checkbox-label">
              <input type="checkbox" bind:checked={useDiscoveredTrace} />
              <span>Auto-load discovered trace: {discoveredTraceFile.name}</span>
            </label>
          </div>
        {/if}

        {#if !discoveredTraceFile || !useDiscoveredTrace}
          <button onclick={handleSmartTracePick} class="btn-secondary" disabled={loading}>
            {loading ? 'Loading...' : 'Select Trace Data (.jsonl)'}
          </button>
        {/if}

        {#if routeData}
          <div class="status-badge success">Route Ready</div>
        {/if}
        {#if traceData}
          <div class="status-badge success">Trace Ready</div>
        {/if}
      </div>

      <div class="fallback-link">
        <button onclick={() => useFileSystemAPI = false} class="btn-link">
          Use traditional file input instead
        </button>
      </div>
    {:else}
      <!-- Traditional file input mode -->
      <div class="upload-section">
        <div class="upload-item">
          <label for="route-file" class="file-label">
            <div class="label-text">Route Data (.bin)</div>
          </label>
          <input bind:this={routeFileInput} id="route-file" type="file" accept=".bin" onchange={handleRouteUpload} class="file-input" />
          {#if routeData}<div class="status-badge success">READY</div>{/if}
        </div>

        <div class="upload-item">
          <label for="trace-file" class="file-label">
            <div class="label-text">Trace Data (.jsonl)</div>
          </label>
          <input bind:this={traceFileInput} id="trace-file" type="file" accept=".jsonl" onchange={handleTraceUpload} class="file-input" />
          {#if traceData}<div class="status-badge success">READY</div>{/if}
        </div>
      </div>

      {#if isFileSystemAccessAPISupported()}
        <div class="fallback-link">
          <button onclick={() => useFileSystemAPI = true} class="btn-link">
            Use smart file picker instead
          </button>
        </div>
      {/if}
    {/if}

    {#if routeData && traceData}
      <div class="ready-message">
        <span class="check">✓</span> Both files loaded — initializing visualization...
      </div>
    {/if}
  </div>
</div>

<style>
  .upload-screen {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100vh;
  }

  .upload-card {
    background-color: #1a1a1a;
    border: 1px solid #333;
    border-radius: 0.5rem;
    padding: 2.5rem;
    width: 450px;
    text-align: center;
    box-shadow: 0 10px 25px rgba(0,0,0,0.5);
  }

  .title {
    font-size: 1.5rem;
    margin-bottom: 0.5rem;
    color: #fff;
  }

  .subtitle {
    font-size: 0.875rem;
    color: #666;
    margin-bottom: 2rem;
  }

  .error-banner {
    background-color: rgba(239, 68, 68, 0.2);
    border: 1px solid #ef4444;
    color: #ef4444;
    padding: 0.75rem;
    border-radius: 0.25rem;
    font-size: 0.875rem;
    margin-bottom: 1rem;
  }

  .upload-section {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    margin-bottom: 1rem;
  }

  .btn-primary,
  .btn-secondary {
    padding: 0.75rem 1.5rem;
    border-radius: 0.25rem;
    font-size: 0.875rem;
    font-family: 'JetBrains Mono', monospace;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .btn-primary {
    background-color: #3b82f6;
    border: 1px solid #3b82f6;
    color: #fff;
  }

  .btn-primary:hover:not(:disabled) {
    background-color: #2563eb;
  }

  .btn-primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn-secondary {
    background-color: transparent;
    border: 1px solid #444;
    color: #e0e0e0;
  }

  .btn-secondary:hover:not(:disabled) {
    border-color: #666;
    background-color: rgba(255, 255, 255, 0.05);
  }

  .discovered-trace {
    background-color: rgba(34, 197, 94, 0.1);
    border: 1px solid #22c55e;
    border-radius: 0.25rem;
    padding: 0.75rem;
  }

  .checkbox-label {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    cursor: pointer;
    font-size: 0.875rem;
    color: #22c55e;
  }

  .checkbox-label input[type="checkbox"] {
    cursor: pointer;
  }

  .status-badge {
    font-size: 0.75rem;
    padding: 0.25rem 0.75rem;
    border-radius: 0.25rem;
  }

  .status-badge.success {
    background-color: rgba(34, 197, 94, 0.2);
    color: #22c55e;
  }

  .upload-item {
    background-color: #111;
    border: 1px dashed #444;
    border-radius: 0.25rem;
    padding: 1rem;
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .file-label {
    cursor: pointer;
  }

  .label-text {
    color: #3b82f6;
    font-size: 0.875rem;
  }

  .file-input {
    display: none;
  }

  .fallback-link {
    margin-top: 1rem;
  }

  .btn-link {
    background: none;
    border: none;
    color: #666;
    font-size: 0.75rem;
    cursor: pointer;
    text-decoration: underline;
  }

  .btn-link:hover {
    color: #888;
  }

  .ready-message {
    background-color: rgba(34, 197, 94, 0.1);
    border: 1px solid #22c55e;
    color: #22c55e;
    padding: 0.75rem;
    border-radius: 0.25rem;
    font-size: 0.875rem;
  }

  .check {
    font-size: 1.25rem;
    margin-right: 0.5rem;
  }
</style>

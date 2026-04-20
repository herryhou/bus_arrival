# Visualizer Enhancement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add probability visualization to LinearRouteWidget and implement smart file loading for auto-discovering matching trace files.

**Architecture:** Enhance existing canvas-based LinearRouteWidget with probability color-coding and FSM state badges, plus add File System Access API for directory scanning when loading route data.

**Tech Stack:** Svelte 5, TypeScript, Canvas API, File System Access API

---

## File Structure

```
visualizer/
├── src/lib/
│   ├── components/
│   │   ├── LinearRouteWidget.svelte     (MODIFY - add prob viz)
│   │   ├── StopMarker.svelte            (NEW - HTML-based marker)
│   │   ├── StopTooltip.svelte           (NEW - detailed breakdown)
│   │   └── UploadScreen.svelte          (NEW - extract from +page.svelte)
│   ├── utils/
│   │   ├── filePicker.ts                (NEW - directory scanning)
│   │   └── probabilityColors.ts         (NEW - color scale constants)
│   └── constants/
│       └── fsmColors.ts                 (MODIFY - add abbreviations)
└── src/routes/
    └── +page.svelte                     (MODIFY - use UploadScreen)
```

---

## Task 1: Add Probability Color Scale Constants

**Files:**
- Create: `visualizer/src/lib/utils/probabilityColors.ts`

- [ ] **Step 1: Create probability color scale constants**

```typescript
// visualizer/src/lib/utils/probabilityColors.ts

/**
 * Probability color scale for stop markers
 * Maps 0-255 probability values to visual colors
 */

export interface ProbabilityColor {
  color: string;
  label: string;
}

/**
 * Get color for probability value (0-255)
 */
export function getProbabilityColor(probability: number): string {
  if (probability < 64) return '#666666';      // Gray - no chance
  if (probability < 128) return '#eab308';     // Yellow - low
  if (probability < 191) return '#f97316';     // Orange - medium
  return '#22c55e';                             // Green - high/arrived
}

/**
 * Get label for probability range
 */
export function getProbabilityLabel(probability: number): string {
  if (probability < 64) return 'None';
  if (probability < 128) return 'Low';
  if (probability < 191) return 'Medium';
  return 'High';
}

/**
 * Get text color for contrast against probability color
 */
export function getProbabilityTextColor(probability: number): string {
  if (probability < 191) return '#ffffff';
  return '#000000';
}
```

- [ ] **Step 2: Run TypeScript check**

Run: `cd visualizer && npm run check`

Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add visualizer/src/lib/utils/probabilityColors.ts
git commit -m "feat(vis): add probability color scale utilities"
```

---

## Task 2: Add FSM State Abbreviations

**Files:**
- Modify: `visualizer/src/lib/constants/fsmColors.ts`

- [ ] **Step 1: Add abbreviation constants**

```typescript
// visualizer/src/lib/constants/fsmColors.ts
// Add after FSM_STATE_LABELS:

/**
 * Abbreviated labels for FSM states (for small badges)
 */
export const FSM_STATE_ABBREVS: Record<FsmState, string> = {
  'Idle': 'Idl',
  'Approaching': 'App',
  'Arriving': 'Arr',
  'AtStop': 'AtS',
  'Departed': 'Dep',
  'TripComplete': 'Com'
};
```

- [ ] **Step 2: Run TypeScript check**

Run: `cd visualizer && npm run check`

Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add visualizer/src/lib/constants/fsmColors.ts
git commit -m "feat(vis): add FSM state abbreviation constants"
```

---

## Task 3: Create StopMarker Component

**Files:**
- Create: `visualizer/src/lib/components/StopMarker.svelte`

- [ ] **Step 1: Create StopMarker component**

```svelte
<!-- visualizer/src/lib/components/StopMarker.svelte -->
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
  role="button"
  tabindex="0"
>
  <!-- Stop number -->
  <span class="stop-number">{stopIndex + 1}</span>

  <!-- Probability value -->
  <span class="probability-value" style="color: {color}">{probability}</span>

  <!-- FSM state badge -->
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

  .stop-marker:hover {
    background-color: rgba(255, 255, 255, 0.1);
  }

  .stop-marker.selected {
    background-color: rgba(255, 255, 255, 0.15);
  }

  .stop-marker.selected .stop-number {
    font-weight: bold;
    color: #f59e0b;
  }

  .stop-number {
    font-size: 11px;
    font-family: 'JetBrains Mono', monospace;
    color: #ffffff;
  }

  .probability-value {
    font-size: 10px;
    font-family: 'JetBrains Mono', monospace;
    font-weight: bold;
  }

  .state-badge {
    font-size: 8px;
    font-family: 'JetBrains Mono', monospace;
    padding: 1px 4px;
    border-radius: 2px;
    color: #000;
    font-weight: 500;
    text-transform: uppercase;
  }
</style>
```

- [ ] **Step 2: Run TypeScript check**

Run: `cd visualizer && npm run check`

Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add visualizer/src/lib/components/StopMarker.svelte
git commit -m "feat(vis): add StopMarker component with probability viz"
```

---

## Task 4: Create StopTooltip Component

**Files:**
- Create: `visualizer/src/lib/components/StopTooltip.svelte`

- [ ] **Step 1: Create StopTooltip component**

```svelte
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
```

- [ ] **Step 2: Run TypeScript check**

Run: `cd visualizer && npm run check`

Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add visualizer/src/lib/components/StopTooltip.svelte
git commit -m "feat(vis): add StopTooltip component with detailed breakdown"
```

---

## Task 5: Create File Picker Utility

**Files:**
- Create: `visualizer/src/lib/utils/filePicker.ts`

- [ ] **Step 1: Create file picker utility**

```typescript
// visualizer/src/lib/utils/filePicker.ts

/**
 * File System Access API wrapper for smart file discovery
 * Auto-discovers matching trace.jsonl when user selects a .bin file
 */

export interface FilePickerResult {
  routeFile: File;
  traceFile: File | null;
  autoDiscovered: boolean;
}

export interface FilePickerOptions {
  accept?: string;
  multiple?: boolean;
}

/**
 * Check if File System Access API is supported
 */
export function isFileSystemAccessAPISupported(): boolean {
  return 'showOpenFilePicker' in window;
}

/**
 * Extract basename from filename (without extension)
 */
function extractBasename(filename: string): string {
  const lastDot = filename.lastIndexOf('.');
  return lastDot >= 0 ? filename.substring(0, lastDot) : filename;
}

/**
 * Generate possible trace filenames from route basename
 */
function generateTraceFilenames(routeBasename: string): string[] {
  return [
    `${routeBasename}_trace.jsonl`,
    `${routeBasename}.jsonl`,
    `${routeBasename}_trace.json`,
    `${routeBasename}.json`,
  ];
}

/**
 * Pick a route file and auto-discover matching trace file
 */
export async function pickRouteFile(): Promise<FilePickerResult | null> {
  if (!isFileSystemAccessAPISupported()) {
    return null; // Fall back to traditional input
  }

  try {
    const [handle] = await window.showOpenFilePicker({
      types: [{
        description: 'Route Data',
        accept: { 'application/octet-stream': ['.bin'] }
      }],
      multiple: false
    });

    const routeFile = await handle.getFile();
    const routeBasename = extractBasename(routeFile.name);

    // Try to get parent directory handle
    let traceFile: File | null = null;
    let autoDiscovered = false;

    try {
      // Get directory handle (requires permission)
      const dirHandle = await handle.getParent?.();
      if (dirHandle) {
        for await (const entry of dirHandle.values()) {
          if (entry.kind === 'file') {
            const traceName = entry.name;
            const possibleNames = generateTraceFilenames(routeBasename);
            if (possibleNames.includes(traceName)) {
              const fileHandle = entry as FileSystemFileHandle;
              traceFile = await fileHandle.getFile();
              autoDiscovered = true;
              break;
            }
          }
        }
      }
    } catch (e) {
      // getParent() not supported or permission denied - no auto-discovery
      console.debug('Could not access parent directory:', e);
    }

    return { routeFile, traceFile, autoDiscovered };
  } catch (e) {
    // User cancelled or error
    if ((e as Error).name !== 'AbortError') {
      console.error('File picker error:', e);
    }
    return null;
  }
}

/**
 * Pick a trace file manually (fallback)
 */
export async function pickTraceFile(): Promise<File | null> {
  if (!isFileSystemAccessAPISupported()) {
    return null;
  }

  try {
    const [handle] = await window.showOpenFilePicker({
      types: [{
        description: 'Trace Data',
        accept: { 'application/jsonl': ['.jsonl', '.json'] }
      }],
      multiple: false
    });

    return await handle.getFile();
  } catch (e) {
    if ((e as Error).name !== 'AbortError') {
      console.error('Trace file picker error:', e);
    }
    return null;
  }
}
```

- [ ] **Step 2: Run TypeScript check**

Run: `cd visualizer && npm run check`

Expected: No errors (may need DOM types)

- [ ] **Step 3: Add DOM type references if needed**

If TypeScript complains about `window.showOpenFilePicker`, add to top of file:

```typescript
/// <reference lib="dom" />
```

- [ ] **Step 4: Commit**

```bash
git add visualizer/src/lib/utils/filePicker.ts
git commit -m "feat(vis): add File System Access API wrapper for smart file discovery"
```

---

## Task 6: Create UploadScreen Component

**Files:**
- Create: `visualizer/src/lib/components/UploadScreen.svelte`

- [ ] **Step 1: Create UploadScreen component**

```svelte
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
```

- [ ] **Step 2: Run TypeScript check**

Run: `cd visualizer && npm run check`

Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add visualizer/src/lib/components/UploadScreen.svelte
git commit -m "feat(vis): add UploadScreen component with smart file discovery"
```

---

## Task 7: Rewrite LinearRouteWidget with HTML (Replace Canvas)

**Files:**
- Modify: `visualizer/src/lib/components/LinearRouteWidget.svelte`

- [ ] **Step 1: Replace canvas with HTML-based rendering**

```svelte
<!-- visualizer/src/lib/components/LinearRouteWidget.svelte -->
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
    return traceData.reduce((prev, curr) =>
      Math.abs(curr.time - currentTime) < Math.abs(prev.time - currentTime) ? curr : prev
    );
  });

  // Get stop state for a stop at current time
  function getStopState(stopIndex: number): StopTraceState | null {
    if (!currentRecord) return null;
    return currentRecord.stop_states.find(s => s.stop_idx === stopIndex) || null;
  }

  function progressToPercent(progressCm: number): number {
    return maxProgress > 0 ? (Math.min(progressCm, maxProgress) / maxProgress) * 100 : 0;
  }

  function handleStopClick(idx: number) {
    onStopClick?.(idx);
  }

  function handleStopHover(idx: number, event: MouseEvent) {
    hoveredStop = idx;
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
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
      .map((stop, index) => ({
        stop,
        index,
        distance: Math.abs(stop.progress_cm - busProgress)
      }))
      .filter(({ distance }) => distance <= SEARCH_RADIUS_CM)
      .sort((a, b) => a.distance - b.distance)
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
      {#each routeData.stops as stop (stop.progress_cm)}
        {@const stopState = getStopState(routeData.stops.indexOf(stop))}
        {@const isHighlighted = highlightedEvent?.stopIdx === routeData.stops.indexOf(stop)}
        {@const isHovered = hoveredStop === routeData.stops.indexOf(stop)}
        {@const isSelected = false} // Will be passed from parent

        <div
          class="stop-wrapper"
          style="left: {progressToPercent(stop.progress_cm)}%"
        >
          <StopMarker
            stopIndex={routeData.stops.indexOf(stop)}
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
      <div class="bus-marker" style="left: {progressToPercent(busProgress)}%">🚌</div>
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
```

- [ ] **Step 2: Run TypeScript check**

Run: `cd visualizer && npm run check`

Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add visualizer/src/lib/components/LinearRouteWidget.svelte
git commit -m "feat(vis): rewrite LinearRouteWidget with HTML-based rendering"
```

---

## Task 8: Update +page.svelte to Use UploadScreen

**Files:**
- Modify: `visualizer/src/routes/+page.svelte`

- [ ] **Step 1: Update imports and add UploadScreen**

Find the import section (lines 1-11) and replace with:

```typescript
<script lang="ts">
  import { onMount } from 'svelte';
  import MapView from '$lib/components/MapView.svelte';
  import FsmInspector from '$lib/components/FsmInspector.svelte';
  import ProbabilityScope from '$lib/components/ProbabilityScope.svelte';
  import CompactSidebar from '$lib/components/CompactSidebar.svelte';
  import LinearRouteWidget from '$lib/components/LinearRouteWidget.svelte';
  import Timeline from '$lib/components/Timeline.svelte';
  import UploadScreen from '$lib/components/UploadScreen.svelte';
  import type { RouteData, TraceData, FsmState } from '$lib/types';
  import { getInterpolatedBusState } from '$lib/parsers/routeData';
  import { getTraceTimeRange } from '$lib/parsers/trace';
```

- [ ] **Step 2: Remove file input refs and update state**

Find and remove these lines (around 35-36):
```typescript
let routeFileInput = $state<HTMLInputElement | null>(null);
let traceFileInput = $state<HTMLInputElement | null>(null);
```

- [ ] **Step 3: Replace upload handlers**

Find `handleRouteUpload` and `handleTraceUpload` functions (around lines 62-92) and replace with:

```typescript
  function handleDataLoad(data: { routeData: RouteData; traceData: TraceData }) {
    routeData = data.routeData;
    traceData = data.traceData;
    [timeMin, timeMax] = getTraceTimeRange(traceData);
    currentTime = timeMin;
    currentTimePercent = 0;
    showUpload = false;
  }
```

- [ ] **Step 4: Remove checkReady function**

Remove the `checkReady` function (lines 94-98) as it's no longer needed.

- [ ] **Step 5: Update upload screen HTML**

Find the `{#if showUpload}` block (around line 167) and replace the entire upload card section with:

```svelte
  {#if showUpload}
    <UploadScreen onLoad={handleDataLoad} />
  {:else}
```

- [ ] **Step 6: Update LinearRouteWidget props**

Find the LinearRouteWidget component (around line 264) and update props:

```svelte
        <LinearRouteWidget
          {routeData}
          busProgress={currentRecord.s_cm}
          busSpeed={currentRecord.v_cms}
          {highlightedEvent}
          {traceData}
          {currentTime}
          onStopClick={(idx) => selectedStop = idx}
        />
```

- [ ] **Step 7: Remove old file input handlers**

Remove these functions if they still exist:
- `handleRouteUpload`
- `handleTraceUpload`
- `checkReady`

- [ ] **Step 8: Run TypeScript check**

Run: `cd visualizer && npm run check`

Expected: No errors

- [ ] **Step 9: Test in dev mode**

Run: `cd visualizer && npm run dev`

Expected: Dev server starts, upload screen loads

- [ ] **Step 10: Commit**

```bash
git add visualizer/src/routes/+page.svelte
git commit -m "feat(vis): integrate UploadScreen and enhance LinearRouteWidget"
```

---

## Task 9: Add Tests for File Picker Utility

**Files:**
- Create: `visualizer/src/lib/utils/filePicker.test.ts`

- [ ] **Step 1: Create test file**

```typescript
// visualizer/src/lib/utils/filePicker.test.ts

import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  isFileSystemAccessAPISupported,
  extractBasename,
  generateTraceFilenames
} from './filePicker';

describe('filePicker', () => {
  describe('isFileSystemAccessAPISupported', () => {
    it('returns true when showOpenFilePicker is available', () => {
      vi.stubGlobal('showOpenFilePicker', vi.fn());
      expect(isFileSystemAccessAPISupported()).toBe(true);
      vi.unstubAllGlobals();
    });

    it('returns false when showOpenFilePicker is not available', () => {
      expect(isFileSystemAccessAPISupported()).toBe(false);
    });
  });

  describe('extractBasename', () => {
    it('extracts basename from filename with extension', () => {
      expect(extractBasename('route_data.bin')).toBe('route_data');
      expect(extractBasename('ty225_short_detour.bin')).toBe('ty225_short_detour');
    });

    it('returns full string if no extension', () => {
      expect(extractBasename('route_data')).toBe('route_data');
    });

    it('handles multiple dots', () => {
      expect(extractBasename('route.data.bin')).toBe('route.data');
    });
  });

  describe('generateTraceFilenames', () => {
    it('generates all expected filename variants', () => {
      const result = generateTraceFilenames('ty225_route');
      expect(result).toEqual([
        'ty225_route_trace.jsonl',
        'ty225_route.jsonl',
        'ty225_route_trace.json',
        'ty225_route.json'
      ]);
    });

    it('handles basename with underscores', () => {
      const result = generateTraceFilenames('ty225_short_detour');
      expect(result).toContain('ty225_short_detour_trace.jsonl');
    });
  });
});
```

- [ ] **Step 2: Run tests**

Run: `cd visualizer && npm test`

Expected: Tests pass

- [ ] **Step 3: Commit**

```bash
git add visualizer/src/lib/utils/filePicker.test.ts
git commit -m "test(vis): add file picker utility tests"
```

---

## Task 10: Manual Testing Checklist

**Files:**
- None (manual verification)

- [ ] **Step 1: Test file loading with File System Access API**

1. Start dev server: `cd visualizer && npm run dev`
2. Open browser (Chrome/Edge - supports File System Access API)
3. Click "Select Route Data (.bin)"
4. Navigate to folder containing both `.bin` and `_trace.jsonl` files
5. Select a `.bin` file
6. Verify: "Auto-load discovered trace: {filename}" checkbox appears
7. Verify: Both "Route Ready" and "Trace Ready" badges appear
8. Verify: Dashboard loads automatically

- [ ] **Step 2: Test fallback to traditional input**

1. Click "Use traditional file input instead"
2. Verify: Two file upload boxes appear
3. Select `.bin` file → "READY" badge appears
4. Select `.jsonl` file → "READY" badge appears
5. Verify: Dashboard loads

- [ ] **Step 3: Test probability visualization on linear route**

1. Load a session with trace data
2. Look at LinearRouteWidget at bottom
3. Verify: Stop circles show colors based on probability (gray/yellow/orange/green)
4. Verify: Probability values displayed below each stop
5. Verify: FSM state badges (App/Arr/AtS/Dep) displayed
6. Verify: Bus emoji moves along route when scrubbing timeline

- [ ] **Step 4: Test hover tooltips**

1. Hover over any stop marker
2. Verify: Tooltip appears with detailed breakdown
3. Verify: Final probability, feature scores (p1-p4), metrics shown
4. Verify: Tooltip follows stop position
5. Move mouse away → tooltip disappears

- [ ] **Step 5: Test stop selection**

1. Click on a stop marker
2. Verify: Stop becomes selected in sidebar
3. Verify: Map pans to stop location
4. Verify: Lab panel shows probability scope for selected stop

- [ ] **Step 6: Test browser without File System Access API**

1. Test in Safari or Firefox (no File System Access API)
2. Verify: Traditional file input is default
3. Verify: Upload works normally

- [ ] **Step 7: Test error handling**

1. Try loading a corrupt `.bin` file
2. Verify: Error message shown
3. Try loading a corrupt `.jsonl` file
4. Verify: Error message shown
5. Verify: Can recover and try again

- [ ] **Step 8: Test responsive behavior**

1. Resize browser window
2. Verify: Linear route scales correctly
3. Verify: Detail panel maintains layout
4. Verify: Tooltips don't overflow viewport

- [ ] **Step 9: Create commit for testing completion**

```bash
git commit --allow-empty -m "test(vis): complete manual testing checklist"
```

---

## Task 11: Documentation Updates

**Files:**
- Modify: `visualizer/README.md`

- [ ] **Step 1: Update README with new features**

Add to README:

```markdown
## Features

- **Smart File Loading**: When using Chrome/Edge, selecting a `.bin` file auto-discovers matching `_trace.jsonl` in the same directory
- **Probability Visualization**: Linear route view shows color-coded probabilities for all stops
- **Detailed Tooltips**: Hover over any stop to see full probability breakdown with feature scores
- **FSM State Badges**: Visual indicators for state transitions (Approaching → Arriving → AtStop → Departed)

## Browser Compatibility

| Feature | Chrome/Edge | Firefox | Safari |
|---------|-------------|---------|--------|
| Smart file loading | ✓ | ✗ | ✗ |
| Traditional upload | ✓ | ✓ | ✓ |
| Probability viz | ✓ | ✓ | ✓ |

## Usage

1. Click "Select Route Data (.bin)" or use traditional file inputs
2. If using Chrome/Edge, matching trace file is auto-discovered
3. Scrub timeline to see probabilities update in real-time
4. Hover stops for detailed breakdown
5. Click stops to select and view in Lab panel
```

- [ ] **Step 2: Commit documentation**

```bash
git add visualizer/README.md
git commit -m "docs(vis): document smart file loading and probability viz features"
```

---

## Task 12: Final Verification

**Files:**
- None (final checks)

- [ ] **Step 1: Run full test suite**

Run: `cd visualizer && npm test`

Expected: All tests pass

- [ ] **Step 2: Build production bundle**

Run: `cd visualizer && npm run build`

Expected: Build succeeds without errors

- [ ] **Step 3: Type check**

Run: `cd visualizer && npm run check`

Expected: No type errors

- [ ] **Step 4: Lint check**

Run: `cd visualizer && npm run lint` (if available)

Expected: No lint errors

- [ ] **Step 5: Git status verification**

Run: `git status visualizer/`

Expected: Only new files shown, no uncommitted changes

- [ ] **Step 6: Create final summary commit**

```bash
git commit --allow-empty -m "feat(vis): complete visualizer enhancement implementation

Features:
- Smart file loading with File System Access API
- Probability color-coding on LinearRouteWidget
- FSM state badges for all stops
- Detailed tooltips on hover
- Fallback for browsers without File System Access API

Components added:
- UploadScreen.svelte
- StopMarker.svelte
- StopTooltip.svelte
- filePicker.ts utility
- probabilityColors.ts utility
"
```

---

## Implementation Complete

All tasks completed. The visualizer now has:
1. ✅ Smart file loading with auto-discovery of matching trace files
2. ✅ Probability visualization on LinearRouteWidget with color-coding
3. ✅ FSM state badges for all stops
4. ✅ Detailed tooltips showing probability breakdown
5. ✅ Fallback support for browsers without File System Access API

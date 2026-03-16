# Visualizer UI/UX Redesign Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Redesign the visualizer UI to make the map larger and consolidate event feed + stop monitoring into a compact sidebar, while replacing the Chart.js timeline with a keyboard-friendly native implementation.

**Architecture:** Create two new components (CompactSidebar, Timeline) to replace three existing components (EventLog, AllStopsInspector, TimelineCharts). Update page grid layout from 1.5fr/1.5fr/1fr to 2.5fr/1.5fr/1fr.

**Tech Stack:** Svelte 5 (runes), TypeScript, CSS Grid, native HTML/CSS (no Chart.js)

---

## Chunk 1: CompactSidebar Component — Events Section

Create the merged sidebar component with compact event rows and helper functions.

### Task 1: Create CompactSidebar.svelte — Script Setup

**Files:**
- Create: `visualizer/src/lib/components/CompactSidebar.svelte`

- [ ] **Step 1: Create file with imports and interfaces**

```svelte
<script lang="ts">
	import type { TraceData, FsmState } from '$lib/types';
	import { FSM_STATE_COLORS } from '$lib/constants/fsmColors';

	interface Props {
		traceData: TraceData;
		currentTime: number;
		v_cms: number;
		selectedStop: number | null;
		onSeek: (time: number) => void;
		onStopSelect: (idx: number) => void;
		onEventClick?: (info: { time: number; stopIdx?: number; state?: FsmState }) => void;
	}

	let { traceData, currentTime, v_cms, selectedStop, onSeek, onStopSelect, onEventClick }: Props = $props();

	type EventType = 'JUMP' | 'RECOVERY' | 'TRANSITION' | 'ARRIVAL';

	interface LogEvent {
		time: number;
		type: EventType;
		message: string;
		stopIdx?: number;
		state?: FsmState;
		index: number; // For alternating row colors
	}
</script>
```

- [ ] **Step 2: Add helper functions**

Add these functions after the interfaces:

```svelte
	// Helper: Get event type label (3-char abbreviation)
	function getEventTypeLabel(type: EventType): string {
		switch (type) {
			case 'ARRIVAL': return 'ARR';
			case 'TRANSITION': return 'TRN';
			case 'JUMP': return 'JMP';
			case 'RECOVERY': return 'REC';
			default: return type.substring(0, 3);
		}
	}

	// Helper: Get event type color
	function getEventTypeColor(type: EventType): string {
		switch (type) {
			case 'ARRIVAL': return '#22c55e';
			case 'TRANSITION': return '#eab308';
			case 'JUMP': return '#ef4444';
			case 'RECOVERY': return '#f59e0b';
			default: return '#888';
		}
	}

	// Helper: Format event time (locale-independent)
	function formatEventTime(seconds: number): string {
		const date = new Date(seconds * 1000);
		const hh = String(date.getHours()).padStart(2, '0');
		const mm = String(date.getMinutes()).padStart(2, '0');
		const ss = String(date.getSeconds()).padStart(2, '0');
		return `${hh}:${mm}:${ss}`;
	}

	// Helper: Check if event is at current time
	function isEventAtCurrentTime(eventTime: number): boolean {
		return Math.abs(eventTime - currentTime) < 1;
	}

	// Helper: Check if event is highlighted (clicked)
	let highlightedEventTime = $state<number | null>(null);

	function isEventHighlighted(eventTime: number): boolean {
		return highlightedEventTime === eventTime;
	}

	function handleEventRowClick(event: LogEvent) {
		highlightedEventTime = event.time;
		onSeek(event.time);
		const eventState = event.state || (event.type === 'ARRIVAL' ? 'AtStop' : undefined);
		if (event.stopIdx !== undefined && eventState) {
			onEventClick?.({
				time: event.time,
				stopIdx: event.stopIdx,
				state: eventState
			});
		}
	}
```

- [ ] **Step 3: Add events derived state**

```svelte
	// Derived: Events list
	let events = $derived.by(() => {
		const log: LogEvent[] = [];
		let lastStates = new Map<number, FsmState>();
		let index = 0;

		traceData.forEach((record) => {
			// GPS Jump
			if (record.gps_jump) {
				log.push({
					time: record.time,
					type: 'JUMP',
					message: `GPS Jump: dist > 200m`,
					index: index++
				});
			}

			// Recovery
			if (record.recovery_idx !== null) {
				log.push({
					time: record.time,
					type: 'RECOVERY',
					message: `Recovery: stop ${record.recovery_idx}`,
					index: index++
				});
			}

			// Stop events
			record.stop_states.forEach((stop) => {
				const lastState = lastStates.get(stop.stop_idx);
				if (lastState && lastState !== stop.fsm_state) {
					log.push({
						time: record.time,
						type: 'TRANSITION',
						message: `Stop ${stop.stop_idx}`,
						stopIdx: stop.stop_idx,
						state: stop.fsm_state,
						index: index++
					});
				}
				lastStates.set(stop.stop_idx, stop.fsm_state);

				if (stop.just_arrived) {
					log.push({
						time: record.time,
						type: 'ARRIVAL',
						message: `Stop ${stop.stop_idx}: ARRIVED`,
						stopIdx: stop.stop_idx,
						state: 'AtStop',
						index: index++
					});
				}
			});
		});

		return log.sort((a, b) => a.time - b.time);
	});

	// Derived: Event count
	let eventCount = $derived.by(() => events.length);
```

- [ ] **Step 4: Commit**

```bash
git add visualizer/src/lib/components/CompactSidebar.svelte
git commit -m "feat(ux): add CompactSidebar script with event helpers"
```

### Task 2: Add CompactSidebar — Events HTML

**Files:**
- Modify: `visualizer/src/lib/components/CompactSidebar.svelte`

- [ ] **Step 1: Add events section HTML**

After the `</script>` tag, add:

```svelte
<div class="compact-sidebar">
	<!-- Top half: Events (50%) -->
	<div class="sidebar-section events-section">
		<div class="section-header">
			<h4>Event Narrative</h4>
			<span class="event-count">{eventCount} events</span>
		</div>
		<div class="section-content">
			{#each events as event (event.time)}
				<div
					class="event-row {event.index % 2 === 0 ? 'odd' : 'even'}"
					class:current={isEventAtCurrentTime(event.time)}
					class:highlighted={isEventHighlighted(event.time)}
					onclick={() => handleEventRowClick(event)}
					data-type={event.type}
				>
					<span class="event-time">{formatEventTime(event.time)}</span>
					<span class="event-type-badge" style="color: {getEventTypeColor(event.type)};">
						{getEventTypeLabel(event.type)}
					</span>
					{#if event.stopIdx !== undefined}
						<span class="event-stop">#{event.stopIdx}</span>
					{/if}
					{#if event.state}
						<span class="event-state" style="color: {FSM_STATE_COLORS[event.state]};">
							{event.state}
						</span>
					{/if}
				</div>
			{/each}

			{#if events.length === 0}
				<div class="empty-events">No events detected</div>
			{/if}
		</div>
	</div>

	<div class="section-divider"></div>

	<!-- Stops section placeholder -->
	<div class="sidebar-section stops-section">
		<div class="section-header">
			<h4>All Stops Monitor</h4>
			<span class="stops-count">0 active</span>
		</div>
		<div class="section-content">
			<div class="empty-stops">Loading...</div>
		</div>
	</div>
</div>
```

- [ ] **Step 2: Add CSS for events section**

After the HTML, add `<style>`:

```svelte
<style>
	.compact-sidebar {
		display: flex;
		flex-direction: column;
		height: 100%;
		background-color: #0a0a0a;
		overflow: hidden;
	}

	.sidebar-section {
		flex: 1;
		display: flex;
		flex-direction: column;
		min-height: 0;
		overflow: hidden;
	}

	.section-header {
		flex-shrink: 0;
		padding: 0.5rem 0.75rem;
		border-bottom: 1px solid #333;
		display: flex;
		justify-content: space-between;
		align-items: center;
		background-color: #111;
	}

	.section-header h4 {
		font-size: 0.65rem;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: #888;
		margin: 0;
	}

	.event-count,
	.stops-count {
		font-size: 0.6rem;
		color: #666;
		background-color: #222;
		padding: 2px 6px;
		border-radius: 8px;
	}

	.section-content {
		flex: 1;
		overflow-y: auto;
		padding: 0.5rem;
	}

	.section-divider {
		height: 1px;
		background-color: #333;
		flex-shrink: 0;
	}

	/* Event rows */
	.event-row {
		display: flex;
		align-items: center;
		gap: 0.4rem;
		padding: 0.25rem 0.4rem;
		font-size: 0.65rem;
		cursor: pointer;
		border-radius: 2px;
		transition: background-color 0.1s;
	}

	.event-row.odd {
		background-color: #0d0d0d;
	}

	.event-row.even {
		background-color: #111;
	}

	.event-row.current {
		background-color: rgba(59, 130, 246, 0.2);
		border: 1px solid rgba(59, 130, 246, 0.4);
	}

	.event-row.highlighted {
		background-color: rgba(59, 130, 246, 0.3);
		border: 1px solid #3b82f6;
	}

	.event-row:hover {
		background-color: #1a1a1a;
	}

	.event-time {
		font-family: 'JetBrains Mono', monospace;
		color: #888;
		min-width: 60px;
		flex-shrink: 0;
	}

	.event-type-badge {
		font-weight: bold;
		font-size: 0.6rem;
		text-transform: uppercase;
		padding: 1px 4px;
		border-radius: 2px;
		background-color: rgba(0, 0, 0, 0.4);
		flex-shrink: 0;
	}

	.event-stop {
		color: #3b82f6;
		font-weight: bold;
		flex-shrink: 0;
	}

	.event-state {
		font-size: 0.6rem;
		font-weight: bold;
		text-transform: uppercase;
		padding: 1px 4px;
		border-radius: 2px;
		background-color: rgba(0, 0, 0, 0.4);
		flex-shrink: 0;
	}

	.empty-events,
	.empty-stops {
		text-align: center;
		color: #555;
		font-size: 0.7rem;
		padding: 2rem;
	}
</style>
```

- [ ] **Step 3: Commit**

```bash
git add visualizer/src/lib/components/CompactSidebar.svelte
git commit -m "feat(ux): add CompactSidebar events section HTML and CSS"
```

---

## Chunk 2: CompactSidebar Component — Stops Section

Complete the CompactSidebar component with the compact stop cards.

### Task 3: Add All Stops State and Helpers

**Files:**
- Modify: `visualizer/src/lib/components/CompactSidebar.svelte`

- [ ] **Step 1: Add stops helpers before the derived states**

Add after the `handleEventRowClick` function:

```svelte
	// Helper: Format probability value (0-255) as padded string
	function formatProb(value: number): string {
		return Math.round(value).toString().padStart(3, '0');
	}

	// Helper: Get probability color
	function getProbColor(prob: number): string {
		if (prob >= 191) return '#22c55e';
		if (prob >= 128) return '#f59e0b';
		return '#ef4444';
	}
```

- [ ] **Step 2: Add stops derived state**

Add after the `eventCount` derived:

```svelte
	// Derived: Current trace record
	let currentRecord = $derived.by(() => {
		if (traceData.length === 0) return null;
		return traceData.reduce((prev, curr) =>
			Math.abs(curr.time - currentTime) < Math.abs(prev.time - currentTime) ? curr : prev
		);
	});

	// Derived: All stop states at current time
	let allStopStates = $derived.by(() => {
		return currentRecord?.stop_states ?? [];
	});

	// Derived: Stop count
	let stopCount = $derived.by(() => allStopStates.length);
```

- [ ] **Step 3: Commit**

```bash
git add visualizer/src/lib/components/CompactSidebar.svelte
git commit -m "feat(ux): add CompactSidebar stops helpers and derived state"
```

### Task 4: Replace Stops HTML with Full Implementation

**Files:**
- Modify: `visualizer/src/lib/components/CompactSidebar.svelte`

- [ ] **Step 1: Replace the stops section placeholder**

Find and replace the entire stops section div (from `<div class="sidebar-section stops-section">` to its closing `</div>`) with:

```svelte
	<div class="sidebar-section stops-section">
		<div class="section-header">
			<h4>All Stops Monitor</h4>
			<span class="stops-count">{stopCount} active</span>
		</div>
		<div class="section-content">
			{#each allStopStates as stopState (stopState.stop_idx)}
				<div
					class="stop-card-compact"
					class:active={selectedStop === stopState.stop_idx}
					style="border-left-color: {FSM_STATE_COLORS[stopState.fsm_state]};"
					onclick={() => onStopSelect(stopState.stop_idx)}
				>
					<!-- Header: Stop # + State -->
					<div class="stop-card-header">
						<span class="stop-index">#{stopState.stop_idx}</span>
						<span class="stop-state-badge" style="color: {FSM_STATE_COLORS[stopState.fsm_state]};">
							{stopState.fsm_state}
						</span>
					</div>

					<!-- Probability gauge -->
					<div class="prob-row">
						<span class="prob-label">Prob</span>
						<div class="prob-gauge-compact">
							<div
								class="prob-bar"
								style="width: {(stopState.probability / 255) * 100}%; background-color: {getProbColor(stopState.probability)};"
							></div>
							<span class="prob-val" style="color: {getProbColor(stopState.probability)};">
								{formatProb(stopState.probability)}
							</span>
						</div>
					</div>

					<!-- Metrics row -->
					<div class="metrics-compact">
						<span class="metric">dist: {Math.round(stopState.distance_cm / 100)}m</span>
						<span class="metric">dwell: {stopState.dwell_time_s}s</span>
					</div>

					<!-- Features inline -->
					<div class="features-compact">
						span>p₁:{stopState.features.p1}</span>
						<span>p₂:{stopState.features.p2}</span>
						<span>p₃:{stopState.features.p3}</span>
						<span>p₄:{stopState.features.p4}</span>
					</div>

					{#if stopState.just_arrived}
						<div class="arrived-badge-mini">ARRIVED</div>
					{/if}
				</div>
			{/each}

			{#if allStopStates.length === 0}
				<div class="empty-stops">No active stops at current time</div>
			{/if}
		</div>
	</div>
```

- [ ] **Step 2: Add stops CSS to the style block**

Add before the closing `</style>`:

```svelte
	/* Stop cards */
	.stops-grid {
		display: flex;
		flex-direction: column;
		gap: 0.4rem;
	}

	.stop-card-compact {
		background-color: #111;
		border: 1px solid #222;
		border-left-width: 3px;
		border-radius: 3px;
		padding: 0.4rem 0.5rem;
		font-size: 0.7rem;
		cursor: pointer;
		transition: all 0.15s;
	}

	.stop-card-compact:hover {
		border-left-width: 4px;
		background-color: #151515;
	}

	.stop-card-compact.active {
		background-color: #1a1a1a;
		border-color: #06b6d4;
		border-left-width: 4px;
		box-shadow: 0 0 8px rgba(6, 182, 212, 0.3);
	}

	.stop-card-header {
		display: flex;
		justify-content: space-between;
		margin-bottom: 0.3rem;
	}

	.stop-index {
		font-weight: bold;
		color: #fff;
		font-size: 0.7rem;
	}

	.stop-state-badge {
		font-size: 0.6rem;
		font-weight: bold;
		text-transform: uppercase;
		padding: 1px 4px;
		border-radius: 2px;
		background-color: rgba(0, 0, 0, 0.4);
	}

	.prob-row {
		display: flex;
		align-items: center;
		gap: 0.35rem;
		margin-bottom: 0.25rem;
	}

	.prob-label {
		font-size: 0.6rem;
		color: #666;
		min-width: 28px;
	}

	.prob-gauge-compact {
		flex: 1;
		height: 14px;
		background-color: #080808;
		border-radius: 2px;
		position: relative;
		overflow: hidden;
		border: 1px solid #333;
	}

	.prob-bar {
		height: 100%;
		transition: width 0.3s, background-color 0.3s;
	}

	.prob-val {
		position: absolute;
		top: 50%;
		left: 50%;
		transform: translate(-50%, -50%);
		font-size: 0.65rem;
		font-weight: bold;
		text-shadow: 0 0 2px rgba(0, 0, 0, 0.8);
	}

	.metrics-compact {
		display: flex;
		justify-content: space-between;
		font-size: 0.65rem;
		color: #888;
		margin-bottom: 0.2rem;
	}

	.features-compact {
		display: flex;
		justify-content: space-between;
		font-size: 0.6rem;
		color: #555;
		background-color: #0a0a0a;
		padding: 0.2rem 0.3rem;
		border-radius: 2px;
	}

	.arrived-badge-mini {
		margin-top: 0.2rem;
		background: linear-gradient(90deg, #22c55e, #16a34a);
		color: white;
		font-size: 0.55rem;
		font-weight: bold;
		text-align: center;
		padding: 0.15rem;
		border-radius: 2px;
		text-transform: uppercase;
		animation: pulse 1s ease-in-out infinite;
	}

	@keyframes pulse {
		0%, 100% { opacity: 1; }
		50% { opacity: 0.7; }
	}
```

- [ ] **Step 3: Commit**

```bash
git add visualizer/src/lib/components/CompactSidebar.svelte
git commit -m "feat(ux): add CompactSidebar stops section with compact cards"
```

---

## Chunk 3: Timeline Component with Keyboard Shortcuts

Create the new Timeline component to replace TimelineCharts.

### Task 5: Create Timeline.svelte — Basic Structure

**Files:**
- Create: `visualizer/src/lib/components/Timeline.svelte`

- [ ] **Step 1: Create Timeline component with props and state**

```svelte
<script lang="ts">
	import type { TraceData } from '$lib/types';

	interface Props {
		traceData: TraceData;
		currentTime: number;
		onTimeChange?: (time: number) => void;
	}

	let { traceData, currentTime, onTimeChange = () => {} }: Props = $props();

	// Get time range
	let timeMin = $derived.by(() => traceData.length > 0 ? traceData[0].time : 0);
	let timeMax = $derived.by(() => traceData.length > 0 ? traceData[traceData.length - 1].time : 0);
	let currentTimePercent = $derived.by(() =>
		timeMax > timeMin ? ((currentTime - timeMin) / (timeMax - timeMin)) * 100 : 0
	);

	// Format time for display
	function formatTime(seconds: number): string {
		return new Date(seconds * 1000).toLocaleTimeString([], { hour12: false });
	}

	// Handle slider change
	function handleSliderChange(event: Event) {
		const target = event.target as HTMLInputElement;
		const percent = parseFloat(target.value);
		currentTime = timeMin + (percent / 100) * (timeMax - timeMin);
		onTimeChange(currentTime);
	}

	// Handle seek
	function handleSeek(time: number) {
		onTimeChange(time);
	}
</script>
```

- [ ] **Step 2: Add HTML structure**

```svelte
<div class="timeline-container">
	<div class="playback-bar">
		<div class="playback-controls">
			<span class="shortcuts-hint" title="Keyboard shortcuts">
				<span class="hint">␣ play</span>
				<span class="hint">←→ seek</span>
				<span class="hint">? help</span>
			</span>
		</div>

		<div class="time-info">
			<span class="current">{formatTime(currentTime)}</span>
			<span class="sep">/</span>
			<span class="total">{formatTime(timeMax)}</span>
		</div>

		<div class="slider-wrapper">
			<input
				type="range"
				min="0"
				max="100"
				step="0.01"
				value={currentTimePercent}
				oninput={handleSliderChange}
				class="time-slider"
				title="Click or drag to seek"
			/>
		</div>
	</div>
</div>
```

- [ ] **Step 3: Add CSS**

```svelte
<style>
	.timeline-container {
		width: 100%;
		height: 100%;
		background-color: #0a0a0a;
		display: flex;
		flex-direction: column;
	}

	.playback-bar {
		padding: 0.5rem 1rem;
		display: flex;
		align-items: center;
		gap: 1rem;
		background-color: #111;
		border-bottom: 1px solid #222;
		flex: 1;
	}

	.playback-controls {
		display: flex;
		gap: 0.5rem;
		align-items: center;
	}

	.shortcuts-hint {
		display: flex;
		gap: 0.5rem;
		font-size: 0.65rem;
		color: #555;
	}

	.hint {
		background-color: #1a1a1a;
		padding: 2px 6px;
		border-radius: 3px;
		border: 1px solid #333;
	}

	.time-info {
		font-size: 0.75rem;
		display: flex;
		gap: 0.5rem;
		min-width: 150px;
	}

	.time-info .current {
		color: #fff;
		font-weight: bold;
	}

	.time-info .sep {
		color: #444;
	}

	.time-info .total {
		color: #666;
	}

	.slider-wrapper {
		flex: 1;
	}

	.time-slider {
		width: 100%;
		height: 4px;
		background: #333;
		border-radius: 2px;
		appearance: none;
		outline: none;
	}

	.time-slider::-webkit-slider-thumb {
		appearance: none;
		width: 12px;
		height: 12px;
		background: #3b82f6;
		border-radius: 50%;
		cursor: pointer;
		box-shadow: 0 0 5px rgba(59, 130, 246, 0.5);
	}

	.time-slider::-moz-range-thumb {
		width: 12px;
		height: 12px;
		background: #3b82f6;
		border-radius: 50%;
		cursor: pointer;
		border: none;
	}
</style>
```

- [ ] **Step 4: Commit**

```bash
git add visualizer/src/lib/components/Timeline.svelte
git commit -m "feat(ux): add Timeline component basic structure"
```

### Task 6: Add Keyboard Shortcuts to Timeline

**Files:**
- Modify: `visualizer/src/lib/components/Timeline.svelte`

- [ ] **Step 1: Add keyboard handler in script section**

Add after the `handleSeek` function:

```svelte
	// Keyboard handler
	let handleKeyDown: (e: KeyboardEvent) => void;

	const SEEK_AMOUNT = 5; // seconds

	onMount(() => {
		handleKeyDown = (e: KeyboardEvent) => {
			// Ignore when typing in file inputs
			if (e.target instanceof HTMLInputElement) return;

			switch (e.key) {
				case ' ':
					e.preventDefault();
					// Toggle play/pause - emit event for parent to handle
					dispatch('toggle-play', {});
					break;
				case 'ArrowLeft':
					e.preventDefault();
					handleSeek(Math.max(timeMin, currentTime - SEEK_AMOUNT));
					break;
				case 'ArrowRight':
					e.preventDefault();
					handleSeek(Math.min(timeMax, currentTime + SEEK_AMOUNT));
					break;
				case '?':
					e.preventDefault();
					dispatch('show-help', {});
					break;
			}
		};

		document.addEventListener('keydown', handleKeyDown);
		return () => document.removeEventListener('keydown', handleKeyDown);
	});
```

- [ ] **Step 2: Add play/pause button and state props**

Update the props interface and add state:

```svelte
	interface Props {
		traceData: TraceData;
		currentTime: number;
		isPlaying: boolean;
		playbackSpeed: number;
		onTimeChange?: (time: number) => void;
		onTogglePlay?: () => void;
		onSpeedChange?: (speed: number) => void;
	}

	let {
		traceData,
		currentTime,
		isPlaying,
		playbackSpeed,
		onTimeChange = () => {},
		onTogglePlay = () => {},
		onSpeedChange = () => {}
	}: Props = $props();
```

- [ ] **Step 3: Update playback-controls HTML**

Replace the playback-controls div:

```svelte
	<div class="playback-controls">
		<button class="play-pause" onclick={onTogglePlay} title="Space: Play/Pause">
			{isPlaying ? '⏸' : '▶'}
		</button>
		<select
			bind:value={playbackSpeed}
			onchange={(e) => onSpeedChange(Number((e.target as HTMLSelectElement).value))}
			class="speed-select"
			title="Playback speed"
		>
			<option value={1}>1x</option>
			<option value={2}>2x</option>
			<option value={5}>5x</option>
			<option value={10}>10x</option>
		</select>

		<span class="shortcuts-hint" title="Keyboard shortcuts">
			<span class="hint">␣ play</span>
			<span class="hint">←→ seek</span>
			<span class="hint">? help</span>
		</span>
	</div>
```

- [ ] **Step 4: Add play-pause and speed-select CSS**

Add to the style block:

```svelte
	.play-pause {
		background-color: #3b82f6;
		border: none;
		color: white;
		width: 30px;
		height: 30px;
		border-radius: 50%;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		font-size: 0.8rem;
	}

	.play-pause:hover {
		background-color: #2563eb;
	}

	.speed-select {
		background-color: #222;
		color: #fff;
		border: 1px solid #444;
		border-radius: 4px;
		font-size: 0.75rem;
		padding: 2px 4px;
	}
```

- [ ] **Step 5: Commit**

```bash
git add visualizer/src/lib/components/Timeline.svelte
git commit -m "feat(ux): add keyboard shortcuts and playback controls to Timeline"
```

---

## Chunk 4: Page Integration — Layout and Component Updates

Update the main page to use the new components and layout.

### Task 7: Update +page.svelte Imports

**Files:**
- Modify: `visualizer/src/routes/+page.svelte`

- [ ] **Step 1: Update imports**

Replace lines 3-8:

```typescript
// OLD imports to remove:
// import TimelineCharts from '$lib/components/TimelineCharts.svelte';
// import EventLog from '$lib/components/EventLog.svelte';
// import AllStopsInspector from '$lib/components/AllStopsInspector.svelte';

// NEW imports to add:
import CompactSidebar from '$lib/components/CompactSidebar.svelte';
import Timeline from '$lib/components/Timeline.svelte';
```

The imports section should now look like:

```typescript
<script lang="ts">
	import { onMount } from 'svelte';
	import MapView from '$lib/components/MapView.svelte';
	import FsmInspector from '$lib/components/FsmInspector.svelte';
	import ProbabilityScope from '$lib/components/ProbabilityScope.svelte';
	import CompactSidebar from '$lib/components/CompactSidebar.svelte';
	import LinearRouteWidget from '$lib/components/LinearRouteWidget.svelte';
	import Timeline from '$lib/components/Timeline.svelte';
	import type { RouteData, TraceData, FsmState } from '$lib/types';
	import { loadRouteData, getInterpolatedBusState } from '$lib/parsers/routeData';
	import { loadTraceFile, getTraceTimeRange } from '$lib/parsers/trace';
```

- [ ] **Step 2: Commit**

```bash
git add visualizer/src/routes/+page.svelte
git commit -m "feat(ux): update imports to use CompactSidebar and Timeline"
```

### Task 8: Update Grid Layout CSS

**Files:**
- Modify: `visualizer/src/routes/+page.svelte`

- [ ] **Step 1: Update grid-template-columns**

Find the `.dashboard-grid` CSS rule (around line 401) and change `grid-template-columns`:

```css
	.dashboard-grid {
		flex: 1;
		display: grid;
		grid-template-columns: 2.5fr 1.5fr 1fr;  /* CHANGED from 1.5fr 1.5fr 1fr */
		grid-template-rows: 1fr auto;
		gap: 1px;
		background-color: #333;
		min-height: 0;
	}
```

- [ ] **Step 2: Update footer height**

Find the `.dashboard-footer` CSS rule (around line 443) and change `height`:

```css
	.dashboard-footer {
		height: 180px;  /* CHANGED from 250px */
		background-color: #0a0a0a;
		border-top: 1px solid #333;
		display: flex;
		flex-direction: column;
	}
```

- [ ] **Step 3: Add sidebar panel CSS**

Add after the `.linear-route-panel` rule:

```css
	.sidebar-panel {
		overflow: hidden;
	}

	.feed-panel {
		/* No longer used, but keeping for reference */
	}
```

- [ ] **Step 4: Commit**

```bash
git add visualizer/src/routes/+page.svelte
git commit -m "feat(ux): update grid layout to 2.5fr/1.5fr/1fr and footer to 180px"
```

### Task 9: Replace Lab Panel Content with Empty State

**Files:**
- Modify: `visualizer/src/routes/+page.svelte`

- [ ] **Step 1: Update lab panel HTML**

Find the lab-panel section (around line 227) and replace the `{:else}` block:

```svelte
				<!-- Center: The Lab (Algorithm) -->
				<section class="panel lab-panel">
					<div class="lab-scroll">
						{#if activeStopState && currentRecord && traceData}
							<!-- Detailed view for selected stop -->
							<ProbabilityScope stopState={activeStopState} v_cms={currentRecord.v_cms} />
							<div class="spacer"></div>
							<FsmInspector {traceData} {selectedStop} {currentTime} />
						{:else}
							<!-- Empty state when no stop selected -->
							<div class="empty-lab">
								<div class="lab-icon">🔬</div>
								<p>Select a stop from the sidebar to see detailed analysis</p>
							</div>
						{/if}
					</div>
				</section>
```

- [ ] **Step 2: Add empty-lab CSS**

Find the `.empty-lab` rule (around line 430) and ensure the styles include the icon:

```css
	.empty-lab {
		height: 100%;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		color: #444;
		text-align: center;
		padding: 2rem;
	}

	.empty-lab p {
		margin: 0;
		font-size: 0.8rem;
	}
```

- [ ] **Step 3: Commit**

```bash
git add visualizer/src/routes/+page.svelte
git commit -m "feat(ux): show empty state in Lab Panel when no stop selected"
```

### Task 10: Replace Event Feed Panel with CompactSidebar

**Files:**
- Modify: `visualizer/src/routes/+page.svelte`

- [ ] **Step 1: Replace feed-panel section**

Find the feed-panel section (around line 243) and replace with sidebar-panel:

```svelte
				<!-- Right: Compact Sidebar -->
				<section class="panel sidebar-panel">
					{#if traceData && currentRecord}
						<CompactSidebar
							{traceData}
							{currentTime}
							v_cms={currentRecord.v_cms}
							{selectedStop}
							onSeek={handleSeek}
							onStopSelect={(idx) => selectedStop = idx}
							onEventClick={handleEventClick}
						/>
					{/if}
				</section>
```

- [ ] **Step 2: Commit**

```bash
git add visualizer/src/routes/+page.svelte
git commit -m "feat(ux): replace EventLog with CompactSidebar component"
```

### Task 11: Replace TimelineCharts with Timeline

**Files:**
- Modify: `visualizer/src/routes/+page.svelte`

- [ ] **Step 1: Update footer content**

Find the footer section (around line 267) and replace the entire footer:

```svelte
			<!-- Bottom: Timeline & Playback -->
			<footer class="dashboard-footer">
				{#if traceData}
					<Timeline
						{traceData}
						{currentTime}
						{isPlaying}
						playbackSpeed={playbackSpeed}
						onTimeChange={handleSeek}
						onTogglePlay={() => isPlaying = !isPlaying}
						onSpeedChange={(speed) => playbackSpeed = speed}
					/>
				{/if}
			</footer>
```

- [ ] **Step 2: Remove old playback-bar and charts-area styles**

Find and remove the `.playback-bar`, `.playback-controls`, `.play-pause`, `.speed-select`, `.time-info`, `.slider-wrapper`, `.time-slider`, and `.charts-area` CSS rules from the style block. These are now handled by the Timeline component.

- [ ] **Step 3: Commit**

```bash
git add visualizer/src/routes/+page.svelte
git commit -m "feat(ux): replace TimelineCharts with Timeline component"
```

---

## Verification

### Task 12: Final Verification and Testing

- [ ] **Step 1: Start dev server**

```bash
cd visualizer
npm run dev
```

Expected: Server starts on http://localhost:5173/

- [ ] **Step 2: Manual testing checklist**

1. Load route + trace data files
2. Verify map panel is larger (2.5fr vs 1.5fr before)
3. Verify sidebar shows events (top) and stops (bottom)
4. Click an event — verify timeline seeks and map highlights
5. Click a stop — verify Lab Panel shows ProbabilityScope + FsmInspector
6. Click empty space in Lab Panel area — verify empty state shows
7. Test Space key toggles play/pause
8. Test Arrow Left seeks -5 seconds
9. Test Arrow Right seeks +5 seconds
10. Verify current event highlighted in event list
11. Verify selected stop has cyan border in stop list
12. Verify all stop details visible (prob, distance, dwell, features)
13. Test playback with speed selector (1x/2x/5x/10x)
14. Verify keyboard shortcuts don't trigger when file inputs focused

- [ ] **Step 3: Fix any issues found**

Address issues found during testing.

- [ ] **Step 4: Final commit**

```bash
git add -A
git commit -m "feat(ux): complete UI/UX redesign implementation"
```

---

## Migration Notes

**Deprecated components** (kept for rollback but no longer used):
- `visualizer/src/lib/components/TimelineCharts.svelte`
- `visualizer/src/lib/components/EventLog.svelte`
- `visualizer/src/lib/components/AllStopsInspector.svelte`

**Rollback plan:** If issues arise, revert the commits modifying `+page.svelte` and restore the original imports and grid CSS.

**Future enhancements** (out of scope for this plan):
- Help modal for ? key
- Timeline event markers (small dots for events)
- Responsive breakpoints for mobile
- Collapse/expand sidebar sections

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

	let routeData = $state<RouteData | null>(null);
	let traceData = $state<TraceData | null>(null);

	let selectedStop = $state<number | null>(null);

	let highlightedEvent = $state<{
		stopIdx: number;
		time: number;
		state: FsmState;
	} | null>(null);

	let mapViewRef: { panToStop: (idx: number) => void } | null = null;
	let currentTime = $state<number>(0);

	let timeMin = $state<number>(0);
	let timeMax = $state<number>(0);
	let currentTimePercent = $state<number>(0);

	let isPlaying = $state(false);
	let playbackSpeed = $state(1); // 1x, 2x, 5x, 10x

	let routeFileInput = $state<HTMLInputElement | null>(null);
	let traceFileInput = $state<HTMLInputElement | null>(null);

	let showUpload = $state(true);
	let loading = $state(false);
	let error = $state<string | null>(null);

	// Playback timer
	onMount(() => {
		const interval = setInterval(() => {
			if (isPlaying && traceData && currentTime < timeMax) {
				const fps = 10; // Base updates per second
				const dt = (1 / fps) * playbackSpeed;
				const nextTime = currentTime + dt;
				if (nextTime >= timeMax) {
					currentTime = timeMax;
					isPlaying = false;
				} else {
					currentTime = nextTime;
				}
				currentTimePercent = ((currentTime - timeMin) / (timeMax - timeMin)) * 100;
			}
		}, 100);

		return () => clearInterval(interval);
	});

	async function handleRouteUpload() {
		const file = routeFileInput?.files?.[0];
		if (!file) return;
		loading = true;
		try {
			routeData = await loadRouteData(file);
			checkReady();
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
		try {
			traceData = await loadTraceFile(file);
			[timeMin, timeMax] = getTraceTimeRange(traceData);
			currentTime = timeMin;
			currentTimePercent = 0;
			checkReady();
		} catch (e) {
			error = `Failed to load trace: ${e instanceof Error ? e.message : String(e)}`;
		} finally {
			loading = false;
		}
	}

	function checkReady() {
		if (routeData && traceData) {
			showUpload = false;
		}
	}

	function handleSeek(time: number) {
		currentTime = time;
		if (timeMax > timeMin) {
			currentTimePercent = ((time - timeMin) / (timeMax - timeMin)) * 100;
		}
	}

	function handleSliderChange(event: Event) {
		const target = event.target as HTMLInputElement;
		const percent = parseFloat(target.value);
		currentTimePercent = percent;
		currentTime = timeMin + (percent / 100) * (timeMax - timeMin);
	}

	function formatTime(seconds: number): string {
		return new Date(seconds * 1000).toLocaleTimeString([], { hour12: false });
	}

	const currentRecord = $derived.by(() => {
		if (!traceData || traceData.length === 0) return null;
		// Binary search or closest record for better performance
		return traceData.reduce((prev, curr) => 
			Math.abs(curr.time - currentTime) < Math.abs(prev.time - currentTime) ? curr : prev
		);
	});

	const busPosition = $derived.by(() => {
		if (!currentRecord || !routeData) return null;
		const interpolated = getInterpolatedBusState(currentRecord.s_cm, routeData);
		return {
			lat: currentRecord.lat,
			lon: currentRecord.lon,
			heading: interpolated.heading_cdeg / 100 // Convert to degrees
		};
	});

	const activeStopState = $derived.by(() => {
		if (!currentRecord || selectedStop === null) return null;
		return currentRecord.stop_states.find(s => s.stop_idx === selectedStop) || null;
	});

	function resetUpload() {
		routeData = null;
		traceData = null;
		selectedStop = null;
		showUpload = true;
		error = null;
	}

	function handleEventClick(info: { time: number; stopIdx?: number; state?: FsmState }) {
		if (info.stopIdx !== undefined && info.state) {
			highlightedEvent = {
				stopIdx: info.stopIdx,
				time: info.time,
				state: info.state
			};
			mapViewRef?.panToStop(info.stopIdx);
		}
	}

	function clearHighlight() {
		highlightedEvent = null;
	}
</script>

<div class="app-container dark">
	{#if showUpload}
		<div class="upload-screen">
			<div class="upload-card">
				<h1 class="title">Bus Arrival Lab</h1>
				<p class="subtitle">Scientific Arrival Detection Visualization</p>

				{#if error}
					<div class="error-banner">{error}</div>
				{/if}

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

				{#if loading}<div class="loading">Parsing binary structures...</div>{/if}
			</div>
		</div>
	{:else}
		<div class="dashboard-layout">
			<!-- Header -->
			<header class="dashboard-header">
				<div class="brand">
					<span class="logo">🚌</span>
					<h1>Bus Arrival Lab <span class="version">v2.0</span></h1>
				</div>
				<div class="controls">
					<button onclick={resetUpload} class="btn-outline">New Session</button>
				</div>
			</header>

			<!-- Main Content Grid -->
			<main class="dashboard-grid">
				<!-- Left: Spatial -->
				<section class="panel spatial-panel">
					{#if routeData}
						<MapView
							{routeData}
							{busPosition}
							{selectedStop}
							{highlightedEvent}
							onStopClick={(idx) => selectedStop = idx}
							onClearHighlight={clearHighlight}
							bind:this={mapViewRef}
						/>
					{/if}
				</section>

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

				<!-- Right: Compact Sidebar -->
				<section class="panel sidebar-panel">
					{#if traceData}
						<CompactSidebar
							{traceData}
							{currentTime}
							v_cms={currentRecord?.v_cms ?? 0}
							{selectedStop}
							onSeek={handleSeek}
							onStopSelect={(idx) => selectedStop = idx}
							onEventClick={handleEventClick}
						/>
					{/if}
				</section>

				<!-- Linear Route Panel -->
				<section class="panel linear-route-panel">
					{#if routeData && currentRecord}
						<LinearRouteWidget
							{routeData}
							busProgress={currentRecord.s_cm}
							{highlightedEvent}
						/>
					{/if}
				</section>
			</main>

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
		</div>
	{/if}
</div>

<style>
	:global(body) {
		margin: 0;
		padding: 0;
		background-color: #000;
		color: #fff;
		overflow: hidden;
	}

	.app-container.dark {
		background-color: #0a0a0a;
		height: 100vh;
		display: flex;
		flex-direction: column;
		font-family: 'JetBrains Mono', 'Monaco', monospace;
	}

	/* Upload Screen */
	.upload-screen {
		display: flex;
		align-items: center;
		justify-content: center;
		height: 100%;
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

	.title { font-size: 1.5rem; margin-bottom: 0.5rem; color: #fff; }
	.subtitle { font-size: 0.875rem; color: #666; margin-bottom: 2rem; }

	.upload-section { display: flex; flex-direction: column; gap: 1rem; }
	.upload-item {
		background-color: #111;
		border: 1px dashed #444;
		border-radius: 0.25rem;
		padding: 1rem;
		display: flex;
		justify-content: space-between;
		align-items: center;
	}

	.file-label { cursor: pointer; color: #3b82f6; font-size: 0.875rem; }
	.file-input { display: none; }
	.status-badge.success { color: #22c55e; font-size: 0.75rem; font-weight: bold; }

	/* Dashboard Layout */
	.dashboard-layout {
		display: flex;
		flex-direction: column;
		height: 100%;
	}

	.dashboard-header {
		height: 50px;
		background-color: #111;
		border-bottom: 1px solid #333;
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 0 1rem;
	}

	.brand { display: flex; align-items: center; gap: 0.75rem; }
	.brand h1 { font-size: 0.875rem; text-transform: uppercase; letter-spacing: 0.1em; margin: 0; }
	.brand .version { color: #444; font-size: 0.6rem; }

	.btn-outline {
		background: none;
		border: 1px solid #444;
		color: #888;
		padding: 4px 12px;
		border-radius: 4px;
		font-size: 0.75rem;
		cursor: pointer;
	}

	.btn-outline:hover { border-color: #666; color: #fff; }

	.dashboard-grid {
		flex: 1;
		display: grid;
		grid-template-columns: 2.5fr 1.5fr 1fr;  /* CHANGED from 1.5fr 1.5fr 1fr */
		grid-template-rows: 1fr auto;
		gap: 1px;
		background-color: #333;
		min-height: 0;
		overflow: hidden;
	}

	.panel { background-color: #0a0a0a; overflow: hidden; position: relative; min-height: 0; }

	.linear-route-panel {
		grid-column: 1 / -1;
		height: 120px;
		min-height: 120px;
	}

	.sidebar-panel {
		overflow: hidden;
	}

	.lab-scroll {
		height: 100%;
		overflow-y: auto;
		padding: 1rem;
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.spacer { height: 1rem; }

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

	.lab-icon {
		font-size: 2rem;
		margin-bottom: 0.5rem;
		opacity: 0.5;
	}

	.empty-lab p {
		margin: 0;
		font-size: 0.8rem;
	}

	.dashboard-footer {
		height: 180px;  /* CHANGED from 250px */
		background-color: #0a0a0a;
		border-top: 1px solid #333;
		display: flex;
		flex-direction: column;
	}
</style>

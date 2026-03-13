<script lang="ts">
	import { onMount } from 'svelte';
	import MapView from '$lib/components/MapView.svelte';
	import TimelineCharts from '$lib/components/TimelineCharts.svelte';
	import FsmInspector from '$lib/components/FsmInspector.svelte';
	import FeatureBreakdown from '$lib/components/FeatureBreakdown.svelte';
	import type { RouteData, TraceData } from '$lib/types';
	import { loadRouteData } from '$lib/parsers/routeData';
	import { loadTraceFile, getTraceTimeRange } from '$lib/parsers/trace';
	import { projectCmToLatLon } from '$lib/parsers/projection';

	let routeData = $state<RouteData | null>(null);
	let traceData = $state<TraceData | null>(null);

	let selectedStop = $state<number | null>(null);
	let currentTime = $state<number>(0);

	let timeMin = $state<number>(0);
	let timeMax = $state<number>(0);
	let currentTimePercent = $state<number>(0);

	let routeFileInput: HTMLInputElement;
	let traceFileInput: HTMLInputElement;

	let showUpload = $state(true);
	let loading = $state(false);
	let error = $state<string | null>(null);

	async function handleRouteUpload() {
		const file = routeFileInput.files?.[0];
		if (!file) return;

		loading = true;
		error = null;

		try {
			routeData = await loadRouteData(file);
			checkReady();
		} catch (e) {
			error = `Failed to load route data: ${e instanceof Error ? e.message : String(e)}`;
		} finally {
			loading = false;
		}
	}

	async function handleTraceUpload() {
		const file = traceFileInput.files?.[0];
		if (!file) return;

		loading = true;
		error = null;

		try {
			traceData = await loadTraceFile(file);
			[timeMin, timeMax] = getTraceTimeRange(traceData);
			currentTime = timeMin;
			currentTimePercent = 0;
			checkReady();
		} catch (e) {
			error = `Failed to load trace data: ${e instanceof Error ? e.message : String(e)}`;
		} finally {
			loading = false;
		}
	}

	function checkReady() {
		if (routeData && traceData) {
			showUpload = false;
		}
	}

	function handleStopClick(stopIndex: number) {
		selectedStop = stopIndex === selectedStop ? null : stopIndex;
	}

	function handleTimeChange(time: number) {
		currentTime = time;
		currentTimePercent = ((time - timeMin) / (timeMax - timeMin)) * 100;
	}

	function handleSliderChange(event: Event) {
		const target = event.target as HTMLInputElement;
		const percent = parseFloat(target.value);
		currentTimePercent = percent;
		currentTime = timeMin + (percent / 100) * (timeMax - timeMin);
	}

	function formatTime(seconds: number): string {
		return new Date(seconds * 1000).toLocaleTimeString();
	}

	function getCurrentBusPosition() {
		if (!traceData) return null;
		const record = traceData.find((r) => r.time === currentTime);
		return record ? { x_cm: record.s_cm, y_cm: 0 } : null;
	}

	function resetUpload() {
		routeData = null;
		traceData = null;
		selectedStop = null;
		showUpload = true;
		error = null;
		if (routeFileInput) routeFileInput.value = '';
		if (traceFileInput) traceFileInput.value = '';
	}
</script>

<div class="app-container">
	{#if showUpload}
		<!-- Upload Screen -->
		<div class="upload-screen">
			<div class="upload-card">
				<h1 class="title">Bus Arrival Visualizer</h1>
				<p class="subtitle">Upload route data and trace file to visualize arrival detection</p>

				{#if error}
					<div class="error-banner">{error}</div>
				{/if}

				<div class="upload-section">
					<div class="upload-item">
						<label for="route-file" class="file-label">
							<div class="label-text">Route Data</div>
							<div class="label-desc">route_data.bin</div>
						</label>
						<input
							bind:this={routeFileInput}
							id="route-file"
							type="file"
							accept=".bin"
							onchange={handleRouteUpload}
							class="file-input"
						/>
						{#if routeData}
							<div class="status-badge success">✓ Loaded</div>
						{/if}
					</div>

					<div class="upload-item">
						<label for="trace-file" class="file-label">
							<div class="label-text">Trace Data</div>
							<div class="label-desc">trace.jsonl</div>
						</label>
						<input
							bind:this={traceFileInput}
							id="trace-file"
							type="file"
							accept=".jsonl"
							onchange={handleTraceUpload}
							class="file-input"
						/>
						{#if traceData}
							<div class="status-badge success">✓ Loaded</div>
						{/if}
					</div>
				</div>

				{#if loading}
					<div class="loading">Loading...</div>
				{/if}
			</div>
		</div>
	{:else}
		<!-- Main Visualizer -->
		<div class="visualizer-layout">
			<!-- Header -->
			<header class="header">
				<h1 class="header-title">Bus Arrival Visualizer</h1>
				<button onclick={resetUpload} class="reset-button">Upload New Files</button>
			</header>

			<!-- Time Slider -->
			<div class="time-slider-container">
				<div class="time-labels">
					<span>{formatTime(timeMin)}</span>
					<span class="current-time">{formatTime(currentTime)}</span>
					<span>{formatTime(timeMax)}</span>
				</div>
				<input
					type="range"
					min="0"
					max="100"
					step="0.1"
					bind:value={currentTimePercent}
					oninput={handleSliderChange}
					class="time-slider"
				/>
			</div>

			<!-- Main Content Grid -->
			<div class="content-grid">
				<!-- Left Column: Map -->
				<div class="map-section">
					{#if routeData}
						<MapView
							routeData={routeData}
							busPosition={getCurrentBusPosition()}
							{selectedStop}
							onStopClick={handleStopClick}
						/>
					{/if}
				</div>

				<!-- Right Column: Inspector and Charts -->
				<div class="right-column">
					<!-- FSM Inspector -->
					<div class="inspector-section">
						{#if traceData}
							<FsmInspector {traceData} {selectedStop} {currentTime} />
						{/if}
					</div>

					<!-- Feature Breakdown -->
					<div class="breakdown-section">
						{#if traceData}
							<FeatureBreakdown {traceData} {selectedStop} {currentTime} />
						{/if}
					</div>
				</div>
			</div>

			<!-- Bottom: Timeline Charts -->
			<div class="charts-section">
				{#if traceData}
					<TimelineCharts
						{traceData}
						{selectedStop}
						{currentTime}
						onTimeChange={handleTimeChange}
					/>
				{/if}
			</div>
		</div>
	{/if}
</div>

<style>
	.app-container {
		width: 100%;
		min-height: 100vh;
		background-color: #f3f4f6;
	}

	/* Upload Screen */
	.upload-screen {
		display: flex;
		align-items: center;
		justify-content: center;
		min-height: 100vh;
		padding: 1rem;
	}

	.upload-card {
		background-color: white;
		border-radius: 0.5rem;
		padding: 2rem;
		box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.1);
		max-width: 500px;
		width: 100%;
	}

	.title {
		font-size: 1.5rem;
		font-weight: 700;
		color: #111827;
		text-align: center;
		margin-bottom: 0.5rem;
	}

	.subtitle {
		font-size: 0.875rem;
		color: #6b7280;
		text-align: center;
		margin-bottom: 2rem;
	}

	.error-banner {
		background-color: #fef2f2;
		color: #991b1b;
		padding: 0.75rem;
		border-radius: 0.375rem;
		margin-bottom: 1rem;
		font-size: 0.875rem;
	}

	.upload-section {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.upload-item {
		display: flex;
		align-items: center;
		gap: 1rem;
		padding: 1rem;
		background-color: #f9fafb;
		border-radius: 0.375rem;
		border: 1px dashed #d1d5db;
	}

	.file-label {
		flex: 1;
		cursor: pointer;
	}

	.label-text {
		font-size: 0.875rem;
		font-weight: 600;
		color: #374151;
	}

	.label-desc {
		font-size: 0.75rem;
		color: #9ca3af;
	}

	.file-input {
		display: none;
	}

	.status-badge {
		padding: 0.25rem 0.75rem;
		border-radius: 0.375rem;
		font-size: 0.75rem;
		font-weight: 600;
	}

	.status-badge.success {
		background-color: #dcfce7;
		color: #166534;
	}

	.loading {
		text-align: center;
		color: #6b7280;
		font-size: 0.875rem;
		margin-top: 1rem;
	}

	/* Visualizer Layout */
	.visualizer-layout {
		display: flex;
		flex-direction: column;
		height: 100vh;
	}

	.header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 1rem 1.5rem;
		background-color: white;
		border-bottom: 1px solid #e5e7eb;
	}

	.header-title {
		font-size: 1.25rem;
		font-weight: 700;
		color: #111827;
		margin: 0;
	}

	.reset-button {
		padding: 0.5rem 1rem;
		background-color: #3b82f6;
		color: white;
		border: none;
		border-radius: 0.375rem;
		font-size: 0.875rem;
		font-weight: 500;
		cursor: pointer;
		transition: background-color 0.2s;
	}

	.reset-button:hover {
		background-color: #2563eb;
	}

	.time-slider-container {
		padding: 1rem 1.5rem;
		background-color: white;
		border-bottom: 1px solid #e5e7eb;
	}

	.time-labels {
		display: flex;
		justify-content: space-between;
		margin-bottom: 0.5rem;
		font-size: 0.75rem;
		color: #6b7280;
	}

	.current-time {
		font-weight: 600;
		color: #111827;
		font-size: 0.875rem;
	}

	.time-slider {
		width: 100%;
		height: 0.5rem;
		-webkit-appearance: none;
		appearance: none;
		background-color: #e5e7eb;
		border-radius: 0.25rem;
		outline: none;
	}

	.time-slider::-webkit-slider-thumb {
		-webkit-appearance: none;
		appearance: none;
		width: 1rem;
		height: 1rem;
		background-color: #3b82f6;
		border-radius: 50%;
		cursor: pointer;
	}

	.time-slider::-moz-range-thumb {
		width: 1rem;
		height: 1rem;
		background-color: #3b82f6;
		border-radius: 50%;
		cursor: pointer;
		border: none;
	}

	.content-grid {
		display: grid;
		grid-template-columns: 1fr 400px;
		gap: 1rem;
		padding: 1rem;
		flex: 1;
		overflow: hidden;
	}

	.map-section {
		background-color: white;
		border-radius: 0.5rem;
		overflow: hidden;
		box-shadow: 0 1px 3px 0 rgba(0, 0, 0, 0.1);
	}

	.right-column {
		display: flex;
		flex-direction: column;
		gap: 1rem;
		overflow: hidden;
	}

	.inspector-section,
	.breakdown-section {
		flex: 1;
		overflow: hidden;
	}

	.charts-section {
		padding: 0 1rem 1rem 1rem;
		height: 350px;
		overflow: hidden;
	}

	@media (max-width: 1024px) {
		.content-grid {
			grid-template-columns: 1fr;
			grid-template-rows: 400px auto auto;
			overflow-y: auto;
		}

		.right-column {
			max-height: none;
		}

		.charts-section {
			height: auto;
		}
	}
</style>

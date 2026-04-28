<script lang="ts">
	import { onMount } from "svelte";
	import type { TraceData } from "$lib/types";

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
		onSpeedChange = () => {},
	}: Props = $props();

	// Get time range
	let timeMin = $derived.by(() =>
		traceData.length > 0 ? traceData[0].time : 0,
	);
	let timeMax = $derived.by(() =>
		traceData.length > 0 ? traceData[traceData.length - 1].time : 0,
	);
	let currentTimePercent = $derived.by(() =>
		timeMax > timeMin
			? ((currentTime - timeMin) / (timeMax - timeMin)) * 100
			: 0,
	);
	let timeOffset = $derived.by(() => Math.round(currentTime - timeMin));

	// Format time for display
	function formatTime(seconds: number): string {
		return new Date(seconds * 1000).toLocaleTimeString([], {
			hour12: false,
		});
	}

	// Handle slider change
	function handleSliderChange(event: Event) {
		const target = event.target as HTMLInputElement;
		const percent = parseFloat(target.value);
		const newTime = timeMin + (percent / 100) * (timeMax - timeMin);
		onTimeChange(newTime);
	}

	// Handle seek
	function handleSeek(time: number) {
		onTimeChange(time);
	}

	// Keyboard handler
	const SEEK_AMOUNT = 1; // seconds

	onMount(() => {
		const handleKeyDown = (e: KeyboardEvent) => {
			// Ignore when typing in file inputs
			if (e.target instanceof HTMLInputElement) return;

			switch (e.key) {
				case " ":
					e.preventDefault();
					e.stopPropagation();
					// Toggle play/pause - call parent's callback
					onTogglePlay();
					break;
				case "ArrowLeft":
					e.preventDefault();
					e.stopPropagation();
					handleSeek(Math.max(timeMin, currentTime - SEEK_AMOUNT));
					break;
				case "ArrowRight":
					e.preventDefault();
					e.stopPropagation();
					handleSeek(Math.min(timeMax, currentTime + SEEK_AMOUNT));
					break;
				case "?":
					e.preventDefault();
					// Help modal - optional, can be added later
					console.log(
						"Keyboard shortcuts: Space=play/pause, Arrows=seek±5s",
					);
					break;
			}
		};

		document.addEventListener("keydown", handleKeyDown);
		return () => document.removeEventListener("keydown", handleKeyDown);
	});
</script>

<div class="timeline-container">
	<div class="playback-bar">
		<div class="playback-controls">
			<button
				class="play-pause"
				onclick={onTogglePlay}
				title="Space: Play/Pause"
			>
				{isPlaying ? "⏸" : "▶"}
			</button>
			<select
				bind:value={playbackSpeed}
				onchange={(e) =>
					onSpeedChange(
						Number((e.target as HTMLSelectElement).value),
					)}
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

		<div class="time-info">
			<span class="current">{formatTime(currentTime)} ({timeOffset})</span
			>
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

<style>
	.timeline-container {
		width: 100%;
		height: 100%;
		background-color: #0a0a0a;
		display: flex;
		flex-direction: column;
	}

	.playback-bar {
		padding: 0.2rem 0.3rem;
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
		vertical-align: middle;
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

<script lang="ts">
	import type { RouteData, FsmState } from '$lib/types';
	import { FSM_STATE_COLORS } from '$lib/constants/fsmColors';

	interface Props {
		routeData: RouteData;
		busProgress: number;
		highlightedEvent?: {
			stopIdx: number;
			state: FsmState;
		} | null;
	}

	let { routeData, busProgress: busProgressProp, highlightedEvent = null }: Props = $props();

	let busProgress = $derived.by(() => busProgressProp);

	// Calculate scale
	const maxProgress = $derived.by(() => {
		if (routeData.nodes.length === 0) return 100000;
		return routeData.nodes[routeData.nodes.length - 1].cum_dist_cm;
	});

	// Find segment for bus highlight
	const currentSegment = $derived.by(() => {
		if (routeData.nodes.length < 2) return null;
		if (busProgress < routeData.nodes[0].cum_dist_cm) return null;
		if (busProgress >= routeData.nodes[routeData.nodes.length - 1].cum_dist_cm) {
			const lastIdx = routeData.nodes.length - 2;
			return {
				start: routeData.nodes[lastIdx].cum_dist_cm,
				end: routeData.nodes[lastIdx + 1].cum_dist_cm
			};
		}

		for (let i = 0; i < routeData.nodes.length - 1; i++) {
			const node = routeData.nodes[i];
			const nextNode = routeData.nodes[i + 1];
			if (busProgress >= node.cum_dist_cm && busProgress < nextNode.cum_dist_cm) {
				return { start: node.cum_dist_cm, end: nextNode.cum_dist_cm };
			}
		}
		return null;
	});

	function getProgressPercent(progressCm: number): number {
		return (progressCm / maxProgress) * 100;
	}

	// Generate scale markers
	const scaleMarkers = $derived.by(() => {
		const markers: { percent: number; label: string }[] = [];
		const intervalCm = 50000; // 500m
		for (let d = 0; d <= maxProgress; d += intervalCm) {
			markers.push({
				percent: getProgressPercent(d),
				label: `${(d / 100000).toFixed(1)}km`
			});
		}
		return markers;
	});
</script>

<div class="linear-route-widget">
	<!-- Top row: Route line with stops and bus -->
	<div class="route-row">
		<div class="route-line-container">
			<!-- Base line -->
			<div class="route-line-base"></div>

			<!-- Current segment highlight -->
			{#if currentSegment}
				<div
					class="segment-highlight"
					style="left: {getProgressPercent(currentSegment.start)}%; right: {100 - getProgressPercent(currentSegment.end)}%;"
				></div>
			{/if}

			<!-- Stops -->
			{#each routeData.stops as stop (stop.progress_cm)}
				{@const percent = getProgressPercent(stop.progress_cm)}
				{@const isHighlighted = highlightedEvent?.stopIdx === stop.index}
				<div
					class="stop-marker"
					style="left: {percent}%;"
					class:highlighted={isHighlighted}
				>
					<div class="stop-label">#{stop.index}</div>
					<div class="stop-tick"></div>
					{#if isHighlighted}
						<div class="highlight-ring" style="background-color: {FSM_STATE_COLORS[highlightedEvent!.state]};"></div>
					{/if}
				</div>
			{/each}

			<!-- Bus position -->
			<div class="bus-marker" style="left: {getProgressPercent(busProgress)}%;">
				<div class="bus-emoji">🚌</div>
			</div>
		</div>
	</div>

	<!-- Middle row: Distance scale -->
	<div class="scale-row">
		{#each scaleMarkers as marker}
			<div class="scale-marker" style="left: {marker.percent}%;">
				<div class="scale-tick"></div>
				<div class="scale-label">{marker.label}</div>
			</div>
		{/each}
	</div>

	<!-- Bottom row: Event info -->
	{#if highlightedEvent}
		{@const stop = routeData.stops[highlightedEvent.stopIdx]}
		{#if stop}
			{@const percent = getProgressPercent(stop.progress_cm)}
			<div class="event-row" style="--event-color: {FSM_STATE_COLORS[highlightedEvent.state]};">
				<div class="event-line" style="left: {percent}%;"></div>
				<div class="event-info" style="left: {percent}%;">
					<span class="event-stop">Stop #{highlightedEvent.stopIdx}</span>
					<span class="event-arrow">→</span>
					<span class="event-state">{highlightedEvent.state}</span>
				</div>
			</div>
		{/if}
	{/if}
</div>

<style>
	.linear-route-widget {
		width: 100%;
		height: 100%;
		padding: 1rem 1.5rem;
		display: flex;
		flex-direction: column;
		gap: 1.25rem;
		background-color: #0a0a0a;
	}

	/* Route row */
	.route-row {
		position: relative;
		height: 40px;
	}

	.route-line-container {
		position: relative;
		width: 100%;
		height: 100%;
	}

	.route-line-base {
		position: absolute;
		top: 50%;
		left: 0;
		right: 0;
		height: 3px;
		background-color: #4a5568;
		border-radius: 2px;
		transform: translateY(-50%);
	}

	.segment-highlight {
		position: absolute;
		top: 50%;
		height: 5px;
		background-color: #3b82f6;
		border-radius: 3px;
		transform: translateY(-50%);
		box-shadow: 0 0 10px rgba(59, 130, 246, 0.6);
		z-index: 1;
	}

	/* Stop markers */
	.stop-marker {
		position: absolute;
		top: 50%;
		transform: translate(-50%, -50%);
		display: flex;
		flex-direction: column;
		align-items: center;
		pointer-events: none;
	}

	.stop-label {
		font-size: 15px;
		font-weight: bold;
		font-family: 'JetBrains Mono', 'Monaco', monospace;
		color: #ffffff;
		margin-bottom: 8px;
	}

	.stop-tick {
		width: 2px;
		height: 14px;
		background-color: #ef4444;
	}

	.highlight-ring {
		position: absolute;
		width: 14px;
		height: 14px;
		border-radius: 50%;
		top: 50%;
		transform: translateY(-50%);
		border: 2px solid #ffffff;
		z-index: 2;
	}

	/* Bus marker */
	.bus-marker {
		position: absolute;
		top: 50%;
		transform: translate(-50%, -50%);
		z-index: 10;
	}

	.bus-emoji {
		font-size: 20px;
		line-height: 1;
	}

	/* Scale row */
	.scale-row {
		position: relative;
		height: 30px;
		border-top: 2px solid #718096;
	}

	.scale-marker {
		position: absolute;
		top: 0;
		transform: translateX(-50%);
	}

	.scale-tick {
		width: 2px;
		height: 8px;
		background-color: #718096;
		margin: 0 auto;
	}

	.scale-label {
		margin-top: 6px;
		font-size: 13px;
		font-family: 'JetBrains Mono', 'Monaco', monospace;
		color: #cbd5e0;
		text-align: center;
		white-space: nowrap;
	}

	/* Event row */
	.event-row {
		position: relative;
		height: 28px;
		--event-color: #3b82f6;
	}

	.event-line {
		position: absolute;
		top: 0;
		width: 2px;
		height: 100%;
		background-image: linear-gradient(to bottom, var(--event-color) 50%, transparent 50%);
		background-size: 2px 8px;
		background-repeat: repeat-y;
		transform: translateX(-50%);
		opacity: 0.7;
	}

	.event-info {
		position: absolute;
		top: 50%;
		transform: translate(-50%, -50%);
		display: flex;
		align-items: center;
		gap: 0.5rem;
		white-space: nowrap;
	}

	.event-stop {
		font-size: 15px;
		font-weight: bold;
		font-family: 'JetBrains Mono', 'Monaco', monospace;
		color: var(--event-color);
	}

	.event-arrow {
		font-size: 14px;
		color: #a0aec0;
	}

	.event-state {
		font-size: 14px;
		font-family: 'JetBrains Mono', 'Monaco', monospace;
		color: #e2e8f0;
		font-weight: 500;
	}
</style>

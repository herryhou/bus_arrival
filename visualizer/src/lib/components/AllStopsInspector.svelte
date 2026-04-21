<script lang="ts">
	import type { TraceData, StopTraceState, FsmState } from '$lib/types';

	interface Props {
		traceData: TraceData;
		currentTime: number;
		v_cms: number;
	}

	let { traceData, currentTime, v_cms }: Props = $props();

	// Find the trace record closest to current time
	const currentRecord = $derived.by(() => {
		return traceData.find((r) => r.time === currentTime)
			?? traceData.reduce((closest, record) => {
				return Math.abs(record.time - currentTime) < Math.abs(closest.time - currentTime)
					? record
					: closest;
			}, traceData[0]);
	});

	// Get all stop states at current time
	const allStopStates = $derived.by(() => {
		return currentRecord?.stop_states ?? [];
	});

	const stateColors: Record<FsmState, string> = {
		Idle: '#9ca3af',
		Approaching: '#3b82f6',
		Arriving: '#f59e0b',
		AtStop: '#22c55e',
		Departed: '#6b7280',
		TripComplete: '#1e3a8a'
	};

	const stateBgColors: Record<FsmState, string> = {
		Idle: 'rgba(156, 163, 175, 0.15)',
		Approaching: 'rgba(59, 130, 246, 0.15)',
		Arriving: 'rgba(245, 158, 11, 0.15)',
		AtStop: 'rgba(34, 197, 94, 0.15)',
		Departed: 'rgba(107, 114, 128, 0.15)',
		TripComplete: 'rgba(30, 58, 138, 0.15)'
	};

	function formatProb(value: number): string {
		return Math.round(value).toString().padStart(3, '0');
	}

	function getProbColor(prob: number): string {
		if (prob >= 191) return '#22c55e';
		if (prob >= 128) return '#f59e0b';
		return '#ef4444';
	}
</script>

<div class="all-stops-container">
	<div class="stops-header">
		<h3>All Stops Monitor</h3>
		<div class="stops-count">{allStopStates.length} active stops</div>
	</div>

	<div class="stops-grid">
		{#each allStopStates as stopState (stopState.stop_idx)}
			<div class="stop-card" style="border-color: {stateColors[stopState.fsm_state]}; background-color: {stateBgColors[stopState.fsm_state]};">
				<div class="stop-card-header">
					<span class="stop-index">Stop {stopState.stop_idx}</span>
					<span class="stop-state" style="color: {stateColors[stopState.fsm_state]};">
						{stopState.fsm_state}
					</span>
				</div>

				<div class="stop-metrics">
					<!-- Probability Gauge -->
					<div class="metric-group">
						<div class="metric-label">Probability</div>
						<div class="prob-gauge">
							<div class="prob-bar" style="width: {(stopState.probability / 255) * 100}%; background-color: {getProbColor(stopState.probability)};"></div>
							<span class="prob-value" style="color: {getProbColor(stopState.probability)};">{formatProb(stopState.probability)}</span>
						</div>
					</div>

					<!-- Distance -->
					<div class="metric-row">
						<span class="metric-label">Distance</span>
						<span class="metric-value">{Math.round(stopState.progress_distance_cm / 100)}m</span>
					</div>

					<!-- Dwell Time -->
					<div class="metric-row">
						<span class="metric-label">Dwell</span>
						<span class="metric-value">{stopState.dwell_time_s}s</span>
					</div>

					<!-- Features Grid -->
					<div class="features-mini">
						<div class="feature-item">
							<span class="feature-label">p₁(距離)</span>
							<span class="feature-val">{stopState.features.p1}</span>
						</div>
						<div class="feature-item">
							<span class="feature-label">p₂(速度)</span>
							<span class="feature-val">{stopState.features.p2}</span>
						</div>
						<div class="feature-item">
							<span class="feature-label">p₃(路線進度)</span>
							<span class="feature-val">{stopState.features.p3}</span>
						</div>
						<div class="feature-item">
							<span class="feature-label">p₄(停留時間)</span>
							<span class="feature-val">{stopState.features.p4}</span>
						</div>
					</div>

					<!-- Just Arrived Badge -->
					{#if stopState.just_arrived}
						<div class="arrived-badge">JUST ARRIVED</div>
					{/if}
				</div>
			</div>
		{/each}
	</div>

	{#if allStopStates.length === 0}
		<div class="empty-state">
			<p>No active stops at current time</p>
		</div>
	{/if}
</div>

<style>
	.all-stops-container {
		background-color: #1e1e1e;
		border: 1px solid #333;
		border-radius: 0.5rem;
		padding: 1rem;
		height: 100%;
		overflow-y: auto;
		font-family: 'JetBrains Mono', 'Monaco', monospace;
	}

	.stops-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: 1rem;
		border-bottom: 1px solid #333;
		padding-bottom: 0.5rem;
	}

	.stops-header h3 {
		font-size: 0.75rem;
		text-transform: uppercase;
		letter-spacing: 0.1em;
		color: #888;
		margin: 0;
	}

	.stops-count {
		font-size: 0.7rem;
		color: #666;
		background-color: #2a2a2a;
		padding: 2px 8px;
		border-radius: 10px;
	}

	.stops-grid {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}

	.stop-card {
		background-color: #1a1a1a;
		border: 1px solid #333;
		border-left-width: 4px;
		border-radius: 0.25rem;
		padding: 0.75rem;
		transition: all 0.2s;
	}

	.stop-card:hover {
		border-left-width: 6px;
	}

	.stop-card-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: 0.5rem;
	}

	.stop-index {
		font-size: 0.8rem;
		font-weight: bold;
		color: #fff;
	}

	.stop-state {
		font-size: 0.65rem;
		font-weight: bold;
		text-transform: uppercase;
		padding: 2px 6px;
		border-radius: 4px;
		background-color: rgba(0, 0, 0, 0.3);
	}

	.stop-metrics {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.metric-group {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
	}

	.metric-label {
		font-size: 0.65rem;
		color: #666;
		text-transform: uppercase;
	}

	.prob-gauge {
		position: relative;
		height: 20px;
		background-color: #0a0a0a;
		border-radius: 2px;
		overflow: hidden;
		border: 1px solid #333;
	}

	.prob-bar {
		height: 100%;
		transition: width 0.3s, background-color 0.3s;
	}

	.prob-value {
		position: absolute;
		top: 50%;
		left: 50%;
		transform: translate(-50%, -50%);
		font-size: 0.7rem;
		font-weight: bold;
		text-shadow: 0 0 2px rgba(0, 0, 0, 0.8);
	}

	.metric-row {
		display: flex;
		justify-content: space-between;
		font-size: 0.75rem;
	}

	.metric-value {
		color: #eee;
		font-weight: bold;
	}

	.features-mini {
		display: grid;
		grid-template-columns: repeat(4, 1fr);
		gap: 0.25rem;
		margin-top: 0.25rem;
	}

	.feature-item {
		background-color: #0a0a0a;
		border: 1px solid #333;
		border-radius: 2px;
		padding: 0.25rem;
		text-align: center;
	}

	.feature-label {
		display: block;
		font-size: 0.6rem;
		color: #3b82f6;
	}

	.feature-val {
		display: block;
		font-size: 0.7rem;
		color: #00ff00;
		font-weight: bold;
	}

	.arrived-badge {
		margin-top: 0.25rem;
		background: linear-gradient(90deg, #22c55e, #16a34a);
		color: white;
		font-size: 0.6rem;
		font-weight: bold;
		text-align: center;
		padding: 0.25rem;
		border-radius: 2px;
		text-transform: uppercase;
		animation: pulse 1s ease-in-out infinite;
	}

	@keyframes pulse {
		0%, 100% { opacity: 1; }
		50% { opacity: 0.7; }
	}

	.empty-state {
		text-align: center;
		color: #555;
		font-size: 0.8rem;
		padding: 2rem;
	}
</style>

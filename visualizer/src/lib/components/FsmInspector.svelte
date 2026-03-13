<script lang="ts">
	import type { TraceData, StopTraceState, FsmState } from '$lib/types';

	interface Props {
		traceData: TraceData;
		selectedStop?: number | null;
		currentTime: number;
	}

	let { traceData, selectedStop = null, currentTime }: Props = $props();

	// Find the trace record closest to current time
	function getCurrentRecord() {
		return traceData.find((r) => r.time === currentTime)
			?? traceData.reduce((closest, record) => {
				return Math.abs(record.time - currentTime) < Math.abs(closest.time - currentTime)
					? record
					: closest;
			}, traceData[0]);
	}

	// Get the stop state for the selected stop at current time
	function getStopState() {
		const currentRecord = getCurrentRecord();
		return selectedStop !== null && currentRecord
			? currentRecord.stop_states.find((s) => s.stop_idx === selectedStop)
			: null;
	}

	// Get state transition history for selected stop
	function getStateHistory() {
		if (selectedStop === null) return [];
		return traceData
			.filter((r) => r.stop_states.some((s) => s.stop_idx === selectedStop))
			.map((r) => {
				const state = r.stop_states.find((s) => s.stop_idx === selectedStop)!;
				return {
					time: r.time,
					state: state.fsm_state,
					justArrived: state.just_arrived
				};
			});
	}

	// Find state transitions
	function getTransitions() {
		const history = getStateHistory();
		return history.filter((entry, i) => {
			if (i === 0) return true;
			return entry.state !== history[i - 1].state || entry.justArrived;
		});
	}

	// FSM state color mapping
	const stateColors: Record<FsmState, string> = {
		Approaching: 'bg-blue-500',
		Arriving: 'bg-amber-500',
		AtStop: 'bg-green-500',
		Departed: 'bg-gray-500'
	};

	const stateDescriptions: Record<FsmState, string> = {
		Approaching: 'Bus is in corridor, approaching stop',
		Arriving: 'Bus is close to stop, high arrival probability',
		AtStop: 'Bus has arrived and is at stop',
		Departed: 'Bus has moved past the stop'
	};

	// Format dwell time
	function formatDwellTime(seconds: number): string {
		if (seconds < 60) return `${seconds}s`;
		const mins = Math.floor(seconds / 60);
		const secs = seconds % 60;
		return `${mins}m ${secs}s`;
	}

	// Format distance
	function formatDistance(cm: number): string {
		if (Math.abs(cm) < 100) return `${cm} cm`;
		const meters = (cm / 100).toFixed(1);
		return `${meters} m`;
	}
</script>

<div class="inspector-container">
	{#if getStopState()}
		{@const stopState = getStopState()}
		{@const currentRecord = getCurrentRecord()}
		<!-- Current State -->
		<div class="state-card">
			<h3 class="card-title">Current State - Stop {selectedStop}</h3>

			<div class="state-display">
				<div class="state-indicator {stateColors[stopState.fsm_state]}"></div>
				<div class="state-info">
					<div class="state-name">{stopState.fsm_state}</div>
					<div class="state-desc">{stateDescriptions[stopState.fsm_state]}</div>
				</div>
			</div>

			{#if stopState.just_arrived}
				<div class="arrival-badge">
					<span class="arrival-icon">🚌</span> Just Arrived!
				</div>
			{/if}
		</div>

		<!-- Metrics -->
		<div class="metrics-grid">
			<div class="metric-card">
				<div class="metric-label">Distance to Stop</div>
				<div class="metric-value">{formatDistance(stopState.distance_cm)}</div>
				<div class="metric-hint">
					{stopState.distance_cm < 0 ? 'Past stop' : stopState.distance_cm < 500 ? 'Very close' : 'Approaching'}
				</div>
			</div>

			<div class="metric-card">
				<div class="metric-label">Dwell Time</div>
				<div class="metric-value">{formatDwellTime(stopState.dwell_time_s)}</div>
				<div class="metric-hint">
					{stopState.dwell_time_s > 0 ? 'Time since arrival' : 'Not at stop yet'}
				</div>
			</div>

			<div class="metric-card">
				<div class="metric-label">Arrival Probability</div>
				<div class="metric-value">{stopState.probability}/255</div>
				<div class="metric-hint">
					{stopState.probability > 191 ? 'Above threshold' : 'Below threshold'}
				</div>
			</div>

			<div class="metric-card">
				<div class="metric-label">Speed</div>
				<div class="metric-value">{getCurrentRecord().v_cms} cm/s</div>
				<div class="metric-hint">
					{getCurrentRecord().v_cms < 50 ? 'Stopped' : getCurrentRecord().v_cms < 200 ? 'Slow' : 'Moving'}
				</div>
			</div>
		</div>

		<!-- State Transitions -->
		<div class="transitions-card">
			<h3 class="card-title">State Transitions</h3>
			<div class="transitions-list">
				{#each getTransitions() as entry}
					<div class="transition-item">
						<div class="transition-time">
							{new Date(entry.time * 1000).toLocaleTimeString()}
						</div>
						<div class="transition-state">
							<span class="transition-dot {stateColors[entry.state]}"></span>
							{entry.state}
							{#if entry.justArrived}
								<span class="arrival-tag">ARRIVED</span>
							{/if}
						</div>
					</div>
				{/each}
			</div>
		</div>
	{:else}
		<div class="empty-state">
			<p>Select a stop to see state machine details</p>
		</div>
	{/if}
</div>

<style>
	.inspector-container {
		display: flex;
		flex-direction: column;
		gap: 1rem;
		padding: 1rem;
		background-color: #f9fafb;
		border-radius: 0.5rem;
		height: 100%;
		overflow-y: auto;
	}

	.state-card {
		background-color: white;
		border-radius: 0.375rem;
		padding: 1rem;
		box-shadow: 0 1px 3px 0 rgba(0, 0, 0, 0.1);
	}

	.card-title {
		font-size: 0.875rem;
		font-weight: 600;
		color: #374151;
		margin-bottom: 0.75rem;
	}

	.state-display {
		display: flex;
		align-items: center;
		gap: 1rem;
	}

	.state-indicator {
		width: 3rem;
		height: 3rem;
		border-radius: 50%;
		flex-shrink: 0;
	}

	.state-info {
		flex: 1;
	}

	.state-name {
		font-size: 1.125rem;
		font-weight: 700;
		color: #111827;
	}

	.state-desc {
		font-size: 0.875rem;
		color: #6b7280;
		margin-top: 0.25rem;
	}

	.arrival-badge {
		margin-top: 1rem;
		padding: 0.5rem 1rem;
		background-color: #dcfce7;
		color: #166534;
		border-radius: 0.375rem;
		font-weight: 600;
		text-align: center;
	}

	.arrival-icon {
		margin-right: 0.5rem;
	}

	.metrics-grid {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		gap: 0.75rem;
	}

	.metric-card {
		background-color: white;
		border-radius: 0.375rem;
		padding: 0.75rem;
		box-shadow: 0 1px 3px 0 rgba(0, 0, 0, 0.1);
	}

	.metric-label {
		font-size: 0.75rem;
		color: #6b7280;
		margin-bottom: 0.25rem;
	}

	.metric-value {
		font-size: 1.125rem;
		font-weight: 700;
		color: #111827;
	}

	.metric-hint {
		font-size: 0.75rem;
		color: #9ca3af;
		margin-top: 0.25rem;
	}

	.transitions-card {
		background-color: white;
		border-radius: 0.375rem;
		padding: 1rem;
		box-shadow: 0 1px 3px 0 rgba(0, 0, 0, 0.1);
	}

	.transitions-list {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		max-height: 200px;
		overflow-y: auto;
	}

	.transition-item {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		padding: 0.5rem;
		background-color: #f3f4f6;
		border-radius: 0.25rem;
	}

	.transition-time {
		font-size: 0.75rem;
		color: #6b7280;
		min-width: 80px;
	}

	.transition-state {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		font-size: 0.875rem;
		font-weight: 500;
		color: #374151;
	}

	.transition-dot {
		width: 0.75rem;
		height: 0.75rem;
		border-radius: 50%;
	}

	.arrival-tag {
		font-size: 0.625rem;
		padding: 0.125rem 0.375rem;
		background-color: #dcfce7;
		color: #166534;
		border-radius: 0.25rem;
		font-weight: 600;
	}

	.empty-state {
		background-color: white;
		border-radius: 0.375rem;
		padding: 2rem;
		box-shadow: 0 1px 3px 0 rgba(0, 0, 0, 0.1);
		text-align: center;
	}

	.empty-state p {
		color: #9ca3af;
		font-size: 0.875rem;
	}

	.bg-blue-500 {
		background-color: #3b82f6;
	}

	.bg-amber-500 {
		background-color: #f59e0b;
	}

	.bg-green-500 {
		background-color: #22c55e;
	}

	.bg-gray-500 {
		background-color: #6b7280;
	}
</style>

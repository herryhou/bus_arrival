<script lang="ts">
	import type { TraceData, StopTraceState, FsmState } from '$lib/types';
	import { FSM_STATE_COLORS } from '$lib/constants/fsmColors';

	interface Props {
		traceData: TraceData;
		selectedStop?: number | null;
		currentTime: number;
	}

	let { traceData, selectedStop = null, currentTime }: Props = $props();

	// Find the trace record closest to current time
	const currentRecord = $derived.by(() => {
		return traceData.find((r) => r.time === currentTime)
			?? traceData.reduce((closest, record) => {
				return Math.abs(record.time - currentTime) < Math.abs(closest.time - currentTime)
					? record
					: closest;
			}, traceData[0]);
	});

	// Get the stop state for the selected stop at current time
	const stopState = $derived.by(() => {
		return selectedStop !== null && currentRecord
			? currentRecord.stop_states.find((s) => s.stop_idx === selectedStop)
			: null;
	});

	const stateColors = FSM_STATE_COLORS;

	const states: FsmState[] = ['Idle', 'Approaching', 'Arriving', 'AtStop', 'Departed', 'TripComplete'];

	// Coordinates for SVG state diagram
	// v8.5: Linear left-to-right layout matching FSM flow: Idle → Approaching → Arriving → AtStop → Departed → TripComplete
	const stateNodes = {
		Idle: { x: 30, y: 50 },
		Approaching: { x: 100, y: 50 },
		Arriving: { x: 170, y: 50 },
		AtStop: { x: 240, y: 50 },
		Departed: { x: 310, y: 50 },
		TripComplete: { x: 380, y: 50 }  // Terminal state at far right
	};

	function formatDwellTime(seconds: number): string {
		if (seconds < 60) return `${seconds}s`;
		const mins = Math.floor(seconds / 60);
		const secs = seconds % 60;
		return `${mins}m ${secs}s`;
	}
</script>

<div class="inspector-container">
	{#if stopState}
		<div class="inspector-header">
			<h3>FSM Inspector: Stop {selectedStop}</h3>
			<div class="status-pill {stopState.fsm_state.toLowerCase()}">
				{stopState.fsm_state}
			</div>
		</div>

		<!-- SVG State Diagram -->
		<div class="diagram-card">
			<svg viewBox="0 0 420 100">
				<!-- Transitions -->
				<defs>
					<marker id="arrow" markerWidth="10" markerHeight="10" refX="8" refY="3" orientation="auto" markerUnits="strokeWidth">
						<path d="M0,0 L0,6 L9,3 z" fill="#444" />
					</marker>
					<marker id="arrow-active" markerWidth="10" markerHeight="10" refX="8" refY="3" orientation="auto" markerUnits="strokeWidth">
						<path d="M0,0 L0,6 L9,3 z" fill="#3b82f6" />
					</marker>
				</defs>

				<!-- Node connections - Linear left-to-right flow -->
				<!-- Idle -> Approaching -->
				<line x1="50" y1="50" x2="79" y2="50" stroke="#444" marker-end="url(#arrow)" />
				<!-- Approaching -> Arriving -->
				<line x1="120" y1="50" x2="149" y2="50" stroke="#444" marker-end="url(#arrow)" />
				<!-- Arriving -> AtStop -->
				<line x1="190" y1="50" x2="219" y2="50" stroke="#444" marker-end="url(#arrow)" />
				<!-- AtStop -> Departed -->
				<line x1="260" y1="50" x2="289" y2="50" stroke="#444" marker-end="url(#arrow)" />
				<!-- Departed -> TripComplete -->
				<line x1="330" y1="50" x2="359" y2="50" stroke="#444" marker-end="url(#arrow)" stroke-dasharray="3,3" />

				<!-- Bidirectional: Approaching -> Idle (corridor exit) -->
				<path d="M 95 35 Q 65 20 35 35" fill="none" stroke="#444" marker-end="url(#arrow)" stroke-dasharray="2,2" />

				<!-- Skip transition (Arriving -> Departed, bypassing AtStop) -->
				<path d="M 180 35 Q 240 10 300 35" fill="none" stroke="#444" marker-end="url(#arrow)" />

				{#each Object.entries(stateNodes) as [name, pos]}
					<g transform="translate({pos.x}, {pos.y})">
						<circle 
							r="20" 
							fill={stopState.fsm_state === name ? stateColors[name as FsmState] : '#1a1a1a'} 
							stroke={stopState.fsm_state === name ? '#fff' : '#444'}
							stroke-width="2"
							class="state-node"
						/>
						<text 
							y="35" 
							text-anchor="middle" 
							font-size="8" 
							fill={stopState.fsm_state === name ? '#fff' : '#888'}
							class="state-label"
						>
							{name}
						</text>
						{#if stopState.fsm_state === name}
							<circle r="24" fill="none" stroke={stateColors[name as FsmState]} stroke-width="1" opacity="0.5">
								<animate attributeName="r" from="20" to="28" dur="1.5s" repeatCount="indefinite" />
								<animate attributeName="opacity" from="0.5" to="0" dur="1.5s" repeatCount="indefinite" />
							</circle>
						{/if}
					</g>
				{/each}
			</svg>
		</div>

		<div class="metrics-list">
			<div class="metric-row">
				<span class="label">Dwell Time</span>
				<span class="value">{formatDwellTime(stopState.dwell_time_s)}</span>
			</div>
			<div class="metric-row">
				<span class="label">Distance</span>
				<span class="value">{Math.round(stopState.progress_distance_cm / 100)}m</span>
			</div>
			<div class="metric-row">
				<span class="label">Just Arrived</span>
				<span class="value {stopState.just_arrived ? 'yes' : 'no'}">{stopState.just_arrived ? 'TRUE' : 'FALSE'}</span>
			</div>
		</div>
	{:else}
		<div class="empty-inspector">
			<p>Select a stop on the map or via the active list to inspect its FSM state.</p>
		</div>
	{/if}
</div>

<style>
	.inspector-container {
		background-color: #1e1e1e;
		border: 1px solid #333;
		border-radius: 0.5rem;
		padding: 1rem;
		height: 100%;
		font-family: 'JetBrains Mono', 'Monaco', monospace;
	}

	.inspector-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: 1rem;
		border-bottom: 1px solid #333;
		padding-bottom: 0.5rem;
	}

	.inspector-header h3 {
		font-size: 0.75rem;
		text-transform: uppercase;
		letter-spacing: 0.1em;
		color: #888;
		margin: 0;
	}

	.status-pill {
		font-size: 0.65rem;
		padding: 2px 8px;
		border-radius: 10px;
		font-weight: bold;
		text-transform: uppercase;
	}

	.idle { background-color: #94a3b8; color: white; }
	.approaching { background-color: #3b82f6; color: white; }
	.arriving { background-color: #f59e0b; color: white; }
	.atstop { background-color: #22c55e; color: white; }
	.departed { background-color: #6b7280; color: white; }
	.tripcomplete { background-color: #1e3a8a; color: white; }

	.diagram-card {
		background-color: #000;
		border: 1px solid #333;
		border-radius: 0.25rem;
		padding: 1rem;
		margin-bottom: 1rem;
	}

	.state-node {
		transition: fill 0.3s, stroke 0.3s;
	}

	.state-label {
		font-weight: bold;
	}

	.metrics-list {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.metric-row {
		display: flex;
		justify-content: space-between;
		font-size: 0.8rem;
		padding-bottom: 0.25rem;
		border-bottom: 1px solid #222;
	}

	.metric-row .label { color: #666; }
	.metric-row .value { color: #eee; font-weight: bold; }
	.metric-row .value.yes { color: #22c55e; }
	.metric-row .value.no { color: #444; }

	.empty-inspector {
		height: 100%;
		display: flex;
		align-items: center;
		justify-content: center;
		text-align: center;
		color: #555;
		font-size: 0.8rem;
		padding: 2rem;
	}
</style>

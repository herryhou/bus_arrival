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
	const TIME_TOLERANCE_SECONDS = 1;

	// Probability thresholds (0-255 range)
	const PROB_HIGH_THRESHOLD = 191; // 75% of 255
	const PROB_MED_THRESHOLD = 128; // 50% of 255

	function isEventAtCurrentTime(eventTime: number): boolean {
		return Math.abs(eventTime - currentTime) < TIME_TOLERANCE_SECONDS;
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

	// Helper: Format probability value (0-255) as padded string
	function formatProb(value: number): string {
		return Math.round(value).toString().padStart(3, '0');
	}

	// Helper: Get probability color
	function getProbColor(prob: number): string {
		if (prob >= PROB_HIGH_THRESHOLD) return '#22c55e';
		if (prob >= PROB_MED_THRESHOLD) return '#f59e0b';
		return '#ef4444';
	}

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

		// traceData is already sorted by time, so events are in chronological order
		return log;
	});

	// Derived: Event count
	let eventCount = $derived.by(() => events.length);

	// Derived: Current trace record
	let currentRecord = $derived.by(() => {
		if (traceData.length === 0) return null;
		return traceData.reduce((prev, curr) =>
			Math.abs(curr.time - currentTime) < Math.abs(prev.time - currentTime) ? curr : prev
		);
	});

	// Derived: All stop states at current time
	let allStopStates = $derived(currentRecord?.stop_states ?? []);

	// Derived: Stop count
	let stopCount = $derived(allStopStates.length);
</script>

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

	<!-- Bottom half: Stops (50%) -->
	<div class="sidebar-section stops-section">
		<div class="section-header">
			<h4>All Stops Monitor</h4>
			<span class="stops-count">{stopCount} active</span>
		</div>
		<div class="section-content">
			{#each allStopStates as stopState (stopState.stop_idx)}
				<div
					class="stop-card-compact"
					class:selected={selectedStop === stopState.stop_idx}
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
						<span>p₁:{stopState.features.p1}</span>
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
</div>

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
		user-select: none;
	}

	.empty-events,
	.empty-stops {
		text-align: center;
		color: #555;
		font-size: 0.7rem;
		padding: 2rem;
		user-select: none;
	}

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

	.stop-card-compact.selected {
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
</style>

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

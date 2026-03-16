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
</script>

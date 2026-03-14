<script lang="ts">
	import type { TraceData, FsmState } from '$lib/types';

	interface Props {
		traceData: TraceData;
		onSeek: (time: number) => void;
	}

	let { traceData, onSeek }: Props = $props();

	type EventType = 'JUMP' | 'RECOVERY' | 'TRANSITION' | 'ARRIVAL';

	interface LogEvent {
		time: number;
		type: EventType;
		message: string;
		stopIdx?: number;
		state?: FsmState;
	}

	// Derived events list
	let events = $derived.by(() => {
		const log: LogEvent[] = [];
		let lastStates = new Map<number, FsmState>();

		traceData.forEach((record, i) => {
			// 1. GPS Jump
			if (record.gps_jump) {
				log.push({
					time: record.time,
					type: 'JUMP',
					message: `GPS Jump detected: dist > 200m`
				});
			}

			// 2. Recovery
			if (record.recovery_idx !== null) {
				log.push({
					time: record.time,
					type: 'RECOVERY',
					message: `Recovery: Switched to stop ${record.recovery_idx}`
				});
			}

			// 3. Stop events
			record.stop_states.forEach((stop) => {
				// Transition
				const lastState = lastStates.get(stop.stop_idx);
				if (lastState && lastState !== stop.fsm_state) {
					log.push({
						time: record.time,
						type: 'TRANSITION',
						message: `Stop ${stop.stop_idx}: ${lastState} → ${stop.fsm_state}`,
						stopIdx: stop.stop_idx,
						state: stop.fsm_state
					});
				}
				lastStates.set(stop.stop_idx, stop.fsm_state);

				// Arrival
				if (stop.just_arrived) {
					log.push({
						time: record.time,
						type: 'ARRIVAL',
						message: `Stop ${stop.stop_idx}: ARRIVED!`,
						stopIdx: stop.stop_idx
					});
				}
			});
		});

		return log.sort((a, b) => a.time - b.time);
	});

	function formatTime(seconds: number): string {
		return new Date(seconds * 1000).toLocaleTimeString([], { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' });
	}
</script>

<div class="event-log">
	<div class="log-header">
		<h3>Event Narrative</h3>
		<span class="count">{events.length} events</span>
	</div>

	<div class="log-container">
		{#each events as event}
			<button class="event-item {event.type.toLowerCase()}" onclick={() => onSeek(event.time)}>
				<div class="event-meta">
					<span class="event-time">{formatTime(event.time)}</span>
					<span class="event-badge">{event.type}</span>
				</div>
				<div class="event-message">{event.message}</div>
			</button>
		{/each}

		{#if events.length === 0}
			<div class="empty-log">No significant events detected in trace.</div>
		{/if}
	</div>
</div>

<style>
	.event-log {
		background-color: #1e1e1e;
		border: 1px solid #333;
		border-radius: 0.5rem;
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
		font-family: 'JetBrains Mono', 'Monaco', monospace;
	}

	.log-header {
		padding: 0.75rem 1rem;
		background-color: #252525;
		border-bottom: 1px solid #333;
		display: flex;
		justify-content: space-between;
		align-items: center;
	}

	.log-header h3 {
		font-size: 0.75rem;
		text-transform: uppercase;
		letter-spacing: 0.1em;
		color: #888;
		margin: 0;
	}

	.count {
		font-size: 0.7rem;
		color: #555;
	}

	.log-container {
		flex: 1;
		overflow-y: auto;
		padding: 0.5rem;
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
	}

	.event-item {
		width: 100%;
		text-align: left;
		padding: 0.5rem;
		background-color: #2a2a2a;
		border: 1px solid #333;
		border-radius: 0.25rem;
		cursor: pointer;
		transition: background-color 0.2s;
	}

	.event-item:hover {
		background-color: #333;
		border-color: #444;
	}

	.event-meta {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: 0.25rem;
	}

	.event-time {
		font-size: 0.7rem;
		color: #666;
	}

	.event-badge {
		font-size: 0.6rem;
		padding: 1px 4px;
		border-radius: 2px;
		font-weight: bold;
		text-transform: uppercase;
	}

	.event-message {
		font-size: 0.8rem;
		color: #ccc;
	}

	/* Type specific styles */
	.jump .event-badge { background-color: #991b1b; color: #fecaca; }
	.jump { border-left: 3px solid #ef4444; }

	.recovery .event-badge { background-color: #92400e; color: #fef3c7; }
	.recovery { border-left: 3px solid #f59e0b; }

	.transition .event-badge { background-color: #1e40af; color: #dbeafe; }
	.transition { border-left: 3px solid #3b82f6; }

	.arrival .event-badge { background-color: #166534; color: #dcfce7; }
	.arrival { border-left: 3px solid #22c55e; }

	.empty-log {
		padding: 2rem;
		text-align: center;
		color: #555;
		font-size: 0.8rem;
	}
</style>

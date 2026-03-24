// visualizer/src/lib/constants/fsmColors.ts
import type { FsmState } from '$lib/types';

/**
 * Color constants for FSM states
 * Used across LinearRouteWidget, MapView, and EventLog for consistency
 * v8.5: Added TripComplete state (terminal state)
 */
export const FSM_STATE_COLORS: Record<FsmState, string> = {
	'Approaching': '#eab308',   // yellow
	'Arriving': '#f97316',      // orange
	'AtStop': '#22c55e',        // green
	'Departed': '#6b7280',     // gray
	'TripComplete': '#1e3a8a'  // dark blue - terminal state
};

/**
 * Human-readable labels for FSM states
 */
export const FSM_STATE_LABELS: Record<FsmState, string> = {
	'Approaching': 'Approaching',
	'Arriving': 'Arriving',
	'AtStop': 'At Stop',
	'Departed': 'Departed',
	'TripComplete': 'Trip Complete'
};
// visualizer/src/lib/constants/fsmColors.ts
import type { FsmState } from '$lib/types';

/**
 * Color constants for FSM states
 * Used across LinearRouteWidget, MapView, and EventLog for consistency
 * v8.5: Added Idle and TripComplete states
 */
export const FSM_STATE_COLORS: Record<FsmState, string> = {
	'Idle': '#94a3b8',           // slate - inactive state
	'Approaching': '#eab308',    // yellow
	'Arriving': '#f97316',       // orange
	'AtStop': '#22c55e',         // green
	'Departed': '#6b7280',       // gray
	'TripComplete': '#1e3a8a'    // dark blue - terminal state
};

/**
 * Human-readable labels for FSM states
 */
export const FSM_STATE_LABELS: Record<FsmState, string> = {
	'Idle': 'Idle',
	'Approaching': 'Approaching',
	'Arriving': 'Arriving',
	'AtStop': 'At Stop',
	'Departed': 'Departed',
	'TripComplete': 'Trip Complete'
};

/**
 * Abbreviated labels for FSM states (for small badges)
 */
export const FSM_STATE_ABBREVS: Record<FsmState, string> = {
	'Idle': 'Idl',
	'Approaching': 'App',
	'Arriving': 'Arr',
	'AtStop': 'AtS',
	'Departed': 'Dep',
	'TripComplete': 'Com'
};
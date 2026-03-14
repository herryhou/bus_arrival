/**
 * Trace data types from Rust arrival detector
 *
 * These types must match the serde serialization format from Rust:
 * - arrival_detector/src/trace.rs
 * - shared/src/lib.rs (FsmState)
 */

/**
 * FSM state - matches Rust FsmState enum serialization
 * Serde serializes enum variants as their string names
 */
export type FsmState = 'Approaching' | 'Arriving' | 'AtStop' | 'Departed';

/**
 * Individual feature scores from Bayesian probability model
 */
export interface FeatureScores {
	/** Distance likelihood (Gaussian) */
	p1: number;
	/** Speed likelihood (Logistic) */
	p2: number;
	/** Progress likelihood (Gaussian) */
	p3: number;
	/** Dwell time likelihood (Linear) */
	p4: number;
}

/**
 * Per-stop trace state for active stops
 */
export interface StopTraceState {
	/** Stop index */
	stop_idx: number;
	/** Distance to stop (cm) - can be negative if past stop */
	distance_cm: number;
	/** FSM state as string name */
	fsm_state: FsmState;
	/** Dwell time (seconds) */
	dwell_time_s: number;
	/** Arrival probability (0-255) */
	probability: number;
	/** Individual feature scores */
	features: FeatureScores;
	/** Just arrived this frame? */
	just_arrived: boolean;
}

/**
 * Trace record for debugging visualization
 * One line per GPS update in trace.jsonl
 */
export interface TraceRecord {
	/** GPS timestamp (seconds since epoch) */
	time: number;
	/** Latitude */
	lat: number;
	/** Longitude */
	lon: number;
	/** Route progress (cm) */
	s_cm: number;
	/** Velocity (cm/s) */
	v_cms: number;
	/** Heading in 0.01 degrees (0-35999) */
	heading_cdeg?: number;
	/** Active stop indices (corridor filter) */
	active_stops: number[];
	/** Per-stop detailed state (only for active stops) */
	stop_states: StopTraceState[];
	/** GPS jump detected? */
	gps_jump: boolean;
	/** Recovery: new stop index if jumped */
	recovery_idx: number | null;
}

/**
 * Parsed JSONL file - array of trace records
 */
export type TraceData = TraceRecord[];

/**
 * Route data from binary route_data.bin file
 */

/** Route node with all precomputed segment coefficients */
export interface RouteNode {
	/** Squared segment length: |P[i+1] - P[i]|² in cm² */
	len2_cm2: bigint;
	/** Line constant: -(line_a × x₀ + line_b × y₀) */
	line_c: bigint;
	/** Segment heading in 0.01° */
	heading_cdeg: number;
	/** Padding */
	_pad: number;
	/** X coordinate (absolute, from fixed origin 120°E, 20°N) in cm */
	x_cm: number;
	/** Y coordinate (absolute, from fixed origin 120°E, 20°N) in cm */
	y_cm: number;
	/** Cumulative distance from route start in cm */
	cum_dist_cm: number;
	/** Segment vector X: x[i+1] - x[i] in cm */
	dx_cm: number;
	/** Segment vector Y: y[i+1] - y[i] in cm */
	dy_cm: number;
	/** Segment length in cm */
	seg_len_cm: number;
	/** Line coefficient A: = -dy */
	line_a: number;
	/** Line coefficient B: = dx */
	line_b: number;
}

/** Bus stop with precomputed corridor boundaries */
export interface Stop {
	/** Position along route in cm */
	progress_cm: number;
	/** Corridor start: progress_cm - 8000 cm */
	corridor_start_cm: number;
	/** Corridor end: progress_cm + 4000 cm */
	corridor_end_cm: number;
}

/** Grid origin for spatial indexing */
export interface GridOrigin {
	/** Fixed origin X coordinate (cm) */
	x0_cm: number;
	/** Fixed origin Y coordinate (cm) */
	y0_cm: number;
}

/** Complete route data from binary file */
export interface RouteData {
	/** Number of route nodes */
	node_count: number;
	/** Number of bus stops */
	stop_count: number;
	/** Grid origin */
	grid_origin: GridOrigin;
	/** Average latitude for projection (computed from route) */
	lat_avg_deg: number;
	/** Route nodes array */
	nodes: RouteNode[];
	/** Bus stops array */
	stops: Stop[];
	/** CRC32 checksum */
	crc32: number;
}

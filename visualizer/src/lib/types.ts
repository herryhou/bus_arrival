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
 * v8.5: Added Idle and TripComplete states
 */
export type FsmState = 'Idle' | 'Approaching' | 'Arriving' | 'AtStop' | 'Departed' | 'TripComplete';

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
	/** GPS distance to stop (cm) - based on raw GPS projection (z_gps_cm), used for p1 */
	gps_distance_cm: number;
	/** Progress distance to stop (cm) - based on Kalman-filtered position (s_cm), used for p3 */
	progress_distance_cm: number;
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

	// === New: Map matching ===
	/** Which route segment we're matched to (null if off-route) */
	segment_idx?: number | null;
	/** Did the heading constraint pass? (±90° rule) */
	heading_constraint_met: boolean;
	// === New: Divergence ===
	/** Raw GPS projection - Kalman filtered position (cm) */
	divergence_cm: number;
	// === New: GPS quality ===
	/** GPS quality: HDOP */
	hdop?: number | null;
	/** GPS quality: number of satellites */
	num_sats?: number | null;
	/** GPS quality: fix type - "none", "2d", "3d" */
	fix_type?: string | null;
	// === New: Kalman state ===
	/** Position variance (cm²), represents filter uncertainty */
	variance_cm2: number;
	// === New: Corridor info ===
	/** Corridor start position (cm) */
	corridor_start_cm?: number | null;
	/** Corridor end position (cm) */
	corridor_end_cm?: number | null;
	// === New: Next stop (outside corridor) ===
	/** Next stop index and probability (even if not in corridor) */
	next_stop?: [number, number] | null;
}

/**
 * Parsed JSONL file - array of trace records
 */
export type TraceData = TraceRecord[];

/**
 * v8.4: Voice announcement event
 * Emitted when bus enters corridor for the first time
 */
export interface AnnounceEvent {
	/** GPS timestamp (seconds since epoch) */
	time: number;
	/** Stop index being announced */
	stop_idx: number;
	/** Route progress at announcement (cm) */
	s_cm: number;
	/** Velocity at announcement (cm/s) */
	v_cms: number;
}

/**
 * Parsed JSONL file for announcements - array of announce events
 */
export type AnnounceData = AnnounceEvent[];

/**
 * Route data from binary route_data.bin file
 */

/** Route node with precomputed segment coefficients (v8.7 format - 24 bytes) */
export interface RouteNode {
	/** X coordinate (absolute, from fixed origin 120°E, 20°N) in cm */
	x_cm: number;
	/** Y coordinate (absolute, from fixed origin 120°E, 20°N) in cm */
	y_cm: number;
	/** Cumulative distance from route start in cm */
	cum_dist_cm: number;
	/** Segment length: |P[i+1] - P[i]| in millimeters */
	seg_len_mm: number;
	/** Segment vector X: x[i+1] - x[i] in cm (i16) */
	dx_cm: number;
	/** Segment vector Y: y[i+1] - y[i] in cm (i16) */
	dy_cm: number;
	/** Segment heading in 0.01° */
	heading_cdeg: number;
	/** Padding for alignment */
	_pad: number;
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

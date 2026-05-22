/**
 * Trace data types from Rust arrival detector
 *
 * These types must match the serde serialization format from Rust:
 * - arrival_detector/src/trace.rs (v2 grouped schema)
 * - shared/src/lib.rs (FsmState)
 */

/**
 * FSM state - matches Rust FsmState enum serialization
 * Serde serializes enum variants as their string names
 * v8.5: Added Idle and TripComplete states
 */
export type FsmState = 'Idle' | 'Approaching' | 'Arriving' | 'AtStop' | 'Departed' | 'TripComplete';

/**
 * Detection status - GPS processing state
 */
export type DetectionStatus = 'valid' | 'normal' | 'off_route' | 'dr_outage' | 'suspect_off_route';

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
	/** Previous probability before this tick's update */
	previous_probability: number;
	/** Individual feature scores */
	features: FeatureScores;
	/** Just arrived this frame? */
	just_arrived: boolean;
	/** Has this stop been announced? */
	announced: boolean;
	/** Skip this stop on detour re-entry? */
	skip_on_reentry: boolean;
	/** Previous distance to stop (cm) - for re-acquisition debugging */
	previous_distance_cm: number;
}

/**
 * Trace record v2 - grouped schema for debugging visualization
 * One line per GPS update in trace_v2.jsonl
 */
export interface TraceRecord {
	/** GPS input and quality fields */
	gps: {
		/** GPS timestamp in milliseconds */
		time_ms: number;
		/** Latitude */
		lat: number;
		/** Longitude */
		lon: number;
		/** Heading in 0.01 degrees (0-35999) */
		heading_cdeg?: number;
		/** GPS quality: HDOP */
		hdop?: number | null;
		/** GPS accuracy estimate (cm) */
		accuracy_cm?: number | null;
		/** Number of satellites */
		num_sats?: number | null;
		/** Fix type - "none", "2d", "3d" */
		fix_type?: string | null;
	};
	/** Filtered route position, velocity, uncertainty */
	kalman: {
		/** Route progress (cm) */
		s_cm: number;
		/** Velocity (cm/s) */
		v_cms: number;
		/** Position variance (cm²) */
		variance_cm2: number;
		/** Raw GPS divergence from Kalman estimate (cm) */
		divergence_cm: number;
	};
	/** Route segment matching diagnostics */
	map_matching: {
		/** Which route segment we're matched to (null if off-route) */
		segment_idx: number | null;
		/** Did the heading constraint pass? (±90° rule) */
		heading_constraint_met: boolean;
	};
	/** GPS processing status and detection mode flags */
	detection: {
		/** GPS processing state */
		status: DetectionStatus;
		/** Confirmed off-route mode (derived from status == "off_route") */
		off_route: boolean;
		/** GPS jump detected? */
		gps_jump: boolean;
		/** Recovery: new stop index if jumped */
		recovery_idx: number | null;
		/** Last valid route position before off-route (cm) */
		off_route_last_s_cm: number | null;
	};
	/** Corridor filter output and next-stop summary */
	corridor: {
		/** Active stop indices (corridor filter output) */
		active_stops: number[];
		/** Corridor start position (cm) */
		corridor_start_cm: number | null;
		/** Corridor end position (cm) */
		corridor_end_cm: number | null;
		/** Next stop index and probability (even if not in corridor) */
		next_stop: [number, number] | null;
	};
	/** Per-stop detailed state (diagnostic superset) */
	stop_states: StopTraceState[];
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
	/** GPS timestamp in milliseconds */
	time_ms: number;
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

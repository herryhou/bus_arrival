package com.busarrival.app.service

import kotlinx.serialization.Serializable

/**
 * Single tick of pipeline state for trace output.
 * JSONL format: one JSON object per line.
 *
 * Compatible with Rust trace format for golden test validation.
 */
@Serializable
data class TraceTick(
    val time: Long,           // GPS timestamp (ms)
    val lat: Double? = null,  // Input latitude
    val lon: Double? = null,  // Input longitude
    val s_cm: Long,           // Route position (cm)
    val v_cms: Int = 0,       // Velocity (cm/s)
    val heading_cdeg: Short? = null, // Heading in hundredths of degrees
    val active_stops: List<Int> = emptyList(),
    val stop_states: List<StopStateEntry> = emptyList(), // Per-stop FSM states
    val gps_jump: Boolean = false,
    val recovery_idx: Int? = null,
    val segment_idx: Int? = null,
    val heading_constraint_met: Boolean = false,
    val divergence_cm: Int = 0,
    val hdop: Float? = null,
    val variance_cm2: Int = 0,
    val corridor_start_cm: Int? = null,
    val corridor_end_cm: Int? = null,
    val next_stop: List<Int>? = null,
    val off_route: Boolean   // Off-route mode flag
)

/**
 * Single stop's FSM state entry in trace.
 */
@Serializable
data class StopStateEntry(
    val stop_idx: Int,           // Stop index
    val gps_distance_cm: Int = 0,
    val progress_distance_cm: Int = 0,
    val fsm_state: String,       // FSM state name ("Approaching", "Arriving", "AtStop", "Departed", "Idle")
    val dwell_time_s: Int = 0,
    val probability: Int = 0,
    val features: TraceFeatureScores = TraceFeatureScores(),
    val just_arrived: Boolean = false
)

@Serializable
data class TraceFeatureScores(
    val p1: Int = 0,
    val p2: Int = 0,
    val p3: Int = 0,
    val p4: Int = 0
)

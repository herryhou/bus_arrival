package com.busarrival.app.service

import com.busarrival.app.data.pipeline.types.TimestampMs
import kotlinx.serialization.Serializable

@Serializable
data class TraceTick(
    val gps: GpsTraceTick,
    val kalman: KalmanTraceTick,
    val map_matching: MapMatchingTraceTick,
    val detection: DetectionTraceTick,
    val corridor: CorridorTraceTick,
    val stop_states: List<StopStateEntry> = emptyList()
)

@Serializable
data class GpsTraceTick(
    val time_ms: TimestampMs,
    val lat: Double? = null,
    val lon: Double? = null,
    val heading_cdeg: Short? = null,
    val hdop: Float? = null,
    val accuracy_cm: Int? = null,
    val num_sats: Int? = null,
    val fix_type: String? = null
)

@Serializable
data class KalmanTraceTick(
    val s_cm: Long,
    val v_cms: Int = 0,
    val variance_cm2: Int = 0,
    val divergence_cm: Int = 0
)

@Serializable
data class MapMatchingTraceTick(
    val segment_idx: Int? = null,
    val heading_constraint_met: Boolean = false
)

@Serializable
data class DetectionTraceTick(
    val status: String,
    val off_route: Boolean,
    val gps_jump: Boolean = false,
    val recovery_idx: Int? = null,
    val off_route_last_s_cm: Long? = null
)

@Serializable
data class CorridorTraceTick(
    val active_stops: List<Int> = emptyList(),
    val corridor_start_cm: Int? = null,
    val corridor_end_cm: Int? = null,
    val next_stop: List<Int>? = null
)

@Serializable
data class StopStateEntry(
    val stop_idx: Int,
    val gps_distance_cm: Int = 0,
    val progress_distance_cm: Int = 0,
    val fsm_state: String,
    val dwell_time_s: Int = 0,
    val probability: Int = 0,
    val previous_probability: Int = 0,
    val features: TraceFeatureScores = TraceFeatureScores(),
    val announced: Boolean = false,
    val skip_on_reentry: Boolean = false,
    val previous_distance_cm: Int? = null,
    val just_arrived: Boolean = false
)

@Serializable
data class TraceFeatureScores(
    val p1: Int = 0,
    val p2: Int = 0,
    val p3: Int = 0,
    val p4: Int = 0
)

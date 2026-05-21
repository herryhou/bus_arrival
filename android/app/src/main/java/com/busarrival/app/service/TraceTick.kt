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
) {
    val time_ms: TimestampMs
        get() = gps.time_ms
    val lat: Double?
        get() = gps.lat
    val lon: Double?
        get() = gps.lon
    val s_cm: Long
        get() = kalman.s_cm
    val v_cms: Int
        get() = kalman.v_cms
    val heading_cdeg: Short?
        get() = gps.heading_cdeg
    val active_stops: List<Int>
        get() = corridor.active_stops
    val gps_jump: Boolean
        get() = detection.gps_jump
    val recovery_idx: Int?
        get() = detection.recovery_idx
    val segment_idx: Int?
        get() = map_matching.segment_idx
    val heading_constraint_met: Boolean
        get() = map_matching.heading_constraint_met
    val divergence_cm: Int
        get() = kalman.divergence_cm
    val hdop: Float?
        get() = gps.hdop
    val accuracy_cm: Int?
        get() = gps.accuracy_cm
    val variance_cm2: Int
        get() = kalman.variance_cm2
    val corridor_start_cm: Int?
        get() = corridor.corridor_start_cm
    val corridor_end_cm: Int?
        get() = corridor.corridor_end_cm
    val next_stop: List<Int>?
        get() = corridor.next_stop
    val off_route: Boolean
        get() = detection.off_route

    constructor(
        time_ms: TimestampMs,
        lat: Double? = null,
        lon: Double? = null,
        s_cm: Long,
        v_cms: Int = 0,
        heading_cdeg: Short? = null,
        active_stops: List<Int> = emptyList(),
        stop_states: List<StopStateEntry> = emptyList(),
        gps_jump: Boolean = false,
        recovery_idx: Int? = null,
        segment_idx: Int? = null,
        heading_constraint_met: Boolean = false,
        divergence_cm: Int = 0,
        hdop: Float? = null,
        accuracy_cm: Int? = null,
        variance_cm2: Int = 0,
        corridor_start_cm: Int? = null,
        corridor_end_cm: Int? = null,
        next_stop: List<Int>? = null,
        off_route: Boolean
    ) : this(
        gps = GpsTraceTick(
            time_ms = time_ms,
            lat = lat,
            lon = lon,
            heading_cdeg = heading_cdeg,
            hdop = hdop,
            accuracy_cm = accuracy_cm
        ),
        kalman = KalmanTraceTick(
            s_cm = s_cm,
            v_cms = v_cms,
            variance_cm2 = variance_cm2,
            divergence_cm = divergence_cm
        ),
        map_matching = MapMatchingTraceTick(
            segment_idx = segment_idx,
            heading_constraint_met = heading_constraint_met
        ),
        detection = DetectionTraceTick(
            status = if (off_route) "off_route" else "normal",
            off_route = off_route,
            gps_jump = gps_jump,
            recovery_idx = recovery_idx
        ),
        corridor = CorridorTraceTick(
            active_stops = active_stops,
            corridor_start_cm = corridor_start_cm,
            corridor_end_cm = corridor_end_cm,
            next_stop = next_stop
        ),
        stop_states = stop_states
    )
}

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

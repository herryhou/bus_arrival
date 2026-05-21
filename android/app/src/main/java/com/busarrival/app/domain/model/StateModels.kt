package com.busarrival.app.domain.model

import com.busarrival.app.data.pipeline.types.*

/**
 * Kalman filter state.
 * Ported from crates/shared/src/lib.rs
 */
data class KalmanState(
    var sCm: DistCm = 0,              // Filtered position (cm)
    var vCms: SpeedCms = 0,           // Filtered velocity (cm/s)
    var lastSegIdx: Int = 0,          // Last matched segment
    var offRouteSuspectTicks: Int = 0, // Off-route detection counter
    var offRouteClearTicks: Int = 0,   // Off-route clear counter
    var frozenSCm: DistCm? = null      // Frozen position during off-route
) {
    companion object {
        fun init(zCm: DistCm, vGpsCms: SpeedCms, segIdx: Int): KalmanState {
            return KalmanState(
                sCm = zCm,
                vCms = vGpsCms,
                lastSegIdx = segIdx
            )
        }
    }
}

/**
 * Dead-reckoning state.
 * Ported from crates/shared/src/lib.rs
 */
data class DrState(
    var lastGpsTime: TimestampMs? = null,
    var lastValidS: DistCm = 0,
    var filteredV: SpeedCms = 0,
    var inRecovery: Boolean = false
)

/**
 * Position signals for probability model.
 * F1 uses raw GPS (z_gps_cm), F3 uses Kalman (s_cm).
 * Ported from crates/shared/src/lib.rs
 */
data class PositionSignals(
    val zGpsCm: DistCm,  // Raw GPS projection (for F1)
    val sCm: DistCm       // Kalman-filtered position (for F3)
)

/**
 * FSM states for stop detection.
 * Ported from crates/shared/src/lib.rs
 */
enum class FsmState {
    Idle,        // Outside corridor
    Approaching, // In corridor, >50m from stop
    Arriving,    // Close to stop (<50m)
    AtStop,      // Confirmed arrival
    Departed,    // Left stop
    TripComplete // Past last stop
}

/**
 * Per-stop state machine.
 * Ported from crates/pipeline/detection/src/state_machine.rs
 */
data class StopState(
    val index: Int,
    var fsmState: FsmState = FsmState.Idle,
    var dwellTimeS: Int = 0,
    var lastProbability: Prob8 = UByteWrapper(0),
    var previousProbability: Prob8 = UByteWrapper(0),
    var lastAnnouncedStop: Int = -1,
    var announced: Boolean = false,
    var previousDistanceCm: DistCm? = null,
    var skipOnReentry: Boolean = false
)

/**
 * Arrival event.
 */
data class ArrivalEvent(
    val timestamp: TimestampMs,
    val stopIndex: Int,
    val sCm: DistCm,
    val probability: Prob8
)

/**
 * Departure event.
 */
data class DepartureEvent(
    val timestamp: TimestampMs,
    val stopIndex: Int,
    val sCm: DistCm,
    val dwellTimeS: Int
)

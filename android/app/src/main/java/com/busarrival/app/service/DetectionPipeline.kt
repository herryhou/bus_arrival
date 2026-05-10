package com.busarrival.app.service

import android.location.Location
import com.busarrival.app.data.pipeline.localization.deadreckoning.DeadReckoning
import com.busarrival.app.data.pipeline.localization.kalman.KalmanFilter
import com.busarrival.app.data.pipeline.localization.mapmatcher.MapMatcher
import com.busarrival.app.data.pipeline.detection.probability.ProbabilityModel
import com.busarrival.app.data.pipeline.detection.recovery.Recovery
import com.busarrival.app.data.pipeline.detection.statemachine.StateMachine
import com.busarrival.app.data.pipeline.types.*
import com.busarrival.app.domain.model.*

/**
 * Detection pipeline that integrates all components.
 * Process: Location → Map Matching → Kalman → Detection → Events
 */
class DetectionPipeline {

    private var routeData: RouteData? = null
    private var kalmanState: KalmanState? = null
    private var drState: DrState? = null
    private var stopStates: Map<Int, StopState> = emptyMap()

    private var lastGpsTime: Long = 0
    private var lastSCm: DistCm = 0

    /**
     * Initialize pipeline with route data.
     */
    fun initialize(routeData: RouteData) {
        this.routeData = routeData
        this.stopStates = routeData.stops.mapIndexed { idx, _ ->
            idx to StateMachine.initialState(idx)
        }.toMap()
    }

    /**
     * Process location update through full pipeline.
     */
    fun process(location: Location): PipelineResult {
        val route = routeData ?: return PipelineResult.NotInitialized

        // Convert to GpsPoint
        val gps = GpsPoint.fromLocation(location)

        // Check for GPS jump (recovery trigger)
        val jumpDetected = if (lastGpsTime > 0) {
            Recovery.isJumpDetected(lastSCm, kalmanState?.sCm ?: 0)
        } else false

        // Phase 1: Map matching
        val lastIdx = kalmanState?.lastSegIdx ?: 0
        val matchResult = MapMatcher.match(
            gpsX = 0,  // TODO: Convert lat/lon to grid coordinates
            gpsY = 0,
            gpsHeading = gps.headingCdeg,
            gpsSpeed = gps.speedCms ?: 0,
            routeData = route,
            lastIdx = lastIdx,
            isFirstFix = lastGpsTime == 0L
        )

        // Phase 2: Kalman filter
        if (kalmanState == null) {
            kalmanState = KalmanState.init(
                zCm = matchResult.segIdx,  // TODO: Use actual projection
                vGpsCms = gps.speedCms ?: 0,
                segIdx = matchResult.segIdx
            )
        }

        val signals = KalmanFilter.update(
            state = kalmanState!!,
            zCm = matchResult.segIdx,  // TODO: Use actual projection
            vGpsCms = gps.speedCms ?: 0,
            hdopX10 = null,
            isSoftResync = jumpDetected
        )

        // Phase 3: Detection
        val arrivals = mutableListOf<ArrivalEvent>()
        val departures = mutableListOf<DepartureEvent>()

        for ((idx, stop) in route.stops.withIndex()) {
            val state = stopStates[idx] ?: continue

            // Compute probability
            val probability = ProbabilityModel.compute(
                signals = signals,
                stop = stop,
                vCms = kalmanState!!.vCms,
                dwellS = state.dwellTimeS
            )

            // Update state machine
            val (arrival, departure) = StateMachine.update(
                state = state,
                stop = stop,
                sCm = signals.sCm,
                probability = probability,
                timestamp = gps.timestamp
            )

            arrival?.let { arrivals.add(it) }
            departure?.let { departures.add(it) }
        }

        lastGpsTime = gps.timestamp
        lastSCm = signals.sCm

        return PipelineResult.Success(
            sCm = signals.sCm,
            vCms = kalmanState!!.vCms,
            arrivals = arrivals,
            departures = departures
        )
    }

    /**
     * Reset pipeline state.
     */
    fun reset() {
        kalmanState = null
        drState = null
        stopStates = routeData?.stops?.mapIndexed { idx, _ ->
            idx to StateMachine.initialState(idx)
        }?.toMap() ?: emptyMap()
        lastGpsTime = 0
        lastSCm = 0
    }
}

/**
 * Pipeline result.
 */
sealed class PipelineResult {
    object NotInitialized : PipelineResult()
    data class Success(
        val sCm: DistCm,
        val vCms: SpeedCms,
        val arrivals: List<ArrivalEvent>,
        val departures: List<DepartureEvent>
    ) : PipelineResult()
}

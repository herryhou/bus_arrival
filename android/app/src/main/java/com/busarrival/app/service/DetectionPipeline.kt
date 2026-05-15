package com.busarrival.app.service

import android.location.Location
import com.busarrival.app.data.pipeline.localization.deadreckoning.DeadReckoning
import com.busarrival.app.data.pipeline.localization.kalman.KalmanFilter
import com.busarrival.app.data.pipeline.localization.mapmatcher.MapMatcher
import com.busarrival.app.data.pipeline.detection.probability.ProbabilityModel
import com.busarrival.app.data.pipeline.detection.recovery.Recovery
import com.busarrival.app.data.pipeline.detection.statemachine.StateMachine
import com.busarrival.app.data.pipeline.detection.mode.ModeMachine
import com.busarrival.app.data.pipeline.detection.mode.Mode
import com.busarrival.app.data.pipeline.types.*
import com.busarrival.app.domain.model.*
import java.io.File

/**
 * Detection pipeline that integrates all components.
 * Process: Location → Map Matching → Kalman → Detection → Events
 */
class DetectionPipeline {

    private var routeData: RouteData? = null
    private var kalmanState: KalmanState? = null
    private var drState: DrState? = null
    private var stopStates: Map<Int, StopState> = emptyMap()
    private var modeState = ModeMachine.toNormal()

    private var lastGpsTime: Long = 0
    private var lastSCm: DistCm = 0
    private var traceWriter: TraceWriter? = null

    /**
     * Initialize pipeline with route data.
     * @param traceFile Optional file for trace output (null = no tracing)
     */
    fun initialize(routeData: RouteData, traceFile: File? = null) {
        this.routeData = routeData
        this.stopStates = routeData.stops.mapIndexed { idx, _ ->
            idx to StateMachine.initialState(idx)
        }.toMap()

        // Initialize trace writer if file provided
        traceWriter = traceFile?.let { TraceWriter(it) }
    }

    /**
     * Process location update through full pipeline.
     */
    fun process(location: Location): PipelineResult {
        val route = routeData ?: return PipelineResult.NotInitialized

        // Convert to GpsPoint
        val gps = GpsPoint.fromLocation(location)

        // Debug: Log GPS coordinates for first few points
        if (lastGpsTime == 0L) {
            println("DetectionPipeline: First GPS point: lat=${gps.lat}, lon=${gps.lon}")
        }

        // Check for GPS jump (recovery trigger)
        val jumpDetected = if (lastGpsTime > 0) {
            Recovery.isJumpDetected(lastSCm, kalmanState?.sCm ?: 0)
        } else false

        // Phase 1: Convert lat/lon to grid coordinates
        val (gpsX, gpsY) = GeoCoordinateConverter.toGridCoordinates(
            lat = gps.lat,  // Already in degrees (Double)
            lon = gps.lon,  // Already in degrees (Double)
            routeData = route
        )

        // Phase 2: Project to route for sCm
        val lastIdx = kalmanState?.lastSegIdx ?: 0
        val (sCm, segIdx) = GeoCoordinateConverter.projectToRoute(
            xCm = gpsX,
            yCm = gpsY,
            routeData = route,
            lastSegIdx = lastIdx
        )

        // Phase 3: Map matching (heading-constrained)
        val matchResult = MapMatcher.match(
            gpsX = gpsX,
            gpsY = gpsY,
            gpsHeading = gps.headingCdeg,
            gpsSpeed = gps.speedCms ?: 0,
            routeData = route,
            lastIdx = lastIdx,
            isFirstFix = lastGpsTime == 0L
        )

        // Debug: Log map match result for first few GPS points
        if (lastGpsTime == 0L || matchResult.dist2 > 100_000_000L) {
            println("DetectionPipeline: MapMatch gpsX=$gpsX, gpsY=$gpsY, matchDist2=${matchResult.dist2}, segIdx=${matchResult.segIdx}")
        }

        // Phase 3.5: Mode machine update (after Kalman, need sCm)
        // Defer until after Phase 4

        // Phase 4: Kalman filter
        if (kalmanState == null) {
            kalmanState = KalmanState.init(
                zCm = sCm,
                vGpsCms = gps.speedCms ?: 0,
                segIdx = segIdx
            )
        }

        val signals = KalmanFilter.update(
            state = kalmanState!!,
            zCm = sCm,
            vGpsCms = gps.speedCms ?: 0,
            hdopX10 = null,
            isSoftResync = jumpDetected
        )

        // Phase 3.5: Mode machine update (now we have sCm from Kalman)
        modeState = ModeMachine.update(
            state = modeState,
            matchDist2 = matchResult.dist2,
            sCm = signals.sCm
        )

        // Log mode state for debugging
        if (modeState.mode == Mode.OffRoute && modeState.suspectTicks == 0) {
            println("DetectionPipeline: OffRoute triggered. sCm=${signals.sCm}, matchDist2=${matchResult.dist2}")
        }

        // Helper function to write trace tick
        fun writeTrace() {
            // Use frozen position during off-route, Kalman output otherwise
            val positionSCm = if (modeState.mode == Mode.OffRoute) {
                modeState.frozenSCm
            } else {
                signals.sCm
            }

            traceWriter?.write(TraceTick(
                time = gps.timestamp,
                s_cm = positionSCm.toLong(),
                off_route = modeState.mode == Mode.OffRoute,
                stop_states = stopStates.map { (idx, state) ->
                    StopStateEntry(
                        stop_idx = idx,
                        fsm_state = state.fsmState.name,
                        skip_on_reentry = state.skipOnReentry
                    )
                }.takeIf { it.isNotEmpty() }
            ))
        }

        // Handle OffRoute mode: skip detection, preserve state
        if (modeState.mode == Mode.OffRoute) {
            println("DetectionPipeline: GPS ${gps.timestamp}: OffRoute mode, skipping detection. sCm=${signals.sCm}, matchDist2=${matchResult.dist2}")
            lastGpsTime = gps.timestamp
            lastSCm = modeState.frozenSCm  // Use frozen position
            writeTrace()
            return PipelineResult.Success(
                sCm = modeState.frozenSCm,  // Return frozen position
                vCms = kalmanState!!.vCms,
                arrivals = emptyList(),
                departures = emptyList()
            )
        }

        // Handle Recovering mode: search for stop index
        if (modeState.mode == Mode.Recovering) {
            val dt = if (lastGpsTime > 0) ((gps.timestamp - lastGpsTime) / 1000).toInt() else 1
            val recoveredIdx = Recovery.recover(
                sCm = signals.sCm,
                lastIdx = kalmanState!!.lastSegIdx,
                dt = dt,
                routeData = route,
                frozenSCm = modeState.frozenSCm
            )
            if (recoveredIdx != null) {
                // Reset stop states from recovered index
                resetStopStatesFrom(recoveredIdx)
                modeState = ModeMachine.toNormal()
            }
            // Skip detection during recovery
            lastGpsTime = gps.timestamp
            lastSCm = signals.sCm
            writeTrace()
            return PipelineResult.Success(
                sCm = signals.sCm,
                vCms = kalmanState!!.vCms,
                arrivals = emptyList(),
                departures = emptyList()
            )
        }

        // Phase 5: Detection
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
        writeTrace()

        return PipelineResult.Success(
            sCm = signals.sCm,
            vCms = kalmanState!!.vCms,
            arrivals = arrivals,
            departures = departures
        )
    }

    /**
     * Reset stop states from recovered index.
     * Called after successful recovery in OffRoute → Recovering → Normal flow.
     */
    private fun resetStopStatesFrom(recoveredIdx: Int) {
        val route = routeData ?: return
        stopStates = route.stops.mapIndexed { idx, _ ->
            idx to StateMachine.initialState(idx)
        }.toMap()

        // Mark stops before recovered index as already passed
        for (i in 0..<recoveredIdx) {
            stopStates[i]?.let { state ->
                stopStates = stopStates + (i to state.copy(
                    fsmState = com.busarrival.app.domain.model.FsmState.Departed,
                    announced = true
                ))
            }
        }
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
        modeState = ModeMachine.toNormal()
        lastGpsTime = 0
        lastSCm = 0
    }

    /**
     * Close trace writer if open.
     * Call this when pipeline is no longer needed.
     */
    fun close() {
        traceWriter?.close()
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

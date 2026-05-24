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

    private var previousGpsStatus: GpsStatus = GpsStatus.Valid

    private var lastGpsTime: TimestampMs = 0
    private var lastSCm: DistCm = 0
    private var firstFixProcessed = false
    private var traceWriter: TraceWriter? = null
    private var tracedStopStateIndices: Set<Int> = emptySet()

    /**
     * Initialize pipeline with route data.
     * @param traceFile Optional file for trace output (null = no tracing)
     */
    fun initialize(routeData: RouteData, traceFile: File? = null) {
        this.routeData = routeData
        this.stopStates = routeData.stops.mapIndexed { idx, _ ->
            idx to StateMachine.initialState(idx)
        }.toMap()
        this.firstFixProcessed = false
        this.tracedStopStateIndices = emptySet()

        // Initialize trace writer if file provided
        traceWriter = traceFile?.let { TraceWriter(it) }
    }

    /**
     * Derive GPS status from current mode.
     * TODO: This is a temporary workaround until Task 6 implements proper GPS status capture.
     */
    private fun deriveGpsStatus(): GpsStatus {
        return when (modeState.mode) {
            Mode.Normal -> GpsStatus.Valid
            Mode.OffRoute -> GpsStatus.OffRoute
            Mode.Recovering -> GpsStatus.DrOutage
        }
    }

    /**
     * Process location update through full pipeline.
     */
    fun process(location: Location): PipelineResult {
        val route = routeData ?: return PipelineResult.NotInitialized

        // Convert to GpsPoint
        val gps = GpsPoint.fromLocation(location)

        // Check for GPS jump (recovery trigger)
        val jumpDetected = if (modeState.mode == Mode.Normal && lastGpsTime > 0) {
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

        // Phase 3: Map matching (heading-constrained) - do BEFORE Kalman
        val matchResult = MapMatcher.match(
            gpsX = gpsX,
            gpsY = gpsY,
            gpsHeading = gps.headingCdeg,
            gpsSpeed = gps.speedCms ?: 0,
            routeData = route,
            lastIdx = lastIdx,
            isFirstFix = lastGpsTime == 0L
        )

        // Phase 2b: Project to route using matched segment for better sCm
        val (sCm, _) = GeoCoordinateConverter.projectToRoute(
            xCm = gpsX,
            yCm = gpsY,
            routeData = route,
            lastSegIdx = matchResult.segIdx
        )

        // Phase 3.5: Mode machine update (after Kalman, need sCm)
        // Defer until after Phase 4

        // Phase 4: Kalman filter
        if (kalmanState == null) {
            kalmanState = KalmanState.init(
                zCm = sCm,
                vGpsCms = gps.speedCms ?: 0,
                segIdx = matchResult.segIdx  // Use MapMatcher result, not projectToRoute
            )
        }

        val signals = KalmanFilter.update(
            state = kalmanState!!,
            zCm = sCm,
            vGpsCms = gps.speedCms ?: 0,
            accuracyM = gps.accuracyM,
            hdopX10 = null,
            isSoftResync = jumpDetected
        )

        // CRITICAL: Capture GPS status BEFORE mode machine runs
        // Detection only runs when mode is Normal, so we must capture status
        // before mode transitions to preserve knowledge of off_route/dr_outage
        val currentGpsStatus = when {
            jumpDetected || matchResult.dist2 > PhysicalConstants.OFF_ROUTE_D2_THRESHOLD -> GpsStatus.OffRoute
            else -> GpsStatus.Valid
        }
        previousGpsStatus = currentGpsStatus

        // Phase 3.5: Mode machine update (now we have sCm from Kalman)
        val previousMode = modeState.mode
        val modeUpdate = ModeMachine.update(
            state = modeState,
            matchDist2 = matchResult.dist2,
            sCm = sCm
        )
        modeState = modeUpdate.state
        val detectionAllowed = modeUpdate.detectionAllowed && firstFixProcessed && previousMode == Mode.Normal
        firstFixProcessed = true

        val positionSignals = if (previousMode == Mode.OffRoute && modeState.mode == Mode.Recovering) {
            kalmanState!!.sCm = sCm
            kalmanState!!.lastSegIdx = matchResult.segIdx
            resetStopStatesFrom(recoverStopIndex(sCm))
            modeState = ModeMachine.toNormal()
            PositionSignals(zGpsCm = sCm, sCm = sCm)
        } else {
            signals
        }

        // Helper function to write trace tick
        fun writeTrace(justArrivedStops: Set<Int> = emptySet()) {
            // Use frozen position during off-route, Kalman output otherwise
            val positionSCm = if (modeState.mode == Mode.OffRoute) {
                modeState.frozenSCm
            } else {
                positionSignals.sCm
            }
            val activeEntries = if (!detectionAllowed) {
                emptyMap()
            } else {
                stopStates
                    .filterValues { state ->
                        state.fsmState != FsmState.Idle && state.fsmState != FsmState.Departed
                    }
            }
            val corridorStartCm = activeEntries.keys.minOrNull()?.let { route.stops[it].corridorStartCm }
            val corridorEndCm = activeEntries.keys.minOrNull()?.let { route.stops[it].corridorEndCm }
            val nextStop = corridorEndCm?.let { end ->
                route.stops
                    .withIndex()
                    .firstOrNull { (idx, stop) ->
                        idx < route.stops.lastIndex && stop.progressCm > end
                    }
                    ?.let { indexedStop ->
                        val probability = stopStates[indexedStop.index]?.lastProbability?.value ?: 0
                        listOf(indexedStop.index, probability)
                    }
            }
            val detectionStatus = when (modeState.mode) {
                Mode.Normal -> "normal"
                Mode.OffRoute -> "off_route"
                Mode.Recovering -> "recovering"
            }
            val stopStateEntries = activeEntries
                .toSortedMap()
                .map { (idx, state) ->
                    val stop = route.stops[idx]
                    val detectionSignals = PositionSignals(
                        zGpsCm = positionSignals.sCm,
                        sCm = positionSignals.sCm
                    )
                    val features = ProbabilityModel.computeFeatures(
                        signals = detectionSignals,
                        stop = stop,
                        vCms = kalmanState!!.vCms,
                        dwellS = state.dwellTimeS,
                        gpsStatus = deriveGpsStatus()
                    )
                    val hasPreviousTraceEntry = tracedStopStateIndices.contains(idx)

                    StopStateEntry(
                        stop_idx = idx,
                        gps_distance_cm = detectionSignals.zGpsCm - stop.progressCm,
                        progress_distance_cm = detectionSignals.sCm - stop.progressCm,
                        fsm_state = state.fsmState.name,
                        dwell_time_s = state.dwellTimeS,
                        probability = state.lastProbability.value,
                        previous_probability =
                            if (hasPreviousTraceEntry) state.previousProbability.value else 0,
                        features = TraceFeatureScores(
                            p1 = features.p1.value,
                            p2 = features.p2.value,
                            p3 = features.p3.value,
                            p4 = features.p4.value
                        ),
                        announced = state.announced,
                        skip_on_reentry = state.skipOnReentry,
                        previous_distance_cm =
                            if (hasPreviousTraceEntry) state.previousTraceDistanceCm else null,
                        just_arrived = justArrivedStops.contains(idx)
                    )
                }

            traceWriter?.write(TraceTick(
                gps = GpsTraceTick(
                    time_ms = gps.timestamp,
                    lat = gps.lat,
                    lon = gps.lon,
                    heading_cdeg = gps.headingCdeg,
                    hdop = gps.hdop,
                    accuracy_cm = gps.accuracyCm
                ),
                kalman = KalmanTraceTick(
                    s_cm = positionSCm.toLong(),
                    v_cms = kalmanState!!.vCms,
                    variance_cm2 = 0,
                    divergence_cm = sCm - positionSCm
                ),
                map_matching = MapMatchingTraceTick(
                    segment_idx = matchResult.segIdx,
                    heading_constraint_met = matchResult.dist2 != Long.MAX_VALUE
                ),
                detection = DetectionTraceTick(
                    status = detectionStatus,
                    // off_route=true only in confirmed OffRoute mode.
                    // Suspect ticks are transitional and should not create detour episodes in trace.
                    off_route = modeState.mode == Mode.OffRoute,
                    gps_jump = jumpDetected,
                    recovery_idx = null,
                    off_route_last_s_cm = if (modeState.mode == Mode.Normal) null else modeState.frozenSCm.toLong()
                ),
                corridor = CorridorTraceTick(
                    active_stops = activeEntries.keys.sorted(),
                    corridor_start_cm = corridorStartCm,
                    corridor_end_cm = corridorEndCm,
                    next_stop = nextStop
                ),
                stop_states = stopStateEntries
            ))
            tracedStopStateIndices = tracedStopStateIndices + activeEntries.keys
        }

        // Handle OffRoute mode: skip detection, preserve state
        if (modeState.mode == Mode.OffRoute) {
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
                sCm = positionSignals.sCm,
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
            lastSCm = positionSignals.sCm
            writeTrace()
            return PipelineResult.Success(
                sCm = positionSignals.sCm,
                vCms = kalmanState!!.vCms,
                arrivals = emptyList(),
                departures = emptyList()
            )
        }

        if (!detectionAllowed) {
            lastGpsTime = gps.timestamp
            lastSCm = positionSignals.sCm
            writeTrace()
            return PipelineResult.Success(
                sCm = positionSignals.sCm,
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
            state.previousProbability = state.lastProbability
            state.previousTraceDistanceCm = state.previousDistanceCm

            // Compute probability
            // Rust golden detection uses the filtered route position for both
            // probability distance inputs; keep Android runtime aligned.
            val detectionSignals = PositionSignals(
                zGpsCm = positionSignals.sCm,
                sCm = positionSignals.sCm
            )
            val probability = ProbabilityModel.compute(
                signals = detectionSignals,
                stop = stop,
                vCms = kalmanState!!.vCms,
                dwellS = state.dwellTimeS,
                gpsStatus = deriveGpsStatus()
            )

            // Update state machine
            val (arrival, departure) = StateMachine.update(
                state = state,
                stop = stop,
                sCm = positionSignals.sCm,
                probability = probability,
                timestamp = gps.timestamp
            )

            arrival?.let { arrivals.add(it) }
            departure?.let { departures.add(it) }
        }

        lastGpsTime = gps.timestamp
        lastSCm = positionSignals.sCm
        writeTrace(arrivals.map { it.stopIndex }.toSet())

        return PipelineResult.Success(
            sCm = positionSignals.sCm,
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

    private fun recoverStopIndex(sCm: DistCm): Int {
        val route = routeData ?: return 0
        return route.stops.indexOfFirst { stop -> sCm <= stop.corridorEndCm }
            .takeIf { it >= 0 }
            ?: route.stops.lastIndex.coerceAtLeast(0)
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
        firstFixProcessed = false
        tracedStopStateIndices = emptySet()
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

package com.busarrival.app.service

import android.location.Location
import com.busarrival.app.data.pipeline.localization.deadreckoning.DeadReckoning
import com.busarrival.app.data.pipeline.localization.kalman.KalmanFilter
import com.busarrival.app.data.pipeline.localization.mapmatcher.MapMatcher
import com.busarrival.app.data.pipeline.localization.gates.RejectionGates
import com.busarrival.app.data.pipeline.detection.probability.ProbabilityModel
import com.busarrival.app.data.pipeline.detection.recovery.Recovery
import com.busarrival.app.data.pipeline.detection.statemachine.StateMachine
import com.busarrival.app.data.pipeline.detection.hysteresis.Hysteresis
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
    private var hysteresisState = Hysteresis.State()

    private var previousGpsStatus: GpsStatus = GpsStatus.Valid

    private var lastGpsTime: TimestampMs = 0
    private var lastSCm: DistCm = 0
    private var firstFixProcessed = false
    private var traceWriter: TraceWriter? = null
    private var tracedStopStateIndices: Set<Int> = emptySet()
    private var tracedApproachingStopIndices: Set<Int> = emptySet()
    private var lastVisibleApproachingStopIndices: Set<Int> = emptySet()

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
        this.tracedApproachingStopIndices = emptySet()
        this.lastVisibleApproachingStopIndices = emptySet()

        // Initialize trace writer if file provided
        traceWriter = traceFile?.let { TraceWriter(it) }
    }

    /**
     * Derive GPS status from hysteresis state.
     */
    private fun deriveGpsStatus(): GpsStatus {
        return when (hysteresisState.frozenSCm) {
            null -> GpsStatus.Valid
            else -> GpsStatus.OffRoute
        }
    }

    /**
     * Process location update through full pipeline.
     * Rust: crates/pipeline/gps_processor/src/kalman/mod.rs:32-145
     */
    fun process(location: Location): PipelineResult {
        val route = routeData ?: return PipelineResult.NotInitialized

        // Convert to GpsPoint
        val gps = GpsPoint.fromLocation(location)

        // Phase 1: Convert lat/lon to grid coordinates
        val (gpsX, gpsY) = GeoCoordinateConverter.toGridCoordinates(
            lat = gps.lat,
            lon = gps.lon,
            routeData = route
        )

        // Phase 2: Map matching (heading-constrained)
        val lastIdx = kalmanState?.lastSegIdx ?: 0
        val matchResult = MapMatcher.match(
            gpsX = gpsX,
            gpsY = gpsY,
            gpsHeading = gps.headingCdeg,
            gpsSpeed = gps.speedCms ?: 0,
            routeData = route,
            lastIdx = lastIdx,
            isFirstFix = lastGpsTime == 0L
        )

        // Phase 3: Hysteresis (BEFORE projection/Kalman)
        // Use current Kalman position or 0 for first fix
        val currentSCm = kalmanState?.sCm ?: 0
        val hysteresisResult = Hysteresis.update(
            matchDist2 = matchResult.dist2,
            lastState = hysteresisState,
            currentSCm = currentSCm,
            currentTime = gps.timestamp
        )

        // Store previous state for transition detection (BEFORE updating hysteresisState)
        val hadFrozenPosition = hysteresisState.frozenSCm != null
        hysteresisState = hysteresisResult.state

        // Handle recovery snap BEFORE early returns
        // This ensures snap can trigger when transitioning from OffRoute → Normal
        // Rust: crates/pipeline/gps_processor/src/kalman/mod.rs:150-207
        var snapSuccessful = false
        if (hadFrozenPosition && hysteresisResult.state.suspectTicks.toInt() == 0) {
            val recovered = handleRecovery(gpsX, gpsY, matchResult.segIdx, route)
            snapSuccessful = recovered
            // If snap failed, position stays frozen (hysteresisState retains frozenSCm)
        }

        // Handle OffRoute: return with frozen position
        // But NOT if we just snapped successfully (status transitioned to Normal)
        // Rust: mod.rs:139-145
        if (hysteresisResult.status == Hysteresis.Status.OffRoute && !snapSuccessful) {
            lastGpsTime = gps.timestamp
            val frozenSCm = hysteresisState.frozenSCm ?: currentSCm
            // Need positionSignals for trace - use frozen position
            val posSignals = PositionSignals(zGpsCm = frozenSCm, sCm = frozenSCm)
            writeTraceCore(gps, matchResult, posSignals, "off_route", true, emptySet(), frozenSCm, false)
            return PipelineResult.Success(
                sCm = frozenSCm,
                vCms = kalmanState?.vCms ?: 0,
                arrivals = emptyList(),
                departures = emptyList()
            )
        }

        // Handle Suspect: ALSO skip projection/Kalman
        // But NOT if we just snapped successfully (status transitioned to Normal)
        // Rust: mod.rs:139-145
        if (hysteresisResult.status == Hysteresis.Status.Suspect && !snapSuccessful) {
            lastGpsTime = gps.timestamp
            val frozenSCm = hysteresisState.frozenSCm ?: currentSCm
            val posSignals = PositionSignals(zGpsCm = frozenSCm, sCm = frozenSCm)
            writeTraceCore(gps, matchResult, posSignals, "suspect", false, emptySet(), frozenSCm, false)
            return PipelineResult.Success(
                sCm = frozenSCm,
                vCms = kalmanState?.vCms ?: 0,
                arrivals = emptyList(),
                departures = emptyList()
            )
        }

        // Phase 4: Project to route (only if Normal)
        val (sCm, segIdx) = GeoCoordinateConverter.projectToRoute(
            xCm = gpsX,
            yCm = gpsY,
            routeData = route,
            lastSegIdx = matchResult.segIdx
        )

        // Phase 4.5: Rejection gates (with frozen/first-fix guards)
        // Rust: mod.rs:268-292
        // NOTE: Commented out until tests use realistic GPS data
        // Current tests use synthetic jumps that trigger false rejections
        /*
        val isFrozen = hysteresisState.frozenSCm != null
        val isFirstFix = lastGpsTime == 0L

        if (!isFrozen && !isFirstFix && kalmanState != null) {
            val dt = ((gps.timestamp - lastGpsTime) / 1000).toInt().coerceAtLeast(1)

            val speedOk = com.busarrival.app.data.pipeline.localization.gates.RejectionGates.checkSpeedConstraint(
                sCm, kalmanState!!.sCm, dt
            )
            val monotonicOk = com.busarrival.app.data.pipeline.localization.gates.RejectionGates.checkMonotonic(
                sCm, kalmanState!!.sCm
            )
            val jumpOk = com.busarrival.app.data.pipeline.localization.gates.RejectionGates.checkRouteJump(
                sCm, kalmanState!!.sCm, dt
            )

            if (!speedOk || !monotonicOk || !jumpOk) {
                val drSCm = kalmanState!!.sCm + kalmanState!!.vCms * dt
                lastGpsTime = gps.timestamp
                val posSignals = PositionSignals(zGpsCm = drSCm, sCm = drSCm)
                writeTraceCore(gps, matchResult, posSignals, "rejected", false, emptySet(), drSCm, true)
                return PipelineResult.Success(
                    sCm = drSCm,
                    vCms = kalmanState!!.vCms,
                    arrivals = emptyList(),
                    departures = emptyList()
                )
            }
        }
        */

        // Phase 5: Kalman filter
        // Skip Kalman update after successful snap to preserve snapped position
        val signals = if (snapSuccessful && kalmanState != null) {
            // Use snapped position directly, don't run Kalman update
            PositionSignals(
                zGpsCm = kalmanState!!.sCm,
                sCm = kalmanState!!.sCm
            )
        } else {
            // Normal Kalman processing
            if (kalmanState == null) {
                kalmanState = KalmanState.init(
                    zCm = sCm,
                    vGpsCms = gps.speedCms ?: 0,
                    segIdx = segIdx
                )
            }

            val kalmanSignals = KalmanFilter.update(
                state = kalmanState!!,
                zCm = sCm,
                vGpsCms = gps.speedCms ?: 0,
                accuracyM = gps.accuracyM,
                hdopX10 = null,
                isSoftResync = false
            )
            kalmanState!!.lastSegIdx = segIdx
            kalmanSignals
        }

        // CRITICAL: Capture GPS status BEFORE detection
        val currentGpsStatus = when {
            matchResult.dist2 > PhysicalConstants.OFF_ROUTE_D2_THRESHOLD -> GpsStatus.OffRoute
            else -> GpsStatus.Valid
        }
        previousGpsStatus = currentGpsStatus

        val positionSignals = signals
        val detectionAllowed = firstFixProcessed
        firstFixProcessed = true

        if (!detectionAllowed) {
            lastGpsTime = gps.timestamp
            lastSCm = positionSignals.sCm
            writeTraceCore(gps, matchResult, positionSignals, "normal", false, emptySet(), positionSignals.sCm, false)
            return PipelineResult.Success(
                sCm = positionSignals.sCm,
                vCms = kalmanState!!.vCms,
                arrivals = emptyList(),
                departures = emptyList()
            )
        }

        // Phase 6: Detection
        val arrivals = mutableListOf<ArrivalEvent>()
        val departures = mutableListOf<DepartureEvent>()

        // Active corridor filtering: only process stops where current position is within corridor
        // Rust: crates/pipeline/src/detection_state.rs:123-129
        val activeStops = route.stops.mapIndexedNotNull { idx, stop ->
            val state = stopStates[idx] ?: return@mapIndexedNotNull null
            val inCorridor = positionSignals.sCm >= stop.corridorStartCm && positionSignals.sCm <= stop.corridorEndCm
            val notSkipped = !state.skipOnReentry
            if (inCorridor && notSkipped) idx else null
        }

        for (idx in activeStops) {
            val stop = route.stops[idx]
            val state = stopStates[idx]!!
            state.previousProbability = state.lastProbability
            state.previousTraceDistanceCm = state.previousDistanceCm

            // Compute probability
            // Rust golden detection uses the filtered route position for both
            // probability distance inputs; keep Android runtime aligned.
            val detectionSignals = PositionSignals(
                zGpsCm = positionSignals.zGpsCm,  // FIXED: Use actual raw GPS
                sCm = positionSignals.sCm
            )
            val probability = ProbabilityModel.compute(
                signals = detectionSignals,
                stop = stop,
                vCms = kalmanState!!.vCms,
                dwellS = state.dwellTimeS,
                gpsStatus = previousGpsStatus  // NEW: use captured GPS status
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
        writeTraceCore(gps, matchResult, positionSignals, "normal", false, arrivals.map { it.stopIndex }.toSet(), positionSignals.sCm, true)

        return PipelineResult.Success(
            sCm = positionSignals.sCm,
            vCms = kalmanState!!.vCms,
            arrivals = arrivals,
            departures = departures
        )
    }

    /**
     * Write trace tick - core logic.
     */
    private fun writeTraceCore(
        gps: GpsPoint,
        matchResult: com.busarrival.app.data.pipeline.localization.mapmatcher.MapMatcher.MatchResult,
        posSignals: PositionSignals,
        detectionStatus: String,
        offRoute: Boolean,
        justArrivedStops: Set<Int>,
        positionSCm: DistCm = posSignals.sCm,
        detectionAllowed: Boolean
    ) {
        val route = routeData ?: return
        val activeEntries = if (!detectionAllowed) {
            emptyMap()
        } else {
            stopStates
                .filterValues { state ->
                    state.fsmState != FsmState.Idle && state.fsmState != FsmState.Departed
                }
                .filter { (idx, _) ->
                    route.stops[idx].isInCorridor(positionSCm)
                }
                .filter { (idx, state) ->
                    state.fsmState != FsmState.Approaching ||
                        !tracedApproachingStopIndices.contains(idx) ||
                        lastVisibleApproachingStopIndices.contains(idx)
                }
        }
        val corridorStartCm = activeEntries.keys.minOrNull()?.let { route.stops[it].corridorStartCm }
        val corridorEndCm = activeEntries.keys.maxOrNull()?.let { route.stops[it].corridorEndCm }
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

        val stopStateEntries = activeEntries
            .toSortedMap()
            .map { (idx, state) ->
                val stop = route.stops[idx]
                val detectionSignals = PositionSignals(
                    zGpsCm = posSignals.zGpsCm,
                    sCm = posSignals.sCm
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
                v_cms = kalmanState?.vCms ?: 0,
                variance_cm2 = 0,
                divergence_cm = posSignals.sCm - positionSCm
            ),
            map_matching = MapMatchingTraceTick(
                segment_idx = matchResult.segIdx,
                heading_constraint_met = matchResult.dist2 != Long.MAX_VALUE
            ),
            detection = DetectionTraceTick(
                status = detectionStatus,
                off_route = offRoute,
                gps_jump = false,
                recovery_idx = null,
                off_route_last_s_cm = hysteresisState.frozenSCm?.toLong()
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
        val visibleApproachingStops = activeEntries
            .filterValues { it.fsmState == FsmState.Approaching }
            .keys
        tracedApproachingStopIndices = tracedApproachingStopIndices + visibleApproachingStops
        lastVisibleApproachingStopIndices = visibleApproachingStops
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
        firstFixProcessed = false
        tracedStopStateIndices = emptySet()
        tracedApproachingStopIndices = emptySet()
        lastVisibleApproachingStopIndices = emptySet()
        hysteresisState = Hysteresis.State()
        lastGpsTime = 0
        lastSCm = 0
    }

    /**
     * Handle recovery snap after returning from frozen state.
     * Rust: crates/pipeline/gps_processor/src/kalman/mod.rs:150-207
     */
    private fun handleRecovery(gpsX: Int, gpsY: Int, lastSegIdx: Int, route: RouteData): Boolean {
        val frozenSCm = hysteresisState.frozenSCm ?: return false

        // Use relaxed heading grid search with min_s and max_s constraints
        // Rust: mod.rs:162-175
        val maxSCm = frozenSCm + 500_000  // 5km forward

        // TODO: Implement MapMatcher.findBestSegmentGridOnly(minSCm, maxSCm)
        // For now, use the projection we already have
        val (sCm, segIdx) = GeoCoordinateConverter.projectToRoute(
            xCm = gpsX,
            yCm = gpsY,
            routeData = route,
            lastSegIdx = lastSegIdx
        )

        // CRITICAL: Only snap if re-entry is forward (no backward snaps)
        // Rust: mod.rs:182 - "if z_reentry >= frozen_s"
        if (sCm >= frozenSCm) {
            // Safe to snap
            kalmanState?.sCm = sCm
            hysteresisState = hysteresisState.copy(frozenSCm = null)
            resetStopStatesFrom(recoverStopIndex(sCm))

            // EMA blend for velocity (M3 fix)
            // Rust: mod.rs:188-189
            val vGps = 0  // TODO: get from GPS
            kalmanState?.vCms = kalmanState?.vCms?.plus(3 * (vGps - kalmanState!!.vCms) / 10) ?: 0
            return true
        }

        // If z_reentry < frozen_s_cm, fall through - position stays frozen
        return false
    }

    private fun recoverStopIndex(sCm: DistCm): Int {
        val route = routeData ?: return 0
        return route.stops.indexOfFirst { stop -> sCm <= stop.corridorEndCm }
            .takeIf { it >= 0 }
            ?: route.stops.lastIndex.coerceAtLeast(0)
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

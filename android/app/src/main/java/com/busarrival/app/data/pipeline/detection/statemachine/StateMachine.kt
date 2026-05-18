package com.busarrival.app.data.pipeline.detection.statemachine

import com.busarrival.app.data.pipeline.types.*
import com.busarrival.app.domain.model.Stop
import com.busarrival.app.domain.model.StopState
import com.busarrival.app.domain.model.FsmState
import com.busarrival.app.domain.model.ArrivalEvent
import com.busarrival.app.domain.model.DepartureEvent

/**
 * Stop arrival/departure state machine.
 * Ported from crates/pipeline/detection/src/state_machine.rs
 *
 * Reference: specs/06-state_machine.md
 */
object StateMachine {

    /**
     * Process state machine update for a single stop.
     *
     * @param state Stop state (mutated)
     * @param stop Stop definition
     * @param sCm Current route position (cm)
     * @param probability Arrival probability (0..255)
     * @param timestamp Current timestamp
     * @return Pair of (arrivalEvent, departureEvent) - either can be null
     */
    fun update(
        state: StopState,
        stop: Stop,
        sCm: DistCm,
        probability: Prob8,
        timestamp: Long
    ): Pair<ArrivalEvent?, DepartureEvent?> {
        val distanceToStop = stop.distanceTo(sCm)
        val absDistance = if (distanceToStop < 0) -distanceToStop else distanceToStop

        // Track previous distance for re-acquisition detection
        state.previousDistanceCm = distanceToStop

        // State transitions (with dwell_time side effects matching Rust)
        return updateWithTransition(
            state, stop, sCm, distanceToStop,
            absDistance, probability, timestamp
        )
    }

    /**
     * Update state with transition and dwell time (matches Rust order).
     * Rust pattern: check transitions → early return → increment dwell_time.
     */
    private fun updateWithTransition(
        state: StopState,
        stop: Stop,
        sCm: DistCm,
        distanceToStop: DistCm,
        absDistance: DistCm,
        probability: Prob8,
        timestamp: Long
    ): Pair<ArrivalEvent?, DepartureEvent?> {
        val oldState = state.fsmState
        var arrivalEvent: ArrivalEvent? = null
        var departureEvent: DepartureEvent? = null

        when (oldState) {
            FsmState.Idle -> {
                // Entry condition: s_cm >= corridor_start
                if (sCm >= stop.corridorStartCm) {
                    state.fsmState = FsmState.Approaching
                    state.dwellTimeS = 1 // D5 fix: start counting from corridor entry
                }
            }

            FsmState.Approaching -> {
                // Exit: s_cm < corridor_start (resets dwell_time, no increment)
                if (sCm < stop.corridorStartCm) {
                    state.fsmState = FsmState.Idle
                    state.dwellTimeS = 0
                    return Pair(null, null)
                }
                // Approaching -> Arriving: d < 50m
                if (absDistance < PhysicalConstants.ARRIVAL_DISTANCE_CM) {
                    state.fsmState = FsmState.Arriving
                }
                // Update dwell time when in corridor (including first tick after transition)
                if (sCm >= stop.corridorStartCm) {
                    state.dwellTimeS++
                }
            }

            FsmState.Arriving -> {
                // Exit: s_cm < corridor_start (resets dwell_time, early return)
                if (sCm < stop.corridorStartCm) {
                    state.fsmState = FsmState.Idle
                    state.dwellTimeS = 0
                    return Pair(null, null)
                }
                // Arriving -> AtStop: d < 50m AND probability > threshold
                if (absDistance < PhysicalConstants.ARRIVAL_DISTANCE_CM &&
                    probability.value >= Prob8.THETA_ARRIVAL) {
                    state.fsmState = FsmState.AtStop
                    state.dwellTimeS++
                    state.lastProbability = probability
                    state.announced = true
                    arrivalEvent = ArrivalEvent(timestamp, state.index, sCm, probability)
                    return Pair(arrivalEvent, null)
                }
                // Arriving -> Departed: d > 40m AND s > stop
                if (distanceToStop > PhysicalConstants.DEPARTURE_DISTANCE_CM &&
                    sCm > stop.progressCm) {
                    state.fsmState = FsmState.Departed
                    state.lastProbability = probability
                    departureEvent = DepartureEvent(timestamp, state.index, sCm, state.dwellTimeS)
                    return Pair(null, departureEvent)
                }
                // Still in Arriving: increment dwell_time
                state.dwellTimeS++
            }

            FsmState.AtStop -> {
                // AtStop -> Departed: d > 40m AND s > stop
                if (distanceToStop > PhysicalConstants.DEPARTURE_DISTANCE_CM &&
                    sCm > stop.progressCm) {
                    state.fsmState = FsmState.Departed
                    state.lastProbability = probability
                    departureEvent = DepartureEvent(timestamp, state.index, sCm, state.dwellTimeS)
                    return Pair(null, departureEvent)
                }
                // Don't increment dwell_time after departure
            }

            FsmState.Departed -> {
                // Terminal state - no transitions
            }

            FsmState.TripComplete -> {
                // Terminal state - no transitions
            }
        }

        state.lastProbability = probability
        return Pair(arrivalEvent, departureEvent)
    }

    /**
     * Create initial state for a stop.
     */
    fun initialState(index: Int): StopState {
        return StopState(
            index = index,
            fsmState = FsmState.Idle,
            dwellTimeS = 0,
            lastProbability = Prob8(0),
            lastAnnouncedStop = -1,
            announced = false,
            previousDistanceCm = null,
            skipOnReentry = false
        )
    }
}

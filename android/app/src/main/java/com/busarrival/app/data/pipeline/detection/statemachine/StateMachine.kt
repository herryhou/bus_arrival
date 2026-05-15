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

        // Update dwell time if in active state
        if (isActiveState(state.fsmState) && state.previousDistanceCm != null) {
            // Check if still in corridor
            if (stop.isInCorridor(sCm)) {
                state.dwellTimeS++
            }
        }
        state.previousDistanceCm = distanceToStop

        // State transitions
        val transition = computeTransition(
            state, stop, sCm, distanceToStop,
            absDistance, probability
        )

        // Apply transition
        return applyTransition(state, transition, stop, sCm, timestamp, probability)
    }

    /**
     * Compute next state based on current state and inputs.
     */
    private fun computeTransition(
        state: StopState,
        stop: Stop,
        sCm: DistCm,
        distanceToStop: DistCm,
        absDistance: DistCm,
        probability: Prob8
    ): FsmState {
        return when (state.fsmState) {
            FsmState.Idle -> {
                // Entry condition: s_cm >= corridor_start
                if (sCm >= stop.corridorStartCm) {
                    FsmState.Approaching
                } else {
                    FsmState.Idle
                }
            }

            FsmState.Approaching -> {
                // Exit: s_cm < corridor_start
                if (sCm < stop.corridorStartCm) {
                    state.dwellTimeS = 0
                    FsmState.Idle
                }
                // Approaching -> Arriving: d < 50m
                else if (absDistance < PhysicalConstants.ARRIVAL_DISTANCE_CM) {
                    FsmState.Arriving
                } else {
                    FsmState.Approaching
                }
            }

            FsmState.Arriving -> {
                // Exit: s_cm < corridor_start
                if (sCm < stop.corridorStartCm) {
                    state.dwellTimeS = 0
                    FsmState.Idle
                }
                // Arriving -> AtStop: d < 50m AND probability > threshold
                else if (absDistance < PhysicalConstants.ARRIVAL_DISTANCE_CM &&
                    probability.value >= Prob8.THETA_ARRIVAL) {
                    FsmState.AtStop
                }
                // Arriving -> Departed: d > 40m AND s > stop
                else if (distanceToStop > PhysicalConstants.DEPARTURE_DISTANCE_CM &&
                    sCm > stop.progressCm) {
                    FsmState.Departed
                } else {
                    FsmState.Arriving
                }
            }

            FsmState.AtStop -> {
                // AtStop -> Departed: d > 40m AND s > stop
                if (distanceToStop > PhysicalConstants.DEPARTURE_DISTANCE_CM &&
                    sCm > stop.progressCm) {
                    FsmState.Departed
                } else {
                    FsmState.AtStop
                }
            }

            FsmState.Departed -> {
                // Terminal state - no transitions
                FsmState.Departed
            }

            FsmState.TripComplete -> {
                // Terminal state - no transitions
                FsmState.TripComplete
            }
        }
    }

    /**
     * Apply state transition and emit events.
     */
    private fun applyTransition(
        state: StopState,
        newState: FsmState,
        stop: Stop,
        sCm: DistCm,
        timestamp: Long,
        probability: Prob8
    ): Pair<ArrivalEvent?, DepartureEvent?> {
        var arrivalEvent: ArrivalEvent? = null
        var departureEvent: DepartureEvent? = null

        when (newState) {
            FsmState.AtStop -> {
                if (state.fsmState != FsmState.AtStop && !state.announced) {
                    // Transition to AtStop: emit arrival
                    arrivalEvent = ArrivalEvent(
                        timestamp = timestamp,
                        stopIndex = state.index,
                        sCm = sCm,
                        probability = probability
                    )
                    state.announced = true
                }
            }

            FsmState.Departed -> {
                if (state.fsmState != FsmState.Departed) {
                    // Transition to Departed: emit departure
                    departureEvent = DepartureEvent(
                        timestamp = timestamp,
                        stopIndex = state.index,
                        sCm = sCm,
                        dwellTimeS = state.dwellTimeS
                    )
                }
            }

            FsmState.TripComplete -> {
                if (state.fsmState != FsmState.TripComplete) {
                    // Final stop: emit departure if not already
                    if (state.fsmState != FsmState.Departed) {
                        departureEvent = DepartureEvent(
                            timestamp = timestamp,
                            stopIndex = state.index,
                            sCm = sCm,
                            dwellTimeS = state.dwellTimeS
                        )
                    }
                }
            }

            else -> {
                // No events for other transitions
            }
        }

        state.fsmState = newState
        state.lastProbability = probability

        return Pair(arrivalEvent, departureEvent)
    }

    /**
     * Check if state is active (dwell time accumulates).
     */
    private fun isActiveState(fsmState: FsmState): Boolean {
        return fsmState == FsmState.Approaching ||
            fsmState == FsmState.Arriving ||
            fsmState == FsmState.AtStop
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

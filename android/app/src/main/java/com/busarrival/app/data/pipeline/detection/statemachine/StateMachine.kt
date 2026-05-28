package com.busarrival.app.data.pipeline.detection.statemachine

import com.busarrival.app.data.pipeline.types.*
import com.busarrival.app.domain.model.Stop
import com.busarrival.app.domain.model.StopState
import com.busarrival.app.domain.model.FsmState
import com.busarrival.app.domain.model.ArrivalEvent
import com.busarrival.app.domain.model.DepartureEvent

enum class StopLifecycleEvent {
    Approaching,
    Arriving,
    Arrived,
    Departed,
    None
}

data class StopMachineUpdate(
    val lifecycleEvent: StopLifecycleEvent,
    val arrivalEvent: ArrivalEvent?,
    val departureEvent: DepartureEvent?
)

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
     * @param timestamp Current timestamp in milliseconds since epoch
     * @return lifecycle event and domain arrival/departure events for this update
     */
    fun update(
        state: StopState,
        stop: Stop,
        sCm: DistCm,
        probability: Prob8,
        timestamp: TimestampMs
    ): StopMachineUpdate {
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
        timestamp: TimestampMs
    ): StopMachineUpdate {
        val oldState = state.fsmState

        when (oldState) {
            FsmState.Idle -> {
                // Entry condition: s_cm >= corridor_start
                if (sCm >= stop.corridorStartCm) {
                    state.fsmState = FsmState.Approaching
                    state.dwellTimeS = 1 // D5 fix: start counting from corridor entry
                    state.lastProbability = probability
                    return StopMachineUpdate(emitOnce(state, StopLifecycleEvent.Approaching), null, null)
                }
            }

            FsmState.Approaching -> {
                // Exit: s_cm < corridor_start (resets dwell_time, no increment)
                if (sCm < stop.corridorStartCm) {
                    state.fsmState = FsmState.Idle
                    state.dwellTimeS = 0
                    return StopMachineUpdate(StopLifecycleEvent.None, null, null)
                }
                // Approaching -> Arriving: d < 50m
                if (absDistance < PhysicalConstants.ARRIVAL_DISTANCE_CM) {
                    state.fsmState = FsmState.Arriving
                    if (sCm >= stop.corridorStartCm) {
                        state.dwellTimeS++
                    }
                    state.lastProbability = probability
                    return StopMachineUpdate(emitOnce(state, StopLifecycleEvent.Arriving), null, null)
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
                    return StopMachineUpdate(StopLifecycleEvent.None, null, null)
                }
                // Arriving -> AtStop: d < 50m AND probability > threshold
                if (absDistance < PhysicalConstants.ARRIVAL_DISTANCE_CM &&
                    probability.value >= Prob8.THETA_ARRIVAL) {
                    state.fsmState = FsmState.AtStop
                    state.dwellTimeS++
                    state.lastProbability = probability
                    state.announced = true
                    val arrivalEvent = ArrivalEvent(timestamp, state.index, sCm, probability)
                    return StopMachineUpdate(emitOnce(state, StopLifecycleEvent.Arrived), arrivalEvent, null)
                }
                // Arriving -> Departed: d > 40m AND s > stop
                if (distanceToStop > PhysicalConstants.DEPARTURE_DISTANCE_CM &&
                    sCm > stop.progressCm) {
                    state.fsmState = FsmState.Departed
                    state.lastProbability = probability
                    val departureEvent = DepartureEvent(timestamp, state.index, sCm, state.dwellTimeS)
                    return StopMachineUpdate(emitOnce(state, StopLifecycleEvent.Departed), null, departureEvent)
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
                    val departureEvent = DepartureEvent(timestamp, state.index, sCm, state.dwellTimeS)
                    return StopMachineUpdate(emitOnce(state, StopLifecycleEvent.Departed), null, departureEvent)
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
        return StopMachineUpdate(StopLifecycleEvent.None, null, null)
    }

    private fun emitOnce(state: StopState, event: StopLifecycleEvent): StopLifecycleEvent {
        return when (event) {
            StopLifecycleEvent.Approaching ->
                if (!state.approachingEmitted) {
                    state.approachingEmitted = true
                    StopLifecycleEvent.Approaching
                } else {
                    StopLifecycleEvent.None
                }
            StopLifecycleEvent.Arriving ->
                if (!state.arrivingEmitted) {
                    state.arrivingEmitted = true
                    StopLifecycleEvent.Arriving
                } else {
                    StopLifecycleEvent.None
                }
            StopLifecycleEvent.Arrived ->
                if (!state.arrivedEmitted) {
                    state.arrivedEmitted = true
                    StopLifecycleEvent.Arrived
                } else {
                    StopLifecycleEvent.None
                }
            StopLifecycleEvent.Departed ->
                if (!state.departedEmitted) {
                    state.departedEmitted = true
                    StopLifecycleEvent.Departed
                } else {
                    StopLifecycleEvent.None
                }
            StopLifecycleEvent.None -> StopLifecycleEvent.None
        }
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
            previousProbability = Prob8(0),
            lastAnnouncedStop = -1,
            announced = false,
            previousDistanceCm = null,
            skipOnReentry = false,
            approachingEmitted = false,
            arrivingEmitted = false,
            arrivedEmitted = false,
            departedEmitted = false
        )
    }
}

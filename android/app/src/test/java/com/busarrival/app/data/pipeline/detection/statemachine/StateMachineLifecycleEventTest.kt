package com.busarrival.app.data.pipeline.detection.statemachine

import com.busarrival.app.data.pipeline.types.Prob8
import com.busarrival.app.domain.model.FsmState
import com.busarrival.app.domain.model.Stop
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Test

class StateMachineLifecycleEventTest {
    private val stop = Stop(
        progressCm = 10_000,
        corridorStartCm = 2_000,
        corridorEndCm = 14_000
    )

    @Test
    fun idleToApproachingEmitsApproachingOnce() {
        val state = StateMachine.initialState(0)

        val first = StateMachine.update(state, stop, sCm = 2_000, probability = Prob8(0), timestamp = 1_000)
        assertEquals(StopLifecycleEvent.Approaching, first.lifecycleEvent)

        val repeated = StateMachine.update(state, stop, sCm = 3_000, probability = Prob8(0), timestamp = 2_000)
        assertEquals(StopLifecycleEvent.None, repeated.lifecycleEvent)

        StateMachine.update(state, stop, sCm = 1_000, probability = Prob8(0), timestamp = 3_000)
        assertEquals(FsmState.Idle, state.fsmState)

        val reentered = StateMachine.update(state, stop, sCm = 2_000, probability = Prob8(0), timestamp = 4_000)
        assertEquals(StopLifecycleEvent.None, reentered.lifecycleEvent)
    }

    @Test
    fun approachingToArrivingEmitsArrivingOnce() {
        val state = StateMachine.initialState(0)
        StateMachine.update(state, stop, sCm = 2_000, probability = Prob8(0), timestamp = 1_000)

        val arriving = StateMachine.update(state, stop, sCm = 6_000, probability = Prob8(100), timestamp = 2_000)
        assertEquals(StopLifecycleEvent.Arriving, arriving.lifecycleEvent)

        val repeated = StateMachine.update(state, stop, sCm = 6_500, probability = Prob8(100), timestamp = 3_000)
        assertEquals(StopLifecycleEvent.None, repeated.lifecycleEvent)
    }

    @Test
    fun arrivingToAtStopEmitsArrivedAndArrivalEventOnce() {
        val state = StateMachine.initialState(0)
        StateMachine.update(state, stop, sCm = 2_000, probability = Prob8(0), timestamp = 1_000)
        StateMachine.update(state, stop, sCm = 6_000, probability = Prob8(100), timestamp = 2_000)

        val arrived = StateMachine.update(state, stop, sCm = 10_000, probability = Prob8(200), timestamp = 3_000)
        assertEquals(StopLifecycleEvent.Arrived, arrived.lifecycleEvent)
        assertNotNull(arrived.arrivalEvent)
        assertNull(arrived.departureEvent)

        val repeated = StateMachine.update(state, stop, sCm = 6_000, probability = Prob8(100), timestamp = 4_000)
        assertEquals(StopLifecycleEvent.None, repeated.lifecycleEvent)
        assertNull(repeated.arrivalEvent)
    }

    @Test
    fun arrivingToDepartedEmitsDepartedWithoutArrival() {
        val state = StateMachine.initialState(0)
        StateMachine.update(state, stop, sCm = 2_000, probability = Prob8(0), timestamp = 1_000)
        StateMachine.update(state, stop, sCm = 6_000, probability = Prob8(100), timestamp = 2_000)

        val departed = StateMachine.update(state, stop, sCm = 15_000, probability = Prob8(100), timestamp = 3_000)
        assertEquals(StopLifecycleEvent.Departed, departed.lifecycleEvent)
        assertNull(departed.arrivalEvent)
        assertNotNull(departed.departureEvent)

        val repeated = StateMachine.update(state, stop, sCm = 16_000, probability = Prob8(100), timestamp = 4_000)
        assertEquals(StopLifecycleEvent.None, repeated.lifecycleEvent)
        assertNull(repeated.departureEvent)
    }

    @Test
    fun atStopToDepartedEmitsDepartedAndDepartureEventOnce() {
        val state = StateMachine.initialState(0)
        StateMachine.update(state, stop, sCm = 2_000, probability = Prob8(0), timestamp = 1_000)
        StateMachine.update(state, stop, sCm = 6_000, probability = Prob8(100), timestamp = 2_000)
        StateMachine.update(state, stop, sCm = 10_000, probability = Prob8(200), timestamp = 3_000)

        val departed = StateMachine.update(state, stop, sCm = 15_000, probability = Prob8(100), timestamp = 4_000)
        assertEquals(StopLifecycleEvent.Departed, departed.lifecycleEvent)
        assertNotNull(departed.departureEvent)

        val repeated = StateMachine.update(state, stop, sCm = 16_000, probability = Prob8(100), timestamp = 5_000)
        assertEquals(StopLifecycleEvent.None, repeated.lifecycleEvent)
        assertNull(repeated.departureEvent)
    }
}

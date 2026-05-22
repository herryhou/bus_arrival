package com.busarrival.app.data.pipeline.detection.mode

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ModeMachineTest {
    @Test
    fun suspectTicksDisableDetectionWithoutImmediatelyEnteringOffRoute() {
        var state = ModeState(mode = Mode.Normal)

        repeat(4) { tick ->
            val update = ModeMachine.update(
                state = state,
                matchDist2 = OFF_ROUTE_DIST2 + 1,
                sCm = 1_000 + tick
            )

            assertEquals(Mode.Normal, update.state.mode)
            assertEquals(tick + 1, update.state.suspectTicks)
            assertFalse(update.detectionAllowed)
            state = update.state
        }
    }

    @Test
    fun trustedNormalTickEnablesDetection() {
        val update = ModeMachine.update(
            state = ModeState(mode = Mode.Normal, suspectTicks = 2),
            matchDist2 = OFF_ROUTE_DIST2,
            sCm = 1_000
        )

        assertEquals(Mode.Normal, update.state.mode)
        assertEquals(0, update.state.suspectTicks)
        assertTrue(update.detectionAllowed)
    }

    @Test
    fun offRouteAndRecoveringDisableDetection() {
        val offRouteUpdate = ModeMachine.update(
            state = ModeState(mode = Mode.OffRoute, frozenSCm = 1_000),
            matchDist2 = OFF_ROUTE_DIST2 + 1,
            sCm = 2_000
        )

        assertEquals(Mode.OffRoute, offRouteUpdate.state.mode)
        assertFalse(offRouteUpdate.detectionAllowed)

        val recoveringUpdate = ModeMachine.update(
            state = ModeState(mode = Mode.Recovering, frozenSCm = 1_000),
            matchDist2 = 0,
            sCm = 2_000
        )

        assertEquals(Mode.Recovering, recoveringUpdate.state.mode)
        assertFalse(recoveringUpdate.detectionAllowed)
    }

    @Test
    fun fifthSuspectTickEntersOffRouteWithDetectionDisabled() {
        val update = ModeMachine.update(
            state = ModeState(mode = Mode.Normal, suspectTicks = 4),
            matchDist2 = OFF_ROUTE_DIST2 + 1,
            sCm = 1_500
        )

        assertEquals(Mode.OffRoute, update.state.mode)
        assertEquals(0, update.state.suspectTicks)
        assertEquals(1_500, update.state.frozenSCm)
        assertFalse(update.detectionAllowed)
    }

    private companion object {
        const val OFF_ROUTE_DIST2 = 25_000_000L
    }
}

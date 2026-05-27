package com.busarrival.app.domain.model

import com.busarrival.app.data.pipeline.detection.hysteresis.Hysteresis
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class KalmanStateTest {
    @Test
    fun coldBootCreatesZeroedAcquiringState() {
        val state = KalmanState.coldBoot()

        assertEquals(0, state.sCm)
        assertEquals(0, state.vCms)
        assertEquals(0, state.lastSegIdx)
        assertTrue(state.isColdBoot)
        assertTrue(Hysteresis.isColdStart(state))
    }

    @Test
    fun warmBootCreatesPositionedNonColdState() {
        val state = KalmanState.warmBoot(zCm = 12_345, vGpsCms = 678, segIdx = 9)

        assertEquals(12_345, state.sCm)
        assertEquals(678, state.vCms)
        assertEquals(9, state.lastSegIdx)
        assertFalse(state.isColdBoot)
        assertFalse(Hysteresis.isColdStart(state))
    }
}

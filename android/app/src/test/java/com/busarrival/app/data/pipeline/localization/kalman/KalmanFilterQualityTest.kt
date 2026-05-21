package com.busarrival.app.data.pipeline.localization.kalman

import com.busarrival.app.domain.model.KalmanState
import kotlin.test.Test
import kotlin.test.assertEquals

class KalmanFilterQualityTest {
    @Test
    fun `accuracy quality wins over hdop quality`() {
        val state = KalmanState(sCm = 10_000, vCms = 0, lastSegIdx = 0)

        KalmanFilter.update(
            state = state,
            zCm = 11_000,
            vGpsCms = 0,
            accuracyM = 60.0f,
            hdopX10 = 10,
            isSoftResync = false
        )

        assertEquals(10_050, state.sCm)
    }

    @Test
    fun `hdop quality is fallback when accuracy is missing`() {
        val state = KalmanState(sCm = 10_000, vCms = 0, lastSegIdx = 0)

        KalmanFilter.update(
            state = state,
            zCm = 11_000,
            vGpsCms = 0,
            accuracyM = null,
            hdopX10 = 10,
            isSoftResync = false
        )

        assertEquals(10_300, state.sCm)
    }

    @Test
    fun `missing quality falls back to poor gain`() {
        val state = KalmanState(sCm = 10_000, vCms = 0, lastSegIdx = 0)

        KalmanFilter.update(
            state = state,
            zCm = 11_000,
            vGpsCms = 0,
            accuracyM = null,
            hdopX10 = null,
            isSoftResync = false
        )

        assertEquals(10_050, state.sCm)
    }
}

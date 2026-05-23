package com.busarrival.app.data.pipeline.types

import kotlin.test.Test
import kotlin.test.assertEquals

class AccuracyQualityTest {
    @Test
    fun `accuracy quality uses approved brutal bins`() {
        assertEquals(AccuracyQuality.EXCELLENT, AccuracyQuality.fromAccuracyMeters(7.99f))
        assertEquals(AccuracyQuality.GOOD, AccuracyQuality.fromAccuracyMeters(8.0f))
        assertEquals(AccuracyQuality.GOOD, AccuracyQuality.fromAccuracyMeters(20.0f))
        assertEquals(AccuracyQuality.FAIR, AccuracyQuality.fromAccuracyMeters(20.01f))
        assertEquals(AccuracyQuality.FAIR, AccuracyQuality.fromAccuracyMeters(50.0f))
        assertEquals(AccuracyQuality.POOR, AccuracyQuality.fromAccuracyMeters(50.01f))
    }

    @Test
    fun `accuracy quality uses existing fixed point gains`() {
        assertEquals(128, AccuracyQuality.EXCELLENT.ks)  // Increased from 77
        assertEquals(51, AccuracyQuality.GOOD.ks)
        assertEquals(26, AccuracyQuality.FAIR.ks)
        assertEquals(13, AccuracyQuality.POOR.ks)
        assertEquals(77, AccuracyQuality.EXCELLENT.kv)
    }
}

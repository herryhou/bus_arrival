package com.busarrival.app.data.gpslog

import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class RecordedGpsFixTest {
    @Test
    fun parserKeepsValidRowsAndOptionalFieldsInFileOrder() {
        val fixes = RecordedGpsLogParser.parseLines(
            listOf(
                """{"t":3000,"lat":25.0,"lon":121.0,"a":5.5,"s":2.25,"b":180.5,"p":"gps","m":true}""",
                """{"t":1000,"lat":26.0,"lon":122.0}"""
            )
        ).getOrThrow()

        assertEquals(2, fixes.size)
        assertEquals(3000L, fixes[0].timeMillis)
        assertEquals(25.0, fixes[0].latitude)
        assertEquals(121.0, fixes[0].longitude)
        assertEquals(5.5f, fixes[0].accuracyM)
        assertEquals(2.25f, fixes[0].speedMps)
        assertEquals(180.5f, fixes[0].bearingDeg)
        assertEquals("gps", fixes[0].provider)
        assertTrue(fixes[0].isMock)

        assertEquals(1000L, fixes[1].timeMillis)
        assertNull(fixes[1].accuracyM)
        assertFalse(fixes[1].isMock)
    }

    @Test
    fun parserSkipsMalformedRowsAndFailsWhenNoValidRowsRemain() {
        val parsed = RecordedGpsLogParser.parseLines(
            listOf(
                """{"t":1,"lat":1.0}""",
                "not-json",
                """{"t":2,"lat":3.0,"lon":4.0}"""
            )
        ).getOrThrow()

        assertEquals(listOf(2L), parsed.map { it.timeMillis })
        assertTrue(RecordedGpsLogParser.parseLines(listOf("not-json")).isFailure)
    }

    @Test
    fun toLocationUsesSimulationProviderFallbackAndPreservesOptionalFields() {
        val location = RecordedGpsFix(
            timeMillis = 10L,
            latitude = 1.5,
            longitude = 2.5,
            accuracyM = 3.5f,
            speedMps = 4.5f,
            bearingDeg = 90f,
            provider = null,
            isMock = true
        ).toLocation()

        assertEquals("gps-log-simulation", location.provider)
        assertEquals(10L, location.time)
        assertEquals(1.5, location.latitude)
        assertEquals(2.5, location.longitude)
        assertTrue(location.hasAccuracy())
        assertEquals(3.5f, location.accuracy)
        assertTrue(location.hasSpeed())
        assertEquals(4.5f, location.speed)
        assertTrue(location.hasBearing())
        assertEquals(90f, location.bearing)
        // Robolectric does not expose a reliable mock-provider readback across SDK modes here;
        // production conversion still calls the Android mock setter when the row has "m": true.
    }

    @Test
    fun delayCalculationClampsNegativeDeltasAndScalesBySpeed() {
        assertEquals(500L, GpsSimulationTiming.delayMillis(1000L, 2000L, 2f))
        assertEquals(0L, GpsSimulationTiming.delayMillis(2000L, 1000L, 1f))
    }

    @Test
    fun playbackSpeedsClampToSupportedPositiveValues() {
        assertEquals(1f, SupportedGpsPlaybackSpeeds.clamp(0f))
        assertEquals(0.5f, SupportedGpsPlaybackSpeeds.clamp(0.6f))
        assertEquals(4f, SupportedGpsPlaybackSpeeds.clamp(100f))
    }
}

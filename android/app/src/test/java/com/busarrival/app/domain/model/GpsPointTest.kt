package com.busarrival.app.domain.model

import android.location.Location
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class GpsPointTest {
    @Test
    fun fromLocation_preservesLocationTimeAsMilliseconds() {
        val location = Location("test").apply {
            time = 1_700_000_000_123L
            latitude = 25.0
            longitude = 121.0
            speed = 4.2f
            bearing = 90.0f
        }

        val gps = GpsPoint.fromLocation(location)

        assertEquals(1_700_000_000_123L, gps.timestamp)
    }

    @Test
    fun fromLocation_headingNull_whenBearingNotSet() {
        val location = Location("test").apply {
            time = 1_700_000_000_123L
            latitude = 25.0
            longitude = 121.0
            // Don't set bearing - hasBearing() will return false
        }

        val gps = GpsPoint.fromLocation(location)

        assertEquals(null, gps.headingCdeg)
    }

    @Test
    fun fromLocation_headingValid_whenBearingZeroDegrees() {
        val location = Location("test").apply {
            time = 1_700_000_000_123L
            latitude = 25.0
            longitude = 121.0
            bearing = 0.0f  // Explicitly set to 0° (North)
        }

        val gps = GpsPoint.fromLocation(location)

        assertEquals(0.toShort(), gps.headingCdeg)  // Valid 0° heading, not null
    }

    @Test
    fun fromLocation_headingCorrect_whenBearingValid() {
        val location = Location("test").apply {
            time = 1_700_000_000_123L
            latitude = 25.0
            longitude = 121.0
            bearing = 267.0f
        }

        val gps = GpsPoint.fromLocation(location)

        println("DEBUG: headingCdeg for bearing=267.0f: ${gps.headingCdeg}, expected: ${(-9300).toShort()}")
        assertEquals((-9300).toShort(), gps.headingCdeg)  // 267° normalized to -93°
    }
}

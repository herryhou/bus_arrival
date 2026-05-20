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
}

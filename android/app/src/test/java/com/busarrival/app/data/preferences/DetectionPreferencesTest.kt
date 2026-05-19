package com.busarrival.app.data.preferences

import com.busarrival.app.domain.model.DetectionParameters
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment

@RunWith(RobolectricTestRunner::class)
class DetectionPreferencesTest {

    @Test
    fun gpsLoggingDisabledByDefault() {
        val prefs = DetectionPreferences(RuntimeEnvironment.getApplication())

        assertFalse(prefs.gpsLoggingEnabled)
    }

    @Test
    fun gpsLoggingEnabledPersists() {
        val prefs = DetectionPreferences(RuntimeEnvironment.getApplication())

        prefs.gpsLoggingEnabled = true

        val reloaded = DetectionPreferences(RuntimeEnvironment.getApplication())
        assertTrue(reloaded.gpsLoggingEnabled)
    }

    @Test
    fun saveParametersStillWorksAfterToggle() {
        val prefs = DetectionPreferences(RuntimeEnvironment.getApplication())

        prefs.gpsLoggingEnabled = true
        prefs.saveParameters(
            DetectionParameters(
                distanceWeight = 11,
                speedWeight = 22,
                progressErrorWeight = 33,
                dwellTimeWeight = 44,
                corridorSize = 5
            )
        )

        assertTrue(prefs.gpsLoggingEnabled)
    }
}

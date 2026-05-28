package com.busarrival.app.presentation.ui.detection.components

import kotlin.test.assertEquals
import org.junit.Test

class VehicleHeadingMarkerTest {

    @Test
    fun vehicleHeadingRotationConvertsGpsBearingToArrowForwardRotation() {
        assertEquals(-90f, vehicleHeadingRotationDegrees(0f))
        assertEquals(0f, vehicleHeadingRotationDegrees(90f))
        assertEquals(90f, vehicleHeadingRotationDegrees(180f))
        assertEquals(180f, vehicleHeadingRotationDegrees(270f))
    }
}

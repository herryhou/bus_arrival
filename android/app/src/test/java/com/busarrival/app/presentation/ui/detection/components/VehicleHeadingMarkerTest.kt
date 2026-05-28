package com.busarrival.app.presentation.ui.detection.components

import androidx.compose.ui.geometry.Offset
import kotlin.test.assertEquals
import org.junit.Test

class VehicleHeadingMarkerTest {

    @Test
    fun vehicleHeadingRotationUsesNavigationIconNorthAsZero() {
        assertEquals(0f, vehicleHeadingRotationDegrees(0f))
        assertEquals(90f, vehicleHeadingRotationDegrees(90f))
        assertEquals(180f, vehicleHeadingRotationDegrees(180f))
        assertEquals(270f, vehicleHeadingRotationDegrees(270f))
    }

    @Test
    fun vehicleHeadingIconUsesHalfMarkerSize() {
        assertEquals(vehicleHeadingMarkerSize / 2f, vehicleHeadingIconSize)
    }

    @Test
    fun vehicleHeadingMarkerTopLeftAnchorsTipAtGpsPosition() {
        val gpsPosition = Offset(100f, 100f)
        val markerPx = 32f
        val iconPx = 16f

        assertEquals(
            Offset(84f, 92f),
            vehicleHeadingMarkerTopLeft(gpsPosition, 0f, markerPx, iconPx)
        )
        assertEquals(
            Offset(76f, 84f),
            vehicleHeadingMarkerTopLeft(gpsPosition, 90f, markerPx, iconPx)
        )
        assertEquals(
            Offset(84f, 76f),
            vehicleHeadingMarkerTopLeft(gpsPosition, 180f, markerPx, iconPx)
        )
        assertEquals(
            Offset(92f, 84f),
            vehicleHeadingMarkerTopLeft(gpsPosition, 270f, markerPx, iconPx)
        )
    }

    @Test
    fun vehicleHeadingMarkerTopLeftCentersUnknownHeadingAtGpsPosition() {
        assertEquals(
            Offset(84f, 84f),
            vehicleHeadingMarkerTopLeft(Offset(100f, 100f), null, markerPx = 32f, iconPx = 16f)
        )
    }
}

package com.busarrival.app.presentation.ui.detection.components

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.unit.dp
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
        assertEquals(64.dp.value, vehicleHeadingMarkerSize.value)
        assertEquals(32.dp.value, vehicleHeadingIconSize.value)
    }

    @Test
    fun vehicleHeadingMarkerTopLeftAnchorsTipAtGpsPosition() {
        val gpsPosition = Offset(100f, 100f)
        val markerPx = 64f  // was 32f
        val iconPx = 32f     // was 16f

        assertEquals(
            Offset(68f, 84f),
            vehicleHeadingMarkerTopLeft(gpsPosition, 0f, markerPx, iconPx)
        )
        assertEquals(
            Offset(52f, 68f),
            vehicleHeadingMarkerTopLeft(gpsPosition, 90f, markerPx, iconPx)
        )
        assertEquals(
            Offset(68f, 52f),
            vehicleHeadingMarkerTopLeft(gpsPosition, 180f, markerPx, iconPx)
        )
        assertEquals(
            Offset(84f, 68f),
            vehicleHeadingMarkerTopLeft(gpsPosition, 270f, markerPx, iconPx)
        )
    }

    @Test
    fun vehicleHeadingMarkerTopLeftCentersUnknownHeadingAtGpsPosition() {
        assertEquals(
            Offset(68f, 68f),
            vehicleHeadingMarkerTopLeft(Offset(100f, 100f), null, markerPx = 64f, iconPx = 32f)
        )
    }
}

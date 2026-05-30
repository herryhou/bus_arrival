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
    fun vehicleHeadingMarkerTopLeftCentersMarkerAtGpsPosition() {
        val gpsPosition = Offset(100f, 100f)
        val markerPx = 64f
        val iconPx = 32f

        // Implementation centers marker regardless of bearing
        val centered = Offset(68f, 68f) // 100 - 64/2 = 68
        assertEquals(
            centered,
            vehicleHeadingMarkerTopLeft(gpsPosition, 0f, markerPx, iconPx)
        )
        assertEquals(
            centered,
            vehicleHeadingMarkerTopLeft(gpsPosition, 90f, markerPx, iconPx)
        )
        assertEquals(
            centered,
            vehicleHeadingMarkerTopLeft(gpsPosition, 180f, markerPx, iconPx)
        )
        assertEquals(
            centered,
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

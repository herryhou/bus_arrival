package com.busarrival.app.presentation.ui.detection.components

import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.junit.Test

class MapViewportRequestTest {

    @Test
    fun viewportCenterWithoutPanMatchesRouteCenter() {
        val routeCenter = LatLon(lat = 25.0330, lon = 121.5654)

        val viewportCenter =
                viewportCenterToLatLon(
                        routeCenter = routeCenter,
                        viewportCenterWorldX = 0f,
                        viewportCenterWorldY = 0f,
                        zoom = 15
                )

        assertEquals(routeCenter.lat, viewportCenter.lat, 1e-4)
        assertEquals(routeCenter.lon, viewportCenter.lon, 1e-4)
    }

    @Test
    fun viewportCenterMovesToCorrectGeographyWhenPanned() {
        val routeCenter = LatLon(lat = 25.0330, lon = 121.5654)
        val viewportCenter =
                viewportCenterToLatLon(
                        routeCenter = routeCenter,
                        viewportCenterWorldX = 256f,
                        viewportCenterWorldY = 256f,
                        zoom = 15
                )

        assertTrue(viewportCenter.lon > routeCenter.lon, "Positive X world shift should move east")
        assertTrue(viewportCenter.lat < routeCenter.lat, "Positive Y world shift should move south")
    }
}

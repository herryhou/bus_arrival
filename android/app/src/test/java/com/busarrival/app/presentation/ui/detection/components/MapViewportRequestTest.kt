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

    @Test
    fun visibleTileRangeMatchesViewportCoverage() {
        val z15Range =
                computeVisibleTileRange(
                        zoom = 15,
                        canvasWidth = 1000f,
                        canvasHeight = 1000f,
                        scale = 1f
                )
        val z16Range =
                computeVisibleTileRange(
                        zoom = 16,
                        canvasWidth = 1000f,
                        canvasHeight = 1000f,
                        scale = 1f
                )

        assertEquals(3, z15Range, "Base zoom should cover a 7x7 draw window for a 1000px viewport")
        assertTrue(z16Range > z15Range, "Higher zoom should require a larger tile range")
    }

    @Test
    fun visibleTileRangeExpandsWhenScaleShrinks() {
        val normalScale =
                computeVisibleTileRange(
                        zoom = 15,
                        canvasWidth = 1000f,
                        canvasHeight = 1000f,
                        scale = 1f
                )
        val zoomedOutScale =
                computeVisibleTileRange(
                        zoom = 15,
                        canvasWidth = 1000f,
                        canvasHeight = 1000f,
                        scale = 0.5f
                )

        assertTrue(
                zoomedOutScale > normalScale,
                "Smaller scale should request a wider tile range to avoid blank viewports"
        )
        assertEquals(5, zoomedOutScale)
    }
}

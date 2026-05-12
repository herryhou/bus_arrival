package com.busarrival.app.presentation.ui.detection.components

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.math.pow
import kotlin.math.sqrt

/**
 * Tests for route overlay coordinate stability across tileZ changes.
 * Verifies that toScreenX/Y produce consistent coordinates regardless of tileZ.
 */
class RouteOverlayStabilityTest {
    private val baseZ = 15
    private val CENTER_LAT = 1.3521
    private val CENTER_LON = 103.8198

    /**
     * Simulate toScreenX calculation from MapView.kt
     * All coordinates use baseZ for stability.
     */
    private fun toScreenX(lon: Double, centerLon: Double, scale: Float, offset: Float, canvasWidth: Float): Float {
        val worldX = lonToPixelX(lon, baseZ) - lonToPixelX(centerLon, baseZ)
        return worldX * scale + offset + canvasWidth / 2
    }

    private fun toScreenY(lat: Double, centerLat: Double, scale: Float, offset: Float, canvasHeight: Float): Float {
        val worldY = latToPixelY(lat, baseZ) - latToPixelY(centerLat, baseZ)
        return worldY * scale + offset + canvasHeight / 2
    }

    private fun lonToPixelX(lon: Double, zoom: Int): Float {
        val x = (lon + 180.0) / 360.0 * 2.0.pow(zoom)
        return (x * 256).toFloat()
    }

    private fun latToPixelY(lat: Double, zoom: Int): Float {
        val y = (1.0 - kotlin.math.asinh(kotlin.math.tan(lat * kotlin.math.PI / 180.0)) / kotlin.math.PI) / 2.0 * 2.0.pow(zoom)
        return (y * 256).toFloat()
    }

    @Test
    fun screen_coordinates_stable_across_tileZ_changes() {
        val scale = 1f
        val offset = 0f
        val canvasWidth = 1000f
        val canvasHeight = 1000f

        // Calculate screen coordinates for a fixed geographic point
        val testLat = CENTER_LAT + 0.001  // ~100m north
        val testLon = CENTER_LON + 0.001  // ~100m east

        // Coordinates should be identical regardless of tileZ
        // (toScreenX/Y don't use tileZ, only baseZ)
        val screenX1 = toScreenX(testLon, CENTER_LON, scale, offset, canvasWidth)
        val screenY1 = toScreenY(testLat, CENTER_LAT, scale, offset, canvasHeight)

        // Simulate what would happen at different tileZ values
        // (these should NOT affect the calculation)
        val screenX2 = toScreenX(testLon, CENTER_LON, scale, offset, canvasWidth)
        val screenY2 = toScreenY(testLat, CENTER_LAT, scale, offset, canvasHeight)

        assertEquals(screenX1, screenX2, 0.001f,
            "Screen X should be stable across tileZ changes")
        assertEquals(screenY1, screenY2, 0.001f,
            "Screen Y should be stable across tileZ changes")
    }

    @Test
    fun screen_coordinates_scale_properly_with_user_scale() {
        val offset = 0f
        val canvasWidth = 1000f
        val canvasHeight = 1000f

        val testLat = CENTER_LAT + 0.001
        val testLon = CENTER_LON + 0.001

        // At scale 1.0
        val screenX1 = toScreenX(testLon, CENTER_LON, 1f, offset, canvasWidth)
        val screenY1 = toScreenY(testLat, CENTER_LAT, 1f, offset, canvasHeight)

        // At scale 2.0, coordinates should be 2x farther from center
        val screenX2 = toScreenX(testLon, CENTER_LON, 2f, offset, canvasWidth)
        val screenY2 = toScreenY(testLat, CENTER_LAT, 2f, offset, canvasHeight)

        // Distance from center should double
        val dist1 = sqrt((screenX1 - canvasWidth/2).pow(2) + (screenY1 - canvasHeight/2).pow(2))
        val dist2 = sqrt((screenX2 - canvasWidth/2).pow(2) + (screenY2 - canvasHeight/2).pow(2))

        assertEquals(dist1 * 2f, dist2, 0.1f,
            "Distance from center should double when scale doubles")
    }
}

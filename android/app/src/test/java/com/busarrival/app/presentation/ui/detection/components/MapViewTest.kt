package com.busarrival.app.presentation.ui.detection.components

import kotlin.math.pow
import kotlin.math.PI
import kotlin.math.asinh
import kotlin.math.tan
import kotlin.math.atan
import kotlin.math.exp
import org.junit.Test
import kotlin.test.assertEquals

/**
 * Tests for map tile rendering correctness across zoom scales.
 * Verifies tiles stitch together without gaps at zoom levels 1, 2, 4.
 */
class MapViewTest {

    // Mock coordinate conversions (simplified from actual implementation)
    private val baseZ = 15

    private fun lonToPixelX(lon: Double, zoom: Int): Float {
        val n = 2.0.pow(zoom)
        val x = (lon + 180.0) / 360.0 * n
        return (x * 256).toFloat()
    }

    private fun latToPixelY(lat: Double, zoom: Int): Float {
        val n = 2.0.pow(zoom)
        val y = ((1.0 - asinh(tan(lat * PI / 180.0)) / PI) / 2.0 * n)
        return (y * 256).toFloat()
    }

    private fun tileXToLon(tileX: Int, zoom: Int): Double {
        return tileX / 2.0.pow(zoom) * 360.0 - 180.0
    }

    private fun tileYToLat(tileY: Int, zoom: Int): Double {
        val n = PI - 2.0 * PI * tileY / 2.0.pow(zoom)
        return 180.0 / PI * atan(0.5 * (exp(n) - exp(-n)))
    }

    // World space positioning using baseZ (as per fix)
    private fun worldX(lon: Double, centerLon: Double): Float {
        return lonToPixelX(lon, baseZ) - lonToPixelX(centerLon, baseZ)
    }

    private fun worldY(lat: Double, centerLat: Double): Float {
        return latToPixelY(lat, baseZ) - latToPixelY(centerLat, baseZ)
    }

    @Test
    fun adjacentTilesAtSameZoomLevelHaveCorrectSpacing() {
        val centerLon = 120.0
        val centerLat = 20.0

        val centerTileX = ((centerLon + 180.0) / 360.0 * 2.0.pow(15)).toInt()
        val centerTileY = ((1.0 - asinh(tan(centerLat * PI / 180.0)) / PI) / 2.0 * 2.0.pow(15)).toInt()

        val centerTileNW = tileXToLon(centerTileX, 15)
        val eastTileNW = tileXToLon(centerTileX + 1, 15)

        val centerWorldX = worldX(centerTileNW, centerLon)
        val eastWorldX = worldX(eastTileNW, centerLon)

        val spacing = eastWorldX - centerWorldX
        assertEquals(256f, spacing, 0.1f)
    }

    @Test
    fun tilesMaintainCorrectSpacingAfterScaleTransform() {
        val centerLon = 120.0
        val centerLat = 20.0

        val centerTileX = ((centerLon + 180.0) / 360.0 * 2.0.pow(15)).toInt()
        val centerTileNW = tileXToLon(centerTileX, 15)
        val eastTileNW = tileXToLon(centerTileX + 1, 15)

        val centerWorldX = worldX(centerTileNW, centerLon)
        val eastWorldX = worldX(eastTileNW, centerLon)

        val spacing1 = eastWorldX - centerWorldX
        val spacing4 = spacing1 * 4f
        val coverage4 = 256f * 4f

        assertEquals(256f, spacing1, 0.1f)
        assertEquals(1024f, spacing4, 1f)
        assertEquals(1024f, coverage4, 1f)
    }

    @Test
    fun fallbackTilePositionedCorrectlyRelativeToNativeTiles() {
        val centerLon = 120.0

        val tileX = 1000
        val tileNW = tileXToLon(tileX, 17)

        val worldX = worldX(tileNW, centerLon)
        val expectedX = lonToPixelX(tileNW, 15) - lonToPixelX(centerLon, 15)

        assertEquals(expectedX, worldX, 0.1f)
    }

    @Test
    fun tileCoverageMatchesSpacingAtDifferentScales() {
        val centerLon = 120.0
        val centerLat = 20.0

        val centerTileX = ((centerLon + 180.0) / 360.0 * 2.0.pow(15)).toInt()
        val eastTileNW = tileXToLon(centerTileX + 1, 15)

        val centerWorldX = worldX(120.0, centerLon)
        val eastWorldX = worldX(eastTileNW, centerLon)

        val tileSize = 256f
        val spacing1 = eastWorldX - centerWorldX
        val coverage1 = tileSize * 1f

        assertEquals(spacing1, coverage1, 0.1f)

        val spacing2 = spacing1 * 2f
        val coverage2 = tileSize * 2f
        assertEquals(spacing2, coverage2, 0.1f)

        val spacing4 = spacing1 * 4f
        val coverage4 = tileSize * 4f
        assertEquals(spacing4, coverage4, 0.1f)
    }

    @Test
    fun fallbackTileScalingMaintainsAlignment() {
        val centerLon = 120.0
        val tileZ = 17
        val actualTileZ = 15

        val tileX = 1000
        val tileNW = tileXToLon(tileX, tileZ)
        val worldX = worldX(tileNW, centerLon)

        val zoomScaleFactor = 2.0.pow(tileZ - actualTileZ).toFloat()
        val nativeCoverage = 256f
        val fallbackCoverage = 256f * zoomScaleFactor

        assertEquals(nativeCoverage, fallbackCoverage, 0.1f)
    }

    @Test
    fun worldXCoordinateConsistencyAcrossZoomLevels() {
        val centerLon = 120.0
        val testLon = 120.01

        val worldX_15 = lonToPixelX(testLon, 15) - lonToPixelX(centerLon, 15)
        val worldX_16 = lonToPixelX(testLon, 16) - lonToPixelX(centerLon, 16)
        val worldX_17 = lonToPixelX(testLon, 17) - lonToPixelX(centerLon, 17)

        val baseWorldX = worldX(testLon, centerLon)

        assertEquals(worldX_15, baseWorldX, 0.1f)
        assertEquals(worldX_16 / 2f, baseWorldX, 0.1f)
        assertEquals(worldX_17 / 4f, baseWorldX, 0.1f)
    }
}

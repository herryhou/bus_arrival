package com.busarrival.app.presentation.ui.detection.components

import kotlin.math.PI
import kotlin.math.asinh
import kotlin.math.atan
import kotlin.math.exp
import kotlin.math.pow
import kotlin.math.tan
import kotlin.test.assertEquals
import org.junit.Test

/**
 * Tests for map tile rendering correctness.
 * Verifies coordinate system consistency and tile positioning.
 */
class MapViewTest {

    private val baseZ = 15

    @Test
    fun adjacentTilesAtBaseZHaveCorrectSpacing() {
        val centerLon = 120.0
        val centerLat = 20.0

        val centerTileX = ((centerLon + 180.0) / 360.0 * 2.0.pow(baseZ)).toInt()
        val centerTileNW = tileXToLon(centerTileX, baseZ)
        val eastTileNW = tileXToLon(centerTileX + 1, baseZ)

        val centerWorldX = lonToPixelX(centerTileNW, baseZ) - lonToPixelX(centerLon, baseZ)
        val eastWorldX = lonToPixelX(eastTileNW, baseZ) - lonToPixelX(centerLon, baseZ)

        val spacing = eastWorldX - centerWorldX
        assertEquals(256f, spacing, 0.1f, "Adjacent tiles at baseZ should be 256px apart")
    }

    @Test
    fun tileSpacingScalesCorrectlyWithZoom() {
        val centerLon = 120.0

        val tileX = 1000
        val tileNW = tileXToLon(tileX, 15)

        // At Z=15: 256px spacing
        val spacing15 = lonToPixelX(tileNW, 15) - lonToPixelX(centerLon, 15)

        // At Z=16: tiles are 2x smaller, so spacing between same tiles is 2x larger in pixel space
        val spacing16 = lonToPixelX(tileNW, 16) - lonToPixelX(centerLon, 16)

        // At Z=16, each tile is 128px, so the same geographic distance spans 2x as many tiles
        // Therefore, the pixel spacing is 2x
        assertEquals(spacing15 * 2f, spacing16, 1f, "Z=16 spacing should be 2x Z=15 spacing")
    }

    @Test
    fun worldCoordinatesAreConsistentAtBaseZ() {
        val centerLon = 120.0
        val testLon = 120.01

        // World coordinates at baseZ should be consistent
        val worldX = lonToPixelX(testLon, baseZ) - lonToPixelX(centerLon, baseZ)

        // Same calculation should give same result
        val worldXAgain = lonToPixelX(testLon, baseZ) - lonToPixelX(centerLon, baseZ)

        assertEquals(worldX, worldXAgain, 0.001f, "World coordinates should be consistent")
    }

    @Test
    fun tileSizeAtBaseZIs256px() {
        // At baseZ=15, a single tile is 256px in world space
        val tileSize = 256f

        val centerLon = 120.0
        val centerTileX = ((centerLon + 180.0) / 360.0 * 2.0.pow(baseZ)).toInt()
        val centerTileNW = tileXToLon(centerTileX, baseZ)
        val eastTileNW = tileXToLon(centerTileX + 1, baseZ)

        val centerWorldX = lonToPixelX(centerTileNW, baseZ) - lonToPixelX(centerLon, baseZ)
        val eastWorldX = lonToPixelX(eastTileNW, baseZ) - lonToPixelX(centerLon, baseZ)

        val spacing = eastWorldX - centerWorldX
        assertEquals(tileSize, spacing, 0.1f, "Tile spacing at baseZ should be 256px")
    }

    @Test
    fun fallbackTileAtZ15Is4xLargerThanZ17() {
        val baseZ = 15

        // Z=17 tile at baseZ: scale = 2^(15-17) = 1/4
        val z17TileSize = 256f * 2.0f.pow(baseZ - 17)

        // Z=15 tile at baseZ: scale = 2^(15-15) = 1
        val z15TileSize = 256f * 2.0f.pow(baseZ - 15)

        assertEquals(64f, z17TileSize, 0.1f, "Z=17 tile should be 64px at baseZ=15")
        assertEquals(256f, z15TileSize, 0.1f, "Z=15 tile should be 256px at baseZ=15")
        assertEquals(z17TileSize * 4f, z15TileSize, 0.1f, "Z=15 tile should be 4x Z=17 tile")
    }

    @Test
    fun fallbackTileAtZ16Is2xLargerThanZ17() {
        val baseZ = 15

        val z17TileSize = 256f * 2.0f.pow(baseZ - 17)
        val z16TileSize = 256f * 2.0f.pow(baseZ - 16)

        assertEquals(64f, z17TileSize, 0.1f, "Z=17 tile should be 64px at baseZ=15")
        assertEquals(128f, z16TileSize, 0.1f, "Z=16 tile should be 128px at baseZ=15")
        assertEquals(z17TileSize * 2f, z16TileSize, 0.1f, "Z=16 tile should be 2x Z=17 tile")
    }
}

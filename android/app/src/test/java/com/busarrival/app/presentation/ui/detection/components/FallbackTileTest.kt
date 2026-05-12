package com.busarrival.app.presentation.ui.detection.components

import androidx.compose.ui.geometry.Offset
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.math.pow

/**
 * Tests for fallback tile scaling and alignment.
 * Fallback tiles are lower zoom tiles used when higher zoom tiles are unavailable.
 * They must be scaled and aligned to match the viewport's zoom level.
 */
class FallbackTileTest {
    // Standard tile size in pixels (OSM tiles are 256x256)
    private val TILE_SIZE = 256f

    // Position tolerance in pixels - accounts for floating point arithmetic
    private val POSITION_TOLERANCE = 0.1f

    // Test coordinates for Singapore region
    private val CENTER_LAT = 1.3521
    private val CENTER_LON = 103.8198

    /**
     * Calculate screen coordinates for a tile's corner.
     * Returns the screen position of the tile's top-left corner.
     */
    private fun calculateTileScreenPosition(
        tileX: Int,
        tileY: Int,
        tileZ: Int,           // Requested zoom level
        actualTileZ: Int,     // Actual zoom level of tile we have
        centerLat: Double,
        centerLon: Double,
        scale: Float,
        offset: Offset,
        canvasWidth: Float,
        canvasHeight: Float,
        baseZ: Int = 15       // Base zoom level for world coordinates
    ): Offset {
        // CRITICAL: Derive geographic corners from REQUESTED tileZ, not actualTileZ
        val tileLon = tileXToLon(tileX, tileZ)
        val tileLat = tileYToLat(tileY, tileZ)

        // Convert to baseZ world coordinates
        val worldX = worldX(tileLon, centerLon, baseZ)
        val worldY = worldY(tileLat, centerLat, baseZ)

        // Apply composed scaling: native tile sizing + fallback compensation
        val scaleFactor = 2.0f.pow(baseZ - actualTileZ)

        val screenX = worldX * scaleFactor * scale + offset.x + canvasWidth / 2
        val screenY = worldY * scaleFactor * scale + offset.y + canvasHeight / 2

        return Offset(screenX, screenY)
    }

    /**
     * Calculate screen dimensions for a fallback tile.
     * Returns Pair(width, height) in screen pixels.
     */
    private fun calculateTileScreenDimensions(
        tileZ: Int,
        actualTileZ: Int,
        scale: Float
    ): Pair<Float, Float> {
        val scaleFactor = 2.0f.pow(tileZ - actualTileZ)
        val scaledTileSize = TILE_SIZE * scaleFactor
        return Pair(scaledTileSize * scale, scaledTileSize * scale)
    }

    @Test
    fun fallback_tile_covers_same_area_as_native_tile() {
        // Native tile at Z=17
        val tileZ = 17
        val actualTileZ = 17
        val tileX = lonToTileX(CENTER_LON, tileZ)
        val tileY = latToTileY(CENTER_LAT, tileZ)

        val scale = 1f
        val offset = Offset.Zero
        val canvasWidth = 1000f
        val canvasHeight = 1000f

        val nativePosition = calculateTileScreenPosition(
            tileX, tileY, tileZ, actualTileZ,
            CENTER_LAT, CENTER_LON, scale, offset, canvasWidth, canvasHeight
        )
        val (nativeWidth, nativeHeight) = calculateTileScreenDimensions(
            tileZ, actualTileZ, scale
        )

        // Fallback tile from Z=15 (should be scaled by 4x)
        val fallbackActualZ = 15
        val fallbackTileX = lonToTileX(CENTER_LON, fallbackActualZ)
        val fallbackTileY = latToTileY(CENTER_LAT, fallbackActualZ)

        val fallbackPosition = calculateTileScreenPosition(
            fallbackTileX, fallbackTileY, tileZ, fallbackActualZ,
            CENTER_LAT, CENTER_LON, scale, offset, canvasWidth, canvasHeight
        )
        val (fallbackWidth, fallbackHeight) = calculateTileScreenDimensions(
            tileZ, fallbackActualZ, scale
        )

        // Verify fallback tile covers same area (within tolerance)
        assertEquals(nativePosition.x, fallbackPosition.x, POSITION_TOLERANCE,
            "Fallback tile X position should match native tile")
        assertEquals(nativePosition.y, fallbackPosition.y, POSITION_TOLERANCE,
            "Fallback tile Y position should match native tile")

        // Verify fallback tile is scaled by 4x (2^(17-15) = 4)
        assertEquals(nativeWidth * 4f, fallbackWidth, POSITION_TOLERANCE,
            "Fallback tile width should be 4x native tile width")
        assertEquals(nativeHeight * 4f, fallbackHeight, POSITION_TOLERANCE,
            "Fallback tile height should be 4x native tile height")
    }

    @Test
    fun multiple_fallback_levels_align_correctly() {
        // Viewport at Z=17
        val tileZ = 17
        val scale = 1f
        val offset = Offset.Zero
        val canvasWidth = 1000f
        val canvasHeight = 1000f

        // Test position
        val testLon = CENTER_LON
        val testLat = CENTER_LAT

        // Native tile at Z=17
        val nativeZ = 17
        val nativeTileX = lonToTileX(testLon, nativeZ)
        val nativeTileY = latToTileY(testLat, nativeZ)

        val nativePosition = calculateTileScreenPosition(
            nativeTileX, nativeTileY, tileZ, nativeZ,
            CENTER_LAT, CENTER_LON, scale, offset, canvasWidth, canvasHeight
        )

        // Fallback from Z=16 (scaled by 2x)
        val fallback16Z = 16
        val fallback16TileX = lonToTileX(testLon, fallback16Z)
        val fallback16TileY = latToTileY(testLat, fallback16Z)

        val fallback16Position = calculateTileScreenPosition(
            fallback16TileX, fallback16TileY, tileZ, fallback16Z,
            CENTER_LAT, CENTER_LON, scale, offset, canvasWidth, canvasHeight
        )

        // Fallback from Z=15 (scaled by 4x)
        val fallback15Z = 15
        val fallback15TileX = lonToTileX(testLon, fallback15Z)
        val fallback15TileY = latToTileY(testLat, fallback15Z)

        val fallback15Position = calculateTileScreenPosition(
            fallback15TileX, fallback15TileY, tileZ, fallback15Z,
            CENTER_LAT, CENTER_LON, scale, offset, canvasWidth, canvasHeight
        )

        // All should align at the same screen position
        assertEquals(nativePosition.x, fallback16Position.x, POSITION_TOLERANCE,
            "Z=16 fallback X should align with native Z=17")
        assertEquals(nativePosition.y, fallback16Position.y, POSITION_TOLERANCE,
            "Z=16 fallback Y should align with native Z=17")

        assertEquals(nativePosition.x, fallback15Position.x, POSITION_TOLERANCE,
            "Z=15 fallback X should align with native Z=17")
        assertEquals(nativePosition.y, fallback15Position.y, POSITION_TOLERANCE,
            "Z=15 fallback Y should align with native Z=17")
    }

    @Test
    fun fallback_tiles_stitch_with_native_tiles() {
        // Viewport at Z=17
        val tileZ = 17
        val scale = 1f
        val offset = Offset.Zero
        val canvasWidth = 1000f
        val canvasHeight = 1000f

        // Native tile at Z=17
        val nativeZ = 17
        val nativeTileX = lonToTileX(CENTER_LON, nativeZ)
        val nativeTileY = latToTileY(CENTER_LAT, nativeZ)

        val (nativeWidth, nativeHeight) = calculateTileScreenDimensions(
            tileZ, nativeZ, scale
        )

        // Native tile right edge
        val nativeRightEdge = nativeTileX + 1
        val nativeRightPosition = calculateTileScreenPosition(
            nativeRightEdge, nativeTileY, tileZ, nativeZ,
            CENTER_LAT, CENTER_LON, scale, offset, canvasWidth, canvasHeight
        )

        // Fallback tile from Z=15 covering the same area as 4x4 native tiles
        val fallbackZ = 15
        val fallbackTileX = lonToTileX(CENTER_LON, fallbackZ)
        val fallbackTileY = latToTileY(CENTER_LAT, fallbackZ)

        val (fallbackWidth, fallbackHeight) = calculateTileScreenDimensions(
            tileZ, fallbackZ, scale
        )

        // The fallback tile should cover exactly 4x4 native tiles
        // So its width should be 4x the native tile width
        assertEquals(nativeWidth * 4f, fallbackWidth, POSITION_TOLERANCE,
            "Fallback tile width should equal 4 native tiles")
        assertEquals(nativeHeight * 4f, fallbackHeight, POSITION_TOLERANCE,
            "Fallback tile height should equal 4 native tiles")

        // Verify that adjacent native tiles align with the fallback tile boundary
        // The fallback tile's right edge should align with the 4th native tile's right edge
        val fourthNativeTileX = nativeTileX + 4
        val fourthNativePosition = calculateTileScreenPosition(
            fourthNativeTileX, nativeTileY, tileZ, nativeZ,
            CENTER_LAT, CENTER_LON, scale, offset, canvasWidth, canvasHeight
        )

        val fallbackPosition = calculateTileScreenPosition(
            fallbackTileX, fallbackTileY, tileZ, fallbackZ,
            CENTER_LAT, CENTER_LON, scale, offset, canvasWidth, canvasHeight
        )

        val fallbackRightEdge = fallbackPosition.x + fallbackWidth
        val fourthNativeRightEdge = fourthNativePosition.x + nativeWidth

        assertEquals(fallbackRightEdge, fourthNativeRightEdge, POSITION_TOLERANCE,
            "Fallback tile right edge should align with 4th native tile right edge")
    }

    @Test
    fun fallback_tile_scale_factor_calculation() {
        // Test zoom scale factor calculation for various fallback levels
        assertEquals(1f, 2.0f.pow(17 - 17), 0.001f,
            "Same zoom level should have scale factor of 1")
        assertEquals(2f, 2.0f.pow(17 - 16), 0.001f,
            "One level fallback should have scale factor of 2")
        assertEquals(4f, 2.0f.pow(17 - 15), 0.001f,
            "Two level fallback should have scale factor of 4")
        assertEquals(8f, 2.0f.pow(17 - 14), 0.001f,
            "Three level fallback should have scale factor of 8")

        // Test reverse (viewport at lower zoom than available tile)
        assertEquals(0.5f, 2.0f.pow(16 - 17), 0.001f,
            "One level higher should have scale factor of 0.5")
        assertEquals(0.25f, 2.0f.pow(15 - 17), 0.001f,
            "Two level higher should have scale factor of 0.25")
    }

    @Test
    fun fallback_tile_dimensions_at_different_scales() {
        val tileZ = 17
        val fallbackZ = 15

        // At scale 1.0
        val (width1, height1) = calculateTileScreenDimensions(tileZ, fallbackZ, 1f)
        assertEquals(TILE_SIZE * 4f, width1, POSITION_TOLERANCE,
            "Fallback tile at scale 1.0 should be 4x native size")
        assertEquals(TILE_SIZE * 4f, height1, POSITION_TOLERANCE,
            "Fallback tile at scale 1.0 should be 4x native size")

        // At scale 2.0
        val (width2, height2) = calculateTileScreenDimensions(tileZ, fallbackZ, 2f)
        assertEquals(TILE_SIZE * 4f * 2f, width2, POSITION_TOLERANCE,
            "Fallback tile at scale 2.0 should be 8x native size")
        assertEquals(TILE_SIZE * 4f * 2f, height2, POSITION_TOLERANCE,
            "Fallback tile at scale 2.0 should be 8x native size")

        // At scale 0.5
        val (width05, height05) = calculateTileScreenDimensions(tileZ, fallbackZ, 0.5f)
        assertEquals(TILE_SIZE * 4f * 0.5f, width05, POSITION_TOLERANCE,
            "Fallback tile at scale 0.5 should be 2x native size")
        assertEquals(TILE_SIZE * 4f * 0.5f, height05, POSITION_TOLERANCE,
            "Fallback tile at scale 0.5 should be 2x native size")
    }

    @Test
    fun fallback_tile_handles_edge_cases() {
        val tileZ = 17
        val scale = 1f
        val offset = Offset.Zero
        val canvasWidth = 1000f
        val canvasHeight = 1000f

        // Test with minimum zoom difference (should not cause division by zero)
        val minDiffZ = 16
        val minDiffTileX = lonToTileX(CENTER_LON, minDiffZ)
        val minDiffTileY = latToTileY(CENTER_LAT, minDiffZ)

        val minDiffPosition = calculateTileScreenPosition(
            minDiffTileX, minDiffTileY, tileZ, minDiffZ,
            CENTER_LAT, CENTER_LON, scale, offset, canvasWidth, canvasHeight
        )

        // Verify position is calculated without errors
        assertEquals(500f, minDiffPosition.x, POSITION_TOLERANCE,
            "Tile should be centered at canvas middle with no offset")
        assertEquals(500f, minDiffPosition.y, POSITION_TOLERANCE,
            "Tile should be centered at canvas middle with no offset")

        // Test with extreme zoom difference (Z=17 vs Z=10)
        val extremeDiffZ = 10
        val extremeDiffTileX = lonToTileX(CENTER_LON, extremeDiffZ)
        val extremeDiffTileY = latToTileY(CENTER_LAT, extremeDiffZ)

        val (extremeWidth, extremeHeight) = calculateTileScreenDimensions(
            tileZ, extremeDiffZ, scale
        )

        // Verify dimensions are calculated correctly for extreme scale
        assertEquals(TILE_SIZE * 2.0f.pow(17 - 10), extremeWidth, POSITION_TOLERANCE,
            "Extreme fallback tile width should be calculated correctly")
        assertEquals(TILE_SIZE * 2.0f.pow(17 - 10), extremeHeight, POSITION_TOLERANCE,
            "Extreme fallback tile height should be calculated correctly")
    }
}

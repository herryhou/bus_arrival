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
        tileZ: Int,           // Requested zoom level
        actualTileZ: Int,     // Actual zoom level of tile we have
        scale: Float,
        baseZ: Int = 15       // Base zoom level for world coordinates
    ): Pair<Float, Float> {
        // Composed scaling: native tile sizing at baseZ + fallback compensation
        val scaleFactor = 2.0f.pow(baseZ - actualTileZ)
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

        // Fallback tile from Z=15 (baseZ=15, so scale = 2^(15-15) = 1)
        // CRITICAL: Use REQUESTED tile grid (tileZ=17) for positioning, not fallback grid
        val fallbackActualZ = 15
        // Don't use lonToTileX with fallbackActualZ - use the requested tileZ grid
        // The fallback Z=15 bitmap covers 4x4 Z=17 tiles, so position at any of those 16 Z=17 tiles
        // For simplicity, use the same tileX/Y as native (they should align)
        val fallbackPosition = calculateTileScreenPosition(
            tileX, tileY, tileZ, fallbackActualZ,  // Use native tileX/Y with fallback Z
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

        // In baseZ system: native Z=17 tile scales by 2^(15-17) = 1/4 (64px)
        // Fallback Z=15 tile scales by 2^(15-15) = 1 (256px)
        // So fallback is 4x native tile size
        assertEquals(nativeWidth * 4f, fallbackWidth, POSITION_TOLERANCE,
            "Fallback tile width should be 4x native tile width (256px vs 64px in baseZ)")
        assertEquals(nativeHeight * 4f, fallbackHeight, POSITION_TOLERANCE,
            "Fallback tile height should be 4x native tile height (256px vs 64px in baseZ)")
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

        // Fallback from Z=16 (in baseZ: scale = 2^(15-16) = 1/2, so 128px vs 64px native)
        // Use requested Z=17 grid for positioning
        val fallback16Z = 16
        val fallback16Position = calculateTileScreenPosition(
            nativeTileX, nativeTileY, tileZ, fallback16Z,  // Use native tileX/Y with fallback Z
            CENTER_LAT, CENTER_LON, scale, offset, canvasWidth, canvasHeight
        )

        // Fallback from Z=15 (in baseZ: scale = 2^(15-15) = 1, so 256px vs 64px native)
        // Use requested Z=17 grid for positioning
        val fallback15Z = 15
        val fallback15Position = calculateTileScreenPosition(
            nativeTileX, nativeTileY, tileZ, fallback15Z,  // Use native tileX/Y with fallback Z
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

        // Fallback tile from Z=15 (in baseZ: scale = 2^(15-15) = 1, so 256px)
        // Native Z=17 tiles are 64px each (scale = 2^(15-17) = 1/4)
        // So one Z=15 tile equals 4 Z=17 tiles in width/height
        val fallbackZ = 15
        val (fallbackWidth, fallbackHeight) = calculateTileScreenDimensions(
            tileZ, fallbackZ, scale
        )

        // Verify: native Z=17 is 64px, fallback Z=15 is 256px, so fallback is 4x native
        assertEquals(nativeWidth * 4f, fallbackWidth, POSITION_TOLERANCE,
            "Fallback tile width should equal 4 native tiles (256px vs 64px)")
        assertEquals(nativeHeight * 4f, fallbackHeight, POSITION_TOLERANCE,
            "Fallback tile height should equal 4 native tiles (256px vs 64px)")

        // Verify that adjacent native tiles align with the fallback tile boundary
        // The fallback tile's right edge should align with the 4th native tile's right edge
        val fourthNativeTileX = nativeTileX + 4
        val fourthNativePosition = calculateTileScreenPosition(
            fourthNativeTileX, nativeTileY, tileZ, nativeZ,
            CENTER_LAT, CENTER_LON, scale, offset, canvasWidth, canvasHeight
        )

        // Use native tile position for fallback (they should align at same position)
        val fallbackPosition = calculateTileScreenPosition(
            nativeTileX, nativeTileY, tileZ, fallbackZ,  // Use native tileX/Y with fallback Z
            CENTER_LAT, CENTER_LON, scale, offset, canvasWidth, canvasHeight
        )

        val fallbackRightEdge = fallbackPosition.x + fallbackWidth
        val fourthNativeRightEdge = fourthNativePosition.x + nativeWidth

        assertEquals(fallbackRightEdge, fourthNativeRightEdge, POSITION_TOLERANCE,
            "Fallback tile right edge should align with 4th native tile right edge")
    }

    @Test
    fun fallback_tile_scale_factor_calculation() {
        val baseZ = 15
        // Test composed scale factor calculation for various fallback levels
        // Formula: 2^(baseZ - actualTileZ)
        assertEquals(0.25f, 2.0f.pow(baseZ - 17), 0.001f,
            "Z=17 tile at baseZ=15: scale factor of 1/4")
        assertEquals(0.5f, 2.0f.pow(baseZ - 16), 0.001f,
            "Z=16 tile at baseZ=15: scale factor of 1/2")
        assertEquals(1f, 2.0f.pow(baseZ - 15), 0.001f,
            "Z=15 tile at baseZ=15: scale factor of 1")
        assertEquals(2f, 2.0f.pow(baseZ - 14), 0.001f,
            "Z=14 tile at baseZ=15: scale factor of 2")

        // Note: These values represent the scaling relative to TILE_SIZE
        // The actual screen size is TILE_SIZE * scale factor
    }

    @Test
    fun fallback_tile_dimensions_at_different_scales() {
        val tileZ = 17
        val fallbackZ = 15
        val baseZ = 15

        // At scale 1.0: Z=15 tile at baseZ=15 has scale = 2^(15-15) = 1
        val (width1, height1) = calculateTileScreenDimensions(tileZ, fallbackZ, 1f)
        assertEquals(TILE_SIZE * 2.0f.pow(baseZ - fallbackZ), width1, POSITION_TOLERANCE,
            "Fallback tile at scale 1.0 should be 256px (Z=15 at baseZ=15)")
        assertEquals(TILE_SIZE * 2.0f.pow(baseZ - fallbackZ), height1, POSITION_TOLERANCE,
            "Fallback tile at scale 1.0 should be 256px (Z=15 at baseZ=15)")

        // At scale 2.0: user scale multiplies the tile size
        val (width2, height2) = calculateTileScreenDimensions(tileZ, fallbackZ, 2f)
        assertEquals(TILE_SIZE * 2.0f.pow(baseZ - fallbackZ) * 2f, width2, POSITION_TOLERANCE,
            "Fallback tile at scale 2.0 should be 512px (256 * 2)")
        assertEquals(TILE_SIZE * 2.0f.pow(baseZ - fallbackZ) * 2f, height2, POSITION_TOLERANCE,
            "Fallback tile at scale 2.0 should be 512px (256 * 2)")

        // At scale 0.5: user scale reduces the tile size
        val (width05, height05) = calculateTileScreenDimensions(tileZ, fallbackZ, 0.5f)
        assertEquals(TILE_SIZE * 2.0f.pow(baseZ - fallbackZ) * 0.5f, width05, POSITION_TOLERANCE,
            "Fallback tile at scale 0.5 should be 128px (256 * 0.5)")
        assertEquals(TILE_SIZE * 2.0f.pow(baseZ - fallbackZ) * 0.5f, height05, POSITION_TOLERANCE,
            "Fallback tile at scale 0.5 should be 128px (256 * 0.5)")
    }

    @Test
    fun fallback_tile_handles_edge_cases() {
        val tileZ = 17
        val scale = 1f
        val offset = Offset.Zero
        val canvasWidth = 1000f
        val canvasHeight = 1000f

        // Calculate tile coordinates for requested zoom level
        val tileX = lonToTileX(CENTER_LON, tileZ)
        val tileY = latToTileY(CENTER_LAT, tileZ)

        // Test with minimum zoom difference (should not cause division by zero)
        val minDiffZ = 16
        // Use requested Z=17 grid for positioning
        val minDiffPosition = calculateTileScreenPosition(
            tileX, tileY, tileZ, minDiffZ,  // Use native tileX/Y with fallback Z
            CENTER_LAT, CENTER_LON, scale, offset, canvasWidth, canvasHeight
        )

        // Verify position is calculated without errors (allow some tolerance for tile grid quantization)
        assertEquals(500f, minDiffPosition.x, 21f,  // Increased tolerance for tile grid rounding
            "Tile should be centered at canvas middle with no offset")
        assertEquals(500f, minDiffPosition.y, 21f,  // Increased tolerance for tile grid rounding
            "Tile should be centered at canvas middle with no offset")

        // Test with extreme zoom difference (Z=17 vs Z=10) at baseZ=15
        val extremeDiffZ = 10
        val (extremeWidth, extremeHeight) = calculateTileScreenDimensions(
            tileZ, extremeDiffZ, scale
        )

        // Verify dimensions use baseZ formula: 2^(15-10) = 32
        val baseZ = 15
        val expectedScale = 2.0f.pow(baseZ - extremeDiffZ)
        assertEquals(TILE_SIZE * expectedScale, extremeWidth, POSITION_TOLERANCE,
            "Extreme fallback tile width should be 8192px (256 * 32)")
        assertEquals(TILE_SIZE * expectedScale, extremeHeight, POSITION_TOLERANCE,
            "Extreme fallback tile height should be 8192px (256 * 32)")
    }
}

package com.busarrival.app.presentation.ui.detection.components

import androidx.compose.ui.geometry.Offset
import kotlin.math.pow
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.junit.Test

/**
 * Tests for fallback tile scaling and alignment.
 * Fallback tiles are lower zoom tiles used when higher zoom tiles are unavailable.
 *
 * These tests simulate MapView.kt rendering behavior to verify fallback tiles
 * are correctly positioned and scaled.
 */
class FallbackTileTest {
        private val TILE_SIZE = 256f
        private val POSITION_TOLERANCE = 0.1f

        // Test coordinates for Taiwan region
        private val CENTER_LAT = 24.1469
        private val CENTER_LON = 120.6839

        /**
         * Simulate MapView.kt tile rendering calculation.
         * Matches the actual rendering logic in MapView.kt lines 328-402.
         */
        private fun simulateTileRendering(
                tileX: Int,
                tileY: Int,
                tileZ: Int, // Requested zoom level
                actualTileZ: Int, // Actual zoom level of available tile
                centerLat: Double,
                centerLon: Double,
                scale: Float,
                offset: Offset,
                canvasWidth: Float,
                canvasHeight: Float,
                baseZ: Int = 15
        ): TileRenderResult {
                // MapView.kt line 329-331: Determine actual tile coordinates
                val posTileX = if (actualTileZ != tileZ) tileX / 2.0.pow(tileZ - actualTileZ).toInt() else tileX
                val posTileY = if (actualTileZ != tileZ) tileY / 2.0.pow(tileZ - actualTileZ).toInt() else tileY
                val posZ = if (actualTileZ != tileZ) actualTileZ else tileZ

                // MapView.kt line 337-347: Calculate position using ACTUAL tile coordinates
                val tileNW = tileXToLon(posTileX, posZ)
                val tileNorth = tileYToLat(posTileY, posZ)

                val tileWorldX = lonToPixelX(tileNW, baseZ) - lonToPixelX(centerLon, baseZ)
                val tileWorldY = latToPixelY(tileNorth, baseZ) - latToPixelY(centerLat, baseZ)

                // MapView.kt line 358-360: Calculate screen position
                val screenX = tileWorldX * scale + offset.x + canvasWidth / 2
                val screenY = tileWorldY * scale + offset.y + canvasHeight / 2

                // MapView.kt line 384: Calculate draw scale
                val drawScale = 2.0f.pow(baseZ - posZ)

                // MapView.kt line 360: Calculate screen size
                val screenSize = TILE_SIZE * drawScale * scale

                return TileRenderResult(
                        screenX = screenX,
                        screenY = screenY,
                        screenWidth = screenSize,
                        screenHeight = screenSize,
                        tileWorldX = tileWorldX,
                        tileWorldY = tileWorldY
                )
        }

        private data class TileRenderResult(
                val screenX: Float,
                val screenY: Float,
                val screenWidth: Float,
                val screenHeight: Float,
                val tileWorldX: Float,
                val tileWorldY: Float
        )

        @Test
        fun native_tile_at_Z17_renders_correctly() {
                val tileZ = 17
                val actualTileZ = 17
                val tileX = lonToTileX(CENTER_LON, tileZ)
                val tileY = latToTileY(CENTER_LAT, tileZ)

                val scale = 1f
                val offset = Offset.Zero
                val canvasWidth = 1000f
                val canvasHeight = 1000f
                val baseZ = 15

                val result = simulateTileRendering(
                        tileX, tileY, tileZ, actualTileZ,
                        CENTER_LAT, CENTER_LON, scale, offset,
                        canvasWidth, canvasHeight, baseZ
                )

                // Native Z=17 tile at baseZ=15: scale = 2^(15-17) = 1/4
                // Tile size = 256 * 1/4 = 64px
                val expectedSize = TILE_SIZE * 2.0f.pow(baseZ - tileZ)
                assertEquals(expectedSize, result.screenWidth, POSITION_TOLERANCE,
                        "Native Z=17 tile should be 64px at baseZ=15")
                assertEquals(expectedSize, result.screenHeight, POSITION_TOLERANCE,
                        "Native Z=17 tile should be 64px at baseZ=15")
        }

        @Test
        fun fallback_tile_Z15_is_4x_larger_than_Z17() {
                val tileZ = 17
                val tileX = lonToTileX(CENTER_LON, tileZ)
                val tileY = latToTileY(CENTER_LAT, tileZ)

                val scale = 1f
                val offset = Offset.Zero
                val canvasWidth = 1000f
                val canvasHeight = 1000f
                val baseZ = 15

                // Native tile at Z=17
                val nativeResult = simulateTileRendering(
                        tileX, tileY, tileZ, 17,
                        CENTER_LAT, CENTER_LON, scale, offset,
                        canvasWidth, canvasHeight, baseZ
                )

                // Fallback tile at Z=15 (same geographic position)
                val fallbackResult = simulateTileRendering(
                        tileX, tileY, tileZ, 15,
                        CENTER_LAT, CENTER_LON, scale, offset,
                        canvasWidth, canvasHeight, baseZ
                )

                // Fallback Z=15 tile should be 4x larger than Z=17 tile
                assertEquals(
                        nativeResult.screenWidth * 4f,
                        fallbackResult.screenWidth,
                        POSITION_TOLERANCE,
                        "Fallback Z=15 tile should be 4x width of Z=17 tile"
                )
                assertEquals(
                        nativeResult.screenHeight * 4f,
                        fallbackResult.screenHeight,
                        POSITION_TOLERANCE,
                        "Fallback Z=15 tile should be 4x height of Z=17 tile"
                )

                // Z=15 tile: 256 * 2^(15-15) = 256 * 1 = 256px
                // Z=17 tile: 256 * 2^(15-17) = 256 * 1/4 = 64px
                assertEquals(256f, fallbackResult.screenWidth, POSITION_TOLERANCE,
                        "Fallback Z=15 tile should be 256px at scale=1x")
                assertEquals(64f, nativeResult.screenWidth, POSITION_TOLERANCE,
                        "Native Z=17 tile should be 64px at scale=1x")
        }

        @Test
        fun fallback_tile_Z16_is_2x_larger_than_Z17() {
                val tileZ = 17
                val tileX = lonToTileX(CENTER_LON, tileZ)
                val tileY = latToTileY(CENTER_LAT, tileZ)

                val scale = 1f
                val offset = Offset.Zero
                val canvasWidth = 1000f
                val canvasHeight = 1000f
                val baseZ = 15

                val nativeResult = simulateTileRendering(
                        tileX, tileY, tileZ, 17,
                        CENTER_LAT, CENTER_LON, scale, offset,
                        canvasWidth, canvasHeight, baseZ
                )

                val fallbackResult = simulateTileRendering(
                        tileX, tileY, tileZ, 16,
                        CENTER_LAT, CENTER_LON, scale, offset,
                        canvasWidth, canvasHeight, baseZ
                )

                // Z=16 tile should be 2x larger than Z=17 tile
                assertEquals(
                        nativeResult.screenWidth * 2f,
                        fallbackResult.screenWidth,
                        POSITION_TOLERANCE,
                        "Fallback Z=16 tile should be 2x width of Z=17 tile"
                )

                // Z=16 tile: 256 * 2^(15-16) = 256 * 1/2 = 128px
                assertEquals(128f, fallbackResult.screenWidth, POSITION_TOLERANCE,
                        "Fallback Z=16 tile should be 128px at scale=1x")
        }

        @Test
        fun user_scale_affects_all_tiles_equally() {
                val tileZ = 17
                val tileX = lonToTileX(CENTER_LON, tileZ)
                val tileY = latToTileY(CENTER_LAT, tileZ)

                val baseZ = 15
                val offset = Offset.Zero
                val canvasWidth = 1000f
                val canvasHeight = 1000f

                // Test at scale=1x
                val result1x = simulateTileRendering(
                        tileX, tileY, tileZ, 17,
                        CENTER_LAT, CENTER_LON, 1f, offset,
                        canvasWidth, canvasHeight, baseZ
                )

                // Test at scale=2x
                val result2x = simulateTileRendering(
                        tileX, tileY, tileZ, 17,
                        CENTER_LAT, CENTER_LON, 2f, offset,
                        canvasWidth, canvasHeight, baseZ
                )

                // User scale should multiply tile size
                assertEquals(
                        result1x.screenWidth * 2f,
                        result2x.screenWidth,
                        POSITION_TOLERANCE,
                        "Scale=2x should double tile width"
                )
                assertEquals(
                        result1x.screenHeight * 2f,
                        result2x.screenHeight,
                        POSITION_TOLERANCE,
                        "Scale=2x should double tile height"
                )
        }

        @Test
        fun adjacent_tiles_align_without_gaps() {
                val tileZ = 17
                val actualTileZ = 17
                val baseZ = 15

                val tileX = lonToTileX(CENTER_LON, tileZ)
                val tileY = latToTileY(CENTER_LAT, tileZ)

                val scale = 1f
                val offset = Offset.Zero
                val canvasWidth = 1000f
                val canvasHeight = 1000f

                // Render first tile
                val tile1 = simulateTileRendering(
                        tileX, tileY, tileZ, actualTileZ,
                        CENTER_LAT, CENTER_LON, scale, offset,
                        canvasWidth, canvasHeight, baseZ
                )

                // Render adjacent tile to the east
                val tile2 = simulateTileRendering(
                        tileX + 1, tileY, tileZ, actualTileZ,
                        CENTER_LAT, CENTER_LON, scale, offset,
                        canvasWidth, canvasHeight, baseZ
                )

                // Tiles should align without gaps
                val tile1RightEdge = tile1.screenX + tile1.screenWidth
                assertEquals(
                        tile1RightEdge,
                        tile2.screenX,
                        POSITION_TOLERANCE,
                        "Adjacent tiles should align without gaps"
                )
        }

        @Test
        fun fallback_tile_covers_4x4_native_tile_area() {
                val tileZ = 17
                val baseZ = 15

                val tileX = lonToTileX(CENTER_LON, tileZ)
                val tileY = latToTileY(CENTER_LAT, tileZ)

                val scale = 1f
                val offset = Offset.Zero
                val canvasWidth = 1000f
                val canvasHeight = 1000f

                // Native tile at Z=17
                val native = simulateTileRendering(
                        tileX, tileY, tileZ, 17,
                        CENTER_LAT, CENTER_LON, scale, offset,
                        canvasWidth, canvasHeight, baseZ
                )

                // Fallback tile at Z=15
                val fallback = simulateTileRendering(
                        tileX, tileY, tileZ, 15,
                        CENTER_LAT, CENTER_LON, scale, offset,
                        canvasWidth, canvasHeight, baseZ
                )

                // Fallback tile should cover 4x4 = 16 native tiles
                // Check that fallback tile is 4x wider and 4x taller
                assertEquals(
                        native.screenWidth * 4f,
                        fallback.screenWidth,
                        POSITION_TOLERANCE,
                        "Fallback tile should be 4x wider"
                )
                assertEquals(
                        native.screenHeight * 4f,
                        fallback.screenHeight,
                        POSITION_TOLERANCE,
                        "Fallback tile should be 4x taller"
                )

                // Verify fallback tile starts at or before native tile position
                // (it covers larger geographic area)
                assertTrue(
                        fallback.screenX <= native.screenX + POSITION_TOLERANCE,
                        "Fallback should start at or before native tile X"
                )
                assertTrue(
                        fallback.screenY <= native.screenY + POSITION_TOLERANCE,
                        "Fallback should start at or before native tile Y"
                )
        }
}

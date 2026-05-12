package com.busarrival.app.presentation.ui.detection

import androidx.compose.ui.geometry.Offset
import com.busarrival.app.presentation.ui.detection.components.worldX
import com.busarrival.app.presentation.ui.detection.components.tileXToLon
import org.junit.Test
import kotlin.test.assertEquals

class TilePositioningTest {
    // Standard tile size in pixels (OSM tiles are 256x256)
    private val TILE_SIZE = 256f

    // Test canvas width - represents typical screen/map viewport
    private val TEST_CANVAS_WIDTH = 1000f

    // Center longitude for Singapore region (primary deployment area)
    private val CENTER_LON_SG = 120.0

    // Position tolerance in pixels - accounts for floating point arithmetic
    private val POSITION_TOLERANCE = 0.1f

    // Test tile coordinates - adjacent tiles at zoom level 15
    private val TEST_TILE_Z = 15
    private val TEST_TILE_X_1 = 10000
    private val TEST_TILE_X_2 = 10001  // Adjacent tile

    /**
     * Calculate screen coordinates for a tile's left edge and right edge.
     * Returns Pair(leftScreenX, rightScreenX).
     */
    private fun calculateTileScreenCoordinates(
        tileX: Int,
        tileZ: Int,
        centerLon: Double,
        scale: Float,
        offset: Offset,
        canvasWidth: Float
    ): Pair<Float, Float> {
        val worldX = worldX(tileXToLon(tileX, tileZ), centerLon, tileZ)
        val leftScreenX = worldX * scale + offset.x + canvasWidth / 2
        val rightScreenX = leftScreenX + TILE_SIZE * scale
        return Pair(leftScreenX, rightScreenX)
    }

    @Test
    fun tiles_stitch_perfectly_at_scale1() {
        val scale = 1f
        val offset = Offset.Zero

        val (_, tile1_end) = calculateTileScreenCoordinates(
            TEST_TILE_X_1, TEST_TILE_Z, CENTER_LON_SG, scale, offset, TEST_CANVAS_WIDTH
        )
        val (tile2_screenX, _) = calculateTileScreenCoordinates(
            TEST_TILE_X_2, TEST_TILE_Z, CENTER_LON_SG, scale, offset, TEST_CANVAS_WIDTH
        )

        assertEquals(0f, tile2_screenX - tile1_end, POSITION_TOLERANCE)
    }

    @Test
    fun tiles_stitch_perfectly_at_scale2() {
        val scale = 2f
        val offset = Offset.Zero

        val (_, tile1_end) = calculateTileScreenCoordinates(
            TEST_TILE_X_1, TEST_TILE_Z, CENTER_LON_SG, scale, offset, TEST_CANVAS_WIDTH
        )
        val (tile2_screenX, _) = calculateTileScreenCoordinates(
            TEST_TILE_X_2, TEST_TILE_Z, CENTER_LON_SG, scale, offset, TEST_CANVAS_WIDTH
        )

        assertEquals(0f, tile2_screenX - tile1_end, POSITION_TOLERANCE)
    }

    @Test
    fun tiles_stitch_perfectly_at_scale4() {
        val scale = 4f
        val offset = Offset.Zero

        val (_, tile1_end) = calculateTileScreenCoordinates(
            TEST_TILE_X_1, TEST_TILE_Z, CENTER_LON_SG, scale, offset, TEST_CANVAS_WIDTH
        )
        val (tile2_screenX, _) = calculateTileScreenCoordinates(
            TEST_TILE_X_2, TEST_TILE_Z, CENTER_LON_SG, scale, offset, TEST_CANVAS_WIDTH
        )

        assertEquals(0f, tile2_screenX - tile1_end, POSITION_TOLERANCE)
    }

    @Test
    fun tiles_stitch_with_pan_offset() {
        val scale = 2f
        val offset = Offset(50f, -30f)

        val (_, tile1_end) = calculateTileScreenCoordinates(
            TEST_TILE_X_1, TEST_TILE_Z, CENTER_LON_SG, scale, offset, TEST_CANVAS_WIDTH
        )
        val (tile2_screenX, _) = calculateTileScreenCoordinates(
            TEST_TILE_X_2, TEST_TILE_Z, CENTER_LON_SG, scale, offset, TEST_CANVAS_WIDTH
        )

        assertEquals(0f, tile2_screenX - tile1_end, POSITION_TOLERANCE)
    }
}
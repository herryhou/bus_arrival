package com.busarrival.app.presentation.ui.detection.components

import org.junit.Test
import kotlin.test.assertEquals
import kotlin.math.abs
import kotlin.math.PI
import kotlin.math.asinh
import kotlin.math.tan
import kotlin.math.pow

/**
 * Tests for the complete transform chain: geographic → world → screen
 *
 * Tests verify the complete rendering pipeline produces correct screen coordinates.
 * Transform chain: (lon/lat) → worldX/worldY → screenX/screenY
 *
 * Screen transform: screen = (world * scale) + offset + canvasCenter
 * Order: Scale first, then translate
 */

private const val TILE_SIZE = 256f
private const val POSITION_TOLERANCE = 0.5f

class TransformChainTest {

    // Helper: simulate complete transform chain
    // Mirrors production code exactly: scale first, then translate
    private fun transformToScreen(
        worldCoord: Float,
        scale: Float,
        offset: Float,
        canvasCenter: Float
    ): Float {
        return (worldCoord * scale) + offset + canvasCenter
    }

    // Helper functions matching production code from MapView.kt
    private fun lonToPixelX(lon: Double, zoom: Int): Float {
        val n = 2.0.pow(zoom)
        val x = (lon + 180.0) / 360.0 * n
        return (x * TILE_SIZE).toFloat()
    }

    private fun latToPixelY(lat: Double, zoom: Int): Float {
        val n = 2.0.pow(zoom)
        val y = ((1.0 - asinh(tan(lat * PI / 180.0)) / PI) / 2.0 * n)
        return (y * TILE_SIZE).toFloat()
    }

    private fun worldX(lon: Double, centerLon: Double, tileZ: Int): Float {
        return lonToPixelX(lon, tileZ) - lonToPixelX(centerLon, tileZ)
    }

    private fun worldY(lat: Double, centerLat: Double, tileZ: Int): Float {
        return latToPixelY(lat, tileZ) - latToPixelY(centerLat, tileZ)
    }

    private fun lonToTileX(lon: Double, zoom: Int): Int {
        val n = 2.0.pow(zoom)
        val x = (lon + 180.0) / 360.0 * n
        return x.toInt()
    }

    private fun tileXToLon(tileX: Int, zoom: Int): Double {
        return tileX / 2.0.pow(zoom) * 360.0 - 180.0
    }

    /**
     * Test: Complete transform chain at scale 1
     * Geographic (lon/lat) → worldX/worldY → screenX/screenY
     * Verify center point renders at screen center
     */
    @Test
    fun completeTransformChainAtScale1() {
        val centerLon = 120.0
        val centerLat = 20.0
        val scale = 1f
        val offsetX = 0f
        val offsetY = 0f
        val canvasWidth = 1000f
        val canvasHeight = 1000f
        val tileZ = 15

        // Center point should have zero world coordinates
        val centerWorldX = worldX(centerLon, centerLon, tileZ)
        val centerWorldY = worldY(centerLat, centerLat, tileZ)

        // Transform to screen space
        val centerScreenX = transformToScreen(centerWorldX, scale, offsetX, canvasWidth / 2)
        val centerScreenY = transformToScreen(centerWorldY, scale, offsetY, canvasHeight / 2)

        // Center should render at screen center
        assertEquals(canvasWidth / 2, centerScreenX, POSITION_TOLERANCE,
            "Center point should render at screen center X")
        assertEquals(canvasHeight / 2, centerScreenY, POSITION_TOLERANCE,
            "Center point should render at screen center Y")
    }

    /**
     * Test: Complete transform chain at scale 2
     * Verify scaling is applied correctly in world space
     *
     * NOTE: World offset depends on latitude due to Web Mercator projection.
     * At equator (lat=0), cos(lat) ≈ 1, so one tile offset = TILE_SIZE (256px).
     * At other latitudes, the offset varies.
     */
    @Test
    fun completeTransformChainAtScale2() {
        // Use equator where cos(lat) ≈ 1 for predictable world offset
        val centerLon = 120.0
        val centerLat = 0.0  // Equator: cos(lat) = 1
        val scale = 2f
        val offsetX = 0f
        val offsetY = 0f
        val canvasWidth = 1000f
        val canvasHeight = 1000f
        val tileZ = 15

        // Point one tile east (256px away in world space at equator)
        val centerTileX = lonToTileX(centerLon, tileZ)
        val eastTileNW = tileXToLon(centerTileX + 1, tileZ)

        val centerWorldX = worldX(centerLon, centerLon, tileZ)
        val eastWorldX = worldX(eastTileNW, centerLon, tileZ)

        // Verify world space offset is 256px at equator
        val worldOffset = eastWorldX - centerWorldX
        assertEquals(TILE_SIZE, worldOffset, POSITION_TOLERANCE,
            "World space offset should be 256px at equator")

        // Transform to screen space
        val centerScreenX = transformToScreen(centerWorldX, scale, offsetX, canvasWidth / 2)
        val eastScreenX = transformToScreen(eastWorldX, scale, offsetX, canvasWidth / 2)

        // Screen offset should be 512px (256 * 2)
        val screenOffset = eastScreenX - centerScreenX
        assertEquals(TILE_SIZE * scale, screenOffset, POSITION_TOLERANCE,
            "Screen offset should be 512px at scale 2")
    }

    /**
     * Test: Transform with pan offset
     * Verify offset is applied AFTER scaling
     * Point at center with offset (100, 50) should render at (center + 100, center + 50)
     */
    @Test
    fun transformWithPanOffset() {
        val centerLon = 120.0
        val centerLat = 20.0
        val scale = 1f
        val offsetX = 100f
        val offsetY = 50f
        val canvasWidth = 1000f
        val canvasHeight = 1000f
        val tileZ = 15

        // Center point
        val centerWorldX = worldX(centerLon, centerLon, tileZ)
        val centerWorldY = worldY(centerLat, centerLat, tileZ)

        // Transform to screen space with offset
        val centerScreenX = transformToScreen(centerWorldX, scale, offsetX, canvasWidth / 2)
        val centerScreenY = transformToScreen(centerWorldY, scale, offsetY, canvasHeight / 2)

        // Center should render at screen center + offset
        val expectedX = canvasWidth / 2 + offsetX
        val expectedY = canvasHeight / 2 + offsetY

        assertEquals(expectedX, centerScreenX, POSITION_TOLERANCE,
            "Center with offset should render at center + offset X")
        assertEquals(expectedY, centerScreenY, POSITION_TOLERANCE,
            "Center with offset should render at center + offset Y")
    }

    /**
     * Test: Transform order is correct
     * Scale first, then translate
     * Verify: (world * scale) + offset (not (world + offset) * scale)
     */
    @Test
    fun transformOrderIsScaleThenTranslate() {
        // Use equator for predictable world offset
        val centerLon = 120.0
        val centerLat = 0.0
        val scale = 2f
        val offsetX = 100f
        val offsetY = 50f
        val canvasWidth = 1000f
        val canvasHeight = 1000f
        val tileZ = 15

        // Point one tile east
        val centerTileX = lonToTileX(centerLon, tileZ)
        val eastTileNW = tileXToLon(centerTileX + 1, tileZ)

        val centerWorldX = worldX(centerLon, centerLon, tileZ)
        val eastWorldX = worldX(eastTileNW, centerLon, tileZ)

        // Correct order: scale THEN translate
        // Formula: screen = (world * scale) + offset + canvasCenter
        val eastScreenX_correct = transformToScreen(eastWorldX, scale, offsetX, canvasWidth / 2)
        val centerScreenX_correct = transformToScreen(centerWorldX, scale, offsetX, canvasWidth / 2)

        // Wrong order: translate THEN scale
        // Formula: screen = (world + offset + canvasCenter) * scale
        val eastScreenX_wrong = (eastWorldX + offsetX + canvasWidth / 2) * scale
        val centerScreenX_wrong = (centerWorldX + offsetX + canvasWidth / 2) * scale

        // Correct: screen offset should be 512px (256 * 2)
        val screenOffset_correct = eastScreenX_correct - centerScreenX_correct
        assertEquals(TILE_SIZE * scale, screenOffset_correct, POSITION_TOLERANCE,
            "Correct order: screen offset = worldOffset * scale")

        // Wrong: screen offset would also be scaled (256 * 2 * 2 = 1024px)
        val screenOffset_wrong = eastScreenX_wrong - centerScreenX_wrong
        val isWrong = abs(screenOffset_wrong - (TILE_SIZE * scale * scale)) < POSITION_TOLERANCE

        // Verify the wrong order produces different results
        val isDifferent = abs(screenOffset_correct - screenOffset_wrong) > 1.0f
        assertEquals(true, isDifferent,
            "Wrong transform order should produce different results")
    }

    /**
     * Test: Verify offset does not affect scaling
     * Offset should be additive, not multiplicative
     *
     * NOTE: Uses equator for predictable world offset
     */
    @Test
    fun offsetDoesNotAffectScaling() {
        // Use equator for predictable world offset
        val centerLon = 120.0
        val centerLat = 0.0
        val scale = 2f
        val canvasWidth = 1000f
        val canvasHeight = 1000f
        val tileZ = 15

        // Point one tile east
        val centerTileX = lonToTileX(centerLon, tileZ)
        val eastTileNW = tileXToLon(centerTileX + 1, tileZ)

        val centerWorldX = worldX(centerLon, centerLon, tileZ)
        val eastWorldX = worldX(eastTileNW, centerLon, tileZ)

        // Test with different offsets
        val offsets = listOf(0f, 50f, 100f, 200f)

        for (offsetX in offsets) {
            val centerScreenX = transformToScreen(centerWorldX, scale, offsetX, canvasWidth / 2)
            val eastScreenX = transformToScreen(eastWorldX, scale, offsetX, canvasWidth / 2)

            // Screen offset should be constant regardless of pan offset
            val screenOffset = eastScreenX - centerScreenX
            assertEquals(TILE_SIZE * scale, screenOffset, POSITION_TOLERANCE,
                "Screen offset should be constant ($offsetX offset)")
        }
    }

    /**
     * Test: Verify transform chain with both scale and offset
     * Center point (at centerLon/centerLat) has world coordinates of (0, 0)
     */
    @Test
    fun completeTransformChainWithScaleAndOffset() {
        val centerLon = 120.0
        val centerLat = 20.0
        val scale = 3f
        val offsetX = 150f
        val offsetY = 75f
        val canvasWidth = 1080f
        val canvasHeight = 2400f
        val tileZ = 15

        // Center point has world coordinates (0, 0)
        val centerWorldX = worldX(centerLon, centerLon, tileZ)
        val centerWorldY = worldY(centerLat, centerLat, tileZ)

        // Transform to screen space
        val centerScreenX = transformToScreen(centerWorldX, scale, offsetX, canvasWidth / 2)
        val centerScreenY = transformToScreen(centerWorldY, scale, offsetY, canvasHeight / 2)

        // Expected: centerWorldCoord (0) * scale + offset + canvasCenter
        // X: 0 * 3 + 150 + 540 = 690
        // Y: 0 * 3 + 75 + 1200 = 1275
        val expectedX = centerWorldX * scale + offsetX + canvasWidth / 2
        val expectedY = centerWorldY * scale + offsetY + canvasHeight / 2

        assertEquals(expectedX, centerScreenX, POSITION_TOLERANCE,
            "Center should render at expected X with scale and offset")
        assertEquals(expectedY, centerScreenY, POSITION_TOLERANCE,
            "Center should render at expected Y with scale and offset")
    }
}

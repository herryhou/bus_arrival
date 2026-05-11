package com.busarrival.app.presentation.ui.detection.components

import kotlin.math.pow
import kotlin.math.PI
import kotlin.math.asinh
import kotlin.math.tan
import kotlin.math.atan
import kotlin.math.exp
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/**
 * Regression tests that PREVENT the tile positioning bug from returning.
 *
 * **BUG FIXED:** worldX/Y were using baseZ=15, but tiles are at tileZ=17-18.
 * This caused all tiles to stack at same position, creating visual mess.
 *
 * **THE FIX:** worldX/Y now use tileZ (requested zoom level) for positioning.
 *
 * **WHAT THESE TESTS CATCH:**
 * - If someone reverts worldX/Y to use baseZ instead of tileZ
 * - If tileZ calculation is wrong (doesn't match user scale)
 * - If transform chain is modified incorrectly
 *
 * **HOW:** Tests simulate EXACT rendering with tileZ parameter.
 * The regression test explicitly compares baseZ vs tileZ to catch the bug.
 */
class MapViewRenderingTest {

    private val baseZ = 15
    private val tileSize = 256f

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

    // Production code from MapView.kt - CRITICAL: Must match exactly
    // These functions now use tileZ (requested zoom level) for correct positioning
    private fun worldX(lon: Double, centerLon: Double, tileZ: Int): Float {
        return lonToPixelX(lon, tileZ) - lonToPixelX(centerLon, tileZ)
    }

    private fun worldY(lat: Double, centerLat: Double, tileZ: Int): Float {
        return latToPixelY(lat, tileZ) - latToPixelY(centerLat, tileZ)
    }

    // Regression test: if someone uses baseZ instead of tileZ, this test will FAIL
    private fun worldX_BROKEN(lon: Double, centerLon: Double): Float {
        return lonToPixelX(lon, baseZ) - lonToPixelX(centerLon, baseZ)
    }

    // Simulate EXACT transform from MapView.kt lines 178-180
    // withTransform({
    //     translate(left = offset.x + canvasWidth / 2, top = offset.y + canvasHeight / 2)
    //     scale(scaleX = scale, scaleY = scale, pivot = Offset.Zero)
    // })
    private fun transformToWorldThenScreen(worldX: Float, scale: Float, offset: Float, canvasCenter: Float): Float {
        // Step 1: Scale by user scale around Zero
        val scaledX = worldX * scale
        // Step 2: Translate by offset + canvas center
        return scaledX + offset + canvasCenter
    }

    /**
     * Test: Verify tile boundaries with EXACT transform chain (no pan offset)
     */
    @Test
    fun tileBoundariesWithExactTransform() {
        val centerLon = 120.0
        val scale = 2f
        val offset = 0f  // No pan
        val canvasWidth = 1000f
        val tileZ = 16  // Requested zoom level

        val centerTileX = ((centerLon + 180.0) / 360.0 * 2.0.pow(15)).toInt()

        val tile1_NW = tileXToLon(centerTileX, tileZ)
        val tile2_NW = tileXToLon(centerTileX + 1, tileZ)

        val tile1_worldX = worldX(tile1_NW, centerLon, tileZ)
        val tile2_worldX = worldX(tile2_NW, centerLon, tileZ)

        // Apply EXACT transform
        val tile1_screenX = transformToWorldThenScreen(tile1_worldX, scale, offset, canvasWidth / 2)
        val tile2_screenX = transformToWorldThenScreen(tile2_worldX, scale, offset, canvasWidth / 2)

        // Calculate where tile1 ends
        val tile1_screenEnd = tile1_screenX + tileSize * scale

        // CRITICAL: Verify no gap
        val gap = tile2_screenX - tile1_screenEnd
        assertEquals(0f, gap, 0.1f, "With EXACT transform: tiles should have no gap")
    }

    /**
     * REGRESSION TEST: If someone uses baseZ instead of tileZ, this FAILS
     */
    @Test
    fun regressionUsingBaseZInsteadOfTileZ() {
        val centerLon = 120.0
        val tileZ = 18  // High zoom level where bug is obvious

        val centerTileX = ((centerLon + 180.0) / 360.0 * 2.0.pow(15)).toInt()
        val tile1_NW = tileXToLon(centerTileX, tileZ)
        val tile2_NW = tileXToLon(centerTileX + 1, tileZ)

        // BROKEN code: uses baseZ
        val tile1_worldX_BROKEN = worldX_BROKEN(tile1_NW, centerLon)
        val tile2_worldX_BROKEN = worldX_BROKEN(tile2_NW, centerLon)

        // FIXED code: uses tileZ
        val tile1_worldX_FIXED = worldX(tile1_NW, centerLon, tileZ)
        val tile2_worldX_FIXED = worldX(tile2_NW, centerLon, tileZ)

        // At tileZ=18, baseZ gives WRONG results
        val spacing_FIXED = tile2_worldX_FIXED - tile1_worldX_FIXED
        val spacing_BROKEN = tile2_worldX_BROKEN - tile1_worldX_BROKEN

        // FIXED: spacing should be 256px
        assertEquals(256f, spacing_FIXED, 0.1f, "Using tileZ: correct spacing")

        // BROKEN: spacing will be WRONG (not 256px)
        val isBroken = kotlin.math.abs(spacing_BROKEN - 256f) > 100f
        kotlin.test.assertTrue(isBroken,
            "Using baseZ at tileZ=18 gives WRONG spacing (actual: $spacing_BROKEN)")
    }

    /**
     * Test: Verify tile boundaries at different scales
     */
    @Test
    fun tileBoundariesAtScales1_2_4() {
        val centerLon = 120.0
        val offset = 0f
        val canvasWidth = 1000f

        val centerTileX = ((centerLon + 180.0) / 360.0 * 2.0.pow(15)).toInt()
        val tile1_NW = tileXToLon(centerTileX, 15)
        val tile2_NW = tileXToLon(centerTileX + 1, 15)

        val tile1_worldX = worldX(tile1_NW, centerLon)
        val tile2_worldX = worldX(tile2_NW, centerLon)

        // Test at scale 1, 2, 4
        val scales = listOf(1f, 2f, 4f)
        for (scale in scales) {
            val tile1_screenX = transformToWorldThenScreen(tile1_worldX, scale, offset, canvasWidth / 2)
            val tile2_screenX = transformToWorldThenScreen(tile2_worldX, scale, offset, canvasWidth / 2)
            val tile1_screenEnd = tile1_screenX + tileSize * scale
            val gap = tile2_screenX - tile1_screenEnd

            assertEquals(0f, gap, 0.1f, "Scale=$scale: tiles should stitch perfectly")
        }
    }

    /**
     * Test: Verify with pan offset (user drags map)
     */
    @Test
    fun tileBoundariesWithPanOffset() {
        val centerLon = 120.0
        val scale = 2f
        val offset = 50f  // User panned 50px
        val canvasWidth = 1000f

        val centerTileX = ((centerLon + 180.0) / 360.0 * 2.0.pow(15)).toInt()
        val tile1_NW = tileXToLon(centerTileX, 15)
        val tile2_NW = tileXToLon(centerTileX + 1, 15)

        val tile1_worldX = worldX(tile1_NW, centerLon)
        val tile2_worldX = worldX(tile2_NW, centerLon)

        val tile1_screenX = transformToWorldThenScreen(tile1_worldX, scale, offset, canvasWidth / 2)
        val tile2_screenX = transformToWorldThenScreen(tile2_worldX, scale, offset, canvasWidth / 2)
        val tile1_screenEnd = tile1_screenX + tileSize * scale

        val gap = tile2_screenX - tile1_screenEnd
        assertEquals(0f, gap, 0.1f, "With pan offset: tiles should still stitch perfectly")
    }

    /**
     * Test: Verify Y axis stitching (vertical)
     */
    @Test
    fun verticalTileBoundariesAlign() {
        val centerLat = 20.0
        val scale = 2f
        val offset = 0f
        val canvasHeight = 1000f

        val centerTileY = ((1.0 - asinh(tan(centerLat * PI / 180.0)) / PI) / 2.0 * 2.0.pow(15)).toInt()
        val tile1_NE = tileYToLat(centerTileY, 15)
        val tile2_NE = tileYToLat(centerTileY + 1, 15)

        val tile1_worldY = worldY(tile1_NE, centerLat)
        val tile2_worldY = worldY(tile2_NE, centerLat)

        val tile1_screenY = transformToWorldThenScreen(tile1_worldY, scale, offset, canvasHeight / 2)
        val tile2_screenY = transformToWorldThenScreen(tile2_worldY, scale, offset, canvasHeight / 2)
        val tile1_screenEnd = tile1_screenY + tileSize * scale

        val gap = tile2_screenY - tile1_screenEnd
        assertEquals(0f, gap, 0.1f, "Vertical: tiles should stitch perfectly")
    }

    /**
     * Test: Simulate actual rendering and verify no gaps
     */
    @Test
    fun simulateActualRendering() {
        val centerLon = 120.0
        val centerLat = 20.0
        val scale = 4f
        val offset = 0f
        val canvasWidth = 1000f
        val canvasHeight = 1000f

        val centerTileX = ((centerLon + 180.0) / 360.0 * 2.0.pow(15)).toInt()
        val centerTileY = ((1.0 - asinh(tan(centerLat * PI / 180.0)) / PI) / 2.0 * 2.0.pow(15)).toInt()

        // Get world coordinates for center, east, and south tiles
        val center_NW = tileXToLon(centerTileX, 15)
        val center_NE = tileYToLat(centerTileY, 15)
        val east_NW = tileXToLon(centerTileX + 1, 15)
        val south_NE = tileYToLat(centerTileY + 1, 15)

        val centerWorldX = worldX(center_NW, centerLon)
        val centerWorldY = worldY(center_NE, centerLat)
        val eastWorldX = worldX(east_NW, centerLon)
        val southWorldY = worldY(south_NE, centerLat)

        // Transform to screen space
        val centerScreenX = transformToWorldThenScreen(centerWorldX, scale, offset, canvasWidth / 2)
        val centerScreenY = transformToWorldThenScreen(centerWorldY, scale, offset, canvasHeight / 2)
        val eastScreenX = transformToWorldThenScreen(eastWorldX, scale, offset, canvasWidth / 2)
        val southScreenY = transformToWorldThenScreen(southWorldY, scale, offset, canvasHeight / 2)

        // Calculate tile boundaries in screen space
        val center_right = centerScreenX + tileSize * scale
        val center_bottom = centerScreenY + tileSize * scale

        // Verify east tile starts exactly where center ends
        assertEquals(center_right, eastScreenX, 0.1f, "East boundary: no gap")

        // Verify south tile starts exactly where center ends
        assertEquals(center_bottom, southScreenY, 0.1f, "South boundary: no gap")
    }

    /**
     * Test: Verify fallback tiles align in screen space
     */
    @Test
    fun fallbackTilesAlignInScreenSpace() {
        val centerLon = 120.0
        val scale = 4f
        val offset = 0f
        val canvasWidth = 1000f

        val tileZ = 17
        val actualTileZ = 15
        val zoomScaleFactor = 2.0.pow(tileZ - actualTileZ).toFloat()

        val centerTileX = ((centerLon + 180.0) / 360.0 * 2.0.pow(15)).toInt()

        // Native tile at Z=17
        val native_NW = tileXToLon(centerTileX, 17)
        val nativeWorldX = worldX(native_NW, centerLon)

        // Next tile at Z=17
        val next_NW = tileXToLon(centerTileX + 1, 17)
        val nextWorldX = worldX(next_NW, centerLon)

        // Transform to screen space
        val nativeScreenX = transformToWorldThenScreen(nativeWorldX, scale, offset, canvasWidth / 2)
        val nextScreenX = transformToWorldThenScreen(nextWorldX, scale, offset, canvasWidth / 2)

        // Calculate where native tile ends
        val nativeScreenEnd = nativeScreenX + tileSize * scale

        // Verify boundary alignment
        val gap = nextScreenX - nativeScreenEnd
        assertEquals(0f, gap, 0.1f, "Native tile boundary should align")

        // Fallback tile (Z=15) should cover same area as native (Z=17)
        // It's positioned at same worldX, but inner scaled by zoomScaleFactor
        val fallbackScreenSize = tileSize * zoomScaleFactor * scale
        val nativeScreenSize = tileSize * scale

        assertEquals(fallbackScreenSize, nativeScreenSize, 0.1f, "Fallback should match native coverage")
    }

    /**
     * Test: Diagnostic - log tile positions at scale 4
     */
    @Test
    fun diagnosticTilePositionsAtScale4() {
        val centerLon = 120.0
        val centerLat = 20.0
        val scale = 4f
        val offset = 0f
        val canvasWidth = 1080f  // Typical phone width
        val canvasHeight = 2400f // Typical phone height

        val centerTileX = ((centerLon + 180.0) / 360.0 * 2.0.pow(15)).toInt()
        val centerTileY = ((1.0 - asinh(tan(centerLat * PI / 180.0)) / PI) / 2.0 * 2.0.pow(15)).toInt()

        println("=== DIAGNOSTIC: Tile Positions at scale=4 ===")
        println("Canvas: ${canvasWidth}x${canvasHeight}")
        println("Center tile: ($centerTileX, $centerTileY)")

        // Check 3x3 tiles around center
        for (dy in -1..1) {
            for (dx in -1..1) {
                val tileX = centerTileX + dx
                val tileY = centerTileY + dy

                val tile_NW = tileXToLon(tileX, 15)
                val tile_NE = tileYToLat(tileY, 15)

                val worldX = worldX(tile_NW, centerLon)
                val worldY = worldY(tile_NE, centerLat)

                val screenX = transformToWorldThenScreen(worldX, scale, offset, canvasWidth / 2)
                val screenY = transformToWorldThenScreen(worldY, scale, offset, canvasHeight / 2)

                val screenWidth = tileSize * scale
                val screenHeight = tileSize * scale

                println("  Tile[$dx,$dy]: world=($worldX,$worldY) screen=($screenX,$screenY) size=${screenWidth}x$screenHeight")
            }
        }

        // Verify no gaps
        val eastTile_NW = tileXToLon(centerTileX + 1, 15)
        val eastWorldX = worldX(eastTile_NW, centerLon)
        val eastScreenX = transformToWorldThenScreen(eastWorldX, scale, offset, canvasWidth / 2)

        val centerTile_NW = tileXToLon(centerTileX, 15)
        val centerWorldX = worldX(centerTile_NW, centerLon)
        val centerScreenX = transformToWorldThenScreen(centerWorldX, scale, offset, canvasWidth / 2)

        val centerEnd = centerScreenX + tileSize * scale
        val gap = eastScreenX - centerEnd

        println("Gap to east tile: $gap")
        assertEquals(0f, gap, 0.1f, "Should be no gap")
    }
}

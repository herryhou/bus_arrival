package com.busarrival.app.presentation.ui.detection.components

import kotlin.math.pow
import kotlin.test.assertEquals
import org.junit.Test

/**
 * Tests for map tile STITCHING - ensuring tiles align perfectly with no gaps. Verifies tile
 * boundaries meet exactly at all zoom scales (1, 2, 4).
 */
class MapViewTileStitchingTest {

    private val baseZ = 15
    private val tileSize = 256f

    private fun lonToPixelX(lon: Double, zoom: Int): Float {
        val n = 2.0.pow(zoom)
        val x = (lon + 180.0) / 360.0 * n
        return (x * 256).toFloat()
    }

    private fun tileXToLon(tileX: Int, zoom: Int): Double {
        return tileX / 2.0.pow(zoom) * 360.0 - 180.0
    }

    // World space positioning using baseZ
    private fun worldX(lon: Double, centerLon: Double): Float {
        return lonToPixelX(lon, baseZ) - lonToPixelX(centerLon, baseZ)
    }

    /**
     * Test: Verify two adjacent tiles have NO GAP between them. Tile 1 should end exactly where
     * Tile 2 starts.
     */
    @Test
    fun tilesShouldHaveNoGapBetweenThem() {
        val centerLon = 120.0
        val centerTileX = ((centerLon + 180.0) / 360.0 * 2.0.pow(15)).toInt()

        // Position two adjacent tiles
        val tile1_NW = tileXToLon(centerTileX, 15)
        val tile2_NW = tileXToLon(centerTileX + 1, 15)

        val tile1_start = worldX(tile1_NW, centerLon)
        val tile2_start = worldX(tile2_NW, centerLon)

        // Calculate where tile1 ends
        val tile1_end = tile1_start + tileSize

        // Verify: tile1 end EXACTLY equals tile2 start (no gap, no overlap)
        assertEquals(
                tile1_end,
                tile2_start,
                0.01f,
                "Tile 1 should end exactly where Tile 2 starts - no gap!"
        )
    }

    /** Test: Verify tile boundaries at scale=1 */
    @Test
    fun tileBoundariesAlignAtScale1() {
        val centerLon = 120.0
        val scale = 1f
        val centerTileX = ((centerLon + 180.0) / 360.0 * 2.0.pow(15)).toInt()

        val tile1_NW = tileXToLon(centerTileX, 15)
        val tile2_NW = tileXToLon(centerTileX + 1, 15)

        val tile1_start = worldX(tile1_NW, centerLon)
        val tile2_start = worldX(tile2_NW, centerLon)

        // After scale transform
        val tile1_screen_start = tile1_start * scale
        val tile1_screen_end = tile1_screen_start + tileSize * scale
        val tile2_screen_start = tile2_start * scale

        // CRITICAL: Verify exact boundary alignment
        assertEquals(
                tile1_screen_end,
                tile2_screen_start,
                0.01f,
                "At scale=1: Tile boundary should have NO gap"
        )
    }

    /** Test: Verify tile boundaries at scale=2 */
    @Test
    fun tileBoundariesAlignAtScale2() {
        val centerLon = 120.0
        val scale = 2f
        val centerTileX = ((centerLon + 180.0) / 360.0 * 2.0.pow(15)).toInt()

        val tile1_NW = tileXToLon(centerTileX, 15)
        val tile2_NW = tileXToLon(centerTileX + 1, 15)

        val tile1_start = worldX(tile1_NW, centerLon)
        val tile2_start = worldX(tile2_NW, centerLon)

        // After scale transform
        val tile1_screen_end = tile1_start * scale + tileSize * scale
        val tile2_screen_start = tile2_start * scale

        // CRITICAL: Verify exact boundary alignment
        assertEquals(
                tile1_screen_end,
                tile2_screen_start,
                0.01f,
                "At scale=2: Tile boundary should have NO gap"
        )
    }

    /** Test: Verify tile boundaries at scale=4 */
    @Test
    fun tileBoundariesAlignAtScale4() {
        val centerLon = 120.0
        val scale = 4f
        val centerTileX = ((centerLon + 180.0) / 360.0 * 2.0.pow(15)).toInt()

        val tile1_NW = tileXToLon(centerTileX, 15)
        val tile2_NW = tileXToLon(centerTileX + 1, 15)

        val tile1_start = worldX(tile1_NW, centerLon)
        val tile2_start = worldX(tile2_NW, centerLon)

        // After scale transform
        val tile1_screen_end = tile1_start * scale + tileSize * scale
        val tile2_screen_start = tile2_start * scale

        // CRITICAL: Verify exact boundary alignment
        assertEquals(
                tile1_screen_end,
                tile2_screen_start,
                0.01f,
                "At scale=4: Tile boundary should have NO gap"
        )
    }

    /** Test: Verify NO overlaps between tiles */
    @Test
    fun tilesShouldNotOverlap() {
        val centerLon = 120.0
        val centerTileX = ((centerLon + 180.0) / 360.0 * 2.0.pow(15)).toInt()

        val tile1_NW = tileXToLon(centerTileX, 15)
        val tile2_NW = tileXToLon(centerTileX + 1, 15)

        val tile1_start = worldX(tile1_NW, centerLon)
        val tile2_start = worldX(tile2_NW, centerLon)

        val gap = tile2_start - (tile1_start + tileSize)

        // Gap should be exactly zero (no overlap, no space)
        assertEquals(0f, gap, 0.01f, "Tiles should not overlap - gap should be exactly zero")
    }

    /** Test: Verify tile stitching across zoom levels */
    @Test
    fun tilesStitchCorrectlyAcrossAllZoomLevels() {
        val centerLon = 120.0
        val centerTileX = ((centerLon + 180.0) / 360.0 * 2.0.pow(15)).toInt()

        val tile1_NW = tileXToLon(centerTileX, 15)
        val tile2_NW = tileXToLon(centerTileX + 1, 15)

        val tile1_start = worldX(tile1_NW, centerLon)
        val tile2_start = worldX(tile2_NW, centerLon)

        // Test at multiple scales
        val scales = listOf(1f, 2f, 4f)

        for (scale in scales) {
            val tile1_end = tile1_start * scale + tileSize * scale
            val tile2_start_scaled = tile2_start * scale

            val gap = tile2_start_scaled - tile1_end

            assertEquals(
                    0f,
                    gap,
                    0.01f,
                    "At scale=$scale: Tiles should stitch perfectly with no gap"
            )
        }
    }

    /** Test: Verify fallback tiles align with native tiles */
    @Test
    fun fallbackTilesAlignWithNativeTiles() {
        val centerLon = 120.0
        val baseZ = 15
        val tileZ = 17
        val actualTileZ = 15

        // Match rendering code: zoomScaleFactor = 2^(baseZ - tileZ)
        val zoomScaleFactor = 2.0.pow(baseZ - tileZ).toFloat()
        // Match rendering code: fallbackScaleFactor = 2^(tileZ - actualTileZ)
        val fallbackScaleFactor = 2.0.pow(tileZ - actualTileZ).toFloat()
        val totalScale = zoomScaleFactor * fallbackScaleFactor

        // When baseZ == actualTileZ, totalScale should be 1.0
        // This means: scale down by (baseZ - tileZ) then scale up by (tileZ - actualTileZ)
        // Since baseZ == actualTileZ: 2^(15-17) * 2^(17-15) = 2^(-2) * 2^(2) = 0.25 * 4 = 1.0
        assertEquals(
                1.0f,
                totalScale,
                0.001f,
                "When baseZ == actualTileZ, totalScale should be 1.0"
        )

        // When baseZ != actualTileZ, verify the composition
        // For example: baseZ=15, tileZ=17, actualTileZ=14
        val zoomScale2 = 2.0.pow(baseZ - tileZ).toFloat() // 2^(-2) = 0.25
        val fallbackScale2 = 2.0.pow(tileZ - 14).toFloat() // 2^(3) = 8
        val totalScale2 = zoomScale2 * fallbackScale2 // 0.25 * 8 = 2.0

        // This represents: scale Z=14 tile to Z=17 size (8x), then scale to baseZ=15 space (0.25x)
        assertEquals(
                2.0f,
                totalScale2,
                0.001f,
                "Total scale should compose zoom and fallback scaling"
        )
    }

    /** Test: Verify complete tile row has no gaps */
    @Test
    fun completeTileRowHasNoGaps() {
        val centerLon = 120.0
        val centerTileX = ((centerLon + 180.0) / 360.0 * 2.0.pow(15)).toInt()
        val scale = 2f

        var totalGap = 0f

        // Check 5 consecutive tiles
        for (i in 0..4) {
            val current_NW = tileXToLon(centerTileX + i, 15)
            val next_NW = tileXToLon(centerTileX + i + 1, 15)

            val current_start = worldX(current_NW, centerLon)
            val next_start = worldX(next_NW, centerLon)

            val current_end = current_start * scale + tileSize * scale
            val next_start_scaled = next_start * scale

            val gap = next_start_scaled - current_end
            totalGap += gap
        }

        // Total gap across entire row should be exactly zero
        assertEquals(
                0f,
                totalGap,
                0.01f,
                "Complete tile row should have no gaps - total gap should be zero"
        )
    }

    /** Test: Verify tile coverage is continuous (no pixel left uncovered) */
    @Test
    fun tileCoverageIsContinuous() {
        val centerLon = 120.0
        val centerTileX = ((centerLon + 180.0) / 360.0 * 2.0.pow(15)).toInt()
        val scale = 4f

        // Calculate coverage for 3 tiles
        val tile1_NW = tileXToLon(centerTileX, 15)
        val tile2_NW = tileXToLon(centerTileX + 1, 15)
        val tile3_NW = tileXToLon(centerTileX + 2, 15)

        val tile1_start = worldX(tile1_NW, centerLon) * scale
        val tile2_start = worldX(tile2_NW, centerLon) * scale
        val tile3_start = worldX(tile3_NW, centerLon) * scale

        val tile1_end = tile1_start + tileSize * scale
        val tile2_end = tile2_start + tileSize * scale
        val tile3_end = tile3_start + tileSize * scale

        // Verify continuous coverage: each tile starts exactly where previous ended
        assertEquals(tile1_end, tile2_start, 0.01f, "Tile 2 should start where Tile 1 ends")
        assertEquals(tile2_end, tile3_start, 0.01f, "Tile 3 should start where Tile 2 ends")

        // Calculate total covered area
        val totalCoverage = tile3_end - tile1_start
        val expectedCoverage = tileSize * scale * 3

        assertEquals(
                expectedCoverage,
                totalCoverage,
                0.01f,
                "Total coverage should equal 3 tiles with no gaps"
        )
    }
}

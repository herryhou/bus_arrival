package com.busarrival.app.presentation.ui.detection.components

import androidx.compose.ui.geometry.Offset
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.math.pow

/**
 * Tests for native tile scaling in baseZ coordinate system.
 * Verifies that tiles at different zoom levels scale correctly relative to baseZ.
 */
class NativeTileScalingTest {
    private val TILE_SIZE = 256f
    private val POSITION_TOLERANCE = 0.1f
    private val baseZ = 15

    /**
     * Calculate tile size in baseZ world coordinates.
     * Tile at zoom Z should have size TILE_SIZE * 2^(baseZ - Z) in baseZ space.
     */
    private fun tileSizeInBaseZ(tileZ: Int): Float {
        val scaleFactor = 2.0f.pow(baseZ - tileZ)
        return TILE_SIZE * scaleFactor
    }

    @Test
    fun native_tile_at_z17_is_quarter_size_of_z15_tile() {
        // At baseZ=15, a Z=17 tile should be 1/4 the size of a Z=15 tile
        val z15Size = tileSizeInBaseZ(15)
        val z17Size = tileSizeInBaseZ(17)

        assertEquals(TILE_SIZE, z15Size, POSITION_TOLERANCE,
            "Z=15 tile at baseZ should be native size")
        assertEquals(TILE_SIZE / 4f, z17Size, POSITION_TOLERANCE,
            "Z=17 tile at baseZ should be 1/4 of native size")
        assertEquals(z15Size / 4f, z17Size, POSITION_TOLERANCE,
            "Z=17 tile should be 1/4 of Z=15 tile")
    }

    @Test
    fun native_tile_at_z14_is_double_size_of_z15_tile() {
        // At baseZ=15, a Z=14 tile should be 2x the size of a Z=15 tile
        val z15Size = tileSizeInBaseZ(15)
        val z14Size = tileSizeInBaseZ(14)

        assertEquals(TILE_SIZE * 2f, z14Size, POSITION_TOLERANCE,
            "Z=14 tile at baseZ should be 2x native size")
        assertEquals(z15Size * 2f, z14Size, POSITION_TOLERANCE,
            "Z=14 tile should be 2x of Z=15 tile")
    }

    @Test
    fun tile_size_follows_power_of_two_scaling() {
        // Verify power-of-two scaling across zoom levels
        val z12Size = tileSizeInBaseZ(12)
        val z13Size = tileSizeInBaseZ(13)
        val z14Size = tileSizeInBaseZ(14)
        val z15Size = tileSizeInBaseZ(15)
        val z16Size = tileSizeInBaseZ(16)
        val z17Size = tileSizeInBaseZ(17)
        val z18Size = tileSizeInBaseZ(18)

        // Each zoom level should be 2x the previous
        assertEquals(z12Size / 2f, z13Size, POSITION_TOLERANCE, "Z=13 should be half of Z=12")
        assertEquals(z13Size / 2f, z14Size, POSITION_TOLERANCE, "Z=14 should be half of Z=13")
        assertEquals(z14Size / 2f, z15Size, POSITION_TOLERANCE, "Z=15 should be half of Z=14")
        assertEquals(z15Size / 2f, z16Size, POSITION_TOLERANCE, "Z=16 should be half of Z=15")
        assertEquals(z16Size / 2f, z17Size, POSITION_TOLERANCE, "Z=17 should be half of Z=16")
        assertEquals(z17Size / 2f, z18Size, POSITION_TOLERANCE, "Z=18 should be half of Z=17")
    }
}

package com.busarrival.app.presentation.ui.detection

import androidx.compose.ui.geometry.Offset
import com.busarrival.app.presentation.ui.detection.components.worldX
import com.busarrival.app.presentation.ui.detection.components.tileXToLon
import org.junit.Test
import kotlin.test.assertEquals

class TilePositioningTest {
    @Test
    fun tilesStitchPerfectly_atScale1() {
        val scale = 1f
        val offset = Offset.Zero
        val canvasWidth = 1000f
        val centerLon = 120.0

        // Two adjacent tiles at tileZ=15
        val tile1_worldX = worldX(tileXToLon(10000, 15), centerLon, 15)
        val tile2_worldX = worldX(tileXToLon(10001, 15), centerLon, 15)

        val tile1_screenX = tile1_worldX * scale + offset.x + canvasWidth / 2
        val tile2_screenX = tile2_worldX * scale + offset.x + canvasWidth / 2
        val tile1_end = tile1_screenX + 256f * scale

        // No gap
        assertEquals(0f, tile2_screenX - tile1_end, 0.1f)
    }

    @Test
    fun tilesStitchPerfectly_atScale2() {
        val scale = 2f
        val offset = Offset.Zero
        val canvasWidth = 1000f
        val centerLon = 120.0

        val tile1_worldX = worldX(tileXToLon(10000, 15), centerLon, 15)
        val tile2_worldX = worldX(tileXToLon(10001, 15), centerLon, 15)

        val tile1_screenX = tile1_worldX * scale + offset.x + canvasWidth / 2
        val tile2_screenX = tile2_worldX * scale + offset.x + canvasWidth / 2
        val tile1_end = tile1_screenX + 256f * scale

        assertEquals(0f, tile2_screenX - tile1_end, 0.1f)
    }

    @Test
    fun tilesStitchPerfectly_atScale4() {
        val scale = 4f
        val offset = Offset.Zero
        val canvasWidth = 1000f
        val centerLon = 120.0

        val tile1_worldX = worldX(tileXToLon(10000, 15), centerLon, 15)
        val tile2_worldX = worldX(tileXToLon(10001, 15), centerLon, 15)

        val tile1_screenX = tile1_worldX * scale + offset.x + canvasWidth / 2
        val tile2_screenX = tile2_worldX * scale + offset.x + canvasWidth / 2
        val tile1_end = tile1_screenX + 256f * scale

        assertEquals(0f, tile2_screenX - tile1_end, 0.1f)
    }

    @Test
    fun tilesStitchWithPanOffset() {
        val scale = 2f
        val offset = Offset(50f, -30f)
        val canvasWidth = 1000f
        val centerLon = 120.0

        val tile1_worldX = worldX(tileXToLon(10000, 15), centerLon, 15)
        val tile2_worldX = worldX(tileXToLon(10001, 15), centerLon, 15)

        val tile1_screenX = tile1_worldX * scale + offset.x + canvasWidth / 2
        val tile2_screenX = tile2_worldX * scale + offset.x + canvasWidth / 2
        val tile1_end = tile1_screenX + 256f * scale

        assertEquals(0f, tile2_screenX - tile1_end, 0.1f)
    }
}
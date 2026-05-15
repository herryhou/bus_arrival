package com.busarrival.app.presentation.ui.detection

import com.busarrival.app.presentation.ui.detection.components.worldX
import com.busarrival.app.presentation.ui.detection.components.tileXToLon
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotEquals

class CoordinateSystemTest {
    @Test
    fun worldX_usesTileZ_notBaseZ() {
        val centerLon = 120.0
        val tileZ = 18

        val tile1_worldX = worldX(tileXToLon(10000, tileZ), centerLon, tileZ)
        val tile2_worldX = worldX(tileXToLon(10001, tileZ), centerLon, tileZ)

        // Spacing should be exactly 256px at tileZ
        assertEquals(256f, tile2_worldX - tile1_worldX, 0.1f)
    }

    @Test
    fun regression_baseZ_notUsedInWorldX() {
        val centerLon = 120.0
        val tileZ = 18
        val baseZ = 15

        val worldX_tileZ = worldX(tileXToLon(10000, tileZ), centerLon, tileZ)
        val worldX_baseZ = worldX(tileXToLon(10000, tileZ), centerLon, baseZ)

        // Should NOT be equal (catches bug if someone uses baseZ)
        assertNotEquals(worldX_tileZ, worldX_baseZ)
    }
}

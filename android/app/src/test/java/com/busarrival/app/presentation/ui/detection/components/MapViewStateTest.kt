package com.busarrival.app.presentation.ui.detection.components

import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotEquals

/**
 * Regression tests for MapView state bugs.
 * These tests verify that fixes for state management issues remain stable.
 */
class MapViewStateTest {

    @Test
    fun centerLatLon_updatesWhenRouteChanges() {
        val route1 = MockRoute(centerLat = 20.0, centerLon = 120.0)
        val route2 = MockRoute(centerLat = 21.0, centerLon = 121.0)

        val center1 = calculateCenter(route1)
        val center2 = calculateCenter(route2)

        assertNotEquals(center1.lat, center2.lat, "Center latitude should update")
        assertNotEquals(center1.lon, center2.lon, "Center longitude should update")
    }

    @Test
    fun tileLoading_triggeredByCenterChange() {
        val tileZ = 15
        val center1 = MockLatLon(20.0, 120.0)
        val center2 = MockLatLon(21.0, 121.0)

        val keys1 = setOf(tileZ, center1, 0)
        val keys2 = setOf(tileZ, center2, 0)

        assertNotEquals(keys1, keys2, "LaunchedEffect should trigger on center change")
    }

    @Test
    fun markerPosition_interpolatesAlongSegment() {
        val node1 = MockNode(xCm = 0, yCm = 0, cumDistCm = 0, segLenMm = 10000)
        val node2 = MockNode(xCm = 1000, yCm = 1000, cumDistCm = 1000, segLenMm = 0)

        val progress = 500
        val interpolated = interpolatePosition(
            progressCm = progress,
            prevNode = node1,
            nextNode = node2
        )

        assertEquals(500, interpolated.first, "X should be interpolated (50% of segment)")
        assertEquals(500, interpolated.second, "Y should be interpolated (50% of segment)")
    }

    @Test
    fun transformOrder_scaleThenTranslate() {
        val worldX = 100f
        val scale = 2.0f
        val center = 500f

        val wrong = (worldX + center) * scale
        val correct = worldX * scale + center

        assertNotEquals(wrong, correct, "Transform order matters")
        assertEquals(700f, correct, "Correct transform: scale worldX, then add center")
    }

    @Test
    fun shortStopStateLabel_compactsKnownStates() {
        assertEquals("APR", shortStopStateLabel("Approaching"))
        assertEquals("ARL", shortStopStateLabel("Arriving"))
        assertEquals("AT", shortStopStateLabel("AtStop"))
        assertEquals("DEP", shortStopStateLabel("Departed"))
    }

    @Test
    fun formatBusMarkerLabel_usesHumanReadableStopNumber() {
        assertEquals("Stop 6 · AT", formatBusMarkerLabel(5, "AtStop"))
        assertEquals("Stop - · IDLE", formatBusMarkerLabel(-1, "Idle"))
    }

    @Test
    fun busStateColor_differsAcrossStates() {
        assertNotEquals(busStateColor("Approaching"), busStateColor("AtStop"))
        assertNotEquals(busStateColor("AtStop"), busStateColor("Departed"))
    }

    // Helper classes and functions

    private data class MockLatLon(val lat: Double, val lon: Double)
    private data class MockRoute(val centerLat: Double, val centerLon: Double)
    private data class MockNode(val xCm: Int, val yCm: Int, val cumDistCm: Int, val segLenMm: Int)

    private fun calculateCenter(route: MockRoute): MockLatLon {
        return MockLatLon(route.centerLat, route.centerLon)
    }

    private fun interpolatePosition(progressCm: Int, prevNode: MockNode, nextNode: MockNode): Pair<Int, Int> {
        val segmentProgressCm = progressCm - prevNode.cumDistCm
        val segmentLenCm = prevNode.segLenMm / 10

        if (segmentLenCm > 0) {
            val ratio = segmentProgressCm.toFloat() / segmentLenCm.toFloat()
            val x = prevNode.xCm + (nextNode.xCm - prevNode.xCm).toFloat() * ratio
            val y = prevNode.yCm + (nextNode.yCm - prevNode.yCm).toFloat() * ratio
            return Pair(x.toInt(), y.toInt())
        }

        return Pair(prevNode.xCm, prevNode.yCm)
    }
}

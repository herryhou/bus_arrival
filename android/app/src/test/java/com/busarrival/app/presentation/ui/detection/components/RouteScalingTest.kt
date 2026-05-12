package com.busarrival.app.presentation.ui.detection.components

import kotlin.test.Test
import kotlin.test.assertEquals

/**
 * Tests to verify route stroke width and marker radii scale correctly with zoom level.
 * Ensures visual consistency across different scales.
 */
class RouteScalingTest {

    /**
     * Calculate stroke width that scales inversely with zoom level.
     * Matches MapView.kt implementation: `4f / scale.coerceAtLeast(0.5f)`
     */
    private fun calculateStrokeWidth(scale: Float): Float {
        return 4f / scale.coerceAtLeast(0.5f)
    }

    /**
     * Calculate stop radius that scales inversely with zoom level.
     * Matches MapView.kt implementation: `8f / scale.coerceAtLeast(0.5f)`
     */
    private fun calculateStopRadius(scale: Float): Float {
        return 8f / scale.coerceAtLeast(0.5f)
    }

    /**
     * Calculate marker radius that scales inversely with zoom level.
     * Matches MapView.kt implementation: `12f / scale.coerceAtLeast(0.5f)`
     */
    private fun calculateMarkerRadius(scale: Float): Float {
        return 12f / scale.coerceAtLeast(0.5f)
    }

    @Test
    fun strokeWidth_atScale1_is4px() {
        val scale = 1.0f
        val strokeWidth = calculateStrokeWidth(scale)
        assertEquals(4f, strokeWidth, 0.01f, "At scale 1.0, stroke should be 4px")
    }

    @Test
    fun strokeWidth_atScale2_is2px() {
        val scale = 2.0f
        val strokeWidth = calculateStrokeWidth(scale)
        assertEquals(2f, strokeWidth, 0.01f, "At scale 2.0, stroke should be 2px (thinner)")
    }

    @Test
    fun strokeWidth_atScale4_is1px() {
        val scale = 4.0f
        val strokeWidth = calculateStrokeWidth(scale)
        assertEquals(1f, strokeWidth, 0.01f, "At scale 4.0, stroke should be 1px (very thin)")
    }

    @Test
    fun strokeWidth_atScale0_5_is8px() {
        val scale = 0.5f
        val strokeWidth = calculateStrokeWidth(scale)
        assertEquals(8f, strokeWidth, 0.01f, "At scale 0.5, stroke should be 8px (thicker)")
    }

    @Test
    fun stopRadius_atScale1_is8px() {
        val scale = 1.0f
        val radius = calculateStopRadius(scale)
        assertEquals(8f, radius, 0.01f, "At scale 1.0, stop radius should be 8px")
    }

    @Test
    fun stopRadius_atScale2_is4px() {
        val scale = 2.0f
        val radius = calculateStopRadius(scale)
        assertEquals(4f, radius, 0.01f, "At scale 2.0, stop radius should be 4px")
    }

    @Test
    fun markerRadius_atScale1_is12px() {
        val scale = 1.0f
        val radius = calculateMarkerRadius(scale)
        assertEquals(12f, radius, 0.01f, "At scale 1.0, marker radius should be 12px")
    }

    @Test
    fun markerRadius_atScale2_is6px() {
        val scale = 2.0f
        val radius = calculateMarkerRadius(scale)
        assertEquals(6f, radius, 0.01f, "At scale 2.0, marker radius should be 6px")
    }

    @Test
    fun markerRadius_atScale4_is3px() {
        val scale = 4.0f
        val radius = calculateMarkerRadius(scale)
        assertEquals(3f, radius, 0.01f, "At scale 4.0, marker radius should be 3px")
    }

    @Test
    fun scalingFormula_preventsDivisionByZero() {
        val scale = 0.01f  // Very small scale
        // coerceAtLeast(0.5f) prevents division by zero and excessive sizes
        val strokeWidth = calculateStrokeWidth(scale)
        val expectedMax = 4f / 0.5f  // Maximum when scale hits the floor
        assertEquals(expectedMax, strokeWidth, 0.01f, "Should use minimum scale of 0.5")
    }

    @Test
    fun scalingFormula_producesConsistentVisualWeight() {
        // Verify that stroke:marker ratio stays constant across scales
        val scales = listOf(0.5f, 1f, 2f, 4f)

        for (scale in scales) {
            val stroke = calculateStrokeWidth(scale)
            val marker = calculateMarkerRadius(scale)
            val ratio = marker / stroke

            // Ratio should always be 3:1 (12px:4px at base scale)
            assertEquals(3f, ratio, 0.01f, "At scale $scale, marker:stroke ratio should be 3:1")
        }
    }

    @Test
    fun allRouteElements_scaleProportionally() {
        val scale = 2.0f

        val stroke = calculateStrokeWidth(scale)
        val stop = calculateStopRadius(scale)
        val marker = calculateMarkerRadius(scale)

        // All should be half their base size at 2x zoom
        assertEquals(2f, stroke, 0.01f)
        assertEquals(4f, stop, 0.01f)
        assertEquals(6f, marker, 0.01f)
    }
}

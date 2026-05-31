package com.busarrival.app.presentation.ui.detection.components

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.unit.IntSize
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class CameraFollowViewportTest {

    private val viewportSize = IntSize(1000, 800)

    @Test
    fun vehicleAtCenterDoesNotTriggerAutoPan() {
        assertFalse(shouldTriggerCameraFollowAutoPan(Offset(500f, 400f), viewportSize))
    }

    @Test
    fun vehicleAtFivePercentMarginTriggersAutoPan() {
        assertTrue(shouldTriggerCameraFollowAutoPan(Offset(50f, 400f), viewportSize))
    }

    @Test
    fun vehicleAtFifteenPercentMarginDoesNotTriggerAutoPan() {
        assertFalse(shouldTriggerCameraFollowAutoPan(Offset(150f, 400f), viewportSize))
    }

    @Test
    fun vehicleOutsideViewportTriggersAutoPan() {
        assertTrue(shouldTriggerCameraFollowAutoPan(Offset(-50f, 400f), viewportSize))
    }

    @Test
    fun vehicleNearRightEdgeTriggersAutoPan() {
        assertTrue(shouldTriggerCameraFollowAutoPan(Offset(950f, 400f), viewportSize))
    }

    @Test
    fun vehicleNearTopEdgeTriggersAutoPan() {
        assertTrue(shouldTriggerCameraFollowAutoPan(Offset(500f, 40f), viewportSize))
    }

    @Test
    fun vehicleNearBottomEdgeTriggersAutoPan() {
        assertTrue(shouldTriggerCameraFollowAutoPan(Offset(500f, 760f), viewportSize))
    }

    @Test
    fun targetOffsetCentersVehicleFromCurrentOffset() {
        val currentOffset = Offset(20f, -10f)
        val target =
                cameraFollowTargetOffset(
                        vehicleScreenPosition = Offset(950f, 760f),
                        currentOffset = currentOffset,
                        viewportSize = viewportSize
                )

        assertEquals(Offset(-430f, -370f), target)
    }

    @Test
    fun firstUserPanWhileFollowEnabledDisablesFollowAndAppliesPan() {
        val result =
                cameraFollowTransformGesture(
                        cameraFollowEnabled = true,
                        autoPanTarget = null,
                        oldScale = 1f,
                        oldOffset = Offset.Zero,
                        canvasSize = IntSize(1000, 800),
                        centroid = Offset(500f, 400f),
                        pan = Offset(24f, -12f),
                        zoom = 1f
                )

        assertTrue(result.shouldDisableCameraFollow)
        assertFalse(result.shouldCancelAutoPan)
        assertEquals(1f, result.newScale)
        assertEquals(Offset(24f, -12f), result.newOffset)
    }

    @Test
    fun firstUserPanDuringAutoPanCancelsAnimationAndAppliesPan() {
        val result =
                cameraFollowTransformGesture(
                        cameraFollowEnabled = true,
                        autoPanTarget = Offset(100f, 100f),
                        oldScale = 1f,
                        oldOffset = Offset(10f, 10f),
                        canvasSize = IntSize(1000, 800),
                        centroid = Offset(500f, 400f),
                        pan = Offset(24f, -12f),
                        zoom = 1f
                )

        assertTrue(result.shouldDisableCameraFollow)
        assertTrue(result.shouldCancelAutoPan)
        assertEquals(1f, result.newScale)
        assertEquals(Offset(34f, -2f), result.newOffset)
    }
}

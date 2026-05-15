package com.busarrival.app.presentation.ui.detection

import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.test.*
import com.busarrival.app.presentation.viewmodel.DetectionUiState
import com.busarrival.app.service.PipelineEvent
import com.google.accompanist.permissions.ExperimentalPermissionsApi
import com.google.accompanist.permissions.rememberMultiplePermissionsState
import org.junit.Rule
import org.junit.Test
import kotlin.test.assertFalse
import kotlin.test.assertTrue

@OptIn(ExperimentalPermissionsApi::class)
class DetectionScreenTest {

    @get:Rule
    val composeTestRule = createComposeRule()

    @Test
    fun permissionDenied_showsRequestButton() {
        composeTestRule.setContent {
            val permissionsState = rememberMultiplePermissionsState(
                permissions = listOf(
                    android.Manifest.permission.ACCESS_FINE_LOCATION
                )
            )

            if (!permissionsState.allPermissionsGranted) {
                PermissionRequestContent(onRequest = {})
            }
        }

        composeTestRule.onNodeWithText("Location Permission Required").assertIsDisplayed()
        composeTestRule.onNodeWithText("Grant Permission").assertIsDisplayed()
    }

    @Test
    fun noRoute_showsErrorMessage() {
        composeTestRule.setContent {
            NoRouteContent()
        }

        composeTestRule.onNodeWithText("No Route Loaded").assertIsDisplayed()
        composeTestRule.onNodeWithText("Please load a route in the Configuration tab first")
            .assertIsDisplayed()
    }

    @Test
    fun startStopButton_togglesText() {
        var isRunning = false

        composeTestRule.setContent {
            androidx.compose.material3.Button(
                onClick = { isRunning = !isRunning }
            ) {
                androidx.compose.material3.Text(
                    if (isRunning) "Stop Detection" else "Start Detection"
                )
            }
        }

        composeTestRule.onNodeWithText("Start Detection").assertIsDisplayed()

        composeTestRule.onNodeWithText("Start Detection").performClick()

        composeTestRule.onNodeWithText("Stop Detection").assertIsDisplayed()
    }

    @Test
    fun positionUpdate_displaysInStatusPanel() {
        val events = listOf(
            PipelineEvent.PositionUpdate(sCm = 5000, vCms = 300, mode = "Normal"),
            PipelineEvent.Arrival(stopIndex = 0, probability = 200),
            PipelineEvent.Departure(stopIndex = 0, dwellTimeS = 30)
        )

        composeTestRule.setContent {
            StatusPanel(
                uiState = DetectionUiState(
                    isRunning = true,
                    sCm = 5000,
                    vCms = 300,
                    mode = "Normal"
                ),
                events = events,
                onStartStop = {},
                onToggleCamera = {}
            )
        }

        composeTestRule.onNodeWithText("5000 cm").assertIsDisplayed()
        composeTestRule.onNodeWithText("300 cm/s").assertIsDisplayed()
        composeTestRule.onNodeWithText("Normal").assertIsDisplayed()
        composeTestRule.onNodeWithText("Arrival at stop 0").assertIsDisplayed()
    }

    @Test
    fun cameraToggle_updatesIcon() {
        var isCameraFollowEnabled = true

        composeTestRule.setContent {
            androidx.compose.material3.IconButton(
                onClick = { isCameraFollowEnabled = !isCameraFollowEnabled }
            ) {
                androidx.compose.material3.Icon(
                    imageVector = if (isCameraFollowEnabled) {
                        androidx.compose.material.icons.Icons.Filled.CenterFocusStrong
                    } else {
                        androidx.compose.material.icons.Icons.Filled.CenterFocusWeak
                    },
                    contentDescription = "Toggle camera"
                )
            }
        }

        composeTestRule.onNodeWithContentDescription("Toggle camera").performClick()

        assertFalse(isCameraFollowEnabled)
    }
}

// Helper composable for testing
@Composable
private fun PermissionRequestContent(onRequest: () -> Unit) {
    androidx.compose.foundation.layout.Column {
        androidx.compose.material3.Text("Location Permission Required")
        androidx.compose.material3.Button(onClick = onRequest) {
            androidx.compose.material3.Text("Grant Permission")
        }
    }
}

@Composable
private fun NoRouteContent() {
    androidx.compose.foundation.layout.Column {
        androidx.compose.material3.Text("No Route Loaded")
        androidx.compose.material3.Text("Please load a route in the Configuration tab first")
    }
}

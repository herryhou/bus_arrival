package com.busarrival.app.presentation.ui.detection.components

import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.test.assertIsOff
import androidx.compose.ui.test.assertIsOn
import androidx.compose.ui.test.assertHasClickAction
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.busarrival.app.presentation.viewmodel.DetectionUiState
import com.busarrival.app.service.PipelineEvent
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class StatusPanelTest {

    @get:Rule
    val composeRule = createComposeRule()

    @Test
    fun gpsLoggingToggleStartsOffAndCanBeEnabled() {
        var toggled = false

        composeRule.setContent {
            MaterialTheme {
                StatusPanel(
                    uiState = DetectionUiState(),
                    events = emptyList<PipelineEvent>(),
                    routeName = "ty225",
                    gpsLoggingEnabled = false,
                    onStartStop = {},
                    onToggleCamera = {},
                    onToggleGpsLogging = { toggled = true }
                )
            }
        }

        composeRule.onNodeWithText("Start Detection").assertHasClickAction()
        composeRule.onNodeWithContentDescription("Toggle GPS logging").assertIsOff()

        composeRule.onNodeWithContentDescription("Toggle GPS logging").performClick()

        assert(toggled)
    }

    @Test
    fun gpsLoggingToggleReflectsEnabledState() {
        composeRule.setContent {
            MaterialTheme {
                StatusPanel(
                    uiState = DetectionUiState(),
                    events = emptyList<PipelineEvent>(),
                    routeName = "ty225",
                    gpsLoggingEnabled = true,
                    onStartStop = {},
                    onToggleCamera = {},
                    onToggleGpsLogging = {}
                )
            }
        }

        composeRule.onNodeWithContentDescription("Toggle GPS logging").assertIsOn()
    }
}

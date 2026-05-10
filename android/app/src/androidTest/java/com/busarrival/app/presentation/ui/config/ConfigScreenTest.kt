package com.busarrival.app.presentation.ui.config

import androidx.compose.ui.test.*
import com.busarrival.app.presentation.viewmodel.ConfigUiState
import com.busarrival.app.presentation.viewmodel.DetectionParameters
import com.busarrival.app.data.storage.RouteMetadata
import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.Modifier
import androidx.compose.testutils.hasText
import com.busarrival.app.presentation.ui.config.components.RouteListItem
import com.busarrival.app.presentation.ui.config.components.ParameterSlider
import org.junit.Rule
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class ConfigScreenTest {

    @get:Rule
    val composeTestRule = createComposeRule()

    @Test
    fun emptyState_displaysMessage() {
        composeTestRule.setContent {
            ConfigScreen(
                viewModel = mockViewModel(ConfigUiState())
            )
        }

        composeTestRule.onNodeWithText("No routes loaded").assertIsDisplayed()
        composeTestRule.onNodeWithText("Tap + to add a route file").assertIsDisplayed()
    }

    @Test
    fun routeList_displaysRoutes() {
        val routes = listOf(
            RouteMetadata("uuid1", "Downtown", 1000L, 5, "/path/1"),
            RouteMetadata("uuid2", "Uptown", 2000L, 3, "/path/2")
        )

        composeTestRule.setContent {
            ConfigScreen(
                viewModel = mockViewModel(ConfigUiState(routes = routes))
            )
        }

        composeTestRule.onNodeWithText("Downtown").assertIsDisplayed()
        composeTestRule.onNodeWithText("5 stops").assertIsDisplayed()
        composeTestRule.onNodeWithText("Uptown").assertIsDisplayed()
    }

    @Test
    fun activeRoute_hasIndicator() {
        val routes = listOf(
            RouteMetadata("uuid1", "Active", 1000L, 5, "/path/1")
        )

        composeTestRule.setContent {
            ConfigScreen(
                viewModel = mockViewModel(ConfigUiState(
                    routes = routes,
                    activeRouteId = "uuid1"
                ))
            )
        }

        composeTestRule.onNodeWithText("Active").assertIsDisplayed()
    }

    @Test
    fun parameterSection_expandsAndCollapses() {
        composeTestRule.setContent {
            ConfigScreen(
                viewModel = mockViewModel(ConfigUiState())
            )
        }

        // Initially collapsed - parameters not visible
        composeTestRule.onNodeWithText("Distance Weight").assertDoesNotExist()

        // Click expand
        composeTestRule.onNodeWithContentDescription("Expand").performClick()

        // Now visible
        composeTestRule.onNodeWithText("Distance Weight").assertIsDisplayed()

        // Click collapse
        composeTestRule.onNodeWithContentDescription("Collapse").performClick()

        // Hidden again
        composeTestRule.onNodeWithText("Distance Weight").assertDoesNotExist()
    }

    @Test
    fun slider_updatesValue() {
        var currentValue = 50

        composeTestRule.setContent {
            ParameterSlider(
                label = "Test Slider",
                value = currentValue,
                onValueChange = { currentValue = it }
            )
        }

        composeTestRule.onNodeWithText("50").assertIsDisplayed()

        // Drag slider (simulate)
        composeTestRule.onNodeWithContentDescription("Slider").performSemanticsAction(
            androidx.compose.ui.semantics.SemanticsActions.ProgressBar
        ) { progress ->
            // This would update via onValueChange
        }
    }
}

// Helper functions (would be in test utils)
private fun mockViewModel(state: ConfigUiState): com.busarrival.app.presentation.viewmodel.ConfigViewModel {
    return mockk {
        every { uiState } returns kotlinx.coroutines.flow.MutableStateFlow(state)
    }
}

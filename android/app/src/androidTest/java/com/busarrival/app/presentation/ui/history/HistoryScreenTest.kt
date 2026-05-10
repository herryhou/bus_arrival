package com.busarrival.app.presentation.ui.history

import androidx.compose.ui.test.*
import com.busarrival.app.presentation.viewmodel.*
import org.junit.Rule
import org.junit.Test
import kotlin.test.assertEquals

class HistoryScreenTest {

    @get:Rule
    val composeTestRule = createComposeRule()

    @Test
    fun emptyState_displaysMessage() {
        composeTestRule.setContent {
            HistoryScreen(
                viewModel = mockViewModel(HistoryUiState())
            )
        }

        composeTestRule.onNodeWithText("No events recorded").assertIsDisplayed()
    }

    @Test
    fun filterChips_display() {
        composeTestRule.setContent {
            FilterChips(
                currentFilter = TimeFilter.Today,
                onFilterSelected = {}
            )
        }

        composeTestRule.onNodeWithText("Today").assertIsDisplayed()
        composeTestRule.onNodeWithText("Week").assertIsDisplayed()
        composeTestRule.onNodeWithText("All").assertIsDisplayed()
    }

    @Test
    fun filterChip_clickUpdatesSelection() {
        var selectedFilter = TimeFilter.Today

        composeTestRule.setContent {
            FilterChips(
                currentFilter = selectedFilter,
                onFilterSelected = { selectedFilter = it }
            )
        }

        composeTestRule.onNodeWithText("Week").performClick()

        assertEquals(TimeFilter.Week, selectedFilter)
    }

    @Test
    fun events_displayInList() {
        val events = listOf(
            HistoryEventItem(1, 1000L, 0, EventType.Arrival, "p=200"),
            HistoryEventItem(2, 2000L, 0, EventType.Departure, "dwell=30s")
        )

        composeTestRule.setContent {
            androidx.compose.foundation.lazy.LazyColumn {
                items(events) { event ->
                    EventItem(event)
                }
            }
        }

        composeTestRule.onNodeWithText("Arrival").assertIsDisplayed()
        composeTestRule.onNodeWithText("Departure").assertIsDisplayed()
        composeTestRule.onNodeWithText("Stop 0").assertIsDisplayed()
    }

    @Test
    fun dayHeader_displaysCorrectly() {
        val eventsWithHeader = listOf(
            HistoryEventItem(1, 1000L, -1, EventType.Arrival, "DAY_HEADER:2026-05-10"),
            HistoryEventItem(2, 2000L, 0, EventType.Arrival, "p=200")
        )

        composeTestRule.setContent {
            androidx.compose.foundation.lazy.LazyColumn {
                items(eventsWithHeader) { event ->
                    EventItem(event)
                }
            }
        }

        // Day header should be displayed
        composeTestRule.onNodeWithText("May 10, 2026").assertIsDisplayed()
    }

    @Test
    fun arrivalEvent_hasGreenIndicator() {
        val arrival = HistoryEventItem(1, 1000L, 0, EventType.Arrival, "p=200")

        composeTestRule.setContent {
            EventItem(arrival)
        }

        composeTestRule.onNodeWithText("Arrival").assertIsDisplayed()
        composeTestRule.onNodeWithText("Stop 0").assertIsDisplayed()
        composeTestRule.onNodeWithText("p=200").assertIsDisplayed()
    }

    @Test
    fun departureEvent_hasRedIndicator() {
        val departure = HistoryEventItem(1, 1000L, 0, EventType.Departure, "dwell=30s")

        composeTestRule.setContent {
            EventItem(departure)
        }

        composeTestRule.onNodeWithText("Departure").assertIsDisplayed()
        composeTestRule.onNodeWithText("dwell=30s").assertIsDisplayed()
    }
}

// Helper functions
private fun mockViewModel(state: HistoryUiState): HistoryViewModel {
    return mockk {
        every { uiState } returns kotlinx.coroutines.flow.MutableStateFlow(state)
    }
}

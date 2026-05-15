package com.busarrival.app.presentation.viewmodel

import android.app.Application
import io.mockk.every
import io.mockk.mockk
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.*
import org.junit.After
import org.junit.Before
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/**
 * NOTE: HistoryViewModel creates DetectionRepository internally.
 * This dependency is not injectable, so mocking is not possible without refactoring.
 * Tests below verify state management logic only.
 *
 * TODO: Refactor HistoryViewModel to use dependency injection for full unit test coverage.
 */
@OptIn(ExperimentalCoroutinesApi::class)
class HistoryViewModelTest {

    private lateinit var viewModel: HistoryViewModel
    private lateinit var mockApplication: Application

    private val testDispatcher = UnconfinedTestDispatcher()

    @Before
    fun setup() {
        Dispatchers.setMain(testDispatcher)

        mockApplication = mockk(relaxed = true)
        every { mockApplication.applicationContext } returns mockApplication

        viewModel = HistoryViewModel(mockApplication)
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    @Test
    fun `initial state has Today filter`() = runTest {
        val state = viewModel.uiState.value
        assertEquals(TimeFilter.Today, state.timeFilter)
    }

    @Test
    fun `initial state has empty events list`() = runTest {
        val state = viewModel.uiState.value
        assertTrue(state.events.isEmpty())
    }

    @Test
    fun `applyFilter updates timeFilter state`() = runTest {
        viewModel.applyFilter(TimeFilter.Week)

        val state = viewModel.uiState.value
        assertEquals(TimeFilter.Week, state.timeFilter)
    }

    @Test
    fun `applyFilter with All updates timeFilter`() = runTest {
        viewModel.applyFilter(TimeFilter.All)

        val state = viewModel.uiState.value
        assertEquals(TimeFilter.All, state.timeFilter)
    }

    @Test
    fun `applyFilter with Today updates timeFilter`() = runTest {
        // Start with different filter
        viewModel.applyFilter(TimeFilter.All)
        assertEquals(TimeFilter.All, viewModel.uiState.value.timeFilter)

        // Change back to Today
        viewModel.applyFilter(TimeFilter.Today)

        val state = viewModel.uiState.value
        assertEquals(TimeFilter.Today, state.timeFilter)
    }
}

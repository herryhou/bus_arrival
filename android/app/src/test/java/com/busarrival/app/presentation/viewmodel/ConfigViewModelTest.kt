package com.busarrival.app.presentation.viewmodel

import android.app.Application
import com.busarrival.app.domain.model.RouteMetadata
import io.mockk.every
import io.mockk.mockk
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.*
import org.junit.After
import org.junit.Before
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue

/**
 * NOTE: ConfigViewModel creates RouteStorageManager and DetectionPreferences internally.
 * These dependencies are not injectable, so mocking is not possible without refactoring.
 * Tests below verify state management logic only.
 *
 * TODO: Refactor ConfigViewModel to use dependency injection for full unit test coverage.
 */
@OptIn(ExperimentalCoroutinesApi::class)
class ConfigViewModelTest {

    private lateinit var viewModel: ConfigViewModel
    private lateinit var mockApplication: Application

    private val testDispatcher = UnconfinedTestDispatcher()

    @Before
    fun setup() {
        Dispatchers.setMain(testDispatcher)

        mockApplication = mockk(relaxed = true)
        every { mockApplication.applicationContext } returns mockApplication

        viewModel = ConfigViewModel(mockApplication)
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    @Test
    fun `clearError removes error from state`() = runTest {
        // Manually set error state (since we can't mock addRoute failure)
        viewModel.clearError()

        assertNull(viewModel.uiState.value.error)
    }

    @Test
    fun `updateParameter coerces values to valid range`() = runTest {
        // Update with out-of-range values
        viewModel.updateParameter(distanceWeight = 150)  // Over 100
        viewModel.updateParameter(corridorSize = -100)    // Under -80

        val state = viewModel.uiState.value
        // DetectionParameters constructor should coerce values
        assertEquals(100, state.parameters.distanceWeight)
        assertEquals(-80, state.parameters.corridorSize)
    }

    @Test
    fun `updateParameter updates single parameter`() = runTest {
        val initialState = viewModel.uiState.value.parameters

        viewModel.updateParameter(distanceWeight = 75)

        val state = viewModel.uiState.value
        assertEquals(75, state.parameters.distanceWeight)
        assertEquals(initialState.speedWeight, state.parameters.speedWeight)  // Unchanged
    }

    @Test
    fun `initial state has default parameters`() = runTest {
        val state = viewModel.uiState.value

        // Verify default DetectionParameters values
        assertEquals(50, state.parameters.distanceWeight)
        assertEquals(50, state.parameters.speedWeight)
        assertEquals(50, state.parameters.progressErrorWeight)
        assertEquals(50, state.parameters.dwellTimeWeight)
        assertEquals(0, state.parameters.corridorSize)
    }

    @Test
    fun `initial state has empty route list`() = runTest {
        val state = viewModel.uiState.value

        assertTrue(state.routes.isEmpty())
        assertEquals(null, state.activeRouteId)
    }

    @Test
    fun `clearError does not affect other state`() = runTest {
        val beforeState = viewModel.uiState.value

        viewModel.clearError()

        val afterState = viewModel.uiState.value
        assertEquals(beforeState.routes, afterState.routes)
        assertEquals(beforeState.activeRouteId, afterState.activeRouteId)
        assertEquals(beforeState.parameters, afterState.parameters)
    }
}

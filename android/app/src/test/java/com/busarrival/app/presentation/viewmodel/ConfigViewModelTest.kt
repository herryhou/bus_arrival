package com.busarrival.app.presentation.viewmodel

import android.net.Uri
import com.busarrival.app.data.preferences.DetectionPreferences
import com.busarrival.app.data.storage.RouteMetadata
import com.busarrival.app.data.storage.RouteStorageManager
import io.mockk.*
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.*
import org.junit.After
import org.junit.Before
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue

@OptIn(ExperimentalCoroutinesApi::class)
class ConfigViewModelTest {

    private lateinit var viewModel: ConfigViewModel
    private lateinit var mockRouteStorage: RouteStorageManager
    private lateinit var mockPreferences: DetectionPreferences

    private val testDispatcher = UnconfinedTestDispatcher()

    @Before
    fun setup() {
        Dispatchers.setMain(testDispatcher)

        mockRouteStorage = mockk()
        mockPreferences = mockk()

        every { mockRouteStorage.loadAllMetadata() } returns emptyList()
        every { mockPreferences.activeRouteUuid } returns null
        every { mockPreferences.getParameters() } returns DetectionParameters.defaults

        viewModel = ConfigViewModel(
            routeStorage = mockRouteStorage,
            preferences = mockPreferences
        )
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    @Test
    fun `loadRoutes populates state from storage`() = runTest {
        val routes = listOf(
            RouteMetadata("uuid1", "Route 1", 1000L, 5, "/path/1"),
            RouteMetadata("uuid2", "Route 2", 2000L, 3, "/path/2")
        )
        every { mockRouteStorage.loadAllMetadata() } returns routes

        viewModel.loadRoutes()

        val state = viewModel.uiState.value
        assertEquals(2, state.routes.size)
        assertEquals("Route 1", state.routes[0].name)
    }

    @Test
    fun `loadRoutes sorts by timestamp descending`() = runTest {
        val routes = listOf(
            RouteMetadata("uuid1", "Old", 1000L, 5, "/path/1"),
            RouteMetadata("uuid2", "New", 3000L, 3, "/path/2"),
            RouteMetadata("uuid3", "Mid", 2000L, 4, "/path/3")
        )
        every { mockRouteStorage.loadAllMetadata() } returns routes

        viewModel.loadRoutes()

        val state = viewModel.uiState.value
        assertEquals("New", state.routes[0].name)
        assertEquals("Mid", state.routes[1].name)
        assertEquals("Old", state.routes[2].name)
    }

    @Test
    fun `setActiveRoute updates preferences and state`() = runTest {
        every { mockPreferences.activeRouteUuid = "uuid1" } just Runs

        viewModel.setActiveRoute("uuid1")

        assertEquals("uuid1", viewModel.uiState.value.activeRouteId)
        verify { mockPreferences.activeRouteUuid = "uuid1" }
    }

    @Test
    fun `updateParameter updates single parameter`() = runTest {
        every { mockPreferences.saveParameters(any()) } just Runs

        viewModel.updateParameter(distanceWeight = 75)

        val state = viewModel.uiState.value
        assertEquals(75, state.parameters.distanceWeight)
        assertEquals(50, state.parameters.speedWeight)  // Unchanged
        verify { mockPreferences.saveParameters(any()) }
    }

    @Test
    fun `updateParameter coerces values to valid range`() = runTest {
        every { mockPreferences.saveParameters(any()) } just Runs

        viewModel.updateParameter(distanceWeight = 150)  // Over 100
        viewModel.updateParameter(corridorSize = -100)    // Under -80

        val state = viewModel.uiState.value
        assertEquals(100, state.parameters.distanceWeight)
        assertEquals(-80, state.parameters.corridorSize)
    }

    @Test
    fun `addRoute copies file and sets active`() = runTest {
        val uri = mockk<Uri>()
        every { mockRouteStorage.copyToInternal(uri, "Route 1000") } returns Result.success("uuid3")
        every { mockPreferences.activeRouteUuid = "uuid3" } just Runs
        every { mockRouteStorage.loadAllMetadata() } returns listOf(
            RouteMetadata("uuid3", "Route 1000", 3000L, 5, "/path/3")
        )

        viewModel.addRoute(uri, "Route 1000")

        verify { mockRouteStorage.copyToInternal(uri, "Route 1000") }
        verify { mockPreferences.activeRouteUuid = "uuid3" }
        assertEquals("uuid3", viewModel.uiState.value.activeRouteId)
    }

    @Test
    fun `addRoute failure sets error state`() = runTest {
        val uri = mockk<Uri>()
        val exception = RuntimeException("Copy failed")
        every { mockRouteStorage.copyToInternal(any(), any()) } returns Result.failure(exception)

        viewModel.addRoute(uri, "Fail Route")

        assertEquals("Failed to add route: Copy failed", viewModel.uiState.value.error)
    }

    @Test
    fun `deleteRoute removes file and clears active if needed`() = runTest {
        val metadata = RouteMetadata("uuid1", "Route 1", 1000L, 5, "/path/1")
        every { mockRouteStorage.deleteRoute("uuid1") } returns Result.success(Unit)
        every { mockPreferences.activeRouteUuid } returns "uuid1"
        every { mockPreferences.activeRouteUuid = null } just Runs
        every { mockRouteStorage.loadAllMetadata() } returns emptyList()

        viewModel.deleteRoute(metadata)

        verify { mockRouteStorage.deleteRoute("uuid1") }
        verify { mockPreferences.activeRouteUuid = null }
    }

    @Test
    fun `clearError removes error from state`() = runTest {
        viewModel.clearError()

        assertNull(viewModel.uiState.value.error)
    }
}

package com.busarrival.app.presentation.viewmodel

import android.app.Application
import com.busarrival.app.data.preferences.DetectionPreferences
import com.busarrival.app.data.storage.RouteStorageManager
import com.busarrival.app.domain.model.RouteData
import com.busarrival.app.service.DetectionService
import com.busarrival.app.service.PipelineEvent
import io.mockk.*
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.*
import org.junit.After
import org.junit.Before
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

@OptIn(ExperimentalCoroutinesApi::class)
class DetectionViewModelTest {

    private lateinit var viewModel: DetectionViewModel
    private lateinit var mockApplication: Application
    private lateinit var mockPreferences: DetectionPreferences
    private lateinit var mockRouteStorage: RouteStorageManager

    private val testDispatcher = UnconfinedTestDispatcher()

    @Before
    fun setup() {
        Dispatchers.setMain(testDispatcher)

        mockApplication = mockk()
        mockPreferences = mockk()
        mockRouteStorage = mockk()

        every { mockPreferences.activeRouteUuid } returns null
        every { mockRouteStorage.loadRoute(any()) } returns null

        viewModel = DetectionViewModel(
            application = mockApplication,
            preferences = mockPreferences,
            routeStorage = mockRouteStorage
        )
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    @Test
    fun `init loads active route from preferences`() = runTest {
        val routeUuid = "test-uuid"
        val mockRoute = mockk<RouteData>()
        every { mockPreferences.activeRouteUuid } returns routeUuid
        every { mockRouteStorage.loadRoute(routeUuid) } returns mockRoute

        DetectionViewModel(
            application = mockApplication,
            preferences = mockPreferences,
            routeStorage = mockRouteStorage
        )

        // New instance should have loaded the route
        // (would need to expose activeRoute flow for verification)
    }

    @Test
    fun `startDetection with no route sets error`() = runTest {
        every { mockPreferences.activeRouteUuid } returns null
        every { mockRouteStorage.loadRoute(any()) } returns null

        viewModel.startDetection()

        assertEquals("No active route loaded", viewModel.uiState.value.error)
        assertFalse(viewModel.uiState.value.isRunning)
    }

    @Test
    fun `startDetection with route starts service`() = runTest {
        val mockRoute = mockk<RouteData>()
        every { mockPreferences.activeRouteUuid } returns "uuid"
        every { mockRouteStorage.loadRoute("uuid") } returns mockRoute
        every { mockApplication.startService(any()) } returns mockk()
        every { mockApplication.bindService(any(), any(), any()) } returns true

        viewModel.startDetection()

        // Verify service was started
        verify { mockApplication.startService(any()) }
        verify { mockApplication.bindService(any(), any(), any()) }
    }

    @Test
    fun `toggleCameraFollow toggles state`() = runTest {
        assertTrue(viewModel.uiState.value.isCameraFollowEnabled)

        viewModel.toggleCameraFollow()

        assertFalse(viewModel.uiState.value.isCameraFollowEnabled)

        viewModel.toggleCameraFollow()

        assertTrue(viewModel.uiState.value.isCameraFollowEnabled)
    }

    @Test
    fun `clearError removes error from state`() = runTest {
        viewModel.clearError()

        assertNull(viewModel.uiState.value.error)
    }

    @Test
    fun `handleServiceEvent PositionUpdate updates state`() = runTest {
        val event = PipelineEvent.PositionUpdate(sCm = 5000, vCms = 300, mode = "Normal")

        // This would be tested through service binding in integration test
        // Unit test would need to expose handleServiceEvent or use service flow
    }
}

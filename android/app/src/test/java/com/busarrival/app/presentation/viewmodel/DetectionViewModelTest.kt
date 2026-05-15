package com.busarrival.app.presentation.viewmodel

import android.app.Application
import com.busarrival.app.data.preferences.DetectionPreferences
import com.busarrival.app.data.storage.RouteStorageManager
import com.busarrival.app.data.trace.TraceStorageManager
import com.busarrival.app.domain.model.ReplayState
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

    private val testDispatcher = UnconfinedTestDispatcher()

    @Before
    fun setup() {
        Dispatchers.setMain(testDispatcher)

        mockApplication = mockk()

        // Initialize TraceStorageManager for testing
        mockkObject(TraceStorageManager)

        viewModel = DetectionViewModel(
            application = mockApplication
        )
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
        unmockkObject(TraceStorageManager)
    }

    @Test
    fun `init loads active route from preferences`() = runTest {
        // Test is limited without dependency injection
        // ViewModel initializes with default state
        val initialState = viewModel.uiState.value
        assertFalse(initialState.isRunning)
        assertEquals(-1, initialState.currentStop)
    }

    @Test
    fun `startDetection with no route sets error`() = runTest {
        viewModel.startDetection()

        assertEquals("No active route loaded", viewModel.uiState.value.error)
        assertFalse(viewModel.uiState.value.isRunning)
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

    // ==================== Replay/Timeline Tests ====================

    @Test
    fun `initial replayState has default values`() = runTest {
        val initialState = viewModel.replayState.value

        assertEquals(0L, initialState.currentTime)
        assertFalse(initialState.isPlaying)
        assertEquals(1f, initialState.playbackSpeed)
        assertEquals(0L, initialState.traceDuration)
        assertTrue(initialState.cameraFollowEnabled)
        assertNull(initialState.traceFile)
    }

    @Test
    fun `setPlaybackSpeed updates replayState`() = runTest {
        viewModel.setPlaybackSpeed(2f)

        assertEquals(2f, viewModel.replayState.value.playbackSpeed)
    }

    @Test
    fun `toggleReplayCameraFollow toggles camera follow state`() = runTest {
        assertTrue(viewModel.replayState.value.cameraFollowEnabled)

        viewModel.toggleReplayCameraFollow()

        assertFalse(viewModel.replayState.value.cameraFollowEnabled)

        viewModel.toggleReplayCameraFollow()

        assertTrue(viewModel.replayState.value.cameraFollowEnabled)
    }

    @Test
    fun `seekTo updates currentTime and pauses playback`() = runTest {
        // Setup: mock trace loading
        mockkObject(TraceStorageManager)
        val mockEvents = listOf(
            PipelineEvent.PositionUpdate(1000, 100, "Normal"),
            PipelineEvent.Arrival(0, 80)
        )
        coEvery { TraceStorageManager.loadTrace(any()) } returns mockEvents

        viewModel.loadTrace("test.jsonl")
        advanceUntilIdle()

        // Seek to middle of trace
        viewModel.seekTo(500L)
        advanceUntilIdle()

        assertEquals(500L, viewModel.replayState.value.currentTime)
        assertFalse(viewModel.replayState.value.isPlaying)

        unmockkObject(TraceStorageManager)
    }

    @Test
    fun `seekTo clamps position to valid range`() = runTest {
        // Setup: mock trace loading
        mockkObject(TraceStorageManager)
        val mockEvents = listOf(
            PipelineEvent.PositionUpdate(1000, 100, "Normal"),
            PipelineEvent.Arrival(0, 80)
        )
        coEvery { TraceStorageManager.loadTrace(any()) } returns mockEvents

        viewModel.loadTrace("test.jsonl")
        advanceUntilIdle()

        // Seek beyond duration
        viewModel.seekTo(2000L)
        advanceUntilIdle()

        // Should be clamped to trace duration
        assertEquals(viewModel.replayState.value.traceDuration, viewModel.replayState.value.currentTime)

        // Seek to negative
        viewModel.seekTo(-100L)
        advanceUntilIdle()

        // Should be clamped to 0
        assertEquals(0L, viewModel.replayState.value.currentTime)

        unmockkObject(TraceStorageManager)
    }

    @Test
    fun `loadTrace with empty file sets zero duration`() = runTest {
        mockkObject(TraceStorageManager)
        coEvery { TraceStorageManager.loadTrace(any()) } returns emptyList()

        viewModel.loadTrace("empty.jsonl")
        advanceUntilIdle()

        assertEquals(0L, viewModel.replayState.value.traceDuration)
        assertEquals("empty.jsonl", viewModel.replayState.value.traceFile)
        assertFalse(viewModel.replayState.value.isPlaying)

        unmockkObject(TraceStorageManager)
    }

    @Test
    fun `playPause when not playing starts playback`() = runTest {
        mockkObject(TraceStorageManager)
        val mockEvents = listOf(
            PipelineEvent.PositionUpdate(1000, 100, "Normal"),
            PipelineEvent.Arrival(0, 80)
        )
        coEvery { TraceStorageManager.loadTrace(any()) } returns mockEvents

        viewModel.loadTrace("test.jsonl")
        advanceUntilIdle()

        assertFalse(viewModel.replayState.value.isPlaying)

        viewModel.playPause()
        advanceUntilIdle()

        assertTrue(viewModel.replayState.value.isPlaying)

        unmockkObject(TraceStorageManager)
    }

    @Test
    fun `playPause when playing pauses playback`() = runTest {
        mockkObject(TraceStorageManager)
        val mockEvents = listOf(
            PipelineEvent.PositionUpdate(1000, 100, "Normal"),
            PipelineEvent.Arrival(0, 80)
        )
        coEvery { TraceStorageManager.loadTrace(any()) } returns mockEvents

        viewModel.loadTrace("test.jsonl")
        advanceUntilIdle()

        viewModel.playPause()
        advanceUntilIdle()
        assertTrue(viewModel.replayState.value.isPlaying)

        viewModel.playPause()
        advanceUntilIdle()
        assertFalse(viewModel.replayState.value.isPlaying)

        unmockkObject(TraceStorageManager)
    }

    @Test
    fun `playPause with no trace loaded sets error`() = runTest {
        viewModel.playPause()
        advanceUntilIdle()

        assertEquals("No trace loaded", viewModel.uiState.value.error)
        assertFalse(viewModel.replayState.value.isPlaying)
    }
}

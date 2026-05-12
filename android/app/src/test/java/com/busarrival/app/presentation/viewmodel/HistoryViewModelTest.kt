package com.busarrival.app.presentation.viewmodel

import app.cash.turbine.test
import com.busarrival.app.data.local.entity.ArrivalEntity
import com.busarrival.app.data.local.entity.DepartureEntity
import com.busarrival.app.data.repository.DetectionRepository
import io.mockk.*
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.test.*
import org.junit.After
import org.junit.Before
import org.junit.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

@OptIn(ExperimentalCoroutinesApi::class)
class HistoryViewModelTest {

    private lateinit var viewModel: HistoryViewModel
    private lateinit var mockRepository: DetectionRepository

    private val testDispatcher = UnconfinedTestDispatcher()

    private val now = System.currentTimeMillis()

    @Before
    fun setup() {
        Dispatchers.setMain(testDispatcher)

        mockRepository = mockk()

        viewModel = HistoryViewModel(
            repository = mockRepository
        )
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    @Test
    fun `applyFilter with Today queries correct time range`() = runTest {
        val arrivals = listOf(
            ArrivalEntity(1, 1001, stopIndex = 0, sCm = 0, probability = 200, routeId = "test_route"),
            ArrivalEntity(2, now, stopIndex = 1, sCm = 0, probability = 180, routeId = "test_route")
        )
        every { mockRepository.getArrivalsByTimeRange(any(), any()) } returns flow { emit(arrivals) }
        every { mockRepository.getDeparturesByTimeRange(any(), any()) } returns flow { emit(emptyList()) }

        viewModel.applyFilter(TimeFilter.Today)

        val state = viewModel.uiState.value
        assertEquals(TimeFilter.Today, state.timeFilter)
        assertTrue(state.events.isNotEmpty())
    }

    @Test
    fun `applyFilter with Week queries last 7 days`() = runTest {
        every { mockRepository.getArrivalsByTimeRange(any(), any()) } returns flow { emit(emptyList()) }
        every { mockRepository.getDeparturesByTimeRange(any(), any()) } returns flow { emit(emptyList()) }

        viewModel.applyFilter(TimeFilter.Week)

        verify { mockRepository.getArrivalsByTimeRange(any(), now) }
        // Week should query from ~7 days ago
        val timeCaptor = mutableListOf<Long>()
        verify { mockRepository.getArrivalsByTimeRange(capture(timeCaptor), any()) }
        assertTrue(timeCaptor[0] < now - 6 * 24 * 60 * 60 * 1000L)
    }

    @Test
    fun `applyFilter with All queries from epoch`() = runTest {
        every { mockRepository.getArrivalsByTimeRange(any(), any()) } returns flow { emit(emptyList()) }
        every { mockRepository.getDeparturesByTimeRange(any(), any()) } returns flow { emit(emptyList()) }

        viewModel.applyFilter(TimeFilter.All)

        val timeCaptor = mutableListOf<Long>()
        verify { mockRepository.getArrivalsByTimeRange(capture(timeCaptor), any()) }
        assertEquals(0L, timeCaptor[0])
    }

    @Test
    fun `events are sorted by timestamp descending`() = runTest {
        val arrivals = listOf(
            ArrivalEntity(1, 1000, stopIndex = 0, sCm = 0, probability = 200, routeId = "test_route"),
            ArrivalEntity(2, 3000, stopIndex = 1, sCm = 0, probability = 180, routeId = "test_route"),
            ArrivalEntity(3, 2000, stopIndex = 2, sCm = 0, probability = 190, routeId = "test_route")
        )
        every { mockRepository.getArrivalsByTimeRange(any(), any()) } returns flow { emit(arrivals) }
        every { mockRepository.getDeparturesByTimeRange(any(), any()) } returns flow { emit(emptyList()) }

        viewModel.applyFilter(TimeFilter.Today)

        val events = viewModel.uiState.value.events
        // Should be sorted: 3000, 2000, 1000
        assertTrue(events[0].timestamp > events[1].timestamp)
        assertTrue(events[1].timestamp > events[2].timestamp)
    }

    @Test
    fun `arrivals and departures are combined into event list`() = runTest {
        val arrivals = listOf(
            ArrivalEntity(1, 1000, stopIndex = 0, sCm = 0, probability = 200, routeId = "test_route")
        )
        val departures = listOf(
            DepartureEntity(1, 2000, stopIndex = 0, sCm = 0, dwellTimeS = 30, routeId = "test_route")
        )
        every { mockRepository.getArrivalsByTimeRange(any(), any()) } returns flow { emit(arrivals) }
        every { mockRepository.getDeparturesByTimeRange(any(), any()) } returns flow { emit(departures) }

        viewModel.applyFilter(TimeFilter.Today)

        val events = viewModel.uiState.value.events
        assertEquals(2, events.size)
        assertTrue(events.any { it.type == EventType.Arrival })
        assertTrue(events.any { it.type == EventType.Departure })
    }

    @Test
    fun `empty state handled correctly`() = runTest {
        every { mockRepository.getArrivalsByTimeRange(any(), any()) } returns flow { emit(emptyList()) }
        every { mockRepository.getDeparturesByTimeRange(any(), any()) } returns flow { emit(emptyList()) }

        viewModel.applyFilter(TimeFilter.Today)

        val state = viewModel.uiState.value
        assertTrue(state.events.isEmpty())
        assertEquals(TimeFilter.Today, state.timeFilter)
    }
}

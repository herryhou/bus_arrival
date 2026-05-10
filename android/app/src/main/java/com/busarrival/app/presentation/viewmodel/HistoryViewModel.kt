package com.busarrival.app.presentation.viewmodel

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.busarrival.app.data.repository.DetectionRepository
import com.busarrival.app.data.local.entity.ArrivalEntity
import com.busarrival.app.data.local.entity.DepartureEntity
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.LocalDateTime
import java.time.temporal.ChronoUnit
import javax.inject.Inject

enum class TimeFilter { Today, Week, All }

data class HistoryEventItem(
    val id: Long,
    val timestamp: Long,
    val stopIndex: Int,
    val type: EventType,
    val details: String
)

enum class EventType { Arrival, Departure }

data class HistoryUiState(
    val events: List<HistoryEventItem> = emptyList(),
    val timeFilter: TimeFilter = TimeFilter.Today,
    val stopFilter: Int? = null,  // null = all stops
    val isLoading: Boolean = false
)

@HiltViewModel
class HistoryViewModel @Inject constructor(
    private val repository: DetectionRepository
) : ViewModel() {

    private val _uiState = MutableStateFlow(HistoryUiState())
    val uiState: StateFlow<HistoryUiState> = _uiState.asStateFlow()

    init {
        loadEvents()
    }

    fun loadEvents() {
        applyFilter(_uiState.value.timeFilter)
    }

    fun applyFilter(timeFilter: TimeFilter) {
        viewModelScope.launch {
            _uiState.value = _uiState.value.copy(isLoading = true)

            val now = System.currentTimeMillis()
            val startTime = when (timeFilter) {
                TimeFilter.Today -> {
                    LocalDate.now().atStartOfDay(ZoneId.systemDefault()).toInstant().toEpochMilli()
                }
                TimeFilter.Week -> {
                    LocalDateTime.now().minusDays(7).atZone(ZoneId.systemDefault()).toInstant().toEpochMilli()
                }
                TimeFilter.All -> 0
            }

            val arrivals = repository.getArrivalsByTimeRange(startTime, now)
            val departures = repository.getDeparturesByTimeRange(startTime, now)

            val events = mutableListOf<HistoryEventItem>()

            arrivals.forEach { arrival ->
                events.add(
                    HistoryEventItem(
                        id = arrival.id,
                        timestamp = arrival.timestamp,
                        stopIndex = arrival.stopIndex,
                        type = EventType.Arrival,
                        details = "p=${arrival.probability}"
                    )
                )
            }

            departures.forEach { departure ->
                events.add(
                    HistoryEventItem(
                        id = departure.id,
                        timestamp = departure.timestamp,
                        stopIndex = departure.stopIndex,
                        type = EventType.Departure,
                        details = "dwell=${departure.dwellTimeS}s"
                    )
                )
            }

            // Sort by timestamp descending
            events.sortByDescending { it.timestamp }

            // Group by day
            val groupedEvents = groupByDay(events)

            _uiState.value = HistoryUiState(
                events = groupedEvents,
                timeFilter = timeFilter,
                isLoading = false
            )
        }
    }

    fun filterByStop(stopIndex: Int?) {
        _uiState.value = _uiState.value.copy(stopFilter = stopIndex)
        // Re-apply current filter with stop filter
        applyFilter(_uiState.value.timeFilter)
    }

    private fun groupByDay(events: List<HistoryEventItem>): List<HistoryEventItem> {
        val grouped = mutableListOf<HistoryEventItem>()
        var lastDay: String? = null

        events.forEach { event ->
            val day = LocalDate.ofInstant(
                Instant.ofEpochMilli(event.timestamp),
                ZoneId.systemDefault()
            ).toString()

            if (day != lastDay) {
                // Add day header as a special event
                grouped.add(
                    HistoryEventItem(
                        id = -grouped.size.toLong() - 1,
                        timestamp = event.timestamp,
                        stopIndex = -1,
                        type = EventType.Arrival,  // Placeholder
                        details = "DAY_HEADER:$day"
                    )
                )
                lastDay = day
            }

            grouped.add(event)
        }

        return grouped
    }
}

package com.busarrival.app.presentation.viewmodel

import android.Manifest
import android.app.Application
import android.content.pm.PackageManager
import androidx.core.content.ContextCompat
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.busarrival.app.service.DetectionService
import com.busarrival.app.service.PipelineEvent
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch

/**
 * ViewModel for detection screen.
 * Manages service connection and permissions.
 */
class DetectionViewModel(application: Application) : AndroidViewModel(application) {

    private val _uiState = MutableStateFlow(DetectionUiState())
    val uiState: StateFlow<DetectionUiState> = _uiState

    private val _events = MutableStateFlow<List<PipelineEvent>>(emptyList())
    val events: StateFlow<List<PipelineEvent>> = _events

    /**
     * Check location permissions.
     */
    fun hasLocationPermission(): Boolean {
        return ContextCompat.checkSelfPermission(
            getApplication(),
            Manifest.permission.ACCESS_FINE_LOCATION
        ) == PackageManager.PERMISSION_GRANTED
    }

    /**
     * Check background location permission.
     */
    fun hasBackgroundLocationPermission(): Boolean {
        return ContextCompat.checkSelfPermission(
            getApplication(),
            Manifest.permission.ACCESS_BACKGROUND_LOCATION
        ) == PackageManager.PERMISSION_GRANTED
    }

    /**
     * Start detection service.
     */
    fun startDetection() {
        DetectionService.startService(getApplication())
        _uiState.value = _uiState.value.copy(isRunning = true)
    }

    /**
     * Stop detection service.
     */
    fun stopDetection() {
        DetectionService.stopService(getApplication())
        _uiState.value = _uiState.value.copy(isRunning = false)
    }

    /**
     * Add event from service.
     */
    fun addEvent(event: PipelineEvent) {
        viewModelScope.launch {
            val current = _events.value.toMutableList()
            current.add(0, event)
            _events.value = current.take(100)  // Keep last 100 events
        }
    }

    /**
     * Clear events.
     */
    fun clearEvents() {
        _events.value = emptyList()
    }
}

/**
 * Detection UI state.
 */
data class DetectionUiState(
    val isRunning: Boolean = false,
    val currentStop: Int = -1,
    val sCm: Int = 0,
    val vCms: Int = 0
)

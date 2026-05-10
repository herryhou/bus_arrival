package com.busarrival.app.presentation.viewmodel

import android.Manifest
import android.app.Application
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.ServiceConnection
import android.content.pm.PackageManager
import android.os.IBinder
import androidx.core.content.ContextCompat
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.busarrival.app.data.preferences.DetectionPreferences
import com.busarrival.app.data.storage.RouteStorageManager
import com.busarrival.app.domain.model.RouteData
import com.busarrival.app.service.DetectionService
import com.busarrival.app.service.PipelineEvent
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

/**
 * ViewModel for detection screen.
 * Manages service connection, active route, and UI state.
 */
@HiltViewModel
class DetectionViewModel @Inject constructor(
    application: Application,
    private val preferences: DetectionPreferences,
    private val routeStorage: RouteStorageManager
) : AndroidViewModel(application) {

    private val _uiState = MutableStateFlow(DetectionUiState())
    val uiState: StateFlow<DetectionUiState> = _uiState.asStateFlow()

    private val _events = MutableStateFlow<List<PipelineEvent>>(emptyList())
    val events: StateFlow<List<PipelineEvent>> = _events.asStateFlow()

    private val _activeRoute = MutableStateFlow<RouteData?>(null)
    val activeRoute: StateFlow<RouteData?> = _activeRoute.asStateFlow()

    private var service: DetectionService? = null

    private val serviceConnection = object : ServiceConnection {
        override fun onServiceConnected(name: ComponentName?, binder: IBinder?) {
            val localBinder = binder as DetectionService.LocalBinder
            service = localBinder.getService()

            // Subscribe to service events
            viewModelScope.launch {
                service?.events?.collect { event ->
                    event?.let { handleServiceEvent(it) }
                }
            }

            // Update running state
            _uiState.value = _uiState.value.copy(isRunning = true)
        }

        override fun onServiceDisconnected(name: ComponentName?) {
            service = null
            _uiState.value = _uiState.value.copy(isRunning = false)
        }
    }

    init {
        loadActiveRoute()
    }

    /**
     * Load active route from preferences.
     */
    private fun loadActiveRoute() {
        val activeUuid = preferences.activeRouteUuid
        if (activeUuid != null) {
            val route = routeStorage.loadRoute(activeUuid)
            _activeRoute.value = route
            if (route == null) {
                _uiState.value = _uiState.value.copy(error = "Active route not found")
            }
        }
    }

    /**
     * Handle events from DetectionService.
     */
    private fun handleServiceEvent(event: PipelineEvent) {
        when (event) {
            is PipelineEvent.PositionUpdate -> {
                _uiState.value = _uiState.value.copy(
                    sCm = event.sCm,
                    vCms = event.vCms,
                    mode = event.mode
                )
            }
            is PipelineEvent.Arrival -> {
                _uiState.value = _uiState.value.copy(currentStop = event.stopIndex)
            }
            is PipelineEvent.Departure -> {
                // Departure handled via event list
            }
        }

        // Add to event list
        val current = _events.value.toMutableList()
        current.add(0, event)
        _events.value = current.take(100)
    }

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
     * Start detection service and bind to it.
     */
    fun startDetection() {
        if (_activeRoute.value == null) {
            _uiState.value = _uiState.value.copy(error = "No active route loaded")
            return
        }

        val intent = Intent(getApplication(), DetectionService::class.java)
        getApplication<Application>().startService(intent)
        getApplication<Application>().bindService(intent, serviceConnection, Context.BIND_AUTO_CREATE)
    }

    /**
     * Stop detection service and unbind.
     */
    fun stopDetection() {
        service?.let {
            getApplication<Application>().unbindService(serviceConnection)
        }
        service = null
        DetectionService.stopService(getApplication())
        _uiState.value = _uiState.value.copy(isRunning = false)
    }

    /**
     * Toggle camera follow mode.
     */
    fun toggleCameraFollow() {
        _uiState.value = _uiState.value.copy(
            isCameraFollowEnabled = !_uiState.value.isCameraFollowEnabled
        )
    }

    /**
     * Clear error message.
     */
    fun clearError() {
        _uiState.value = _uiState.value.copy(error = null)
    }

    override fun onCleared() {
        super.onCleared()
        service?.let {
            getApplication<Application>().unbindService(serviceConnection)
        }
    }
}

/**
 * Detection UI state.
 */
data class DetectionUiState(
    val isRunning: Boolean = false,
    val currentStop: Int = -1,
    val sCm: Int = 0,
    val vCms: Int = 0,
    val isCameraFollowEnabled: Boolean = true,
    val mode: String = "Normal",
    val error: String? = null
)

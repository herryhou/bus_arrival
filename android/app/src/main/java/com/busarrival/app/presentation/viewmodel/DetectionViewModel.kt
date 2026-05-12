package com.busarrival.app.presentation.viewmodel

import android.Manifest
import android.app.Application
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.ServiceConnection
import android.content.pm.PackageManager
import android.os.IBinder
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.ImageBitmap
import androidx.core.content.ContextCompat
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.busarrival.app.data.preferences.DetectionPreferences
import com.busarrival.app.data.storage.RouteStorageManager
import com.busarrival.app.data.trace.TraceStorageManager
import com.busarrival.app.domain.model.ReplayState
import com.busarrival.app.domain.model.RouteData
import com.busarrival.app.service.DetectionService
import com.busarrival.app.service.PipelineEvent
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.Job

/**
 * ViewModel for detection screen.
 * Manages service connection, active route, and UI state.
 */
class DetectionViewModel(
    application: Application
) : AndroidViewModel(application) {
    private val preferences = DetectionPreferences(application)
    private val routeStorage = RouteStorageManager(application, com.google.gson.Gson())
    private val gson = com.google.gson.Gson()

    private val _uiState = MutableStateFlow(DetectionUiState())
    val uiState: StateFlow<DetectionUiState> = _uiState.asStateFlow()

    private val _events = MutableStateFlow<List<PipelineEvent>>(emptyList())
    val events: StateFlow<List<PipelineEvent>> = _events.asStateFlow()

    private val _activeRoute = MutableStateFlow<RouteData?>(null)
    val activeRoute: StateFlow<RouteData?> = _activeRoute.asStateFlow()

    // Replay state for timeline/replay functionality
    private val _replayState = MutableStateFlow(ReplayState())
    val replayState: StateFlow<ReplayState> = _replayState.asStateFlow()

    // Map state - persists across screen switches
    private val _mapScale = MutableStateFlow(1f)
    val mapScale: StateFlow<Float> = _mapScale.asStateFlow()

    private val _mapOffset = MutableStateFlow(Offset.Zero)
    val mapOffset: StateFlow<Offset> = _mapOffset.asStateFlow()

    private val _tileCache = MutableStateFlow<Map<String, ImageBitmap>>(emptyMap())
    val tileCache: StateFlow<Map<String, ImageBitmap>> = _tileCache.asStateFlow()

    // Playback state
    private var playbackJob: Job? = null
    private var replayEvents: List<PipelineEvent> = emptyList()

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
        // Initialize TraceStorageManager
        TraceStorageManager.init(getApplication(), gson)
    }

    /**
     * Load active route from preferences.
     */
    private fun loadActiveRoute() {
        val activeUuid = preferences.activeRouteUuid
        android.util.Log.d("DetectionViewModel", "Loading active route: $activeUuid")
        if (activeUuid != null) {
            val route = routeStorage.loadRoute(activeUuid)
            _activeRoute.value = route
            android.util.Log.d("DetectionViewModel", "Route loaded: ${route != null}, nodes: ${route?.nodes?.size ?: 0}")
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

        val intent = Intent(getApplication<Application>(), DetectionService::class.java)
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
        DetectionService.stopService(getApplication<Application>())
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

    /**
     * Update map scale.
     */
    fun updateMapScale(scale: Float) {
        _mapScale.value = scale
    }

    /**
     * Update map offset.
     */
    fun updateMapOffset(offset: Offset) {
        _mapOffset.value = offset
    }

    /**
     * Update map state (scale + offset).
     */
    fun updateMapState(scale: Float, offset: Offset) {
        _mapScale.value = scale
        _mapOffset.value = offset
    }

    /**
     * Add tiles to cache.
     */
    fun addTiles(tiles: Map<String, ImageBitmap>) {
        val current = _tileCache.value.toMutableMap()
        current.putAll(tiles)
        _tileCache.value = current
    }

    /**
     * Clear tile cache (for testing/debugging).
     */
    fun clearTileCache() {
        _tileCache.value = emptyMap()
        android.util.Log.d("DetectionViewModel", "Tile cache cleared")
    }

    // ==================== Replay/Timeline Functions ====================

    /**
     * Load trace file and initialize replay state.
     */
    fun loadTrace(traceFile: String) {
        viewModelScope.launch {
            try {
                val events = TraceStorageManager.loadTrace(traceFile)
                replayEvents = events

                // Estimate duration based on number of events
                // Assuming 1 event per second (1Hz GPS), duration = event count * 1000ms
                val duration = events.size * 1000L

                _replayState.value = ReplayState(
                    currentTime = 0,
                    isPlaying = false,
                    playbackSpeed = 1f,
                    traceDuration = duration,
                    cameraFollowEnabled = true,
                    traceFile = traceFile
                )

                android.util.Log.d("DetectionViewModel", "Loaded trace: $traceFile, events: ${events.size}, duration: ${duration}ms")
            } catch (e: Exception) {
                android.util.Log.e("DetectionViewModel", "Failed to load trace: $traceFile", e)
                _uiState.value = _uiState.value.copy(error = "Failed to load trace: ${e.message}")
            }
        }
    }

    /**
     * Toggle playback state.
     */
    fun playPause() {
        val current = _replayState.value
        if (current.isPlaying) {
            // Pause playback
            playbackJob?.cancel()
            playbackJob = null
            _replayState.value = current.copy(isPlaying = false)
            android.util.Log.d("DetectionViewModel", "Playback paused at ${current.currentTime}ms")
        } else {
            // Start playback
            if (replayEvents.isEmpty()) {
                _uiState.value = _uiState.value.copy(error = "No trace loaded")
                return
            }

            _replayState.value = current.copy(isPlaying = true)
            startPlayback()
            android.util.Log.d("DetectionViewModel", "Playback started from ${current.currentTime}ms")
        }
    }

    /**
     * Set playback speed.
     * @param speed Speed multiplier (0.5x, 1x, 2x, 4x)
     */
    fun setPlaybackSpeed(speed: Float) {
        _replayState.value = _replayState.value.copy(playbackSpeed = speed)
        android.util.Log.d("DetectionViewModel", "Playback speed set to ${speed}x")

        // Restart playback if currently playing to apply new speed
        if (_replayState.value.isPlaying) {
            playbackJob?.cancel()
            startPlayback()
        }
    }

    /**
     * Seek to specific position in trace.
     * @param position Position in milliseconds
     */
    fun seekTo(position: Long) {
        val wasPlaying = _replayState.value.isPlaying

        // Cancel current playback
        playbackJob?.cancel()
        playbackJob = null

        // Update position
        _replayState.value = _replayState.value.copy(
            currentTime = position.coerceIn(0, _replayState.value.traceDuration),
            isPlaying = false
        )

        // Update UI state to reflect position in trace
        updateUiForPosition(position)

        android.util.Log.d("DetectionViewModel", "Seeked to ${position}ms")

        // Resume playback if it was playing
        if (wasPlaying) {
            _replayState.value = _replayState.value.copy(isPlaying = true)
            startPlayback()
        }
    }

    /**
     * Toggle camera follow mode for replay.
     */
    fun toggleReplayCameraFollow() {
        _replayState.value = _replayState.value.copy(
            cameraFollowEnabled = !_replayState.value.cameraFollowEnabled
        )
        android.util.Log.d("DetectionViewModel", "Replay camera follow: ${_replayState.value.cameraFollowEnabled}")
    }

    /**
     * Update current replay position (called during playback).
     */
    private fun updateReplayPosition(position: Long) {
        _replayState.value = _replayState.value.copy(
            currentTime = position.coerceIn(0, _replayState.value.traceDuration)
        )
        updateUiForPosition(position)
    }

    /**
     * Start playback coroutine.
     */
    private fun startPlayback() {
        playbackJob = viewModelScope.launch {
            val startTime = _replayState.value.currentTime
            val speed = _replayState.value.playbackSpeed
            val targetDuration = _replayState.value.traceDuration

            if (targetDuration <= 0) {
                _uiState.value = _uiState.value.copy(error = "Invalid trace duration")
                _replayState.value = _replayState.value.copy(isPlaying = false)
                return@launch
            }

            val tickDelayMs = (50 / speed).toLong() // Update every 50ms adjusted for speed

            while (_replayState.value.isPlaying && _replayState.value.currentTime < targetDuration) {
                kotlinx.coroutines.delay(tickDelayMs)

                val increment = (50 * speed).toLong() // Increment based on speed
                val newPosition = _replayState.value.currentTime + increment
                updateReplayPosition(newPosition)
            }

            // Playback finished
            if (_replayState.value.currentTime >= targetDuration) {
                _replayState.value = _replayState.value.copy(isPlaying = false)
                android.util.Log.d("DetectionViewModel", "Playback finished at ${_replayState.value.currentTime}ms")
            }
        }
    }

    /**
     * Update UI state for a given replay position.
     * Finds the relevant PositionUpdate events and applies them to UI.
     *
     * LIMITATION: PositionUpdate events don't have timestamps, so we map position to event index.
     * This is approximate since events aren't guaranteed to be evenly spaced in time.
     * A more accurate approach would require adding timestamps to PipelineEvent.
     */
    private fun updateUiForPosition(position: Long) {
        // Find the most recent PositionUpdate before this position
        val positionUpdates = replayEvents.filterIsInstance<PipelineEvent.PositionUpdate>()
        val latestUpdate = positionUpdates.lastOrNull { event ->
            // Map position to event index (approximate - see limitation above)
            val eventIndex = replayEvents.indexOf(event)
            val targetIndex = (position.toFloat() / _replayState.value.traceDuration * replayEvents.size).toInt()
            eventIndex <= targetIndex
        }

        latestUpdate?.let { event ->
            _uiState.value = _uiState.value.copy(
                sCm = event.sCm,
                vCms = event.vCms,
                mode = event.mode
            )
        }

        // Find arrivals/departures at this position
        val eventIndex = (position.toFloat() / _replayState.value.traceDuration * replayEvents.size).toInt()
        val currentEvents = replayEvents.take(eventIndex)

        val lastArrival = currentEvents.filterIsInstance<PipelineEvent.Arrival>().lastOrNull()
        val lastDeparture = currentEvents.filterIsInstance<PipelineEvent.Departure>().lastOrNull()

        // Update current stop based on last arrival/departure
        // State machine: at stop if arrived after last departure, otherwise at next stop
        val currentStop = when {
            lastArrival == null && lastDeparture == null -> -1 // No stop events yet
            lastDeparture == null -> lastArrival?.stopIndex ?: -1 // Arrived, never departed
            lastArrival == null -> lastDeparture.stopIndex + 1 // Departed, never arrived
            (lastArrival?.stopIndex ?: -1) > lastDeparture.stopIndex -> lastArrival?.stopIndex ?: -1 // Arrived after departure
            else -> lastDeparture.stopIndex + 1 // Departed after arrival
        }
        _uiState.value = _uiState.value.copy(currentStop = currentStop)
    }

    override fun onCleared() {
        super.onCleared()
        playbackJob?.cancel()
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

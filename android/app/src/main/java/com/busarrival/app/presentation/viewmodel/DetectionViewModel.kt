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
import com.busarrival.app.data.gpslog.GpsLogStorageManager
import com.busarrival.app.data.gpslog.RecordedGpsLogParser
import com.busarrival.app.data.gpslog.SupportedGpsPlaybackSpeeds
import com.busarrival.app.data.pipeline.detection.statemachine.StopLifecycleEvent
import com.busarrival.app.data.preferences.DetectionPreferences
import com.busarrival.app.data.storage.RouteStorageManager
import com.busarrival.app.data.trace.TraceStorageManager
import com.busarrival.app.domain.model.EventHint
import com.busarrival.app.domain.model.GpsFixState
import com.busarrival.app.domain.model.HintType
import com.busarrival.app.domain.model.ReplayState
import com.busarrival.app.domain.model.RouteData
import com.busarrival.app.service.DetectionService
import com.busarrival.app.service.GpsLogActions
import com.busarrival.app.service.GpsLogStatus
import com.busarrival.app.service.PipelineEvent
import com.busarrival.app.service.StopEventCallback
import com.busarrival.app.service.StopUiEvent
import java.util.LinkedHashMap
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

/** ViewModel for detection screen. Manages service connection, active route, and UI state. */
class DetectionViewModel(application: Application) : AndroidViewModel(application) {
    companion object {
        private const val MAX_MEMORY_TILES = 160
    }

    private val preferences = DetectionPreferences(application)
    private val routeStorage = RouteStorageManager(application, com.google.gson.Gson())
    private val gson = com.google.gson.Gson()

    private val _uiState = MutableStateFlow(DetectionUiState())
    val uiState: StateFlow<DetectionUiState> = _uiState.asStateFlow()

    private val _events = MutableStateFlow<List<PipelineEvent>>(emptyList())
    val events: StateFlow<List<PipelineEvent>> = _events.asStateFlow()

    private val _activeRoute = MutableStateFlow<RouteData?>(null)
    val activeRoute: StateFlow<RouteData?> = _activeRoute.asStateFlow()

    private val _activeRouteMetadata =
            MutableStateFlow<com.busarrival.app.domain.model.RouteMetadata?>(null)
    val activeRouteMetadata: StateFlow<com.busarrival.app.domain.model.RouteMetadata?> =
            _activeRouteMetadata.asStateFlow()

    // Replay state for timeline/replay functionality
    private val _replayState = MutableStateFlow(ReplayState())
    val replayState: StateFlow<ReplayState> = _replayState.asStateFlow()

    // Map state - persists across screen switches
    private val _mapScale = MutableStateFlow(1f)
    val mapScale: StateFlow<Float> = _mapScale.asStateFlow()

    private val _mapOffset = MutableStateFlow(Offset.Zero)
    val mapOffset: StateFlow<Offset> = _mapOffset.asStateFlow()

    private val _mapLabelZoomBias = MutableStateFlow(preferences.mapLabelZoomBias)
    val mapLabelZoomBias: StateFlow<Int> = _mapLabelZoomBias.asStateFlow()

    private val _cameraFollowEnabled = MutableStateFlow(preferences.cameraFollowEnabled)
    val cameraFollowEnabled: StateFlow<Boolean> = _cameraFollowEnabled.asStateFlow()

    private val _gpsLoggingEnabled = MutableStateFlow(preferences.gpsLoggingEnabled)
    val gpsLoggingEnabled: StateFlow<Boolean> = _gpsLoggingEnabled.asStateFlow()

    private val _lastGpsLogReference = MutableStateFlow(preferences.lastGpsLogReference)
    val lastGpsLogReference: StateFlow<String?> = _lastGpsLogReference.asStateFlow()

    private val _gpsLogActive = MutableStateFlow(false)
    val gpsLogActive: StateFlow<Boolean> = _gpsLogActive.asStateFlow()

    private val _tileCache = MutableStateFlow<Map<String, ImageBitmap>>(emptyMap())
    val tileCache: StateFlow<Map<String, ImageBitmap>> = _tileCache.asStateFlow()

    private val _gpsFixState =
            MutableStateFlow<com.busarrival.app.domain.model.GpsFixState>(
                    com.busarrival.app.domain.model.GpsFixState.NoSignal
            )
    val gpsFixState: StateFlow<com.busarrival.app.domain.model.GpsFixState> =
            _gpsFixState.asStateFlow()

    private val _eventHints = MutableStateFlow<com.busarrival.app.domain.model.EventHint?>(null)
    val eventHints: StateFlow<com.busarrival.app.domain.model.EventHint?> =
            _eventHints.asStateFlow()

    // Playback state
    private var playbackJob: Job? = null
    private var replayEvents: List<PipelineEvent> = emptyList()
    private var simulationLogReference: String? = null
    private var simulationLogName: String? = null

    private var service: DetectionService? = null
    private var serviceEventJob: Job? = null
    private var gpsLogStatusJob: Job? = null
    private val stopEventCallback =
            object : StopEventCallback {
                override fun onStopEvent(event: StopUiEvent) {
                    handleStopUiEvent(event)
                }
            }

    private val serviceConnection =
            object : ServiceConnection {
                override fun onServiceConnected(name: ComponentName?, binder: IBinder?) {
                    val localBinder = binder as DetectionService.LocalBinder
                    service = localBinder.getService()
                    val connectedService = service ?: return
                    connectedService.setStopEventCallback(stopEventCallback)

                    // Subscribe to service events
                    serviceEventJob?.cancel()
                    serviceEventJob =
                            viewModelScope.launch {
                                connectedService.events.collect { event ->
                                    event?.let { handleServiceEvent(it) }
                                }
                            }

                    gpsLogStatusJob?.cancel()
                    gpsLogStatusJob =
                            viewModelScope.launch {
                                connectedService.gpsLogStatus.collect { status ->
                                    handleGpsLogStatus(status)
                                }
                            }

                    connectedService.gpsLogStatus.value.let { handleGpsLogStatus(it) }

                    // Update running state
                    _uiState.value = _uiState.value.copy(isRunning = true)
                }

                override fun onServiceDisconnected(name: ComponentName?) {
                    service?.setStopEventCallback(null)
                    service = null
                    serviceEventJob?.cancel()
                    serviceEventJob = null
                    gpsLogStatusJob?.cancel()
                    gpsLogStatusJob = null
                    _uiState.value = _uiState.value.copy(isRunning = false)
                }
            }

    private fun handleStopUiEvent(event: StopUiEvent) {
        val hintType =
                when (event.event) {
                    StopLifecycleEvent.Approaching -> HintType.APPROACHING
                    StopLifecycleEvent.Arriving -> HintType.ARRIVING
                    StopLifecycleEvent.Arrived -> HintType.ATSTOP
                    StopLifecycleEvent.Departed -> HintType.DEPART
                    StopLifecycleEvent.None -> return
                }
        _eventHints.value =
                EventHint(type = hintType, stopIndex = event.stopIndex, timestamp = event.timestamp)
    }

    init {
        loadActiveRoute()
        // Initialize TraceStorageManager
        TraceStorageManager.init(getApplication(), gson)
    }

    /** Load active route from preferences. */
    fun loadActiveRoute() {
        val activeUuid = preferences.activeRouteUuid
        android.util.Log.d("DetectionViewModel", "Loading active route: $activeUuid")
        if (activeUuid != null) {
            val route = routeStorage.loadRoute(activeUuid)
            _activeRoute.value = route

            // Load route metadata for name display
            val allMetadata = routeStorage.loadAllMetadata()
            val metadata = allMetadata.find { it.uuid == activeUuid }
            _activeRouteMetadata.value = metadata

            android.util.Log.d(
                    "DetectionViewModel",
                    "Route loaded: ${route != null}, nodes: ${route?.nodes?.size ?: 0}, name: ${metadata?.name}"
            )
            if (route == null) {
                _uiState.value = _uiState.value.copy(error = "Active route not found")
            }
        }
    }

    private fun computeGpsFixState(
            accuracyM: Float,
            satellites: Int,
            bearing: Float?
    ): com.busarrival.app.domain.model.GpsFixState {
        return when {
            accuracyM == Float.MAX_VALUE -> com.busarrival.app.domain.model.GpsFixState.NoSignal
            accuracyM > 20f -> com.busarrival.app.domain.model.GpsFixState.Searching
            satellites < 6 -> com.busarrival.app.domain.model.GpsFixState.Acquiring(satellites)
            else ->
                    com.busarrival.app.domain.model.GpsFixState.Ready(
                            accuracyM,
                            satellites,
                            bearing
                    )
        }
    }

    /** Handle events from DetectionService. */
    private fun handleServiceEvent(event: PipelineEvent) {
        when (event) {
            is PipelineEvent.PositionUpdate -> {
                android.util.Log.d(
                        "DetectionViewModel",
                        "handleServiceEvent: stop=${event.activeStopIndex} state=${event.activeStopState} sCm=${event.sCm}"
                )
                _uiState.value =
                        _uiState.value.copy(
                                sCm = event.sCm,
                                vCms = event.vCms,
                                mode = event.mode,
                                currentStop = event.activeStopIndex,
                                currentStopState = event.activeStopState,
                                gpsLat = event.lat,
                                gpsLon = event.lon,
                                gpsBearing = event.bearing
                        )
                // Update GPS fix state
                _gpsFixState.value =
                        computeGpsFixState(
                                accuracyM = event.accuracyM,
                                satellites = event.satellites,
                                bearing = event.bearing
                        )
            }
            is PipelineEvent.Arrival -> {
                _uiState.value = _uiState.value.copy(currentStop = event.stopIndex)
            }
            is PipelineEvent.Departure -> {}
        }

        // Add to event list
        val current = _events.value.toMutableList()
        current.add(0, event)
        _events.value = current.take(100)
    }

    /** Check location permissions. */
    fun hasLocationPermission(): Boolean {
        return ContextCompat.checkSelfPermission(
                getApplication(),
                Manifest.permission.ACCESS_FINE_LOCATION
        ) == PackageManager.PERMISSION_GRANTED
    }

    /** Check background location permission. */
    fun hasBackgroundLocationPermission(): Boolean {
        return ContextCompat.checkSelfPermission(
                getApplication(),
                Manifest.permission.ACCESS_BACKGROUND_LOCATION
        ) == PackageManager.PERMISSION_GRANTED
    }

    /** Start detection service and bind to it. */
    fun startDetection() {
        if (_activeRoute.value == null) {
            _uiState.value = _uiState.value.copy(error = "No active route loaded")
            return
        }

        clearGpsLogSimulation()
        DetectionService.startService(getApplication())
        val intent = Intent(getApplication<Application>(), DetectionService::class.java)
        getApplication<Application>()
                .bindService(intent, serviceConnection, Context.BIND_AUTO_CREATE)
    }

    /** Stop detection service and unbind. */
    fun stopDetection() {
        service?.setStopEventCallback(null)
        service?.let { getApplication<Application>().unbindService(serviceConnection) }
        service = null
        DetectionService.stopService(getApplication<Application>())
        _uiState.value = _uiState.value.copy(isRunning = false)
    }

    fun consumePendingGpsLogSimulation() {
        val pending = preferences.consumePendingSimulationGpsLog() ?: return
        loadGpsLogSimulation(pending.first, pending.second)
    }

    private fun loadGpsLogSimulation(reference: String, displayName: String) {
        viewModelScope.launch {
            val fixes =
                    RecordedGpsLogParser.parseLines(
                                    GpsLogStorageManager.loadLog(getApplication(), reference)
                            )
                            .getOrElse {
                                _uiState.value =
                                        _uiState.value.copy(
                                                error = it.message ?: "Failed to load GPS log"
                                        )
                                return@launch
                            }
            val duration = (fixes.last().timeMillis - fixes.first().timeMillis).coerceAtLeast(0L)
            simulationLogReference = reference
            simulationLogName = displayName
            replayEvents = emptyList()
            playbackJob?.cancel()
            val truncatedName =
                    displayName.take(20).let {
                        if (it.length < displayName.length) "$it..." else it
                    }
            _replayState.value =
                    ReplayState(
                            currentTime = 0,
                            isPlaying = false,
                            playbackSpeed = 1f,
                            traceDuration = duration,
                            traceFile = truncatedName
                    )
            _uiState.value = _uiState.value.copy(mode = "Simulating $truncatedName", error = null)
        }
    }

    private fun clearGpsLogSimulation() {
        simulationLogReference = null
        simulationLogName = null
        playbackJob?.cancel()
        if (replayEvents.isEmpty() && _replayState.value.traceFile != null) {
            _replayState.value = ReplayState()
        }
    }

    /** Clear error message. */
    fun clearError() {
        _uiState.value = _uiState.value.copy(error = null)
    }

    /** Update map scale. */
    fun updateMapScale(scale: Float) {
        _mapScale.value = scale
    }

    /** Update map offset. */
    fun updateMapOffset(offset: Offset) {
        _mapOffset.value = offset
    }

    /** Update map state (scale + offset). */
    fun updateMapState(scale: Float, offset: Offset) {
        _mapScale.value = scale
        _mapOffset.value = offset
    }

    /** Update label zoom bias for raster map tiles. */
    fun setMapLabelZoomBias(bias: Int) {
        val clampedBias = bias.coerceIn(0, 2)
        preferences.mapLabelZoomBias = clampedBias
        _mapLabelZoomBias.value = clampedBias
        clearTileCache()
    }

    /** Reload label zoom bias from shared preferences. */
    fun refreshMapLabelZoomBias() {
        _mapLabelZoomBias.value = preferences.mapLabelZoomBias
    }

    /** Reload GPS logging toggle from shared preferences. */
    fun refreshGpsLoggingEnabled() {
        _gpsLoggingEnabled.value = preferences.gpsLoggingEnabled
    }

    /** Reload the last known GPS log reference from shared preferences. */
    fun refreshLastGpsLogReference() {
        _lastGpsLogReference.value = preferences.lastGpsLogReference
    }

    /** Toggle GPS logging preference. */
    fun toggleGpsLogging() {
        val enabled = !preferences.gpsLoggingEnabled
        preferences.gpsLoggingEnabled = enabled
        _gpsLoggingEnabled.value = enabled
    }

    fun toggleCameraFollow() {
        val enabled = !_cameraFollowEnabled.value
        preferences.cameraFollowEnabled = enabled
        _cameraFollowEnabled.value = enabled
    }

    fun disableCameraFollow() {
        preferences.cameraFollowEnabled = false
        _cameraFollowEnabled.value = false
    }

    fun shareGpsLog(context: Context) {
        val reference = _lastGpsLogReference.value ?: return
        if (service?.gpsLogStatus?.value is GpsLogStatus.Active) return
        GpsLogActions.share(context, reference)
    }

    fun deleteGpsLog(context: Context): Boolean {
        val reference = _lastGpsLogReference.value ?: return false
        if (service?.gpsLogStatus?.value is GpsLogStatus.Active) return false
        val deleted = GpsLogActions.delete(context, reference)
        if (deleted) {
            preferences.lastGpsLogReference = null
            _lastGpsLogReference.value = null
        }
        return deleted
    }

    /** Add tiles to cache. */
    fun addTiles(tiles: Map<String, ImageBitmap>) {
        val current = LinkedHashMap(_tileCache.value)
        current.putAll(tiles)
        trimTileCache(current)
        _tileCache.value = current
    }

    /** Clear tile cache (for testing/debugging). */
    fun clearTileCache() {
        _tileCache.value = emptyMap()
        android.util.Log.d("DetectionViewModel", "Tile cache cleared")
    }

    private fun handleGpsLogStatus(status: GpsLogStatus) {
        when (status) {
            is GpsLogStatus.Active -> {
                preferences.lastGpsLogReference = status.description
                _lastGpsLogReference.value = status.description
                _gpsLogActive.value = true
            }
            is GpsLogStatus.Disabled -> {
                _gpsLogActive.value = false
            }
        }
    }

    private fun trimTileCache(cache: LinkedHashMap<String, ImageBitmap>) {
        while (cache.size > MAX_MEMORY_TILES) {
            val oldestKey = cache.entries.iterator().next().key
            cache.remove(oldestKey)
        }
    }

    // ==================== Replay/Timeline Functions ====================

    /** Load trace file and initialize replay state. */
    fun loadTrace(traceFile: String) {
        viewModelScope.launch {
            try {
                val events = TraceStorageManager.loadTrace(traceFile)
                replayEvents = events

                // Estimate duration based on number of events
                // Assuming 1 event per second (1Hz GPS), duration = event count * 1000ms
                val duration = events.size * 1000L

                _replayState.value =
                        ReplayState(
                                currentTime = 0,
                                isPlaying = false,
                                playbackSpeed = 1f,
                                traceDuration = duration,
                                traceFile = traceFile
                        )

                android.util.Log.d(
                        "DetectionViewModel",
                        "Loaded trace: $traceFile, events: ${events.size}, duration: ${duration}ms"
                )
            } catch (e: Exception) {
                android.util.Log.e("DetectionViewModel", "Failed to load trace: $traceFile", e)
                _uiState.value = _uiState.value.copy(error = "Failed to load trace: ${e.message}")
            }
        }
    }

    /** Toggle playback state. */
    fun playPause() {
        simulationLogReference?.let { reference ->
            val current = _replayState.value
            if (current.isPlaying) {
                playbackJob?.cancel()
                playbackJob = null
                DetectionService.stopService(getApplication<Application>())
                _replayState.value = current.copy(isPlaying = false)
                _uiState.value = _uiState.value.copy(isRunning = false)
            } else {
                _replayState.value = current.copy(isPlaying = true)
                DetectionService.startSimulation(
                        getApplication(),
                        reference,
                        simulationLogName ?: current.traceFile.orEmpty(),
                        current.playbackSpeed
                )
                val intent = Intent(getApplication<Application>(), DetectionService::class.java)
                getApplication<Application>()
                        .bindService(intent, serviceConnection, Context.BIND_AUTO_CREATE)
                startPlayback()
            }
            return
        }

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
            android.util.Log.d(
                    "DetectionViewModel",
                    "Playback started from ${current.currentTime}ms"
            )
        }
    }

    /**
     * Set playback speed.
     * @param speed Speed multiplier (0.5x, 1x, 2x, 4x)
     */
    fun setPlaybackSpeed(speed: Float) {
        val clampedSpeed = SupportedGpsPlaybackSpeeds.clamp(speed)
        _replayState.value = _replayState.value.copy(playbackSpeed = clampedSpeed)
        android.util.Log.d("DetectionViewModel", "Playback speed set to ${speed}x")

        if (simulationLogReference != null) {
            DetectionService.setSimulationSpeed(getApplication(), clampedSpeed)
        }

        // Restart playback if currently playing to apply new speed
        if (_replayState.value.isPlaying && simulationLogReference == null) {
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
        _replayState.value =
                _replayState.value.copy(
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

    /** Update current replay position (called during playback). */
    private fun updateReplayPosition(position: Long) {
        _replayState.value =
                _replayState.value.copy(
                        currentTime = position.coerceIn(0, _replayState.value.traceDuration)
                )
        // Only use replay-based UI updates for trace file replay
        // GPS log replay uses service events (handleServiceEvent)
        if (simulationLogReference == null) {
            updateUiForPosition(position)
        }
    }

    /** Start playback coroutine. */
    private fun startPlayback() {
        playbackJob =
                viewModelScope.launch {
                    val startTime = _replayState.value.currentTime
                    val speed = _replayState.value.playbackSpeed
                    val targetDuration = _replayState.value.traceDuration

                    if (targetDuration <= 0) {
                        _uiState.value = _uiState.value.copy(error = "Invalid trace duration")
                        _replayState.value = _replayState.value.copy(isPlaying = false)
                        return@launch
                    }

                    val tickDelayMs = (50 / speed).toLong() // Update every 50ms adjusted for speed

                    while (_replayState.value.isPlaying &&
                            _replayState.value.currentTime < targetDuration) {
                        kotlinx.coroutines.delay(tickDelayMs)

                        val increment = (50 * speed).toLong() // Increment based on speed
                        val newPosition = _replayState.value.currentTime + increment
                        updateReplayPosition(newPosition)
                    }

                    // Playback finished
                    if (_replayState.value.currentTime >= targetDuration) {
                        _replayState.value = _replayState.value.copy(isPlaying = false)
                        android.util.Log.d(
                                "DetectionViewModel",
                                "Playback finished at ${_replayState.value.currentTime}ms"
                        )
                    }
                }
    }

    /**
     * Update UI state for a given replay position. Finds the relevant PositionUpdate events and
     * applies them to UI.
     *
     * Priority: PositionUpdate embedded state (source of truth) → Arrival/Departure fallback
     *
     * LIMITATION: PositionUpdate events don't have timestamps, so we map position to event index.
     * This is approximate since events aren't guaranteed to be evenly spaced in time. A more
     * accurate approach would require adding timestamps to PipelineEvent.
     */
    private fun updateUiForPosition(position: Long) {
        // Find the most recent PositionUpdate before this position
        val positionUpdates = replayEvents.filterIsInstance<PipelineEvent.PositionUpdate>()
        val latestUpdate =
                positionUpdates.lastOrNull { event ->
                    // Map position to event index (approximate - see limitation above)
                    val eventIndex = replayEvents.indexOf(event)
                    val targetIndex =
                            (position.toFloat() / _replayState.value.traceDuration *
                                            replayEvents.size)
                                    .toInt()
                    eventIndex <= targetIndex
                }

        if (latestUpdate != null) {
            // PositionUpdate has embedded stop state from StateMachine - use it as source of truth
            android.util.Log.d(
                    "DetectionViewModel",
                    "updateUiForPosition: from PositionUpdate stop=${latestUpdate.activeStopIndex} state=${latestUpdate.activeStopState}"
            )
            _uiState.value =
                    _uiState.value.copy(
                            sCm = latestUpdate.sCm,
                            vCms = latestUpdate.vCms,
                            mode = latestUpdate.mode,
                            currentStop = latestUpdate.activeStopIndex,
                            currentStopState = latestUpdate.activeStopState
                    )
            return
        }

        // Fallback: No PositionUpdate found, compute from Arrival/Departure events
        android.util.Log.d(
                "DetectionViewModel",
                "updateUiForPosition: NO PositionUpdate, fallback to Arrival/Departure"
        )
        val eventIndex =
                (position.toFloat() / _replayState.value.traceDuration * replayEvents.size).toInt()
        val currentEvents = replayEvents.take(eventIndex)

        val lastArrival = currentEvents.filterIsInstance<PipelineEvent.Arrival>().lastOrNull()
        val lastDeparture = currentEvents.filterIsInstance<PipelineEvent.Departure>().lastOrNull()

        // Update current stop based on last arrival/departure
        // State machine: at stop if arrived after last departure, otherwise at next stop
        val currentStop =
                when {
                    lastArrival == null && lastDeparture == null -> -1 // No stop events yet
                    lastDeparture == null -> lastArrival?.stopIndex ?: -1 // Arrived, never departed
                    lastArrival == null -> lastDeparture.stopIndex + 1 // Departed, never arrived
                    (lastArrival?.stopIndex ?: -1) > lastDeparture.stopIndex ->
                            lastArrival?.stopIndex ?: -1 // Arrived after departure
                    else -> lastDeparture.stopIndex + 1 // Departed after arrival
                }

        android.util.Log.d(
                "DetectionViewModel",
                "updateUiForPosition: fallback FINAL stop=$currentStop state=${if (currentStop >= 0) _uiState.value.currentStopState else "Idle"}"
        )

        _uiState.value =
                _uiState.value.copy(
                        currentStop = currentStop,
                        currentStopState =
                                if (currentStop >= 0) _uiState.value.currentStopState else "Idle"
                )
    }

    override fun onCleared() {
        super.onCleared()
        playbackJob?.cancel()
        serviceEventJob?.cancel()
        gpsLogStatusJob?.cancel()
        service?.setStopEventCallback(null)
        service?.let { getApplication<Application>().unbindService(serviceConnection) }
    }
}

/** Detection UI state. */
data class DetectionUiState(
        val isRunning: Boolean = false,
        val currentStop: Int = -1,
        val currentStopState: String = "Idle",
        val sCm: Int = 0,
        val vCms: Int = 0,
        val mode: String = "Normal",
        val error: String? = null,
        val gpsLat: Double = 0.0,
        val gpsLon: Double = 0.0,
        val gpsBearing: Float? = null
)

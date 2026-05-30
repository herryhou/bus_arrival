package com.busarrival.app.service

import android.app.Notification
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.Uri
import android.os.Binder
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationCompat
import com.busarrival.app.R
import com.busarrival.app.data.gpslog.GpsLogStorageManager
import com.busarrival.app.data.gpslog.GpsSimulationTiming
import com.busarrival.app.data.gpslog.RecordedGpsLogParser
import com.busarrival.app.data.gpslog.SupportedGpsPlaybackSpeeds
import com.busarrival.app.data.preferences.DetectionPreferences
import com.busarrival.app.data.storage.RouteStorageManager
import com.busarrival.app.presentation.MainActivity
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch

/**
 * Foreground service for GPS processing and arrival detection. Full pipeline: Location → MapMatcher
 * → Kalman → StateMachine → Events
 */
class DetectionService : Service() {

    private lateinit var routeStorage: RouteStorageManager
    private lateinit var preferences: DetectionPreferences

    private val serviceScope = CoroutineScope(Dispatchers.Default + Job())
    private val binder = LocalBinder()

    private lateinit var locationManager: LocationManager
    private lateinit var gpsLogWriter: GpsLogWriter
    private var simulationJob: Job? = null
    @Volatile private var simulationPlaybackSpeed: Float = 1f
    @Volatile private var sourceGeneration: Long = 0L
    private var sourceMode: DetectionSourceMode = DetectionSourceMode.Stopped

    private val _isRunning = MutableStateFlow(false)
    val isRunning: StateFlow<Boolean> = _isRunning

    private val _events = MutableStateFlow<PipelineEvent?>(null)
    val events: StateFlow<PipelineEvent?> = _events

    private val _gpsLogStatus = MutableStateFlow<GpsLogStatus>(GpsLogStatus.Disabled("Not started"))
    val gpsLogStatus: StateFlow<GpsLogStatus> = _gpsLogStatus

    // Pipeline state
    private var activeRoute: com.busarrival.app.domain.model.RouteData? = null
    private var detectionPipeline = DetectionPipeline()
    private var stopEventCallback: StopEventCallback? = null

    inner class LocalBinder : Binder() {
        fun getService(): DetectionService = this@DetectionService
    }

    fun setStopEventCallback(callback: StopEventCallback?) {
        stopEventCallback = callback
    }

    override fun onCreate() {
        super.onCreate()
        routeStorage = RouteStorageManager(this, com.google.gson.Gson())
        preferences = DetectionPreferences(this)
        locationManager = LocationManager(this)
        gpsLogWriter = GpsLogWriter(NoopGpsLogStore())
    }

    override fun onBind(intent: Intent?): IBinder {
        return binder
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_START -> startDetection()
            ACTION_START_SIMULATION -> {
                val reference = intent.getStringExtra(EXTRA_GPS_LOG_REFERENCE)
                val displayName = intent.getStringExtra(EXTRA_GPS_LOG_NAME) ?: reference
                val speed = intent.getFloatExtra(EXTRA_PLAYBACK_SPEED, 1f)
                if (reference.isNullOrBlank()) {
                    emitError("No GPS log selected")
                } else {
                    startSimulation(reference, displayName.orEmpty(), speed)
                }
            }
            ACTION_SET_SIMULATION_SPEED ->
                    simulationPlaybackSpeed =
                            SupportedGpsPlaybackSpeeds.clamp(
                                    intent.getFloatExtra(EXTRA_PLAYBACK_SPEED, 1f)
                            )
            ACTION_STOP -> stopDetection()
        }
        return START_STICKY
    }

    private fun startDetection() {
        if (_isRunning.value && sourceMode == DetectionSourceMode.Live) return
        if (_isRunning.value) stopDetection()

        // Load active route
        val activeUuid = preferences.activeRouteUuid
        if (activeUuid == null) {
            emitError("No active route configured")
            return
        }

        activeRoute = routeStorage.loadRoute(activeUuid)
        if (activeRoute == null) {
            emitError("Failed to load active route")
            return
        }

        if (!locationManager.hasLocationPermission()) {
            emitError("Location permission not granted")
            return
        }

        // Initialize stop state machines
        initializePipeline()
        sourceMode = DetectionSourceMode.Live
        sourceGeneration++

        // Start foreground service with the service type only on API 29+.
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            startForeground(
                    NOTIFICATION_ID,
                    createNotification(),
                    ServiceInfo.FOREGROUND_SERVICE_TYPE_LOCATION
            )
        } else {
            startForeground(NOTIFICATION_ID, createNotification())
        }

        if (preferences.gpsLoggingEnabled) {
            gpsLogWriter = GpsLogWriter(createGpsLogStore())
            _gpsLogStatus.value = gpsLogWriter.open(routeId = activeUuid)
        } else {
            gpsLogWriter.close()
            gpsLogWriter = GpsLogWriter(NoopGpsLogStore())
            _gpsLogStatus.value = GpsLogStatus.Disabled("Logging disabled")
        }

        // Start location updates
        serviceScope.launch {
            locationManager.startLocationUpdates { location ->
                processLocation(location, sourceGeneration)
            }

            _isRunning.value = true
        }
    }

    private fun startSimulation(reference: String, displayName: String, speed: Float) {
        if (_isRunning.value) stopDetection()

        val activeUuid = preferences.activeRouteUuid
        if (activeUuid == null) {
            emitError("No active route configured")
            return
        }

        activeRoute = routeStorage.loadRoute(activeUuid)
        if (activeRoute == null) {
            emitError("Failed to load active route")
            return
        }

        if (!locationManager.hasLocationPermission()) {
            emitError("Location permission not granted")
            return
        }

        initializePipeline()
        sourceMode = DetectionSourceMode.Simulation
        sourceGeneration++
        val generation = sourceGeneration
        simulationPlaybackSpeed = SupportedGpsPlaybackSpeeds.clamp(speed)
        gpsLogWriter.close()
        gpsLogWriter = GpsLogWriter(NoopGpsLogStore())
        _gpsLogStatus.value = GpsLogStatus.Disabled("Simulation mode")
        _isRunning.value = true

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            startForeground(
                    NOTIFICATION_ID,
                    createNotification("Simulating ${displayName.ifBlank { "GPS log" }}"),
                    ServiceInfo.FOREGROUND_SERVICE_TYPE_LOCATION
            )
        } else {
            startForeground(
                    NOTIFICATION_ID,
                    createNotification("Simulating ${displayName.ifBlank { "GPS log" }}")
            )
        }

        simulationJob =
                serviceScope.launch {
                    val fixes =
                            RecordedGpsLogParser.parseLines(
                                            GpsLogStorageManager.loadLog(
                                                    this@DetectionService,
                                                    reference
                                            )
                                    )
                                    .getOrElse {
                                        if (generation == sourceGeneration) {
                                            emitError(it.message ?: "Failed to load GPS log")
                                            stopDetection()
                                        }
                                        return@launch
                                    }

                    var previousTime = fixes.first().timeMillis
                    fixes.forEachIndexed { index, fix ->
                        if (generation != sourceGeneration ||
                                        sourceMode != DetectionSourceMode.Simulation
                        ) {
                            return@launch
                        }
                        if (index > 0) {
                            delay(
                                    GpsSimulationTiming.delayMillis(
                                            previousTime,
                                            fix.timeMillis,
                                            simulationPlaybackSpeed
                                    )
                            )
                        }
                        previousTime = fix.timeMillis
                        processLocation(fix.toLocation(), generation)
                    }
                    if (generation == sourceGeneration &&
                                    sourceMode == DetectionSourceMode.Simulation
                    ) {
                        stopDetection()
                    }
                }
    }

    private fun stopDetection() {
        if (!_isRunning.value) {
            simulationJob?.cancel()
            simulationJob = null
            gpsLogWriter.close()
            _gpsLogStatus.value = GpsLogStatus.Disabled("Not running")
            return
        }

        sourceGeneration++
        sourceMode = DetectionSourceMode.Stopped
        simulationJob?.cancel()
        simulationJob = null
        locationManager.stopLocationUpdates()
        _isRunning.value = false
        gpsLogWriter.close()
        _gpsLogStatus.value = GpsLogStatus.Disabled("Not running")
        resetPipeline()

        // Stop foreground service
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    private fun initializePipeline() {
        val route = activeRoute ?: return

        detectionPipeline = DetectionPipeline()
        detectionPipeline.initialize(route)
    }

    private fun resetPipeline() {
        detectionPipeline.close()
        detectionPipeline = DetectionPipeline()
        activeRoute = null
    }

    private fun processLocation(
            location: android.location.Location,
            generation: Long = sourceGeneration
    ) {
        if (generation != sourceGeneration) return
        gpsLogWriter.append(location)
        if (activeRoute == null) return

        serviceScope.launch {
            if (generation != sourceGeneration) return@launch
            try {
                val gps = com.busarrival.app.domain.model.GpsPoint.fromLocation(location)
                val result = detectionPipeline.process(location)

                when (result) {
                    PipelineResult.NotInitialized -> return@launch
                    is PipelineResult.Acquiring -> {
                        _events.value =
                                PipelineEvent.PositionUpdate(
                                        sCm = 0,
                                        vCms = 0,
                                        mode = "acquiring",
                                        activeStopIndex = -1,
                                        activeStopState = "Idle",
                                        accuracyM = gps.accuracyM ?: Float.MAX_VALUE,
                                        satellites = gps.hdop?.toInt() ?: 0,
                                        bearing = gps.headingCdeg?.toFloat()?.div(100f),
                                        lat = gps.lat,
                                        lon = gps.lon
                                )
                    }
                    is PipelineResult.Success -> {
                        result.stopEvents.forEach { event -> stopEventCallback?.onStopEvent(event) }
                        result.arrivals.forEach { arrival ->
                            _events.value =
                                    PipelineEvent.Arrival(
                                            stopIndex = arrival.stopIndex,
                                            probability = arrival.probability.value
                                    )
                        }
                        result.departures.forEach { departure ->
                            _events.value =
                                    PipelineEvent.Departure(
                                            stopIndex = departure.stopIndex,
                                            dwellTimeS = departure.dwellTimeS
                                    )
                        }

                        _events.value =
                                PipelineEvent.PositionUpdate(
                                        sCm = result.sCm,
                                        vCms = result.vCms,
                                        mode = result.mode,
                                        activeStopIndex = result.activeStopIndex,
                                        activeStopState = result.activeStopState,
                                        accuracyM = gps.accuracyM ?: Float.MAX_VALUE,
                                        satellites = gps.hdop?.toInt() ?: 0,
                                        bearing = gps.headingCdeg?.toFloat()?.div(100f),
                                        lat = gps.lat,
                                        lon = gps.lon
                                )
                    }
                }
            } catch (e: Exception) {
                emitError("Pipeline error: ${e.message}")
            }
        }
    }

    private fun emitError(message: String) {
        _events.value = PipelineEvent.PositionUpdate(0, 0, "Error: $message")
    }

    private fun createGpsLogStore(): GpsLogStore {
        val treeUri = preferences.gpsLogTreeUri
        if (treeUri != null) {
            return SafGpsLogStore(contentResolver, Uri.parse(treeUri))
        }
        val logDir = File(getExternalFilesDir(null) ?: filesDir, "gps-logs")
        return FileGpsLogStore(logDir)
    }

    private fun createNotification(
            contentText: String = "Processing GPS updates..."
    ): Notification {
        val intent = Intent(this, MainActivity::class.java)
        val pendingIntent =
                PendingIntent.getActivity(
                        this,
                        0,
                        intent,
                        PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT
                )

        return NotificationCompat.Builder(this, CHANNEL_ID)
                .setContentTitle(getString(R.string.service_notification_title))
                .setContentText(contentText)
                .setSmallIcon(R.drawable.ic_launcher_foreground)
                .setContentIntent(pendingIntent)
                .setOngoing(true)
                .build()
    }

    override fun onDestroy() {
        super.onDestroy()
        stopDetection()
        serviceScope.cancel()
    }

    companion object {
        const val CHANNEL_ID = "detection_service_channel"
        const val NOTIFICATION_ID = 1

        const val ACTION_START = "com.busarrival.app.START_DETECTION"
        const val ACTION_STOP = "com.busarrival.app.STOP_DETECTION"
        const val ACTION_START_SIMULATION = "com.busarrival.app.START_GPS_LOG_SIMULATION"
        const val ACTION_SET_SIMULATION_SPEED = "com.busarrival.app.SET_GPS_LOG_SIMULATION_SPEED"
        const val EXTRA_GPS_LOG_REFERENCE = "gps_log_reference"
        const val EXTRA_GPS_LOG_NAME = "gps_log_name"
        const val EXTRA_PLAYBACK_SPEED = "playback_speed"

        fun startService(context: Context) {
            val intent =
                    Intent(context, DetectionService::class.java).apply { action = ACTION_START }
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                context.startForegroundService(intent)
            } else {
                context.startService(intent)
            }
        }

        fun stopService(context: Context) {
            val intent =
                    Intent(context, DetectionService::class.java).apply { action = ACTION_STOP }
            context.startService(intent)
        }

        fun startSimulation(
                context: Context,
                reference: String,
                displayName: String,
                playbackSpeed: Float
        ) {
            val intent =
                    Intent(context, DetectionService::class.java).apply {
                        action = ACTION_START_SIMULATION
                        putExtra(EXTRA_GPS_LOG_REFERENCE, reference)
                        putExtra(EXTRA_GPS_LOG_NAME, displayName)
                        putExtra(EXTRA_PLAYBACK_SPEED, playbackSpeed)
                    }
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                context.startForegroundService(intent)
            } else {
                context.startService(intent)
            }
        }

        fun setSimulationSpeed(context: Context, playbackSpeed: Float) {
            val intent =
                    Intent(context, DetectionService::class.java).apply {
                        action = ACTION_SET_SIMULATION_SPEED
                        putExtra(EXTRA_PLAYBACK_SPEED, playbackSpeed)
                    }
            context.startService(intent)
        }
    }
}

private enum class DetectionSourceMode {
    Stopped,
    Live,
    Simulation
}

interface StopEventCallback {
    fun onStopEvent(event: StopUiEvent)
}

private class NoopGpsLogStore : GpsLogStore {
    override fun create(routeId: String?, startedAtMillis: Long): GpsLogSession {
        return object : GpsLogSession {
            override val description: String = "logging-disabled"
            override fun append(line: String) = Unit
            override fun close() = Unit
        }
    }
}

/** Pipeline event for UI updates. */
sealed class PipelineEvent {
    data class Arrival(val stopIndex: Int, val probability: Int) : PipelineEvent()
    data class Departure(val stopIndex: Int, val dwellTimeS: Int) : PipelineEvent()
    data class PositionUpdate(
            val sCm: Int,
            val vCms: Int,
            val mode: String = "Normal",
            val activeStopIndex: Int = -1,
            val activeStopState: String = "Idle",
            val accuracyM: Float = Float.MAX_VALUE,
            val satellites: Int = 0,
            val bearing: Float? = null,
            val lat: Double = 0.0,
            val lon: Double = 0.0
    ) : PipelineEvent()
}

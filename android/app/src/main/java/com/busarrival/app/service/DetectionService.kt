package com.busarrival.app.service

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Binder
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationCompat
import com.busarrival.app.R
import com.busarrival.app.data.pipeline.detection.probability.ProbabilityModel
import com.busarrival.app.data.pipeline.detection.statemachine.StateMachine
import com.busarrival.app.data.pipeline.localization.kalman.KalmanFilter
import com.busarrival.app.data.pipeline.localization.mapmatcher.MapMatcher
import com.busarrival.app.data.preferences.DetectionPreferences
import com.busarrival.app.data.storage.RouteStorageManager
import com.busarrival.app.presentation.MainActivity
import com.busarrival.app.domain.model.ArrivalEvent
import com.busarrival.app.domain.model.DepartureEvent
import com.busarrival.app.domain.model.FsmState
import com.busarrival.app.domain.model.KalmanState
import com.busarrival.app.domain.model.StopState
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch

/**
 * Foreground service for GPS processing and arrival detection.
 * Full pipeline: Location → MapMatcher → Kalman → StateMachine → Events
 */
class DetectionService : Service() {

    private lateinit var routeStorage: RouteStorageManager
    private lateinit var preferences: DetectionPreferences

    private val serviceScope = CoroutineScope(Dispatchers.Default + Job())
    private val binder = LocalBinder()

    private lateinit var locationManager: LocationManager

    private val _isRunning = MutableStateFlow(false)
    val isRunning: StateFlow<Boolean> = _isRunning

    private val _events = MutableStateFlow<PipelineEvent?>(null)
    val events: StateFlow<PipelineEvent?> = _events

    // Pipeline state
    private var activeRoute: com.busarrival.app.domain.model.RouteData? = null
    private var kalmanState: KalmanState? = null
    private var stopStates: Map<Int, StopState> = emptyMap()
    private var lastGpsTime: Long = 0

    inner class LocalBinder : Binder() {
        fun getService(): DetectionService = this@DetectionService
    }

    override fun onCreate() {
        super.onCreate()
        routeStorage = RouteStorageManager(this, com.google.gson.Gson())
        preferences = DetectionPreferences(this)
        locationManager = LocationManager(this)
    }

    override fun onBind(intent: Intent?): IBinder {
        return binder
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_START -> startDetection()
            ACTION_STOP -> stopDetection()
        }
        return START_STICKY
    }

    private fun startDetection() {
        if (_isRunning.value) return

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

        // Initialize stop state machines
        initializePipeline()

        // Start foreground service
        startForeground(NOTIFICATION_ID, createNotification(), ServiceInfo.FOREGROUND_SERVICE_TYPE_LOCATION)

        // Start location updates
        serviceScope.launch {
            locationManager.startLocationUpdates { location ->
                processLocation(location)
            }

            _isRunning.value = true
        }
    }

    private fun stopDetection() {
        if (!_isRunning.value) return

        locationManager.stopLocationUpdates()
        resetPipeline()
        _isRunning.value = false

        // Stop foreground service
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    private fun initializePipeline() {
        val route = activeRoute ?: return

        // Initialize stop state machines
        stopStates = route.stops.mapIndexed { idx, _ ->
            idx to StateMachine.initialState(idx)
        }.toMap()

        kalmanState = null
        lastGpsTime = 0
    }

    private fun resetPipeline() {
        kalmanState = null
        stopStates = emptyMap()
        lastGpsTime = 0
        activeRoute = null
    }

    private fun processLocation(location: android.location.Location) {
        val route = activeRoute ?: return

        serviceScope.launch {
            try {
                // Convert to GpsPoint
                val gps = com.busarrival.app.domain.model.GpsPoint.fromLocation(location)

                // Check for GPS jump (recovery trigger)
                val isFirstFix = lastGpsTime == 0L
                val dt = if (lastGpsTime > 0) {
                    ((gps.timestamp - lastGpsTime) / 1000).toInt().coerceAtLeast(1)
                } else {
                    1
                }

                // Convert to grid coordinates
                val (xCm, yCm) = GeoCoordinateConverter.toGridCoordinates(gps.lat, gps.lon, route)

                // Phase 1: Map matching
                val lastSegIdx = kalmanState?.lastSegIdx ?: 0
                val matchResult = MapMatcher.match(
                    gpsX = xCm,
                    gpsY = yCm,
                    gpsHeading = gps.headingCdeg,
                    gpsSpeed = gps.speedCms ?: 0,
                    routeData = route,
                    lastIdx = lastSegIdx,
                    isFirstFix = isFirstFix
                )

                // Project to route progress (simplified - uses node position)
                val zCm = route.nodes[matchResult.segIdx].cumDistCm

                // Phase 2: Kalman filter
                if (kalmanState == null) {
                    // First fix: initialize
                    kalmanState = KalmanState.init(
                        zCm = zCm,
                        vGpsCms = gps.speedCms ?: 0,
                        segIdx = matchResult.segIdx
                    )
                }

                val signals = KalmanFilter.update(
                    state = kalmanState!!,
                    zCm = zCm,
                    vGpsCms = gps.speedCms ?: 0,
                    hdopX10 = null,  // Location API doesn't provide HDOP
                    isSoftResync = false
                )

                // Update segment index
                kalmanState!!.lastSegIdx = matchResult.segIdx

                // Phase 3: Detection - process all stops
                for ((idx, stop) in route.stops.withIndex()) {
                    val state = stopStates[idx] ?: continue

                    // Compute probability
                    val probability = ProbabilityModel.compute(
                        signals = signals,
                        stop = stop,
                        vCms = kalmanState!!.vCms,
                        dwellS = state.dwellTimeS
                    )

                    // Update state machine
                    val (arrival, departure) = StateMachine.update(
                        state = state,
                        stop = stop,
                        sCm = signals.sCm,
                        probability = probability,
                        timestamp = gps.timestamp
                    )

                    // Emit arrival event
                    arrival?.let {
                        _events.value = PipelineEvent.Arrival(
                            stopIndex = it.stopIndex,
                            probability = it.probability.value
                        )
                    }

                    // Emit departure event
                    departure?.let {
                        _events.value = PipelineEvent.Departure(
                            stopIndex = it.stopIndex,
                            dwellTimeS = it.dwellTimeS
                        )
                    }
                }

                // Emit position update
                _events.value = PipelineEvent.PositionUpdate(
                    sCm = signals.sCm,
                    vCms = kalmanState!!.vCms,
                    mode = "Normal"
                )

                lastGpsTime = gps.timestamp

            } catch (e: Exception) {
                emitError("Pipeline error: ${e.message}")
            }
        }
    }

    private fun emitError(message: String) {
        _events.value = PipelineEvent.PositionUpdate(0, 0, "Error: $message")
    }

    private fun createNotification(): Notification {
        val intent = Intent(this, MainActivity::class.java)
        val pendingIntent = PendingIntent.getActivity(
            this, 0, intent,
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT
        )

        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle(getString(R.string.service_notification_title))
            .setContentText("Processing GPS updates...")
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

        fun startService(context: Context) {
            val intent = Intent(context, DetectionService::class.java).apply {
                action = ACTION_START
            }
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                context.startForegroundService(intent)
            } else {
                context.startService(intent)
            }
        }

        fun stopService(context: Context) {
            val intent = Intent(context, DetectionService::class.java).apply {
                action = ACTION_STOP
            }
            context.startService(intent)
        }
    }
}

/**
 * Pipeline event for UI updates.
 */
sealed class PipelineEvent {
    data class Arrival(val stopIndex: Int, val probability: Int) : PipelineEvent()
    data class Departure(val stopIndex: Int, val dwellTimeS: Int) : PipelineEvent()
    data class PositionUpdate(val sCm: Int, val vCms: Int, val mode: String = "Normal") : PipelineEvent()
}

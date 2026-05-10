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
import com.busarrival.app.presentation.MainActivity
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch

/**
 * Foreground service for GPS processing and arrival detection.
 * Runs the full pipeline in background.
 */
class DetectionService : Service() {

    private val serviceScope = CoroutineScope(Dispatchers.Default + Job())
    private val binder = LocalBinder()

    private lateinit var locationManager: LocationManager
    private lateinit var pipeline: DetectionPipeline

    private val _isRunning = MutableStateFlow(false)
    val isRunning: StateFlow<Boolean> = _isRunning

    private val _events = MutableStateFlow<List<PipelineEvent>>(emptyList())
    val events: StateFlow<List<PipelineEvent>> = _events

    inner class LocalBinder : Binder() {
        fun getService(): DetectionService = this@DetectionService
    }

    override fun onCreate() {
        super.onCreate()
        locationManager = LocationManager(this)
        pipeline = DetectionPipeline()
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

        // Start foreground service
        startForeground(NOTIFICATION_ID, createNotification(), ServiceInfo.FOREGROUND_SERVICE_TYPE_LOCATION)

        // Initialize pipeline
        serviceScope.launch {
            // TODO: Load route data
            // pipeline.initialize(routeData)

            // Start location updates
            locationManager.startLocationUpdates { location ->
                processLocation(location)
            }

            _isRunning.value = true
        }
    }

    private fun stopDetection() {
        if (!_isRunning.value) return

        locationManager.stopLocationUpdates()
        _isRunning.value = false

        // Stop foreground service
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    private fun processLocation(location: android.location.Location) {
        serviceScope.launch {
            // TODO: Process location through pipeline
            // val result = pipeline.process(location)

            // Emit events
            // _events.value = result.events
        }
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
    data class PositionUpdate(val sCm: Int, val vCms: Int) : PipelineEvent()
}

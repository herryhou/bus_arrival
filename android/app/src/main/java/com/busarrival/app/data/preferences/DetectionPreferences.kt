package com.busarrival.app.data.preferences

import android.content.Context
import android.content.SharedPreferences
import com.busarrival.app.domain.model.DetectionParameters

/**
 * SharedPreferences wrapper for detection settings.
 */
class DetectionPreferences(context: Context) {
    private val prefs: SharedPreferences = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)

    companion object {
        private const val PREFS_NAME = "detection_prefs"
        private const val KEY_ACTIVE_ROUTE = "active_route_uuid"
        private const val KEY_DISTANCE_WEIGHT = "distance_weight"
        private const val KEY_SPEED_WEIGHT = "speed_weight"
        private const val KEY_PROGRESS_ERROR_WEIGHT = "progress_error_weight"
        private const val KEY_DWELL_TIME_WEIGHT = "dwell_time_weight"
        private const val KEY_CORRIDOR_SIZE = "corridor_size"
        private const val KEY_MAP_LABEL_ZOOM_BIAS = "map_label_zoom_bias"
        private const val KEY_GPS_LOG_TREE_URI = "gps_log_tree_uri"
        private const val KEY_GPS_LOG_ENABLED = "gps_log_enabled"
        private const val KEY_LAST_GPS_LOG_REFERENCE = "last_gps_log_reference"
        private const val KEY_PENDING_SIMULATION_GPS_LOG_REFERENCE = "pending_simulation_gps_log_reference"
        private const val KEY_PENDING_SIMULATION_GPS_LOG_NAME = "pending_simulation_gps_log_name"
        private const val DEFAULT_MAP_LABEL_ZOOM_BIAS = 1

        val DEFAULT_PARAMETERS = DetectionParameters(
            distanceWeight = 50,
            speedWeight = 50,
            progressErrorWeight = 50,
            dwellTimeWeight = 50,
            corridorSize = 0
        )
    }

    var activeRouteUuid: String?
        get() = prefs.getString(KEY_ACTIVE_ROUTE, null)
        set(value) = prefs.edit().putString(KEY_ACTIVE_ROUTE, value).apply()

    var distanceWeight: Int
        get() = prefs.getInt(KEY_DISTANCE_WEIGHT, DEFAULT_PARAMETERS.distanceWeight)
        set(value) = prefs.edit().putInt(KEY_DISTANCE_WEIGHT, value.coerceIn(0, 100)).apply()

    var speedWeight: Int
        get() = prefs.getInt(KEY_SPEED_WEIGHT, DEFAULT_PARAMETERS.speedWeight)
        set(value) = prefs.edit().putInt(KEY_SPEED_WEIGHT, value.coerceIn(0, 100)).apply()

    var progressErrorWeight: Int
        get() = prefs.getInt(KEY_PROGRESS_ERROR_WEIGHT, DEFAULT_PARAMETERS.progressErrorWeight)
        set(value) = prefs.edit().putInt(KEY_PROGRESS_ERROR_WEIGHT, value.coerceIn(0, 100)).apply()

    var dwellTimeWeight: Int
        get() = prefs.getInt(KEY_DWELL_TIME_WEIGHT, DEFAULT_PARAMETERS.dwellTimeWeight)
        set(value) = prefs.edit().putInt(KEY_DWELL_TIME_WEIGHT, value.coerceIn(0, 100)).apply()

    var corridorSize: Int
        get() = prefs.getInt(KEY_CORRIDOR_SIZE, DEFAULT_PARAMETERS.corridorSize)
        set(value) = prefs.edit().putInt(KEY_CORRIDOR_SIZE, value.coerceIn(-80, 40)).apply()

    var mapLabelZoomBias: Int
        get() = prefs.getInt(KEY_MAP_LABEL_ZOOM_BIAS, DEFAULT_MAP_LABEL_ZOOM_BIAS)
        set(value) = prefs.edit().putInt(KEY_MAP_LABEL_ZOOM_BIAS, value.coerceIn(0, 2)).apply()

    var gpsLogTreeUri: String?
        get() = prefs.getString(KEY_GPS_LOG_TREE_URI, null)
        set(value) = prefs.edit().putString(KEY_GPS_LOG_TREE_URI, value).apply()

    var gpsLoggingEnabled: Boolean
        get() = prefs.getBoolean(KEY_GPS_LOG_ENABLED, false)
        set(value) = prefs.edit().putBoolean(KEY_GPS_LOG_ENABLED, value).apply()

    var lastGpsLogReference: String?
        get() = prefs.getString(KEY_LAST_GPS_LOG_REFERENCE, null)
        set(value) = prefs.edit().putString(KEY_LAST_GPS_LOG_REFERENCE, value).apply()

    var pendingSimulationGpsLogReference: String?
        get() = prefs.getString(KEY_PENDING_SIMULATION_GPS_LOG_REFERENCE, null)
        set(value) = prefs.edit().putString(KEY_PENDING_SIMULATION_GPS_LOG_REFERENCE, value).apply()

    var pendingSimulationGpsLogName: String?
        get() = prefs.getString(KEY_PENDING_SIMULATION_GPS_LOG_NAME, null)
        set(value) = prefs.edit().putString(KEY_PENDING_SIMULATION_GPS_LOG_NAME, value).apply()

    fun setPendingSimulationGpsLog(reference: String, name: String) {
        prefs.edit()
            .putString(KEY_PENDING_SIMULATION_GPS_LOG_REFERENCE, reference)
            .putString(KEY_PENDING_SIMULATION_GPS_LOG_NAME, name)
            .apply()
    }

    fun consumePendingSimulationGpsLog(): Pair<String, String>? {
        val reference = pendingSimulationGpsLogReference ?: return null
        val name = pendingSimulationGpsLogName ?: reference.substringAfterLast('/')
        prefs.edit()
            .remove(KEY_PENDING_SIMULATION_GPS_LOG_REFERENCE)
            .remove(KEY_PENDING_SIMULATION_GPS_LOG_NAME)
            .apply()
        return reference to name
    }

    fun getParameters(): DetectionParameters = DetectionParameters(
        distanceWeight = distanceWeight,
        speedWeight = speedWeight,
        progressErrorWeight = progressErrorWeight,
        dwellTimeWeight = dwellTimeWeight,
        corridorSize = corridorSize
    )

    fun saveParameters(params: DetectionParameters) {
        distanceWeight = params.distanceWeight
        speedWeight = params.speedWeight
        progressErrorWeight = params.progressErrorWeight
        dwellTimeWeight = params.dwellTimeWeight
        corridorSize = params.corridorSize
    }

    fun clear() {
        prefs.edit().clear().apply()
    }
}

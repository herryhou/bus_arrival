package com.busarrival.app.service

import android.annotation.SuppressLint
import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.os.Looper
import androidx.core.content.ContextCompat
import com.google.android.gms.location.FusedLocationProviderClient
import com.google.android.gms.location.LocationCallback
import com.google.android.gms.location.LocationRequest
import com.google.android.gms.location.LocationResult
import com.google.android.gms.location.LocationServices
import com.google.android.gms.location.Priority

/**
 * Location manager using FusedLocationProviderClient.
 * Provides 1Hz GPS updates for the detection pipeline.
 */
class LocationManager(private val context: Context) {

    private val fusedLocationClient: FusedLocationProviderClient =
        LocationServices.getFusedLocationProviderClient(context)

    private var locationCallback: LocationCallback? = null
    private var onLocationUpdate: ((android.location.Location) -> Unit)? = null

    /**
     * Check if location permissions are granted.
     */
    fun hasLocationPermission(): Boolean {
        return ContextCompat.checkSelfPermission(
            context,
            Manifest.permission.ACCESS_FINE_LOCATION
        ) == PackageManager.PERMISSION_GRANTED
    }

    /**
     * Start location updates with 1Hz frequency.
     */
    @SuppressLint("MissingPermission")
    fun startLocationUpdates(onUpdate: (android.location.Location) -> Unit) {
        if (!hasLocationPermission()) {
            throw SecurityException("Location permission not granted")
        }

        this.onLocationUpdate = onUpdate

        val locationRequest = LocationRequest.Builder(
            Priority.PRIORITY_HIGH_ACCURACY,
            1000  // 1 second interval
        ).apply {
            setMinUpdateIntervalMillis(1000)
            setMinUpdateDistanceMeters(0f)
            setWaitForAccurateLocation(true)
        }.build()

        locationCallback = object : LocationCallback() {
            override fun onLocationResult(result: LocationResult) {
                result.lastLocation?.let { location ->
                    onLocationUpdate?.invoke(location)
                }
            }
        }

        fusedLocationClient.requestLocationUpdates(
            locationRequest,
            locationCallback!!,
            Looper.getMainLooper()
        )
    }

    /**
     * Stop location updates.
     */
    fun stopLocationUpdates() {
        locationCallback?.let {
            fusedLocationClient.removeLocationUpdates(it)
        }
        locationCallback = null
        onLocationUpdate = null
    }

    /**
     * Get last known location.
     */
    fun getLastKnownLocation(): android.location.Location? {
        if (!hasLocationPermission()) return null

        return try {
            fusedLocationClient.lastLocation.result
        } catch (e: SecurityException) {
            null
        }
    }
}

data class GpsMetadata(
    val accuracyM: Float,
    val satellites: Int,
    val bearing: Float?
)

fun extractGpsMetadata(location: android.location.Location): GpsMetadata {
    return GpsMetadata(
        accuracyM = if (location.hasAccuracy()) location.accuracy else Float.MAX_VALUE,
        satellites = location.extras?.getInt("satellites", 0) ?: 0,
        bearing = if (location.hasBearing()) location.bearing else null
    )
}

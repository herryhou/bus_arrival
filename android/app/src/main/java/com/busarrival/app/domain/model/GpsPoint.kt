package com.busarrival.app.domain.model

import android.location.Location
import com.busarrival.app.data.pipeline.types.*

/**
 * GPS data point from Android Location API.
 * Ported from crates/shared/src/lib.rs
 */
data class GpsPoint(
    val timestamp: TimestampMs,
    val lat: Double,  // Degrees (not centidegrees) to preserve precision
    val lon: Double,  // Degrees (not centidegrees) to preserve precision
    val headingCdeg: HeadCdeg?,
    val speedCms: SpeedCms?,
    val hdop: Float?,
    val hasFix: Boolean
) {
    companion object {
        private var callCount = 0
        fun fromLocation(location: Location): GpsPoint {
            val result = GpsPoint(
                timestamp = location.time,
                lat = location.latitude,  // Keep as Double (degrees)
                lon = location.longitude,  // Keep as Double (degrees)
                headingCdeg = location.bearing?.toCdeg(),
                speedCms = location.speed?.toCms(),
                hdop = null, // Location API doesn't provide HDOP directly
                hasFix = true // Location API only provides valid fixes
            )
            // Debug: Log first 5 conversions
            if (callCount < 5) {
                println("GpsPoint.fromLocation #$callCount: lat=${location.latitude}, lon=${location.longitude}")
                callCount++
            }
            return result
        }
    }
}

// Extension functions for Location conversion
private fun Double.toCdeg(): Short = kotlin.math.round(this * 100).toInt().toShort()
private fun Float.toCdeg(): Short? = if (this >= 0) kotlin.math.round(this * 100).toInt().toShort() else null
private fun Float.toCms(): Int? = if (this >= 0) (this * 100).toInt() else null

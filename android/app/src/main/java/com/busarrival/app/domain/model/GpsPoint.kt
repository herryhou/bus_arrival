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
    val accuracyM: Float?,
    val hdop: Float?,
    val accuracyCm: Int?,
    val hasFix: Boolean
) {
    companion object {
        private var callCount = 0
        fun fromLocation(location: Location): GpsPoint {
            val result = GpsPoint(
                timestamp = location.time,
                lat = location.latitude,  // Keep as Double (degrees)
                lon = location.longitude,  // Keep as Double (degrees)
                headingCdeg = location.bearing.toCdeg(),
                speedCms = location.speed.toCms(),
                accuracyM = if (location.hasAccuracy()) location.accuracy else null,
                hdop = null, // Location API doesn't provide HDOP directly
                accuracyCm = if (location.hasAccuracy()) (location.accuracy * 100).toInt() else null,
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
// Note: Android Location.bearing uses 0.0 to indicate "no bearing" (not valid heading 0°)
// Normalize to [-180°, +180°] range to match Rust json_bearing_to_cdeg() behavior
private fun Float.toCdeg(): Short? {
    if (this <= 0) return null  // 0.0 means "no bearing" in Android Location API
    var heading = kotlin.math.round(this * 100).toInt()
    if (heading > 18000) heading -= 36000  // Normalize to [-180°, +180°]
    return heading.toShort()
}
private fun Double.toCdeg(): Short? {
    if (this <= 0) return null
    var heading = kotlin.math.round(this * 100).toInt()
    if (heading > 18000) heading -= 36000
    return heading.toShort()
}
private fun Float.toCms(): Int? = if (this >= 0) (this * 100).toInt() else null

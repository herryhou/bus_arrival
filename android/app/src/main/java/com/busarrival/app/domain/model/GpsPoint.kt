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
                headingCdeg = location.bearing.toCdeg(location),
                speedCms = location.speed.toCms(),
                accuracyM = if (location.hasAccuracy()) location.accuracy else null,
                hdop = null, // Location API doesn't provide HDOP directly
                accuracyCm = if (location.hasAccuracy()) (location.accuracy * 100).toInt() else null,
                hasFix = location.latitude != 0.0 && location.longitude != 0.0  // Infer from Location state
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
// CRITICAL: Use hasBearing() to distinguish missing bearing from valid 0°
// Android Location.bearing defaults to 0.0 when no bearing, but 0° is also a valid heading
// Rust: crates/shared/src/lib.rs:199 - uses Option<HeadCdeg> where 0° is valid
private fun Float.toCdeg(location: Location): Short? {
    if (!location.hasBearing()) return null  // Use hasBearing() to detect missing
    var heading = kotlin.math.round(this * 100).toInt()
    if (heading > 18000) heading -= 36000  // Normalize to [-180°, +180°]
    return heading.toShort()
}
private fun Double.toCdeg(location: Location): Short? {
    if (!location.hasBearing()) return null  // Use hasBearing() to detect missing
    var heading = kotlin.math.round(this * 100).toInt()
    if (heading > 18000) heading -= 36000
    return heading.toShort()
}
private fun Float.toCms(): Int? = if (this >= 0) (this * 100).toInt() else null

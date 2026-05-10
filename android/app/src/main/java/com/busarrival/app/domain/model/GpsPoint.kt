package com.busarrival.app.domain.model

import android.location.Location
import com.busarrival.app.data.pipeline.types.*

/**
 * GPS data point from Android Location API.
 * Ported from crates/shared/src/lib.rs
 */
data class GpsPoint(
    val timestamp: Long,
    val lat: GeoCdeg,
    val lon: GeoCdeg,
    val headingCdeg: HeadCdeg?,
    val speedCms: SpeedCms?,
    val hdop: Float?,
    val hasFix: Boolean
) {
    companion object {
        fun fromLocation(location: Location): GpsPoint {
            return GpsPoint(
                timestamp = location.time,
                lat = location.latitude.toCdeg(),
                lon = location.longitude.toCdeg(),
                headingCdeg = location.bearing?.toCdeg(),
                speedCms = location.speed?.toCms(),
                hdop = null, // Location API doesn't provide HDOP directly
                hasFix = true // Location API only provides valid fixes
            )
        }
    }
}

// Extension functions for Location conversion
private fun Double.toCdeg(): Short = (this * 100).toInt().toShort()
private fun Float.toCdeg(): Short? = if (this >= 0) (this * 100).toInt().toShort() else null
private fun Float.toCms(): Int? = if (this >= 0) (this * 100).toInt() else null

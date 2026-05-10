package com.busarrival.app.data.pipeline.types

import kotlin.math.abs

/**
 * Semantic type system for bus arrival detection.
 * Ported from crates/shared/src/lib.rs
 */

// Type aliases for semantic integers (no runtime overhead)
typealias DistCm = Int           // Distance in centimeters (±214 km)
typealias SpeedCms = Int         // Speed in cm/s (0..214 km/h)
typealias HeadCdeg = Short       // Heading in 0.01° (-180°..+180°)
typealias GeoCdeg = Short        // Lat/lon in 0.01° (-180°..+180°)
typealias Prob8 = UByteWrapper   // Probability × 255 (0..255)
typealias Dist2 = Long           // Squared distance (cm²)

/**
 * Unsigned byte wrapper for Prob8.
 * Kotlin Byte is signed (-128..127), so we use Int for unsigned 0..255.
 */
@JvmInline
value class UByteWrapper(val value: Int) {
    init {
        require(value in 0..255) { "Prob8 must be 0..255, got $value" }
    }

    companion object {
        const val MAX = 255
        const val THETA_ARRIVAL = 191  // 75% threshold
    }
}

// Constants (from specs/00-constraints.md)
object PhysicalConstants {
    const val V_MAX_CMS: SpeedCms = 1667        // 60 km/h
    const val SIGMA_GPS_CM: DistCm = 2000       // 20 m
    const val GPS_JUMP_THRESHOLD: DistCm = 5000 // 50 m
    const val OFF_ROUTE_D2_THRESHOLD: Dist2 = 25000000L // 50 m squared

    const val SIGMA_D_CM: DistCm = 2750         // F1 sigma (27.5 m)
    const val SIGMA_P_CM: DistCm = 2000         // F3 sigma (20 m)
    const val V_STOP_CMS: SpeedCms = 200        // Stop speed threshold

    const val GAUSSIAN_LUT_SIZE = 256
    const val SPEED_LUT_MAX_IDX = 127

    // Corridor sizes
    const val CORRIDOR_BEFORE_CM: DistCm = 8000 // 80 m before stop
    const val CORRIDOR_AFTER_CM: DistCm = 4000  // 40 m after stop

    // Arrival/departure distances
    const val ARRIVAL_DISTANCE_CM: DistCm = 5000  // 50 m
    const val DEPARTURE_DISTANCE_CM: DistCm = 4000 // 40 m

    // Recovery
    const val RECOVERY_JUMP_THRESHOLD: DistCm = 20000 // 200 m
    const val RECOVERY_INDEX_PENALTY: Int = 5000
}

// Kalman gains (HDOP-adaptive, from specs/02-kalman_filter.md)
enum class HdopQuality(val ks: Int, val kv: Int = 77) {
    EXCELLENT(77),   // 0.0 - 2.0 HDOP
    GOOD(51),        // 2.1 - 3.0 HDOP
    FAIR(26),        // 3.1 - 5.0 HDOP
    POOR(13)         // > 5.0 HDOP
    ;

    companion object {
        fun fromHdop(hdop: Float): HdopQuality {
            return when {
                hdop <= 2.0f -> EXCELLENT
                hdop <= 3.0f -> GOOD
                hdop <= 5.0f -> FAIR
                else -> POOR
            }
        }
    }
}

// Extension functions for unit conversions
fun Double.toCm(): DistCm = (this * 100).toInt()
fun Double.toMm(): Int = (this * 1000).toInt()
fun Double.toCms(): SpeedCms = (this * 100).toInt()
fun Double.toCdeg(): Short = (this * 100).toInt().toShort()

fun DistCm.toMeters(): Double = this / 100.0
fun SpeedCms.toKmh(): Double = this * 3.6 / 100.0
fun HeadCdeg.toDegrees(): Double = this / 100.0

// Heading difference calculation (from specs/01-map_matching.md)
fun headingDiffCdeg(a: HeadCdeg, b: HeadCdeg): HeadCdeg {
    val diff = abs(a - b).toInt() % 36000
    return if (diff > 18000) {
        (36000 - diff).toInt().toShort()
    } else {
        diff.toInt().toShort()
    }
}

// Speed constraint check (from specs/00-constraints.md)
fun checkSpeedConstraint(zNew: DistCm, zPrev: DistCm, dt: Int): Boolean {
    val distAbs = abs(zNew - zPrev)
    val maxDist = PhysicalConstants.V_MAX_CMS * dt.coerceAtLeast(1) + PhysicalConstants.SIGMA_GPS_CM
    return distAbs <= maxDist
}

// Monotonicity check (from specs/00-constraints.md)
fun checkMonotonic(zNew: DistCm, zPrev: DistCm): Boolean {
    return zNew >= zPrev - 5000
}

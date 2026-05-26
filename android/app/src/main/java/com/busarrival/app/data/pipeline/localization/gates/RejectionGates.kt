package com.busarrival.app.data.pipeline.localization.gates

import com.busarrival.app.data.pipeline.types.*

/**
 * Rejection gates for GPS quality filtering.
 * Ported from crates/pipeline/gps_processor/src/kalman/mod.rs:268-292
 *
 * CRITICAL: These gates must be SKIPed when frozen or first fix.
 * See DetectionPipeline integration for guard logic.
 */
object RejectionGates {
    private const val V_MAX_CMS = 1667  // Max bus speed ~60km/h
    private const val SIGMA_GPS_CM = 2000  // GPS accuracy sigma
    private const val MAX_JUMP_FACTOR = 10
    private const val MONOTONIC_TOLERANCE_CM = 5000

    /**
     * Speed constraint: GPS position jump must be physically possible.
     * Rust: mod.rs:268-270
     */
    fun checkSpeedConstraint(
        zNew: DistCm,
        zPrev: DistCm,
        dt: Int
    ): Boolean {
        val distAbs = kotlin.math.abs(zNew - zPrev)
        val maxDist = V_MAX_CMS * dt.coerceAtLeast(1) + SIGMA_GPS_CM
        return distAbs <= maxDist
    }

    /**
     * Monotonic constraint: Allow small backward movements (GPS noise).
     * Rust: mod.rs:270-271
     */
    fun checkMonotonic(zNew: DistCm, zPrev: DistCm): Boolean {
        return zNew >= zPrev - MONOTONIC_TOLERANCE_CM
    }

    /**
     * Route jump constraint: Reject extreme multi-hop jumps.
     * Rust: mod.rs:271-272
     */
    fun checkRouteJump(
        zNew: DistCm,
        zPrev: DistCm,
        dt: Int
    ): Boolean {
        val maxPossible = (V_MAX_CMS * dt.coerceAtLeast(1) + SIGMA_GPS_CM)
        val actual = kotlin.math.abs(zNew - zPrev)
        return actual <= MAX_JUMP_FACTOR * maxPossible
    }
}

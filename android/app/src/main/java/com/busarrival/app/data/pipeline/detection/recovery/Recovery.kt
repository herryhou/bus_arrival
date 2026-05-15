package com.busarrival.app.data.pipeline.detection.recovery

import com.busarrival.app.data.pipeline.types.*
import com.busarrival.app.domain.model.RouteData
import com.busarrival.app.domain.model.Stop
import kotlin.math.abs
import kotlin.math.max
import kotlin.math.min

/**
 * Stop index recovery after GPS jump.
 * Ported from crates/pipeline/detection/src/recovery.rs
 *
 * Reference: specs/07-stop_recovery.md
 */
object Recovery {

    private const val GPS_JUMP_THRESHOLD: DistCm = 20000  // 200 m
    private const val V_MAX_CMS = 1667  // 60 km/h
    private const val INDEX_PENALTY = 5000
    private const val MAX_BACKWARD_RECOVERY_CM = 10000  // 100m backward limit
    private const val BASE_VELOCITY_CMS = 200  // 2 m/s base velocity for reachability
    private const val SPATIAL_ANCHOR_PENALTY_PER_50M = 20  // Penalty per 50m going back past frozen position

    /**
     * Find best stop after GPS jump.
     * Enhanced with spatial anchor penalty and velocity-based reachability.
     *
     * @param sCm Current route position (cm)
     * @param lastIdx Last known stop index
     * @param dt Time since last fix (seconds)
     * @param routeData Route data
     * @param frozenSCm Frozen position during off-route (for spatial anchor penalty)
     * @return Recovered stop index, or null if no valid stop found
     */
    fun recover(
        sCm: DistCm,
        lastIdx: Int,
        dt: Int,
        routeData: RouteData,
        frozenSCm: DistCm? = null
    ): Int? {
        val stops = routeData.stops
        if (stops.isEmpty()) return null

        var bestIdx: Int? = null
        var bestScore: Long = Long.MAX_VALUE

        // Search window: ±10 stops from hint
        val searchStart = max(0, lastIdx - 10)
        val searchEnd = min(stops.size - 1, lastIdx + 10)

        for (idx in searchStart..searchEnd) {
            val stop = stops[idx]

            // Filter: Only stops within ±200 m
            val distToStop = abs(stop.progressCm - sCm)
            if (distToStop > GPS_JUMP_THRESHOLD) continue

            // Velocity constraint: Exclude impossible stops
            if (!checkVelocityConstraintEnhanced(sCm, stop, dt)) continue

            // Calculate spatial anchor penalty
            val spatialAnchorPenalty = if (frozenSCm != null && stop.progressCm < frozenSCm) {
                // Penalty for going back past frozen position
                val backwardDist = frozenSCm - stop.progressCm
                (backwardDist / 5000 * SPATIAL_ANCHOR_PENALTY_PER_50M).toInt()
            } else {
                0
            }

            // Score: distance + index penalty + spatial anchor penalty
            val score = distToStop.toLong() +
                INDEX_PENALTY * max(0, lastIdx - idx).toLong() +
                spatialAnchorPenalty.toLong()

            if (score < bestScore) {
                bestScore = score
                bestIdx = idx
            }
        }

        return bestIdx
    }

    /**
     * Find nearest stop using spatial grid for faster search.
     * Similar to host's Recovery.find_nearest_stop()
     *
     * @param sCm Current route position (cm)
     * @param hintIdx Hint for likely stop index
     * @param routeData Route data
     * @param frozenSCm Frozen position during off-route
     * @return Nearest stop index, or null if no valid stop found
     */
    fun findNearestStop(
        sCm: DistCm,
        hintIdx: Int,
        routeData: RouteData,
        frozenSCm: DistCm? = null
    ): Int? {
        val stops = routeData.stops
        if (stops.isEmpty()) return null

        var bestIdx: Int? = null
        var bestScore: Long = Long.MAX_VALUE

        // Search window: ±10 stops from hint
        val searchStart = max(0, hintIdx - 10)
        val searchEnd = min(stops.size - 1, hintIdx + 10)

        for (idx in searchStart..searchEnd) {
            val stop = stops[idx]

            // Filter: Only stops within ±200 m
            val distToStop = abs(stop.progressCm - sCm)
            if (distToStop > GPS_JUMP_THRESHOLD) continue

            // Filter: Don't go too far back
            if (frozenSCm != null && stop.progressCm < frozenSCm - MAX_BACKWARD_RECOVERY_CM) continue

            // Calculate spatial anchor penalty
            val spatialAnchorPenalty = if (frozenSCm != null && stop.progressCm < frozenSCm) {
                val backwardDist = frozenSCm - stop.progressCm
                (backwardDist / 5000 * SPATIAL_ANCHOR_PENALTY_PER_50M).toInt()
            } else {
                0
            }

            // Score: distance + index penalty + spatial anchor penalty
            val score = distToStop.toLong() +
                INDEX_PENALTY * max(0, hintIdx - idx).toLong() +
                spatialAnchorPenalty.toLong()

            if (score < bestScore) {
                bestScore = score
                bestIdx = idx
            }
        }

        return bestIdx
    }

    /**
     * Check velocity constraint for recovery.
     * Stop must be reachable within V_MAX × dt.
     */
    private fun checkVelocityConstraint(
        sCm: DistCm,
        stop: Stop,
        lastIdx: Int,
        dt: Int,
        stops: List<Stop>
    ): Boolean {
        if (dt <= 0) return true  // No constraint for dt=0

        val maxDist = V_MAX_CMS * dt

        // If stop is ahead, check distance
        if (stop.progressCm > sCm) {
            val dist = stop.progressCm - sCm
            if (dist > maxDist) return false
        }

        // If stop is behind, check if it's too far back
        if (stop.progressCm < sCm) {
            // Only allow one stop backward
            if (stop.progressCm < stops[lastIdx.coerceAtLeast(0)].progressCm - maxDist) {
                return false
            }
        }

        return true
    }

    /**
     * Enhanced velocity constraint for recovery.
     * Uses base velocity (2 m/s) + dynamic (v × dt) with caps.
     */
    private fun checkVelocityConstraintEnhanced(
        sCm: DistCm,
        stop: Stop,
        dt: Int
    ): Boolean {
        if (dt <= 0) return true

        val dist = abs(stop.progressCm - sCm)

        // Base reachability: 2 m/s × dt
        val baseReachable = BASE_VELOCITY_CMS * dt

        // Cap at max speed (60 km/h)
        val maxReachable = baseReachable.toLong().coerceAtMost((V_MAX_CMS * dt).toLong())

        return dist.toLong() <= maxReachable
    }

    /**
     * Check if GPS jump detected.
     */
    fun isJumpDetected(sCm: DistCm, lastSCm: DistCm): Boolean {
        return abs(sCm - lastSCm) > GPS_JUMP_THRESHOLD
    }
}

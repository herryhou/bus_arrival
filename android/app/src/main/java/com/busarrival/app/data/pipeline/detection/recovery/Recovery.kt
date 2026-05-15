package com.busarrival.app.data.pipeline.detection.recovery

import com.busarrival.app.data.pipeline.types.*
import com.busarrival.app.domain.model.RouteData
import com.busarrival.app.domain.model.Stop
import kotlin.math.abs
import kotlin.math.max

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

    /**
     * Find best stop after GPS jump.
     *
     * @param sCm Current route position (cm)
     * @param lastIdx Last known stop index
     * @param dt Time since last fix (seconds)
     * @param routeData Route data
     * @return Recovered stop index, or null if no valid stop found
     */
    fun recover(
        sCm: DistCm,
        lastIdx: Int,
        dt: Int,
        routeData: RouteData
    ): Int? {
        val stops = routeData.stops
        if (stops.isEmpty()) return null

        var bestIdx: Int? = null
        var bestScore: Long = Long.MAX_VALUE

        for ((idx, stop) in stops.withIndex()) {
            // Filter: Only stops ≥ last_index - 1
            if (idx < lastIdx - 1) continue

            // Filter: Only stops within ±200 m
            val distToStop = abs(stop.progressCm - sCm)
            if (distToStop > GPS_JUMP_THRESHOLD) continue

            // Velocity constraint: Exclude impossible stops
            if (!checkVelocityConstraint(sCm, stop, lastIdx, dt, stops)) continue

            // Score: distance + index penalty
            val score = distToStop.toLong() +
                INDEX_PENALTY * max(0, lastIdx - idx).toLong()

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
     * Check if GPS jump detected.
     */
    fun isJumpDetected(sCm: DistCm, lastSCm: DistCm): Boolean {
        return abs(sCm - lastSCm) > GPS_JUMP_THRESHOLD
    }
}

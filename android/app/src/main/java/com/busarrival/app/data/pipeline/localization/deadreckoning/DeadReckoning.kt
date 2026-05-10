package com.busarrival.app.data.pipeline.localization.deadreckoning

import com.busarrival.app.data.pipeline.types.*
import com.busarrival.app.domain.model.DrState

/**
 * Dead reckoning for GPS outage handling.
 * Ported from crates/pipeline/gps_processor/src/kalman.rs
 *
 * Reference: specs/03-dead_reckoning.md
 */
object DeadReckoning {

    private const val MAX_OUTAGE_S = 10  // Maximum outage: 10 seconds

    // DR decay factors: (9/10)^dt × 10000
    private val DR_DECAY_NUMERATOR = listOf(
        10000,  // dt=0: 1.0
        9000,   // dt=1: 0.9
        8100,   // dt=2: 0.81
        7290,   // dt=3: 0.729
        6561,   // dt=4: 0.6561
        5905,   // dt=5: 0.5905
        5314,   // dt=6: 0.5314
        4783,   // dt=7: 0.4783
        4305,   // dt=8: 0.4305
        3874,   // dt=9: 0.3874
        3487    // dt=10: 0.3487
    )

    /**
     * Update EMA velocity filter.
     * v_filtered(t) = v_filtered(t-1) + 3*(v_gps - v_filtered(t-1))/10
     */
    fun updateEma(vFilteredPrev: SpeedCms, vGps: SpeedCms): SpeedCms {
        return vFilteredPrev + 3 * (vGps - vFilteredPrev) / 10
    }

    /**
     * Project position using DR.
     * s(t) = s(t-1) + v_filtered * dt
     */
    fun projectPosition(lastValidS: DistCm, vFiltered: SpeedCms, dt: Int): DistCm {
        return lastValidS + vFiltered * dt
    }

    /**
     * Apply DR decay factor to velocity.
     */
    fun decayVelocity(vCms: SpeedCms, dt: Int): SpeedCms {
        val idx = dt.coerceIn(0, DR_DECAY_NUMERATOR.size - 1)
        return (vCms * DR_DECAY_NUMERATOR[idx]) / 10000
    }

    /**
     * Check if GPS outage is valid.
     */
    fun isValidOutage(dt: Int): Boolean {
        return dt in 0..MAX_OUTAGE_S
    }

    /**
     * Get maximum outage duration.
     */
    fun maxOutage(): Int = MAX_OUTAGE_S
}

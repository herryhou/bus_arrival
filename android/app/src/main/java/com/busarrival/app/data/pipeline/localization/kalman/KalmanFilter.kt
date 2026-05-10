package com.busarrival.app.data.pipeline.localization.kalman

import com.busarrival.app.data.pipeline.types.*
import com.busarrival.app.domain.model.KalmanState
import com.busarrival.app.domain.model.PositionSignals

/**
 * 1D Kalman filter for route progress estimation.
 * Ported from crates/pipeline/gps_processor/src/kalman.rs
 *
 * Fixed-point arithmetic with HDOP-adaptive gains.
 * Reference: specs/02-kalman_filter.md
 */
object KalmanFilter {

    private const val Ks_STANDARD = 51   // Position gain (standard)
    private const val Kv_STANDARD = 77   // Velocity gain (fixed)

    // Soft resync gains (for recovery mode)
    private const val Ks_SOFT = 20       // 2/10 * 256
    private const val Kv_SOFT = 20

    /**
     * Process GPS update and return position signals.
     *
     * @param state Kalman state (mutated)
     * @param zCm Raw GPS projection (cm)
     * @param vGpsCms GPS speed (cm/s)
     * @param hdopX10 HDOP × 10 (null if unavailable)
     * @param isSoftResync Use conservative gains for recovery
     * @return PositionSignals for probability model
     */
    fun update(
        state: KalmanState,
        zCm: DistCm,
        vGpsCms: SpeedCms,
        hdopX10: Int?,
        isSoftResync: Boolean = false
    ): PositionSignals {
        // Prediction step
        val sPred = state.sCm + state.vCms
        val vPred = state.vCms

        // Update step
        if (isSoftResync) {
            // Soft resync for recovery
            state.sCm = sPred + (Ks_SOFT * (zCm - sPred)) / 256
            state.vCms = (vPred + (Kv_SOFT * (vGpsCms - vPred)) / 256).coerceAtLeast(0)
        } else {
            // Standard update with HDOP-adaptive gain
            val quality = hdopX10?.let { HdopQuality.fromHdop(it / 10f) }
            val ks = quality?.ks ?: Ks_STANDARD

            state.sCm = sPred + (ks * (zCm - sPred)) / 256
            state.vCms = (vPred + (Kv_STANDARD * (vGpsCms - vPred)) / 256).coerceAtLeast(0)
        }

        return PositionSignals(
            zGpsCm = zCm,
            sCm = state.sCm
        )
    }

    /**
     * Predict position without measurement (for dead reckoning).
     */
    fun predict(state: KalmanState, dt: Int): DistCm {
        return state.sCm + state.vCms * dt
    }
}

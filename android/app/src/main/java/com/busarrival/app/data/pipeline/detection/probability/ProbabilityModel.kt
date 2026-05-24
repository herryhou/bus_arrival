package com.busarrival.app.data.pipeline.detection.probability

import com.busarrival.app.data.pipeline.types.*
import com.busarrival.app.domain.model.PositionSignals
import com.busarrival.app.domain.model.Stop

/**
 * 4-feature probability model for arrival detection.
 * Ported from crates/pipeline/detection/src/probability.rs
 *
 * Features:
 * - F1: Distance likelihood (raw GPS, σ=2750 cm)
 * - F2: Speed likelihood (threshold at 200 cm/s)
 * - F3: Progress likelihood (Kalman, σ=2000 cm)
 * - F4: Dwell time likelihood (T_ref=10s)
 *
 * Reference: specs/05-arrival_probability.md
 */
object ProbabilityModel {

    private const val GAUSSIAN_LUT_SIZE = 256
    private const val SPEED_LUT_MAX_IDX = 127
    private const val DWELL_REF_S = 10

    // Standard weights: (13, 6, 10, 3) sums to 32
    private const val W1_STD = 13
    private const val W2_STD = 6
    private const val W3_STD = 10
    private const val W4_STD = 3

    // Adaptive weights for close stops: (14, 7, 11, 0) - remove p4
    private const val W1_ADAPT = 14
    private const val W2_ADAPT = 7
    private const val W3_ADAPT = 11
    private const val W4_ADAPT = 0

    // LUTs
    private val gaussianLut = buildGaussianLut()
    private val speedLut = buildSpeedLut()
    private val dwellLut = buildDwellLut()

    /**
     * Compute arrival probability.
     *
     * @param signals Position signals (raw GPS + Kalman)
     * @param stop Target stop
     * @param vCms Current velocity (cm/s)
     * @param dwellS Dwell time in corridor (seconds)
     * @return Probability (0..255)
     */
    fun compute(
        signals: PositionSignals,
        stop: Stop,
        vCms: SpeedCms,
        dwellS: Int,
        gpsStatus: GpsStatus
    ): Prob8 {
        val features = computeFeatures(signals, stop, vCms, dwellS, gpsStatus)
        val p = if (features.isClose) {
            (W1_ADAPT * features.p1.value + W2_ADAPT * features.p2.value + W3_ADAPT * features.p3.value + W4_ADAPT * features.p4.value) / 32
        } else {
            (W1_STD * features.p1.value + W2_STD * features.p2.value + W3_STD * features.p3.value + W4_STD * features.p4.value) / 32
        }

        return Prob8(p.coerceIn(0, 255))
    }

    fun computeFeatures(
        signals: PositionSignals,
        stop: Stop,
        vCms: SpeedCms,
        dwellS: Int,
        gpsStatus: GpsStatus
    ): ProbabilityFeatures {
        val dCm = stop.distanceTo(signals.sCm)
        val absDCm = if (dCm < 0) -dCm else dCm

        return ProbabilityFeatures(
            p1 = computeDistanceLikelihood(signals.zGpsCm, signals.sCm, stop.progressCm, gpsStatus),
            p2 = computeSpeedLikelihood(vCms),
            p3 = computeProgressLikelihood(signals.sCm, signals.zGpsCm, stop.progressCm, gpsStatus),
            p4 = computeDwellLikelihood(dwellS),
            isClose = absDCm < 12000
        )
    }

    /**
     * F1: Distance likelihood using Gaussian.
     * P(d|A) = exp(-0.5 * (d/σ_d)²)
     * σ_d = 2750 cm
     */
    private fun computeDistanceLikelihood(zCm: DistCm, stopProgressCm: DistCm): Prob8 {
        val dCm = zCm - stopProgressCm
        val absDCm = if (dCm < 0) -dCm else dCm

        val idx = (absDCm * 64 / PhysicalConstants.SIGMA_D_CM)
            .coerceIn(0, GAUSSIAN_LUT_SIZE - 1)

        return Prob8(gaussianLut[idx])
    }

    /**
     * F2: Speed likelihood using logistic function.
     * P(v|A) = 1 / (1 + exp((v - v_stop)/k))
     * v_stop = 200 cm/s
     */
    private fun computeSpeedLikelihood(vCms: SpeedCms): Prob8 {
        val idx = (vCms / 10)
            .coerceIn(0, SPEED_LUT_MAX_IDX)

        return Prob8(speedLut[idx])
    }

    /**
     * F3: Progress likelihood using Gaussian.
     * P(p|A) = exp(-0.5 * (p/σ_p)²)
     * σ_p = 2000 cm
     */
    private fun computeProgressLikelihood(sCm: DistCm, stopProgressCm: DistCm): Prob8 {
        val pCm = sCm - stopProgressCm
        val absPCm = if (pCm < 0) -pCm else pCm

        val idx = (absPCm * 64 / PhysicalConstants.SIGMA_P_CM)
            .coerceIn(0, GAUSSIAN_LUT_SIZE - 1)

        return Prob8(gaussianLut[idx])
    }

    /**
     * F4: Dwell time likelihood using linear ramp.
     * P(t|A) = min(t / T_ref, 1.0)
     * T_ref = 10s
     */
    private fun computeDwellLikelihood(dwellS: Int): Prob8 {
        val idx = dwellS.coerceIn(0, DWELL_REF_S * 2)
        return Prob8(dwellLut[idx])
    }

    /**
     * Build Gaussian LUT: 256 entries.
     * exp(-0.5 * x²) for x = 0..4.0
     */
    private fun buildGaussianLut(): IntArray {
        val lut = IntArray(GAUSSIAN_LUT_SIZE)
        for (i in 0 until GAUSSIAN_LUT_SIZE) {
            val x = i / 64.0  // 0 to 4.0
            val g = kotlin.math.exp(-0.5 * x * x)
            lut[i] = (g * 255).toInt().coerceIn(0, 255)
        }
        return lut
    }

    /**
     * Build speed LUT: 128 entries.
     * Logistic: 1 / (1 + exp((v - 200) / 50))
     */
    private fun buildSpeedLut(): IntArray {
        val lut = IntArray(SPEED_LUT_MAX_IDX + 1)
        for (i in 0..SPEED_LUT_MAX_IDX) {
            val v = i * 10  // 0 to 1270 cm/s
            val logistic = 1.0 / (1.0 + kotlin.math.exp((v - PhysicalConstants.V_STOP_CMS) / 50.0))
            lut[i] = (logistic * 255).toInt().coerceIn(0, 255)
        }
        return lut
    }

    /**
     * Build dwell time LUT: 20 entries (0..20s).
     * Linear ramp: min(t / 10, 1.0)
     */
    private fun buildDwellLut(): IntArray {
        val lut = IntArray(DWELL_REF_S * 2 + 1)
        for (i in 0..DWELL_REF_S * 2) {
            val t = i / DWELL_REF_S.toDouble()
            lut[i] = ((t.coerceAtMost(1.0)) * 255).toInt()
        }
        return lut
    }
}

data class ProbabilityFeatures(
    val p1: Prob8,
    val p2: Prob8,
    val p3: Prob8,
    val p4: Prob8,
    val isClose: Boolean
)

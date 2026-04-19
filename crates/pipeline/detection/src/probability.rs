//! 4-feature Bayesian arrival probability model

use shared::{DistCm, SpeedCms, Prob8};
use core::cmp::Ord;

#[cfg(feature = "std")]
use crate::trace::FeatureScores;

use shared::{probability_constants::*, PositionSignals, Stop};

/// Normalized Gaussian LUT: exp(-x²/2)
/// Index i = (x / sigma) * 64. Range [0, 4.0).
#[cfg(feature = "std")]
pub fn build_gaussian_lut() -> [u8; 256] {
    let mut lut = [0u8; 256];
    for i in 0..256 {
        let x = (i as f64) / 64.0;  // 0 to 4.0
        let g = (-0.5 * x * x).exp();
        lut[i] = (g * 255.0).min(255.0).round() as u8;  // Use .round() for proper rounding
    }
    lut
}

/// Logistic LUT for speed likelihood: 1 / (1 + exp(k * (v - v_stop)))
/// v_stop = 200 cm/s, k = 0.01.
/// Index i = v / 10. Range [0, 1270] cm/s.
#[cfg(feature = "std")]
pub fn build_logistic_lut() -> [u8; 128] {
    let mut lut = [0u8; 128];
    let k = 0.01;
    let v_stop = 200.0;
    for i in 0..128 {
        let v = (i as f64) * 10.0;  // 0 to 1270 cm/s
        let l = 1.0 / (1.0 + (k * (v - v_stop)).exp());
        lut[i] = (l * 255.0).min(255.0).round() as u8;  // Use .round() for proper rounding
    }
    lut
}

/// Shared feature computation for arrival probability
/// Now accepts PositionSignals to separate F1 (raw GPS) from F3 (Kalman)
fn compute_features(
    signals: PositionSignals,
    v_cms: SpeedCms,
    stop: &Stop,
    dwell_time_s: u16,
    gps_status: GpsStatus,
    gaussian_lut: &[u8; 256],
    logistic_lut: &[u8; 128],
) -> (u32, u32, u32, u32) {
    // Feature 1: Distance likelihood (sigma_d = 2750 cm)
    // Uses RAW GPS projection z_gps_cm per spec Section 13.2
    // Defensive: blend z_gps_cm and s_cm based on divergence to handle
    // cases where map matcher produces poor projections during normal operation
    let divergence = signals.divergence_cm();
    let (d1_cm, use_fallback) = if gps_status == GpsStatus::Valid && divergence > 2000 {
        // When z_gps_cm and s_cm diverge significantly, use s_cm for p1
        // This prevents poor map matching from dragging down probability
        ((signals.s_cm - stop.progress_cm).abs(), true)
    } else {
        // Normal case: use z_gps_cm as per spec
        ((signals.z_gps_cm - stop.progress_cm).abs(), false)
    };
    let idx1 = ((d1_cm as i64 * 64) / SIGMA_D_CM as i64).min(255) as usize;
    let p1 = gaussian_lut[idx1] as u32;

    // Feature 2: Speed likelihood (near 0 -> higher, v_stop = 200 cm/s)
    let idx2 = (v_cms / 10).max(0).min(SPEED_LUT_MAX_IDX as SpeedCms) as usize;
    let p2 = logistic_lut[idx2] as u32;

    // Feature 3: Progress difference likelihood (sigma_p = 2000 cm)
    // Uses KALMAN-FILTERED position s_cm per spec Section 13.2
    // Neutralize to 128 during dr_outage or off_route when s_cm may be phantom
    let p3 = if gps_status != GpsStatus::Valid && divergence > PHANTOM_DIVERGENCE_CM {
        128 // neutral: neither confirms nor denies arrival
    } else {
        let d3_cm = (signals.s_cm - stop.progress_cm).abs();
        let idx3 = ((d3_cm as i64 * 64) / SIGMA_P_CM as i64).min(255) as usize;
        gaussian_lut[idx3] as u32
    };

    // Feature 4: Dwell time likelihood (T_ref = 10s)
    let p4 = ((dwell_time_s as u32) * 255 / 10).min(255) as u32;

    (p1, p2, p3, p4)
}

/// GPS processing status for phantom arrival detection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpsStatus {
    /// GPS is being processed normally
    Valid,
    /// GPS is being rejected (dr_outage)
    DrOutage,
    /// GPS is off-route (position frozen)
    OffRoute,
}

/// Arrival threshold: 75% probability
pub const THETA_ARRIVAL: Prob8 = 191;

/// Compute arrival probability using LUTs (no_std compatible)
/// Uses PositionSignals to separate F1 (raw GPS) from F3 (Kalman)
pub fn compute_arrival_probability(
    signals: PositionSignals,
    v_cms: SpeedCms,
    stop: &Stop,
    dwell_time_s: u16,
    gps_status: GpsStatus,
    gaussian_lut: &[u8; 256],
    logistic_lut: &[u8; 128],
) -> Prob8 {
    let (p1, p2, p3, p4) = compute_features(signals, v_cms, stop, dwell_time_s, gps_status, gaussian_lut, logistic_lut);
    ((13 * p1 + 6 * p2 + 10 * p3 + 3 * p4) / 32) as u8
}

/// Compute arrival probability with adaptive weights for close stops.
///
/// When next sequential stop is < 120m away, removes dwell time (p4)
/// weight and redistributes: (14, 7, 11, 0) instead of (13, 6, 10, 3).
pub fn compute_arrival_probability_adaptive(
    signals: PositionSignals,
    v_cms: SpeedCms,
    stop: &Stop,
    dwell_time_s: u16,
    gps_status: GpsStatus,
    gaussian_lut: &[u8; 256],
    logistic_lut: &[u8; 128],
    next_stop: Option<&Stop>,
) -> Prob8 {
    let (p1, p2, p3, p4) = compute_features(signals, v_cms, stop, dwell_time_s, gps_status, gaussian_lut, logistic_lut);

    // Adaptive weights based on next stop distance
    let (w1, w2, w3, w4) = if let Some(next) = next_stop {
        let dist_to_next = (next.progress_cm - stop.progress_cm).abs();
        if dist_to_next < 12_000 {
            // Close stop: remove p4, scale remaining to sum=32
            (14, 7, 11, 0)
        } else {
            // Normal stop: standard weights
            (13, 6, 10, 3)
        }
    } else {
        // Last stop: standard weights
        (13, 6, 10, 3)
    };

    ((w1 * p1 + w2 * p2 + w3 * p3 + w4 * p4) / 32) as u8
}

/// Compute arrival probability (0-255)
///
/// DEPRECATED: Use [`compute_arrival_probability`] with PositionSignals instead.
/// This function maintains backward compatibility by using s_cm for both F1 and F3.
pub fn arrival_probability(
    s_cm: DistCm,
    v_cms: SpeedCms,
    stop: &shared::Stop,
    dwell_time_s: u16,
    gaussian_lut: &[u8; 256],
    logistic_lut: &[u8; 128],
) -> Prob8 {
    // For backward compatibility, use s_cm for both signals and Valid status
    let signals = PositionSignals::new(s_cm, s_cm);
    compute_arrival_probability(signals, v_cms, stop, dwell_time_s, GpsStatus::Valid, gaussian_lut, logistic_lut)
}

/// Compute arrival probability with adaptive weights for close stops.
///
/// DEPRECATED: Use [`compute_arrival_probability_adaptive`] with PositionSignals instead.
/// This function maintains backward compatibility by using s_cm for both F1 and F3.
///
/// When next stop is <120m away, removes dwell time (p4) weight and
/// redistributes proportionally: (14, 7, 11, 0) instead of (13, 6, 10, 3).
///
/// # Arguments
/// * `next_stop` - Next sequential stop in route (not next active stop)
pub fn arrival_probability_adaptive(
    s_cm: DistCm,
    v_cms: SpeedCms,  // Type alias for i32
    stop: &shared::Stop,
    dwell_time_s: u16,
    gaussian_lut: &[u8; 256],
    logistic_lut: &[u8; 128],
    next_stop: Option<&shared::Stop>,
) -> Prob8 {
    // For backward compatibility, use s_cm for both signals and Valid status
    let signals = PositionSignals::new(s_cm, s_cm);
    compute_arrival_probability_adaptive(signals, v_cms, stop, dwell_time_s, GpsStatus::Valid, gaussian_lut, logistic_lut, next_stop)
}

/// Compute individual feature scores for trace output
/// Enables verification that F1 and F3 use independent signals
#[cfg(feature = "std")]
pub fn compute_feature_scores(
    signals: PositionSignals,
    v_cms: SpeedCms,
    stop: &shared::Stop,
    dwell_time_s: u16,
    gaussian_lut: &[u8; 256],
    logistic_lut: &[u8; 128],
) -> FeatureScores {
    let (p1, p2, p3, p4) = compute_features(signals, v_cms, stop, dwell_time_s, GpsStatus::Valid, gaussian_lut, logistic_lut);
    FeatureScores { p1: p1 as u8, p2: p2 as u8, p3: p3 as u8, p4: p4 as u8 }
}

/// Compute arrival probability (0-255) with built-in LUTs.
///
/// Simplified interface that builds LUTs on first use.
///
/// In no_std environments, use [`compute_probability_with_luts`] instead.
#[cfg(feature = "std")]
pub fn compute_probability(
    s_cm: DistCm,
    v_cms: SpeedCms,
    stop_progress: DistCm,
    dwell_time_s: u16,
) -> Prob8 {
    compute_probability_with_luts(s_cm, v_cms, stop_progress, dwell_time_s, &gaussian_lut(), &logistic_lut())
}

/// Get or build the Gaussian LUT (cached in std environments)
#[cfg(feature = "std")]
pub fn gaussian_lut() -> &'static [u8; 256] {
    use std::sync::OnceLock;
    static GAUSSIAN_LUT: OnceLock<[u8; 256]> = OnceLock::new();
    GAUSSIAN_LUT.get_or_init(build_gaussian_lut)
}

/// Get or build the logistic LUT (cached in std environments)
#[cfg(feature = "std")]
pub fn logistic_lut() -> &'static [u8; 128] {
    use std::sync::OnceLock;
    static LOGISTIC_LUT: OnceLock<[u8; 128]> = OnceLock::new();
    LOGISTIC_LUT.get_or_init(build_logistic_lut)
}

/// Compute arrival probability (0-255) with provided LUTs.
///
/// This is the no_std-compatible version. In std environments, you can use
/// [`compute_probability`] which manages LUTs internally.
///
/// For embedded/no_std usage, you should build the LUTs once at startup
/// and reuse them:
///
/// ```ignore
/// let g_lut = build_gaussian_lut();
/// let l_lut = build_logistic_lut();
/// // ... later ...
/// let prob = compute_probability_with_luts(s_cm, v_cms, stop_progress, dwell_time_s, &g_lut, &l_lut);
/// ```
pub fn compute_probability_with_luts(
    s_cm: DistCm,
    v_cms: SpeedCms,
    stop_progress: DistCm,
    dwell_time_s: u16,
    gaussian_lut: &[u8; 256],
    logistic_lut: &[u8; 128],
) -> Prob8 {
    let stop = shared::Stop {
        progress_cm: stop_progress,
        corridor_start_cm: 0,
        corridor_end_cm: 0,
    };

    arrival_probability(s_cm, v_cms, &stop, dwell_time_s, gaussian_lut, logistic_lut)
}

#[cfg(test)]
mod tests {
    use shared::{Stop, PositionSignals};

    #[test]
    fn test_lut_generation() {
        let g_lut = super::build_gaussian_lut();
        assert_eq!(g_lut[0], 255);  // x=0 → max probability
        assert!(g_lut[255] < 10);   // x=4.0 → near zero

        // Verify Gaussian LUT is monotonically decreasing
        for i in 1..g_lut.len() {
            assert!(g_lut[i] <= g_lut[i - 1],
                "Gaussian LUT should be monotonically decreasing at index {}", i);
        }

        let l_lut = super::build_logistic_lut();
        assert!(l_lut[0] > 200);    // v=0 cm/s → high probability (approx 0.88 → 224)
        assert_eq!(l_lut[20], 128); // v=200 cm/s → exactly at v_stop (0.5 → 127.5 → 128)
        assert!(l_lut[100] < 20);   // v=1000 cm/s → low probability
    }

    #[test]
    fn test_probability_range() {
        let g_lut = super::build_gaussian_lut();
        let l_lut = super::build_logistic_lut();
        let stop = Stop { progress_cm: 10000, corridor_start_cm: 2000, corridor_end_cm: 14000 };

        // At stop with zero speed and 10s dwell should be high probability
        let p_high = super::arrival_probability(10000, 0, &stop, 10, &g_lut, &l_lut);
        assert!(p_high > 200, "At stop with 0 speed and 10s dwell should be high probability");

        // Far from stop with high speed should be low probability
        let p_low = super::arrival_probability(50000, 1000, &stop, 0, &g_lut, &l_lut);
        assert!(p_low < 100, "Far from stop with high speed should be low probability");
    }

    #[test]
    fn test_adaptive_probability_close_stop() {
        let g_lut = super::build_gaussian_lut();
        let l_lut = super::build_logistic_lut();

        let stop_current = Stop {
            progress_cm: 100_000,
            corridor_start_cm: 90_000,
            corridor_end_cm: 110_000,
        };

        let stop_next = Stop {
            progress_cm: 108_000, // 8,000cm away (<12,000 threshold)
            corridor_start_cm: 98_000,
            corridor_end_cm: 118_000,
        };

        // At stop, moderate speed, some dwell
        let prob = super::arrival_probability_adaptive(
            100_000,  // s_cm (at stop)
            600,      // v_cms (approaching)
            &stop_current,
            5,        // dwell_time_s
            &g_lut,
            &l_lut,
            Some(&stop_next),
        );

        // With close stop, p4 weight is removed, should be higher
        assert!(prob > 190, "Expected probability > 190 for close stop, got {}", prob);
    }

    #[test]
    fn test_adaptive_probability_normal_stop() {
        let g_lut = super::build_gaussian_lut();
        let l_lut = super::build_logistic_lut();

        let stop_current = Stop {
            progress_cm: 100_000,
            corridor_start_cm: 90_000,
            corridor_end_cm: 110_000,
        };

        let stop_next = Stop {
            progress_cm: 125_000, // 25,000cm away (>12,000 threshold)
            corridor_start_cm: 115_000,
            corridor_end_cm: 135_000,
        };

        let prob_adaptive = super::arrival_probability_adaptive(
            100_000, 600, &stop_current, 5, &g_lut, &l_lut, Some(&stop_next)
        );

        let prob_standard = super::arrival_probability(
            100_000, 600, &stop_current, 5, &g_lut, &l_lut
        );

        // Normal stop (>12m to next) should use standard weights
        // Therefore adaptive should equal standard
        assert_eq!(prob_adaptive, prob_standard,
            "Normal stop should use standard weights (13, 6, 10, 3)");
    }

    #[test]
    fn test_adaptive_probability_last_stop() {
        let g_lut = super::build_gaussian_lut();
        let l_lut = super::build_logistic_lut();

        let stop = Stop {
            progress_cm: 100_000,
            corridor_start_cm: 90_000,
            corridor_end_cm: 110_000,
        };

        // Last stop (next_stop = None)
        let prob = super::arrival_probability_adaptive(
            100_000, 0, &stop, 10, &g_lut, &l_lut, None
        );

        // Should use standard weights
        assert!(prob > 150); // At stop with 10s dwell should be high
    }

    #[test]
    fn test_position_signals_f1_uses_z_gps() {
        // Test that F1 (distance likelihood) uses raw GPS projection z_gps_cm
        // when divergence is below fallback threshold (< 2000 cm)
        let g_lut = super::build_gaussian_lut();
        let l_lut = super::build_logistic_lut();
        let stop = Stop { progress_cm: 10000, corridor_start_cm: 2000, corridor_end_cm: 14000 };

        // Case 1: Both signals aligned at stop
        let signals_aligned = PositionSignals::new(10000, 10000);
        let prob_aligned = super::compute_arrival_probability(
            signals_aligned, 0, &stop, 10, super::GpsStatus::Valid, &g_lut, &l_lut
        );

        // Case 2: Raw GPS far from stop, Kalman at stop (GPS noise scenario)
        // Use 1500 cm deviation (< 2000 cm threshold) to avoid fallback logic
        // F1 should drop because z_gps_cm is far, F3 should stay high because s_cm is at stop
        let signals_divergent = PositionSignals::new(11500, 10000); // z_gps_cm=11.5m, s_cm=10m, divergence=1500cm
        let prob_divergent = super::compute_arrival_probability(
            signals_divergent, 0, &stop, 10, super::GpsStatus::Valid, &g_lut, &l_lut
        );

        // Divergent signals should produce lower probability than aligned
        assert!(prob_divergent < prob_aligned,
            "Divergent signals (z_gps_cm far, s_cm at stop) should have lower probability than aligned signals");

        // Case 3: Raw GPS at stop, Kalman far from stop (Kalman lag scenario)
        // Use 1500 cm deviation (< 2000 cm threshold) to avoid fallback logic
        // F1 should stay high because z_gps_cm is at stop, F3 should drop because s_cm is far
        let signals_lag = PositionSignals::new(10000, 11500); // z_gps_cm=10m, s_cm=11.5m, divergence=1500cm
        let prob_lag = super::compute_arrival_probability(
            signals_lag, 0, &stop, 10, super::GpsStatus::Valid, &g_lut, &l_lut
        );

        // Lag scenario should also produce lower probability than aligned
        assert!(prob_lag < prob_aligned,
            "Lag scenario (z_gps_cm at stop, s_cm far) should have lower probability than aligned signals");
    }

    #[test]
    fn test_position_signals_independence() {
        // Test that F1 and F3 are computed independently (when divergence < fallback threshold)
        let g_lut = super::build_gaussian_lut();
        let l_lut = super::build_logistic_lut();
        let stop = Stop { progress_cm: 10000, corridor_start_cm: 2000, corridor_end_cm: 14000 };

        // Test with different sigma values
        // F1 uses sigma_d=2750cm, F3 uses sigma_p=2000cm
        // Same distance deviation should affect F3 more than F1 (smaller sigma = steeper drop)
        // Use 1500 cm deviation (< 2000 cm threshold) to avoid fallback logic
        let distance_deviation = 1500; // 15m deviation

        // Case 1: Only z_gps_cm deviates (affects F1)
        let signals_f1_deviation = PositionSignals::new(
            10000 + distance_deviation, // z_gps_cm deviates
            10000                       // s_cm at stop
        );
        let (p1_only, _, p3_only, _) = super::compute_features(
            signals_f1_deviation, 0, &stop, 10, super::GpsStatus::Valid, &g_lut, &l_lut
        );

        // Case 2: Only s_cm deviates (affects F3)
        let signals_f3_deviation = PositionSignals::new(
            10000,                     // z_gps_cm at stop
            10000 + distance_deviation // s_cm deviates
        );
        let (p1_alt, _, p3_alt, _) = super::compute_features(
            signals_f3_deviation, 0, &stop, 10, super::GpsStatus::Valid, &g_lut, &l_lut
        );

        // When only s_cm deviates, F1 should be high (255) and F3 should be lower
        assert_eq!(p1_alt, 255, "F1 should be max when z_gps_cm is at stop");
        assert!(p3_alt < 255, "F3 should be lower when s_cm deviates from stop");

        // When only z_gps_cm deviates, F3 should be high (255) and F1 should be lower
        assert_eq!(p3_only, 255, "F3 should be max when s_cm is at stop");
        assert!(p1_only < 255, "F1 should be lower when z_gps_cm deviates from stop");
    }

    #[test]
    fn test_compute_arrival_probability_adaptive_with_signals() {
        // Test adaptive probability with PositionSignals
        let g_lut = super::build_gaussian_lut();
        let l_lut = super::build_logistic_lut();

        let stop_current = Stop {
            progress_cm: 100_000,
            corridor_start_cm: 90_000,
            corridor_end_cm: 110_000,
        };

        let stop_next_close = Stop {
            progress_cm: 108_000, // 8,000cm away (<12,000 threshold)
            corridor_start_cm: 98_000,
            corridor_end_cm: 118_000,
        };

        let stop_next_far = Stop {
            progress_cm: 125_000, // 25,000cm away (>12,000 threshold)
            corridor_start_cm: 115_000,
            corridor_end_cm: 135_000,
        };

        // At stop with divergent signals (GPS noise scenario)
        let signals = PositionSignals::new(105_000, 100_000); // z_gps_cm=5m ahead, s_cm at stop

        // Close stop: should use (14, 7, 11, 0) weights
        let prob_close = super::compute_arrival_probability_adaptive(
            signals, 0, &stop_current, 5, super::GpsStatus::Valid, &g_lut, &l_lut, Some(&stop_next_close)
        );

        // Far stop: should use (13, 6, 10, 3) weights
        let prob_far = super::compute_arrival_probability_adaptive(
            signals, 0, &stop_current, 5, super::GpsStatus::Valid, &g_lut, &l_lut, Some(&stop_next_far)
        );

        // Close stop should have higher probability (no p4 penalty)
        assert!(prob_close >= prob_far,
            "Close stop should have equal or higher probability than far stop when p4 weight is removed");
    }

    #[test]
    fn test_backward_compatibility() {
        // Test that old API still works
        let g_lut = super::build_gaussian_lut();
        let l_lut = super::build_logistic_lut();
        let stop = Stop { progress_cm: 10000, corridor_start_cm: 2000, corridor_end_cm: 14000 };

        // Old API: arrival_probability with single s_cm value
        let prob_old = super::arrival_probability(10000, 0, &stop, 10, &g_lut, &l_lut);

        // New API: compute_arrival_probability with PositionSignals
        let signals = PositionSignals::new(10000, 10000);
        let prob_new = super::compute_arrival_probability(signals, 0, &stop, 10, super::GpsStatus::Valid, &g_lut, &l_lut);

        // Should produce identical results when signals are aligned
        assert_eq!(prob_old, prob_new,
            "Old and new API should produce identical results when signals are aligned");

        // Test adaptive functions as well
        let prob_old_adaptive = super::arrival_probability_adaptive(
            10000, 0, &stop, 5, &g_lut, &l_lut, None
        );
        let prob_new_adaptive = super::compute_arrival_probability_adaptive(
            signals, 0, &stop, 5, super::GpsStatus::Valid, &g_lut, &l_lut, None
        );

        assert_eq!(prob_old_adaptive, prob_new_adaptive,
            "Old and new adaptive API should produce identical results when signals are aligned");
    }

    #[test]
    fn test_f1_uses_raw_gps() {
        let g_lut = super::build_gaussian_lut();
        let l_lut = super::build_logistic_lut();
        let stop = Stop { progress_cm: 10_000, corridor_start_cm: 0, corridor_end_cm: 20_000 };

        // Raw GPS is 5m from stop, Kalman shows 0m (perfect arrival)
        let signals = PositionSignals { z_gps_cm: 10_500, s_cm: 10_000 };

        let scores = super::compute_feature_scores(signals, 0, &stop, 10, &g_lut, &l_lut);

        // F1 (raw GPS) should be lower than F3 (Kalman)
        assert!(scores.p1 < scores.p3, "F1 should reflect raw GPS distance");
    }

    #[test]
    fn test_f3_uses_kalman() {
        let g_lut = super::build_gaussian_lut();
        let l_lut = super::build_logistic_lut();
        let stop = Stop { progress_cm: 10_000, corridor_start_cm: 0, corridor_end_cm: 20_000 };

        // Raw GPS shows 20m error, Kalman shows 2m (filtered)
        let signals = PositionSignals { z_gps_cm: 12_000, s_cm: 10_200 };

        let scores = super::compute_feature_scores(signals, 0, &stop, 10, &g_lut, &l_lut);

        // F3 should be higher (closer) than F1
        assert!(scores.p3 > scores.p1, "F3 should reflect Kalman smoothing");
    }

    #[test]
    fn test_signals_independent() {
        let signals = PositionSignals { z_gps_cm: 10_000, s_cm: 10_200 };
        assert_eq!(signals.divergence_cm(), 200);
        assert!(!signals.is_converged());
    }

    #[test]
    fn test_signals_converged() {
        let signals = PositionSignals { z_gps_cm: 10_000, s_cm: 10_000 };
        assert_eq!(signals.divergence_cm(), 0);
        assert!(signals.is_converged());
    }

    #[test]
    fn test_gps_noise_f1_drops_f3_stable() {
        // Setup: bus at stop 10_000 cm, steady state
        let g_lut = super::build_gaussian_lut();
        let l_lut = super::build_logistic_lut();
        let stop = Stop { progress_cm: 10_000, corridor_start_cm: 0, corridor_end_cm: 20_000 };

        // Normal conditions: both signals agree
        let signals_normal = PositionSignals { z_gps_cm: 10_100, s_cm: 10_050 };
        let scores_normal = super::compute_feature_scores(signals_normal, 0, &stop, 5, &g_lut, &l_lut);

        // GPS noise event: raw GPS jumps 20m, Kalman filters to 5m
        // Use divergence < 2000 cm to avoid fallback logic (13000 - 10500 = 2500 would trigger fallback)
        let signals_noise = PositionSignals { z_gps_cm: 12_000, s_cm: 10_500 };
        let scores_noise = super::compute_feature_scores(signals_noise, 0, &stop, 6, &g_lut, &l_lut);

        // F1 should drop significantly (raw GPS noise)
        assert!(scores_noise.p1 < scores_normal.p1 - 50, "F1 should drop on GPS noise");

        // F3 should remain stable (Kalman smoothing)
        assert!(scores_noise.p3 > scores_normal.p3 - 30, "F3 should remain stable");
    }
}

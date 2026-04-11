//! 4-feature Bayesian arrival probability model

use shared::{DistCm, SpeedCms, Prob8};
use core::cmp::Ord;

#[cfg(feature = "std")]
use crate::trace::FeatureScores;

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

/// Arrival threshold: 75% probability
pub const THETA_ARRIVAL: Prob8 = 191;

/// Compute arrival probability (0-255)
pub fn arrival_probability(
    s_cm: DistCm,
    v_cms: SpeedCms,
    stop: &shared::Stop,
    dwell_time_s: u16,
    gaussian_lut: &[u8; 256],
    logistic_lut: &[u8; 128],
) -> Prob8 {
    // Feature 1: Distance likelihood (sigma_d = 2750 cm)
    let d_cm = (s_cm - stop.progress_cm).abs();
    let idx1 = ((d_cm as i64 * 64) / 2750).min(255) as usize;
    let p1 = gaussian_lut[idx1] as u32;

    // Feature 2: Speed likelihood (near 0 → higher, v_stop = 200 cm/s)
    let idx2 = (v_cms / 10).max(0).min(127) as usize;
    let p2 = logistic_lut[idx2] as u32;

    // Feature 3: Progress difference likelihood (sigma_p = 2000 cm)
    let idx3 = ((d_cm as i64 * 64) / 2000).min(255) as usize;
    let p3 = gaussian_lut[idx3] as u32;

    // Feature 4: Dwell time likelihood (T_ref = 10s)
    let p4 = ((dwell_time_s as u32) * 255 / 10).min(255) as u32;

    // Weighted sum: (13p₁ + 6p₂ + 10p₃ + 3p₄) / 32
    // Weights: 0.40625, 0.1875, 0.3125, 0.09375 (Approx 4:2:3:1)
    ((13 * p1 + 6 * p2 + 10 * p3 + 3 * p4) / 32) as u8
}

/// Compute arrival probability with adaptive weights for close stops.
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
    // Feature calculations (same as arrival_probability)
    let d_cm = (s_cm - stop.progress_cm).abs();
    let idx1 = ((d_cm as i64 * 64) / 2750).min(255) as usize;
    let p1 = gaussian_lut[idx1] as u32;

    let idx2 = (v_cms / 10).max(0).min(127) as usize;
    let p2 = logistic_lut[idx2] as u32;

    let idx3 = ((d_cm as i64 * 64) / 2000).min(255) as usize;
    let p3 = gaussian_lut[idx3] as u32;

    let p4 = ((dwell_time_s as u32) * 255 / 10).min(255) as u32;

    // Adaptive weights based on next stop distance
    let (w1, w2, w3, w4) = if let Some(next) = next_stop {
        let dist_to_next = (next.progress_cm - stop.progress_cm).abs();
        if dist_to_next < 12_000 {
            // Close stop: remove p4, scale remaining to sum=32
            // Original: 13+6+10+3=32, without p4: 29, scale factor = 32/29
            // 13/29*32 ≈ 14, 6/29*32 ≈ 7, 10/29*32 ≈ 11
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

/// Compute individual feature scores for trace output
#[cfg(feature = "std")]
pub fn compute_feature_scores(
    s_cm: DistCm,
    v_cms: SpeedCms,
    stop: &shared::Stop,
    dwell_time_s: u16,
    gaussian_lut: &[u8; 256],
    logistic_lut: &[u8; 128],
) -> FeatureScores {
    // Feature 1: Distance likelihood (sigma_d = 2750 cm)
    let d_cm = (s_cm - stop.progress_cm).abs();
    let idx1 = ((d_cm as i64 * 64) / 2750).min(255) as usize;
    let p1 = gaussian_lut[idx1];

    // Feature 2: Speed likelihood (near 0 → higher, v_stop = 200 cm/s)
    let idx2 = (v_cms / 10).max(0).min(127) as usize;
    let p2 = logistic_lut[idx2];

    // Feature 3: Progress difference likelihood (sigma_p = 2000 cm)
    let idx3 = ((d_cm as i64 * 64) / 2000).min(255) as usize;
    let p3 = gaussian_lut[idx3];

    // Feature 4: Dwell time likelihood (T_ref = 10s)
    let p4 = ((dwell_time_s as u32) * 255 / 10).min(255) as u8;

    FeatureScores { p1, p2, p3, p4 }
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
fn gaussian_lut() -> &'static [u8; 256] {
    use std::sync::OnceLock;
    static GAUSSIAN_LUT: OnceLock<[u8; 256]> = OnceLock::new();
    GAUSSIAN_LUT.get_or_init(build_gaussian_lut)
}

/// Get or build the logistic LUT (cached in std environments)
#[cfg(feature = "std")]
fn logistic_lut() -> &'static [u8; 128] {
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
    use shared::Stop;

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
        assert!(prob <= 255);
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
        assert!(prob <= 255);
        assert!(prob > 150); // At stop with 10s dwell should be high
    }
}

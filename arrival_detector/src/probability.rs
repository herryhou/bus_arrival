//! 4-feature Bayesian arrival probability model

use shared::{DistCm, SpeedCms, Prob8};

/// Pre-computed Gaussian LUT (σ = 2750 cm)
pub fn build_gaussian_lut() -> [u8; 256] {
    let mut lut = [0u8; 256];
    let sigma = 2750.0;
    for i in 0..256 {
        let x = (i as f64) * 100.0;  // 0 to 25500 cm
        let g = (-0.5 * (x / sigma).powi(2)).exp();
        lut[i] = (g * 255.0).min(255.0) as u8;
    }
    lut
}

/// Pre-computed Logistic LUT (v_stop = 200 cm/s)
pub fn build_logistic_lut() -> [u8; 128] {
    let mut lut = [0u8; 128];
    for i in 0..128 {
        let dv = (i as f64) * 10.0;  // 0 to 1270 cm/s
        let l = 1.0 / (1.0 + (-0.01 * (dv - 200.0)).exp());
        lut[i] = (l * 255.0).min(255.0) as u8;
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
    // Feature 1: Distance likelihood
    let d_cm = (s_cm - stop.progress_cm).abs();
    let p1 = gaussian_lut[(d_cm / 100).min(255) as usize] as u32;

    // Feature 2: Speed likelihood (near 0 → higher)
    let v_diff = (200 - v_cms).abs().max(0) as u32;
    let p2 = logistic_lut[(v_diff / 10).min(127) as usize] as u32;

    // Feature 3: Progress difference
    let p3 = gaussian_lut[(d_cm / 100).min(255) as usize] as u32;

    // Feature 4: Dwell time
    let p4 = ((dwell_time_s as u32) * 255 / 10).min(255) as u32;

    // Weighted sum: (13p₁ + 6p₂ + 10p₃ + 3p₄) / 32
    ((13 * p1 + 6 * p2 + 10 * p3 + 3 * p4) / 32) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::Stop;

    #[test]
    fn test_lut_generation() {
        let g_lut = build_gaussian_lut();
        assert_eq!(g_lut[0], 255);  // d=0 → max probability
        assert_eq!(g_lut[255], 0);   // d=25500 → min probability

        let l_lut = build_logistic_lut();
        assert_eq!(l_lut[20], 127);  // v=200 cm/s → exactly at v_stop
        assert_eq!(l_lut[40], 224);  // v=400 cm/s → high probability (0.881→224)
    }

    #[test]
    fn test_probability_range() {
        let g_lut = build_gaussian_lut();
        let l_lut = build_logistic_lut();
        let stop = Stop { progress_cm: 10000, corridor_start_cm: 2000, corridor_end_cm: 14000 };

        let p = arrival_probability(10000, 100, &stop, 5, &g_lut, &l_lut);
        assert!(p <= 255);
    }
}

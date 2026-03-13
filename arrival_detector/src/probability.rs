//! 4-feature Bayesian arrival probability model

use shared::{DistCm, SpeedCms, Prob8};

/// Normalized Gaussian LUT: exp(-x²/2)
/// Index i = (x / sigma) * 64. Range [0, 4.0).
pub fn build_gaussian_lut() -> [u8; 256] {
    let mut lut = [0u8; 256];
    for i in 0..256 {
        let x = (i as f64) / 64.0;  // 0 to 4.0
        let g = (-0.5 * x * x).exp();
        lut[i] = (g * 255.0).min(255.0) as u8;
    }
    lut
}

/// Logistic LUT for speed likelihood: 1 / (1 + exp(k * (v - v_stop)))
/// v_stop = 200 cm/s, k = 0.01.
/// Index i = v / 10. Range [0, 1270] cm/s.
pub fn build_logistic_lut() -> [u8; 128] {
    let mut lut = [0u8; 128];
    let k = 0.01;
    let v_stop = 200.0;
    for i in 0..128 {
        let v = (i as f64) * 10.0;  // 0 to 1270 cm/s
        let l = 1.0 / (1.0 + (k * (v - v_stop)).exp());
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

#[cfg(test)]
mod tests {
    use super::*;
    use shared::Stop;

    #[test]
    fn test_lut_generation() {
        let g_lut = build_gaussian_lut();
        assert_eq!(g_lut[0], 255);  // x=0 → max probability
        assert!(g_lut[255] < 10);   // x=4.0 → near zero

        let l_lut = build_logistic_lut();
        assert!(l_lut[0] > 200);    // v=0 cm/s → high probability (approx 0.88 → 224)
        assert_eq!(l_lut[20], 127); // v=200 cm/s → exactly at v_stop (0.5 → 127)
        assert!(l_lut[100] < 20);   // v=1000 cm/s → low probability
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

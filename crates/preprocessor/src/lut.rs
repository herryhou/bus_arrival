// Lookup Table (LUT) generation for arrival probability model
//
// Generates precomputed tables that match arrival_detector runtime formulas:
// - Gaussian: exp(-x²/2) for distance and progress similarity
// - Logistic: 1/(1 + exp(k*(v-v_stop))) for speed similarity (v_stop=200cm/s, k=0.01)
//
// All outputs are scaled to u8 (0..255) to represent 0.0..1.0.

/// Gaussian LUT generator
///
/// x / sigma range [0, 4.0) mapped to 256 entries.
/// Returns exp(-x²/2) scaled to u8 (0..255).
pub fn generate_gaussian_lut() -> Vec<u8> {
    (0..256)
        .map(|i| {
            let x = i as f64 / 64.0; // x range [0, 4.0)
            let val = (-x * x / 2.0).exp();
            (val * 255.0).round() as u8
        })
        .collect()
}

/// Logistic LUT generator (for speed)
///
/// Matches arrival_detector formula: 1 / (1 + exp(k * (v - v_stop)))
/// where v_stop = 200 cm/s, k = 0.01.
/// Index i = v / 10. Range [0, 1270] cm/s mapped to 128 entries.
pub fn generate_logistic_lut() -> Vec<u8> {
    const V_STOP: f64 = 200.0;  // cm/s, speed at which probability = 50%
    const K: f64 = 0.01;         // logistic growth factor

    (0..128)
        .map(|i| {
            let v = i as f64 * 10.0; // v range [0, 1270] cm/s
            // Logistic function: 1 / (1 + exp(k * (v - v_stop)))
            // At v = v_stop (200 cm/s), probability = 0.5
            let val = 1.0 / (1.0 + (K * (v - V_STOP)).exp());
            (val * 255.0).round() as u8
        })
        .collect()
}

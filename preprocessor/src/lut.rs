// Lookup Table (LUT) generation for arrival probability model
//
// Generates precomputed tables for:
// - Gaussian: exp(-x²/2) for distance and progress similarity
// - Logistic: 1/(1 + exp(-x)) for speed similarity
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
/// x / v_stop range [0, 4.0) mapped to 128 entries.
/// Returns 1/(1 + exp(x-2)) or similar sigmoid, scaled to u8 (0..255).
/// Specifically for speed where low speed = high probability:
/// f(v) = 1 / (1 + exp((v - v_stop) / scale))
pub fn generate_logistic_lut() -> Vec<u8> {
    // We'll generate a 128-entry table for v/v_stop
    (0..128)
        .map(|i| {
            let v_ratio = i as f64 / 32.0; // v/v_stop range [0, 4.0)
            // Logistic function: 1 / (1 + exp(v_ratio * 4.0 - 2.0))
            // This centers the drop-off around v_ratio = 0.5 (v = 0.5 * v_stop)
            let val = 1.0 / (1.0 + (v_ratio * 6.0 - 3.0).exp());
            (val * 255.0).round() as u8
        })
        .collect()
}

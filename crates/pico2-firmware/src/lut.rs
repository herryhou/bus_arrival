//! Lookup tables for arrival probability computation
//!
//! These tables are computed at compile time to avoid runtime floating point
//! operations in the no_std embedded environment.

use shared::Prob8;

// ===== LUT Builders =====

/// Build Gaussian LUT: exp(-x²/2) for arrival probability computation
/// Index i = (x / sigma) * 64. Range [0, 4.0).
const fn build_gaussian_lut() -> [u8; 256] {
    let mut lut = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        // x = i / 64.0 (0 to 4.0)
        // For no_std, we use a simple approximation
        // exp(-x²/2) where x in [0, 4)
        let x = i as i32; // x / 64.0 scaled by 64
        let x2 = x * x;
        // Simple approximation: 255 when x=0, decreasing
        let val = if x2 < 64 {
            255 - (x2 / 64) * 50
        } else if x2 < 256 {
            200 - ((x2 - 64) / 192) * 100
        } else if x2 < 576 {
            100 - ((x2 - 256) / 320) * 60
        } else {
            40 - ((x2 - 576) / 64) * 10
        };
        lut[i] = if val < 0 { 0 } else { val as u8 };
        i += 1;
    }
    lut
}

/// Build logistic LUT for speed likelihood: 1 / (1 + exp(k * (v - v_stop)))
/// v_stop = 200 cm/s, k = 0.01.
/// Index i = v / 10. Range [0, 1270] cm/s.
const fn build_logistic_lut() -> [u8; 128] {
    let mut lut = [0u8; 128];
    let mut i = 0;
    while i < 128 {
        let v = i as i32 * 10; // 0 to 1270 cm/s
        // Simple logistic approximation: 1 / (1 + exp(k * (v - v_stop)))
        // k = 0.01, v_stop = 200
        let delta = v - 200;
        // Approximate exp(delta / 100) for small delta
        let exp_val = if delta < -200 {
            0 // exp(-2) ~ 0.135, treat as 0
        } else if delta < 0 {
            1 // exp(negative small) ~ 1 to 2
        } else if delta < 200 {
            2 + (delta / 100) // exp(0 to 2) ~ 1 to 7.4
        } else if delta < 400 {
            4 + ((delta - 200) / 200) * 3 // exp(2 to 4) ~ 7.4 to 54.6
        } else {
            20 + ((delta - 400) / 100) // exp(4+) grows rapidly
        };
        let l = 255 / (1 + exp_val);
        lut[i] = if l < 0 { 0 } else { l as u8 };
        i += 1;
    }
    lut
}

// ===== Public LUTs =====

/// Gaussian LUT for distance features
pub static GAUSSIAN_LUT: [u8; 256] = build_gaussian_lut();

/// Logistic LUT for speed features
pub static LOGISTIC_LUT: [u8; 128] = build_logistic_lut();

/// Arrival threshold: 75% probability
pub const THETA_ARRIVAL: Prob8 = 191;

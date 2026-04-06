//! Lookup tables for arrival probability computation
//!
//! These tables are precomputed using f64 precision and embedded as constants
//! to avoid runtime floating point operations in the no_std embedded environment.

use shared::Prob8;

// ===== Precomputed LUTs =====

/// Gaussian LUT: exp(-x²/2) for arrival probability computation
///
/// Formula: exp(-0.5 * x²) where x = i / 64.0 (range [0, ~4.0))
///
/// Values generated using f64 precision for accuracy:
/// - i=0: exp(0) = 1.0 → 255
/// - i=64: exp(-0.5) ≈ 0.607 → 154
/// - i=128: exp(-2.0) ≈ 0.135 → 34
///
/// Generated from: crates/pipeline/detection/src/probability.rs
pub const GAUSSIAN_LUT: [u8; 256] = [
    255, 254, 254, 254, 254, 254, 253, 253, 253, 252, 251, 251, 250, 249, 248, 248,
    247, 246, 245, 244, 242, 241, 240, 239, 237, 236, 234, 233, 231, 230, 228, 226,
    225, 223, 221, 219, 217, 215, 213, 211, 209, 207, 205, 203, 201, 199, 196, 194,
    192, 190, 187, 185, 183, 180, 178, 176, 173, 171, 169, 166, 164, 161, 159, 157,
    154, 152, 149, 147, 145, 142, 140, 137, 135, 133, 130, 128, 125, 123, 121, 119,
    116, 114, 112, 109, 107, 105, 103, 101, 99, 96, 94, 92, 90, 88, 86, 84,
    82, 80, 78, 77, 75, 73, 71, 69, 68, 66, 64, 63, 61, 59, 58, 56,
    55, 53, 52, 50, 49, 47, 46, 45, 43, 42, 41, 40, 39, 37, 36, 35,
    34, 33, 32, 31, 30, 29, 28, 27, 26, 25, 24, 24, 23, 22, 21, 21,
    20, 19, 18, 18, 17, 16, 16, 15, 15, 14, 14, 13, 13, 12, 12, 11,
    11, 10, 10, 9, 9, 9, 8, 8, 8, 7, 7, 7, 6, 6, 6, 6,
    5, 5, 5, 5, 4, 4, 4, 4, 4, 3, 3, 3, 3, 3, 3, 2,
    2, 2, 2, 2, 2, 2, 2, 2, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

/// Logistic LUT for speed likelihood: 1 / (1 + exp(k * (v - v_stop)))
///
/// Formula: 1 / (1 + exp(k * (v - v_stop))) where k=0.01, v_stop=200 cm/s
/// Index i = v / 10, v in range [0, 1270] cm/s
///
/// Values generated using f64 precision for accuracy:
/// - i=0 (v=0): exp(2)≈7.39 → 1/8.39≈0.119 → 30 (actual: 224 due to different formula)
/// - i=20 (v=200): exp(0)=1 → 1/2=0.5 → 127
/// - i=127 (v=1270): exp(10.7)≈44355 → ~0
///
/// Generated from: crates/pipeline/detection/src/probability.rs
pub const LOGISTIC_LUT: [u8; 128] = [
    224, 221, 218, 215, 212, 208, 204, 200, 195, 191, 186, 181, 175, 170, 164, 158,
    152, 146, 140, 133, 127, 121, 114, 108, 102, 96, 90, 84, 79, 73, 68, 63,
    59, 54, 50, 46, 42, 39, 36, 33, 30, 27, 25, 23, 21, 19, 17, 16,
    14, 13, 12, 10, 9, 9, 8, 7, 6, 6, 5, 5, 4, 4, 3, 3,
    3, 2, 2, 2, 2, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

/// Arrival threshold: 75% probability
pub const THETA_ARRIVAL: Prob8 = 191;

//! LUTs for arrival probability computation
//! Generated at build time from pipeline probability module
#![allow(dead_code)]

include!(concat!(env!("OUT_DIR"), "/lut_generated.rs"));

use shared::Prob8;

/// Arrival threshold: 75% probability
pub const THETA_ARRIVAL: Prob8 = 191;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lut_spot_check() {
        // Check boundary values
        assert_eq!(
            GAUSSIAN_LUT[0], 255,
            "LUT[0] should be 255 (max probability)"
        );

        // Check middle values with tolerance (updated for current LUT generation)
        assert!(
            (GAUSSIAN_LUT[64] as i32 - 155).abs() < 5,
            "LUT[64] should be ~155"
        );
        assert!(
            (GAUSSIAN_LUT[128] as i32 - 33).abs() < 5,
            "LUT[128] should be ~33"
        );

        // Check near-zero value
        assert!(GAUSSIAN_LUT[255] < 10, "LUT[255] should be near 0");

        // Verify monotonic decreasing property
        for i in 1..GAUSSIAN_LUT.len() {
            assert!(
                GAUSSIAN_LUT[i] <= GAUSSIAN_LUT[i - 1],
                "LUT should be monotonically decreasing: LUT[{}] = {} > LUT[{}] = {}",
                i,
                GAUSSIAN_LUT[i],
                i - 1,
                GAUSSIAN_LUT[i - 1]
            );
        }
    }
}

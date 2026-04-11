//! LUTs for arrival probability computation
//! Auto-generated from pipeline source

include!(concat!(env!("OUT_DIR"), "/lut_generated.rs"));

use shared::Prob8;

/// Arrival threshold: 75% probability
pub const THETA_ARRIVAL: Prob8 = 191;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lut_spot_check() {
        assert_eq!(GAUSSIAN_LUT[0], 255);
        assert!((GAUSSIAN_LUT[64] as i32 - 170).abs() < 5);
    }
}

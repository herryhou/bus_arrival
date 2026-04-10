//! Shared probability model parameters
//! Single source of truth for both pipeline (LUT generation) and firmware (detection)

use crate::SpeedCms;

/// Distance likelihood sigma (cm) - Section 13.1 of tech report
pub const SIGMA_D_CM: i32 = 2750;

/// Progress difference sigma (cm) - Section 13.1 of tech report
pub const SIGMA_P_CM: i32 = 2000;

/// Stop speed threshold (cm/s) - 200 cm/s = 7.2 km/h - Section 13.2
pub const V_STOP_CMS: SpeedCms = 200;

/// Logistic LUT resolution: 0-127 cm/s -> 0-255 probability
pub const SPEED_LUT_MAX_IDX: usize = 127;

/// Gaussian LUT resolution: 0-255 index -> 0-255 probability
pub const GAUSSIAN_LUT_SIZE: usize = 256;

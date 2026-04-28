//! Kalman filter state — isolated estimation component
//!
//! This is a refactor of gps_processor::kalman with control state removed.
//! No freeze_ctx, no off_route counters — pure estimation.

use shared::{DistCm, SpeedCms};

pub struct KalmanState {
    pub s_cm: DistCm,
    pub v_cms: SpeedCms,
    pub last_seg_idx: usize,
}

impl KalmanState {
    pub fn new() -> Self {
        Self {
            s_cm: 0,
            v_cms: 0,
            last_seg_idx: 0,
        }
    }
}

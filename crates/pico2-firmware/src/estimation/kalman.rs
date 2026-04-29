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

    /// HDOP-adaptive Kalman update
    pub fn update_adaptive(&mut self, z_raw: DistCm, v_gps: SpeedCms, hdop_x10: u16) {
        // HDOP-adaptive gain
        let k_pos = if hdop_x10 <= 20 {
            77
        } else if hdop_x10 <= 30 {
            51
        } else if hdop_x10 <= 50 {
            26
        } else {
            13
        };

        // Predict step: propagate velocity into position
        let s_pred = self.s_cm + self.v_cms;

        // Innovation term uses predicted position
        let innovation = z_raw - s_pred;

        // Update step with innovation
        self.s_cm = s_pred + k_pos * innovation / 256;

        // Velocity update (fixed gain)
        self.v_cms = self.v_cms + 77 * (v_gps - self.v_cms) / 256;
        self.v_cms = self.v_cms.max(0);
    }
}

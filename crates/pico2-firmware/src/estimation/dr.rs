//! Dead-reckoning state — isolated estimation component

use shared::SpeedCms;

pub struct DrState {
    pub filtered_v: SpeedCms,
    pub last_gps_time: Option<u64>,
    pub in_recovery: bool,
}

impl DrState {
    pub fn new() -> Self {
        Self {
            filtered_v: 0,
            last_gps_time: None,
            in_recovery: false,
        }
    }
}

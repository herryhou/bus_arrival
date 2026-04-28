//! Estimation layer — isolated GPS → position pipeline
//!
//! This layer is isolated from control layer concerns.
//! It maintains internal Kalman + DR state but does NOT access:
//! - mode, last_stop_index, frozen_s_cm

pub mod kalman;
pub mod dr;

use shared::{GpsPoint, binfile::RouteData};

pub use kalman::KalmanState;
pub use dr::DrState;

/// Combined estimation state (internal only)
pub struct EstimationState {
    pub kalman: KalmanState,
    pub dr: DrState,
}

impl EstimationState {
    pub fn new() -> Self {
        Self {
            kalman: KalmanState::new(),
            dr: DrState::new(),
        }
    }
}

/// Estimation input — GPS + route data
pub struct EstimationInput<'a> {
    pub gps: GpsPoint,
    pub route_data: &'a RouteData<'a>,
    pub is_first_fix: bool,
}

/// Estimation output — all derived position signals
pub struct EstimationOutput {
    /// Raw GPS projection onto route (for F1 probability)
    pub z_gps_cm: shared::DistCm,
    /// Kalman-filtered position (primary position in Normal mode)
    pub s_cm: shared::DistCm,
    /// Filtered velocity (cm/s)
    pub v_cms: shared::SpeedCms,
    /// Divergence from route (squared distance from map matching)
    pub divergence_d2: shared::Dist2,
    /// Confidence signal (0-255, higher is better)
    pub confidence: u8,
    /// Whether GPS has valid fix
    pub has_fix: bool,
}

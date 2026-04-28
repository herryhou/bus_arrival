//! Recovery module — pure stop index recovery
//!
//! This is a pure function with all dependencies passed explicitly.
//! No access to KalmanState or control layer state.

pub mod search;

pub use search::recover;

/// Recovery input — all parameters explicit
#[derive(Debug)]
pub struct RecoveryInput {
    /// Current position (use z_gps_cm during recovery)
    pub s_cm: shared::DistCm,
    /// Filtered velocity (cm/s)
    pub v_cms: shared::SpeedCms,
    /// Time since freeze/recovery began (seconds)
    pub dt_seconds: u64,
    /// All stops on route
    pub stops: heapless::Vec<shared::Stop, 256>,
    /// Hint: last known stop index (from control layer)
    pub hint_idx: u8,
    /// Optional spatial anchor (frozen position)
    /// Only Some() when called from OffRoute/Recovering
    pub frozen_s_cm: Option<shared::DistCm>,
    /// Search window: ±N stops from hint_idx (default 10)
    pub search_window: u8,
}

//! Control layer — state machine and orchestration
//!
//! This layer manages system mode (Normal/OffRoute/Recovering) and
//! orchestrates the isolated estimation and detection layers.

pub mod mode;
pub mod timeout;

use crate::estimation::EstimationOutput;
use detection::StopState;
use shared::{DistCm, binfile::RouteData};

pub use mode::{SystemMode, TransitionAction};
pub use timeout::{check_recovering_timeout, find_closest_stop_index};

/// Top-level system state (control layer)
pub struct SystemState<'a> {
    /// Current operational mode
    pub mode: SystemMode,
    /// Last confirmed stop index (for recovery hint)
    pub last_stop_index: u8,
    /// Frozen position during OffRoute/Recovering (None in Normal mode)
    pub frozen_s_cm: Option<DistCm>,
    /// Hysteresis counter for OffRoute → Normal transition
    pub off_route_clear_ticks: u8,
    /// Hysteresis counter for Normal → OffRoute transition
    pub off_route_suspect_ticks: u8,
    /// Timestamp when OffRoute was entered (for recovery dt calculation)
    pub off_route_since: Option<u64>,
    /// Timestamp when Recovering was entered (for timeout)
    pub recovering_since: Option<u64>,
    /// Recovery failed flag (set after timeout, suppresses announcements)
    pub recovery_failed: bool,
    /// Route data reference (immutable, XIP-friendly)
    pub route_data: &'a RouteData<'a>,
    /// Stop FSM states (detection layer)
    pub stop_states: heapless::Vec<StopState, 256>,
    /// Pending persisted state from flash
    pub pending_persisted: Option<shared::PersistedState>,
    /// Last stop index that was persisted to flash
    pub last_persisted_stop: u8,
    /// Ticks since last persist operation
    pub ticks_since_persist: u16,
}

impl<'a> SystemState<'a> {
    pub fn new(route_data: &'a RouteData<'a>, persisted: Option<shared::PersistedState>) -> Self {
        let stop_count = route_data.stop_count;
        let mut stop_states = heapless::Vec::new();
        for i in 0..stop_count {
            let _ = stop_states.push(StopState::new(i as u8));
        }

        Self {
            mode: SystemMode::Normal,
            last_stop_index: 0,
            frozen_s_cm: None,
            off_route_clear_ticks: 0,
            off_route_suspect_ticks: 0,
            off_route_since: None,
            recovering_since: None,
            recovery_failed: false,
            route_data,
            stop_states,
            pending_persisted: persisted,
            last_persisted_stop: persisted.map(|p| p.last_stop_index).unwrap_or(0),
            ticks_since_persist: 0,
        }
    }

    pub fn current_position(&self, est: &EstimationOutput) -> DistCm {
        match self.mode {
            SystemMode::Normal => est.s_cm,
            SystemMode::OffRoute => self.frozen_s_cm.expect("Invariant: frozen_s_cm set in OffRoute"),
            SystemMode::Recovering => est.z_gps_cm,
        }
    }
}

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

    /// Transition to OffRoute mode
    fn transition_to_offroute(&mut self, est: &EstimationOutput, now: u64) {
        self.mode = SystemMode::OffRoute;
        self.frozen_s_cm = Some(est.s_cm);
        self.off_route_clear_ticks = 0;
        self.off_route_since = Some(now);
    }

    /// Transition to Normal mode (direct from OffRoute)
    fn transition_offroute_to_normal(&mut self) {
        self.mode = SystemMode::Normal;
        self.frozen_s_cm = None;
        self.off_route_since = None;
        self.off_route_clear_ticks = 0;
        self.off_route_suspect_ticks = 0;
    }

    /// Transition to Recovering mode
    fn transition_to_recovering(&mut self, now: u64) {
        self.mode = SystemMode::Recovering;
        self.recovering_since = Some(now);
        // frozen_s_cm is preserved from OffRoute
    }

    /// Recovery success handler
    fn recovery_success(&mut self, recovered_idx: usize, s_cm: DistCm) {
        self.mode = SystemMode::Normal;
        self.last_stop_index = recovered_idx as u8;
        self.frozen_s_cm = None;
        self.recovering_since = None;
        self.recovery_failed = false;

        // Reset stop states with new index
        self.reset_stop_states_after_recovery(recovered_idx, s_cm);
    }

    /// Reset stop states after recovery
    fn reset_stop_states_after_recovery(&mut self, recovered_idx: usize, current_s_cm: DistCm) {
        use shared::FsmState;

        // Reset all stop states
        for i in 0..self.stop_states.len() {
            self.stop_states[i] = detection::StopState::new(i as u8);
        }

        // Stops before recovered stop are already passed
        for i in 0..recovered_idx.min(self.stop_states.len()) {
            self.stop_states[i].fsm_state = FsmState::Departed;
            self.stop_states[i].announced = true;
        }

        // Recovered stop is Approaching if within corridor
        if let Some(stop) = self.route_data.get_stop(recovered_idx) {
            if let Some(state) = self.stop_states.get_mut(recovered_idx) {
                if current_s_cm >= stop.corridor_start_cm && current_s_cm <= stop.corridor_end_cm {
                    state.fsm_state = FsmState::Approaching;
                }
            }
        }
    }

    /// Find closest stop index (for recovery timeout fallback)
    fn find_closest_stop_index_internal(&self, s_cm: DistCm) -> u8 {
        let mut closest_idx = 0;
        let mut closest_dist = i32::MAX;

        for i in 0..self.route_data.stop_count {
            if let Some(stop) = self.route_data.get_stop(i) {
                let dist = (s_cm - stop.progress_cm).abs();
                if dist < closest_dist {
                    closest_dist = dist;
                    closest_idx = i;
                }
            }
        }

        closest_idx as u8
    }

    /// Collect stops into heapless Vec (for recovery input)
    fn collect_stops(&self) -> heapless::Vec<shared::Stop, 256> {
        let mut stops = heapless::Vec::new();
        for i in 0..self.route_data.stop_count {
            if let Some(stop) = self.route_data.get_stop(i) {
                let _ = stops.push(stop);
            }
        }
        stops
    }

    /// Attempt recovery (in Recovering mode only)
    fn attempt_recovery(&mut self, est: &EstimationOutput, now: u64) -> Option<usize> {
        // Check timeout first
        if check_recovering_timeout(self.mode, self.recovering_since, now) {
            // Fallback to geometric search
            let best_idx = self.find_closest_stop_index_internal(est.s_cm);

            self.recovery_failed = true;
            self.mode = SystemMode::Normal;
            self.last_stop_index = best_idx;
            self.frozen_s_cm = None;
            self.recovering_since = None;

            self.reset_stop_states_after_recovery(best_idx as usize, est.s_cm);

            return Some(best_idx as usize);
        }

        // Build RecoveryInput
        let dt = self.off_route_since
            .map(|t| now.saturating_sub(t))
            .unwrap_or(1);

        let input = crate::recovery::RecoveryInput {
            s_cm: est.z_gps_cm,
            v_cms: est.v_cms,
            dt_seconds: dt,
            stops: self.collect_stops(),
            hint_idx: self.last_stop_index,
            frozen_s_cm: self.frozen_s_cm,
            search_window: 10,
        };

        // Call pure recovery function
        crate::recovery::recover(input).map(|idx| idx as usize)
    }
}

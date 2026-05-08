//! Control layer — state machine and orchestration
//!
//! This layer manages system mode (Normal/OffRoute/Recovering) via ModeMachine
//! and orchestrates the isolated estimation and detection layers.
//!
//! # Architecture
//!
//! - **ModeMachine**: Pure state machine for mode transitions with hysteresis
//! - **SystemState**: Orchestrator that coordinates estimation, mode, detection, recovery
//! - **Isolation**: Estimation layer has no access to control state; mode logic has no access to position

pub mod machine;
pub mod mode;
pub mod timeout;

use shared::{DistCm, binfile::RouteData, GpsPoint, ArrivalEvent};
use crate::estimation::EstimationOutput;
use crate::estimation::EstimationInput;

pub use mode::{SystemMode, TransitionAction};
pub use timeout::{check_recovering_timeout, find_closest_stop_index};
pub use machine::{ModeMachine, ModeInput, ModeOutput, ModeAction};

/// Top-level system state (control layer)
pub struct SystemState<'a> {
    /// Mode machine — encapsulates mode transitions and hysteresis
    mode_machine: ModeMachine,
    /// Last confirmed stop index (for recovery hint)
    pub last_stop_index: u8,
    /// Frozen position during OffRoute/Recovering (None in Normal mode)
    pub frozen_s_cm: Option<DistCm>,
    /// Timestamp when OffRoute was entered (for recovery dt calculation)
    pub off_route_since: Option<u64>,
    /// Timestamp when Recovering was entered (for timeout)
    pub recovering_since: Option<u64>,
    /// Recovery failed flag (set after timeout, suppresses announcements)
    pub recovery_failed: bool,
    /// Flag indicating recovery should run on next valid GPS after off-route
    pub needs_recovery_on_reacquisition: bool,
    /// Route data reference (immutable, XIP-friendly)
    pub route_data: &'a RouteData<'a>,
    /// Pending persisted state from flash
    pub pending_persisted: Option<shared::PersistedState>,
    /// Last stop index that was persisted to flash
    pub last_persisted_stop: u8,
    /// Ticks since last persist operation
    pub ticks_since_persist: u16,
    /// Previous position for monotonic checking
    pub last_s_cm: DistCm,
    /// Counter for backward jump events (GPS health monitoring)
    pub backward_jump_count: u32,
    /// Whether we've received the first valid GPS fix (for cold-start initialization)
    has_received_first_fix: bool,
    /// Valid GPS ticks where detection ran (for warmup)
    detection_warmup_ticks: u8,
}

impl<'a> SystemState<'a> {
    pub fn new(route_data: &'a RouteData<'a>, persisted: Option<shared::PersistedState>) -> Self {
        Self {
            mode_machine: ModeMachine::new(),
            last_stop_index: 0,
            frozen_s_cm: None,
            off_route_since: None,
            recovering_since: None,
            recovery_failed: false,
            needs_recovery_on_reacquisition: false,
            route_data,
            pending_persisted: persisted,
            last_persisted_stop: persisted.map(|p| p.last_stop_index).unwrap_or(0),
            ticks_since_persist: 0,
            last_s_cm: 0,
            backward_jump_count: 0,
            has_received_first_fix: false,
            detection_warmup_ticks: 0,
        }
    }

    /// Get current mode (via ModeMachine)
    pub fn mode(&self) -> SystemMode {
        self.mode_machine.mode()
    }

    /// Get current stop index (for persistence)
    pub fn current_stop_index(&self) -> Option<u8> {
        // Return last known stop index
        Some(self.last_stop_index)
    }

    /// Check if state should be persisted (rate limited)
    pub fn should_persist(&self, current_stop: u8) -> bool {
        // Only persist if stop index changed and enough ticks have passed
        if current_stop != self.last_persisted_stop {
            // Rate limit: at least 60 ticks (60 seconds at 1Hz) between persists
            self.ticks_since_persist >= 60
        } else {
            false
        }
    }

    /// Mark that state has been persisted
    pub fn mark_persisted(&mut self, stop_index: u8) {
        self.last_persisted_stop = stop_index;
        self.ticks_since_persist = 0;
    }

    /// Returns the single authoritative position for the current mode.
    ///
    /// # Spatial Contract
    /// - Normal: Kalman-filtered position (`est.s_cm`)
    /// - OffRoute: Frozen position from entry (`self.frozen_s_cm`)
    /// - Recovering: Raw GPS projection (`est.z_gps_cm`)
    ///
    /// This is the ONLY function that should be used to query "where are we?"
    pub fn current_position(&self, est: &EstimationOutput) -> DistCm {
        match self.mode_machine.mode() {
            SystemMode::Normal => est.s_cm,
            SystemMode::OffRoute => self.frozen_s_cm.expect("Invariant: frozen_s_cm set in OffRoute"),
            SystemMode::Recovering => est.z_gps_cm,
        }
    }

    /// Transition to OffRoute mode
    fn transition_to_offroute(&mut self, est: &EstimationOutput, now: u64) {
        self.frozen_s_cm = Some(est.s_cm);
        self.off_route_since = Some(now);
    }

    /// Transition to Normal mode (direct from OffRoute)
    fn transition_offroute_to_normal(&mut self) {
        self.frozen_s_cm = None;
        self.off_route_since = None;
        // Set flag for re-acquisition recovery when we get GPS fix without snap
        self.needs_recovery_on_reacquisition = true;
    }

    /// Transition to Recovering mode
    fn transition_to_recovering(&mut self, now: u64) {
        self.recovering_since = Some(now);
        // frozen_s_cm is preserved from OffRoute
    }

    /// Recovery success handler
    fn recovery_success(&mut self, recovered_idx: usize, s_cm: DistCm) {
        self.mode_machine.transition_to_normal();
        self.last_stop_index = recovered_idx as u8;
        self.frozen_s_cm = None;
        self.recovering_since = None;
        self.recovery_failed = false;
        self.last_s_cm = s_cm;
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
        if check_recovering_timeout(self.mode_machine.mode(), self.recovering_since, now) {
            // Fallback to geometric search
            let best_idx = self.find_closest_stop_index_internal(est.s_cm);

            self.recovery_failed = true;
            self.mode_machine.transition_to_normal();
            self.last_stop_index = best_idx;
            self.frozen_s_cm = None;
            self.recovering_since = None;

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

    /// Main tick function — control layer orchestrator
    ///
    /// # Responsibilities
    /// 1. Call isolated estimation layer
    /// 2. Enforce monotonic invariant at system boundary
    /// 3. Execute state machine transitions
    /// 4. Run detection (only in Normal mode)
    /// 5. Emit events
    ///
    /// # Invariants
    /// - Recovery ONLY runs in Recovering mode
    /// - frozen_s_cm only accessed in OffRoute/Recovering modes
    /// - Only ONE transition executes per tick
    pub fn tick(&mut self, gps: &GpsPoint, est_state: &mut crate::estimation::EstimationState) -> Option<ArrivalEvent> {
        // STEP 1: Isolated estimation
        let input = EstimationInput {
            gps: gps.clone(),
            route_data: self.route_data,
            is_first_fix: !self.has_received_first_fix,
        };
        let est = crate::estimation::estimate(input, est_state);

        // Handle GPS outage
        if !est.has_fix {
            // TODO: handle outage
            return None;
        }

        // Mark first fix as received after successful GPS fix
        if est.has_fix {
            self.has_received_first_fix = true;
        }

        // STEP 1.5: Enforce monotonic invariant
        // CRITICAL: Use current_position() to get mode-specific position
        // Normal → est.s_cm, Recovering → est.z_gps_cm, OffRoute → frozen_s_cm
        let s_raw = self.current_position(&est);
        let (s_cm_for_detection, did_jump) = if self.last_s_cm == 0 {
            // First fix: skip check, initialize directly
            (s_raw, false)
        } else {
            enforce_monotonic(s_raw, self.last_s_cm, self.mode_machine.mode())
        };
        if did_jump {
            self.backward_jump_count += 1;
        }
        self.last_s_cm = s_cm_for_detection;

        // STEP 2: State machine transitions (via ModeMachine)
        let old_mode = self.mode_machine.mode();

        // Build mode input
        let mode_input = ModeInput {
            divergence_d2: est.divergence_d2,
            has_gps_fix: est.has_fix,
            frozen_s_cm: self.frozen_s_cm,
            current_z_gps_cm: est.z_gps_cm,
        };
        let mode_output = self.mode_machine.update(mode_input);

        // Handle ModeAction
        match mode_output.action {
            ModeAction::FreezePosition => {
                self.transition_to_offroute(&est, gps.timestamp);
                return None;  // Suppress detection during transition
            }
            ModeAction::BeginRecovery => {
                self.transition_to_recovering(gps.timestamp);
                // Fall through to recovery handling
            }
            ModeAction::ResumeNormal => {
                self.transition_offroute_to_normal();
                return None;  // Will resume detection next tick
            }
            ModeAction::None => {
                // No transition action
            }
        }

        // INVARIANT CHECK (debug builds only)
        #[cfg(debug_assertions)]
        {
            if old_mode != mode_output.mode {
                // Mode changed — should be exactly one transition
                debug_assert!(
                    mode_output.mode != SystemMode::Recovering || old_mode == SystemMode::OffRoute,
                    "Invariant violated: unexpected mode transition"
                );
            }

            // INVARIANT: frozen_s_cm consistency
            match mode_output.mode {
                SystemMode::Normal => {
                    debug_assert!(
                        self.frozen_s_cm.is_none(),
                        "Invariant violated: frozen_s_cm set in Normal mode"
                    );
                }
                SystemMode::OffRoute | SystemMode::Recovering => {
                    debug_assert!(
                        self.frozen_s_cm.is_some(),
                        "Invariant violated: frozen_s_cm not set in OffRoute/Recovering"
                    );
                }
            }
        }

        // STEP 3: Recovery (ONLY in Recovering mode)
        if mode_output.mode == SystemMode::Recovering {
            if let Some(idx) = self.attempt_recovery(&est, gps.timestamp) {
                self.recovery_success(idx, s_cm_for_detection);
                // Continue to detection
            } else {
                return None;  // Recovery failed, stay in Recovering
            }
        }

        // STEP 4: Detection (ONLY in Normal mode)
        if mode_output.mode == SystemMode::Normal {
            return self.run_detection(&est, s_cm_for_detection, gps.timestamp);
        }

        None
    }

    /// Run arrival detection (Normal mode only)
    fn run_detection(&mut self, est: &EstimationOutput, s_cm: DistCm, timestamp: u64) -> Option<ArrivalEvent> {
        use crate::detection::{self, GpsStatus};
        use shared::PositionSignals;

        // Block detection during warmup
        if self.detection_warmup_ticks < 3 {
            self.detection_warmup_ticks += 1;
            return None;
        }

        // Create position signals for detection
        let signals = PositionSignals {
            z_gps_cm: est.z_gps_cm,
            s_cm: est.s_cm,
        };

        // Find active stops (corridor filter)
        let active_indices = detection::find_active_stops(signals, self.route_data);

        // Check each active stop for arrival
        for &stop_idx in &active_indices {
            let stop = self.route_data.get_stop(stop_idx as usize)?;

            // Get next stop for dwell time calculation
            let next_stop = self.route_data.get_stop(stop_idx as usize + 1);

            // Compute arrival probability
            let prob = detection::compute_arrival_probability_adaptive(
                signals,
                est.v_cms,
                &stop,
                0, // dwell_time_s - not tracking for now
                GpsStatus::Valid,
                next_stop.as_ref(),
            );

            // High probability threshold for arrival detection
            if prob > 200 {
                self.last_stop_index = stop_idx as u8;
                return Some(ArrivalEvent {
                    time: timestamp,
                    stop_idx: stop_idx as u8,
                    s_cm,
                    v_cms: est.v_cms,
                    probability: prob,
                    event_type: shared::ArrivalEventType::Arrival,
                });
            }
        }

        None
    }
}

/// Enforce hard monotonic invariant at system boundary.
///
/// # Returns
/// * (s_cm, false) - position is valid, use as-is
/// * (s_prev, true) - backward jump detected, clamped to previous
///
/// # Mode behavior
/// * Normal: strict monotonic (s_new >= s_prev)
/// * Recovering: allow backward (re-localization may need it)
/// * OffRoute: frozen (returns s_prev, no jump counted)
pub fn enforce_monotonic(
    s_new: DistCm,
    s_prev: DistCm,
    mode: SystemMode,
) -> (DistCm, bool) {
    match mode {
        SystemMode::Normal => {
            if s_new < s_prev {
                (s_prev, true)
            } else {
                (s_new, false)
            }
        }
        SystemMode::Recovering => {
            (s_new, false)
        }
        SystemMode::OffRoute => {
            (s_prev, false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enforce_monotonic_normal_forward() {
        let (s_cm, did_jump) = enforce_monotonic(10500, 10000, SystemMode::Normal);
        assert_eq!(s_cm, 10500);
        assert!(!did_jump);
    }

    #[test]
    fn test_enforce_monotonic_normal_backward_jump() {
        let (s_cm, did_jump) = enforce_monotonic(9800, 10500, SystemMode::Normal);
        assert_eq!(s_cm, 10500);
        assert!(did_jump);
    }

    #[test]
    fn test_enforce_monotonic_normal_exact_equality() {
        let (s_cm, did_jump) = enforce_monotonic(10000, 10000, SystemMode::Normal);
        assert_eq!(s_cm, 10000);
        assert!(!did_jump);
    }

    #[test]
    fn test_enforce_monotonic_recovering_backward_allowed() {
        let (s_cm, did_jump) = enforce_monotonic(9500, 10500, SystemMode::Recovering);
        assert_eq!(s_cm, 9500);
        assert!(!did_jump);
    }

    #[test]
    fn test_enforce_monotonic_recovering_forward() {
        let (s_cm, did_jump) = enforce_monotonic(11000, 10500, SystemMode::Recovering);
        assert_eq!(s_cm, 11000);
        assert!(!did_jump);
    }

    #[test]
    fn test_enforce_monotonic_offroute_frozen() {
        let (s_cm, did_jump) = enforce_monotonic(11000, 10000, SystemMode::OffRoute);
        assert_eq!(s_cm, 10000);
        assert!(!did_jump);
    }
}

//! Control layer — state machine and orchestration
//!
//! This layer manages system mode (Normal/OffRoute/Recovering) and
//! orchestrates the isolated estimation and detection layers.

pub mod mode;
pub mod timeout;

use shared::{DistCm, binfile::RouteData, GpsPoint, ArrivalEvent};
use crate::estimation::EstimationOutput;
use crate::estimation::EstimationInput;

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
    /// Per-stop FSM states for arrival detection
    pub stop_states: heapless::Vec<detection::state_machine::StopState, 256>,
    /// First fix flag - true until first GPS fix is received
    pub first_fix: bool,
    /// Warmup: valid GPS ticks where estimation ran
    pub estimation_ready_ticks: u8,
    /// Warmup: total ticks since first fix (timeout safety valve)
    pub estimation_total_ticks: u8,
    /// Detection gating: valid ticks since estimation ready
    pub detection_enabled_ticks: u8,
    /// Detection gating: total ticks for timeout
    pub detection_total_ticks: u8,
    /// Flag indicating state was just reset (e.g., after GPS outage)
    pub just_reset: bool,
    /// Ticks remaining in snap cooldown period (prevents recovery interference)
    pub just_snapped_ticks: u8,
    /// Last valid GPS timestamp for recovery dt calculation
    pub last_gps_timestamp: u64,
}

impl<'a> SystemState<'a> {
    pub fn new(route_data: &'a RouteData<'a>, persisted: Option<shared::PersistedState>) -> Self {
        // Initialize stop_states
        let mut stop_states = heapless::Vec::new();
        for i in 0..route_data.stop_count {
            let _ = stop_states.push(detection::state_machine::StopState::new(i as u8));
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
            pending_persisted: persisted,
            last_persisted_stop: persisted.map(|p| p.last_stop_index).unwrap_or(0),
            ticks_since_persist: 0,
            last_s_cm: 0,
            backward_jump_count: 0,
            // NEW FIELDS
            stop_states,
            first_fix: true,
            estimation_ready_ticks: 0,
            estimation_total_ticks: 0,
            detection_enabled_ticks: 0,
            detection_total_ticks: 0,
            just_reset: false,
            just_snapped_ticks: 0,
            last_gps_timestamp: 0,
        }
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
        self.last_s_cm = s_cm;

        // TODO: Reset stop states when detection layer is integrated
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

            // TODO: Reset stop states when detection layer is integrated

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
            is_first_fix: false,  // TODO: track first fix
        };
        let est = crate::estimation::estimate(input, est_state);

        // Handle GPS outage
        if !est.has_fix {
            // TODO: handle outage
            return None;
        }

        // STEP 1.5: Enforce monotonic invariant
        // CRITICAL: Use current_position() to get mode-specific position
        // Normal → est.s_cm, Recovering → est.z_gps_cm, OffRoute → frozen_s_cm
        let s_raw = self.current_position(&est);
        let (s_cm_for_detection, did_jump) = if self.last_s_cm == 0 {
            // First fix: skip check, initialize directly
            (s_raw, false)
        } else {
            enforce_monotonic(s_raw, self.last_s_cm, self.mode)
        };
        if did_jump {
            self.backward_jump_count += 1;
        }
        self.last_s_cm = s_cm_for_detection;

        // STEP 2: State machine transitions (unified triggers)
        let old_mode = self.mode;

        match self.mode {
            SystemMode::Normal => {
                // Check: divergence > 50m for 5 ticks
                if mode::check_normal_to_offroute(est.divergence_d2, &mut self.off_route_suspect_ticks) {
                    self.transition_to_offroute(&est, gps.timestamp);
                    return None;  // Suppress detection during transition
                }
            }
            SystemMode::OffRoute => {
                // Priority: Check Recovering (large displacement) BEFORE Normal
                let action = mode::check_offroute_transition(
                    est.divergence_d2,
                    &mut self.off_route_clear_ticks,
                    self.frozen_s_cm,
                    est.z_gps_cm,
                );

                match action {
                    TransitionAction::ToRecovering => {
                        self.transition_to_recovering(gps.timestamp);
                        // Fall through to recovery handling
                    }
                    TransitionAction::ToNormal => {
                        self.transition_offroute_to_normal();
                        return None;  // Will resume detection next tick
                    }
                    TransitionAction::Stay => {
                        // Stay in OffRoute
                        return None;
                    }
                }
            }
            SystemMode::Recovering => {
                // Recovery handling below
            }
        }

        // INVARIANT CHECK (debug builds only)
        #[cfg(debug_assertions)]
        {
            if old_mode != self.mode {
                // Mode changed — should be exactly one transition
                debug_assert!(
                    self.mode != SystemMode::Recovering || old_mode == SystemMode::OffRoute,
                    "Invariant violated: unexpected mode transition"
                );
            }

            // INVARIANT: frozen_s_cm consistency
            match self.mode {
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
        if self.mode == SystemMode::Recovering {
            if let Some(idx) = self.attempt_recovery(&est, gps.timestamp) {
                self.recovery_success(idx, s_cm_for_detection);
                // Continue to detection
            } else {
                return None;  // Recovery failed, stay in Recovering
            }
        }

        // STEP 4: Detection (ONLY in Normal mode)
        if self.mode == SystemMode::Normal {
            return self.run_detection(&est, s_cm_for_detection, gps.timestamp);
        }

        None
    }

    /// Run arrival detection (Normal mode only)
    fn run_detection(&mut self, est: &EstimationOutput, s_cm: DistCm, timestamp: u64) -> Option<ArrivalEvent> {
        use crate::detection;
        use shared::PositionSignals;

        // Create position signals for detection
        let signals = PositionSignals {
            z_gps_cm: est.z_gps_cm,
            s_cm: est.s_cm,
        };

        // Find active stops (corridor filter)
        let active_indices = detection::find_active_stops(signals, self.route_data);

        // TODO: Implement detection FSM when stop_states are integrated
        // For now, return None to indicate no events
        let _ = active_indices;
        let _ = s_cm;
        let _ = timestamp;

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

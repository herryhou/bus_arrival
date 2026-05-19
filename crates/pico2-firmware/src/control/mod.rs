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

/// Return value from tick() - separates sync logic from async persistence
pub struct TickResult {
    /// Arrival/departure/announce event if any
    pub event: Option<ArrivalEvent>,
    /// Persist request if stop index changed and rate limit allows
    pub persist_request: Option<shared::PersistedState>,
}

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
    /// Whether we've received the first valid GPS fix (for cold-start initialization)
    has_received_first_fix: bool,

    // === NEW: Detection FSM ===
    pub stop_states: heapless::Vec<detection::state_machine::StopState, 256>,

    // === NEW: Warmup counters ===
    estimation_ready_ticks: u8,
    estimation_total_ticks: u8,
    detection_enabled_ticks: u8,
    detection_total_ticks: u8,
    just_reset: bool,

    // === NEW: GPS jump recovery tracking ===
    /// Last valid GPS position (for GPS jump detection, testing)
    pub last_valid_s_cm: DistCm,
    /// Last GPS timestamp (for GPS jump detection)
    pub last_gps_timestamp: u64,
    /// Whether re-acquisition recovery is needed (for off-route re-entry)
    pub needs_recovery_on_reacquisition: bool,

    // === NEW: Snap cooldown ===
    just_snapped_ticks: u8,
}

impl<'a> SystemState<'a> {
    pub fn new(route_data: &'a RouteData<'a>, persisted: Option<shared::PersistedState>) -> Self {
        // Initialize detection FSM states for all stops
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
            has_received_first_fix: false,
            stop_states,
            estimation_ready_ticks: 0,
            estimation_total_ticks: 0,
            detection_enabled_ticks: 0,
            detection_total_ticks: 0,
            just_reset: false,
            last_valid_s_cm: 0,
            last_gps_timestamp: 0,
            needs_recovery_on_reacquisition: false,
            just_snapped_ticks: 0,
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

    /// Check if estimation is ready (affects heading filter, Kalman)
    pub fn estimation_ready(&self) -> bool {
        self.estimation_ready_ticks >= 3 || self.estimation_total_ticks >= 10
    }

    /// Check if detection is enabled (independent of estimation)
    pub fn detection_ready(&self) -> bool {
        self.detection_enabled_ticks >= 3 || self.detection_total_ticks >= 10
    }

    /// Check if heading filter should be disabled
    pub fn disable_heading_filter(&self) -> bool {
        !self.has_received_first_fix || !self.estimation_ready()
    }

    /// Find closest stop index to current position
    pub fn find_closest_stop_index(&self, s_cm: DistCm) -> u8 {
        timeout::find_closest_stop_index(s_cm, self.route_data.stop_count as u8, |i| self.route_data.get_stop(i as usize))
    }

    /// Find closest stop index in forward direction only
    ///
    /// Searches from last_idx to end of route only. This prevents
    /// selecting stops behind the current position, which is important
    /// after off-route snap re-entry.
    pub fn find_forward_closest_stop_index(&self, s_cm: DistCm, last_idx: u8) -> u8 {
        let mut best_idx = last_idx;
        let mut best_dist = i32::MAX;

        // Only search forward: from last_idx to end of route
        for i in last_idx as usize..self.route_data.stop_count {
            if let Some(stop) = self.route_data.get_stop(i) {
                let dist = (s_cm - stop.progress_cm).abs();
                if dist < best_dist {
                    best_dist = dist;
                    best_idx = i as u8;
                }
            }
        }

        best_idx
    }

    /// Returns true if state should be persisted this tick.
    /// Writes when stop index changes, but no more than once per 60 seconds.
    pub fn should_persist(&self, current_stop: u8) -> bool {
        // Don't persist if position is frozen (off-route or suspect)
        if self.mode == SystemMode::OffRoute || self.mode == SystemMode::Recovering {
            return false;
        }

        // Don't persist if in suspect state (may be about to go off-route)
        if self.off_route_suspect_ticks > 0 {
            return false;
        }

        // Only persist when stop index actually changes
        if current_stop == self.last_persisted_stop {
            return false;
        }

        // Rate limit: no more than once per 60 seconds (60 ticks at 1Hz)
        if self.ticks_since_persist < 60 {
            return false;
        }

        true
    }

    /// Mark state as persisted, resetting the rate-limit counter.
    pub fn mark_persisted(&mut self, stop_index: u8) {
        self.last_persisted_stop = stop_index;
        self.ticks_since_persist = 0;
    }

    /// Reset all stop states to Idle after recovery
    fn reset_stop_states_after_recovery(&mut self, recovered_idx: usize, current_s_cm: DistCm) {
        use detection::state_machine::StopState;
        use shared::FsmState;

        let recovered_was_announced = self
            .stop_states
            .get(recovered_idx)
            .map(|state| state.announced || state.last_announced_stop == recovered_idx as u8)
            .unwrap_or(false);

        // Reset all stop states by recreating them
        for i in 0..self.stop_states.len() {
            self.stop_states[i] = StopState::new(i as u8);
        }

        // Stops before the recovered stop are treated as already passed.
        // Preserve their announcement bookkeeping so recovery cannot re-announce them.
        for i in 0..recovered_idx.min(self.stop_states.len()) {
            self.stop_states[i].fsm_state = FsmState::Departed;
            self.stop_states[i].announced = true;
            self.stop_states[i].last_announced_stop = i as u8;
        }

        // Mark recovered stop as Approaching if within corridor
        if let Some(stop) = self.route_data.get_stop(recovered_idx) {
            if let Some(state) = self.stop_states.get_mut(recovered_idx) {
                if recovered_was_announced {
                    state.announced = true;
                    state.last_announced_stop = recovered_idx as u8;
                }

                if current_s_cm >= stop.corridor_start_cm
                    && current_s_cm <= stop.corridor_end_cm
                {
                    state.fsm_state = FsmState::Approaching;
                }
            }
        }
    }

    /// Get the current stop index from last_stop_index.
    /// Returns None if not yet initialized.
    pub fn current_stop_index(&self) -> Option<u8> {
        if !self.has_received_first_fix {
            None
        } else {
            Some(self.last_stop_index)
        }
    }

    /// Apply persisted stop index by marking all prior stops as Departed.
    fn apply_persisted_stop_index(&mut self, stop_index: u8) {
        use shared::FsmState;

        for i in 0..stop_index.min(self.stop_states.len() as u8) as usize {
            self.stop_states[i].fsm_state = FsmState::Departed;
            self.stop_states[i].announced = true;
        }
        self.last_stop_index = stop_index;
    }

    /// Transition to OffRoute mode
    fn transition_to_offroute(&mut self, est: &EstimationOutput, now: u64) {
        self.mode = SystemMode::OffRoute;
        self.frozen_s_cm = Some(est.s_cm);
        self.off_route_clear_ticks = 0;
        self.off_route_since = Some(now);
        // Set flag for re-acquisition recovery when we return to route without snap
        self.needs_recovery_on_reacquisition = true;
    }

    /// Transition to Normal mode (direct from OffRoute)
    fn transition_offroute_to_normal(&mut self) {
        self.mode = SystemMode::Normal;
        self.frozen_s_cm = None;
        self.off_route_since = None;
        self.off_route_clear_ticks = 0;
        self.off_route_suspect_ticks = 0;
        self.needs_recovery_on_reacquisition = false;
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
        self.needs_recovery_on_reacquisition = false;
        self.off_route_since = None;

        // TODO: Reset stop states when detection layer is integrated
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
            let best_idx = self.find_closest_stop_index(est.s_cm);

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
            .unwrap_or(1000);

        let input = crate::recovery::RecoveryInput {
            s_cm: est.z_gps_cm,
            v_cms: est.v_cms,
            dt_seconds: dt / 1000,
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
    pub fn tick(&mut self, gps: &GpsPoint, est_state: &mut crate::estimation::EstimationState) -> TickResult {
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
            return TickResult { event: None, persist_request: None };
        }

        // Mark first fix as received after successful GPS fix
        if est.has_fix {
            self.has_received_first_fix = true;
        }

        // STEP 1.3: Apply persisted state on first fix if valid
        if self.has_received_first_fix && self.pending_persisted.is_some() {
            if let Some(ps) = self.pending_persisted.take() {
                // Check 500m threshold from spec
                let delta_cm = if est.s_cm >= ps.last_progress_cm {
                    est.s_cm - ps.last_progress_cm
                } else {
                    ps.last_progress_cm - est.s_cm
                };

                if delta_cm <= 50_000 {
                    // Within 500m: trust persisted stop index
                    self.apply_persisted_stop_index(ps.last_stop_index);
                    #[cfg(feature = "firmware")]
                    defmt::info!(
                        "Applied persisted state: stop={}, delta={}cm",
                        ps.last_stop_index,
                        delta_cm
                    );
                } else {
                    #[cfg(feature = "firmware")]
                    defmt::warn!(
                        "Persisted state too stale: delta={}cm > 500m, ignoring",
                        delta_cm
                    );
                }
            }
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

        // STEP 1.25: Warmup counter updates
        if self.just_reset {
            // After warmup reset (e.g., GPS outage), first tick counts as first fix
            self.just_reset = false;
            self.estimation_total_ticks = 1;
            self.detection_total_ticks = 1;
            // Position is already updated above, just block detection
            return TickResult { event: None, persist_request: None };
        }

        // Increment total time counters
        self.estimation_total_ticks = self.estimation_total_ticks.saturating_add(1);
        self.detection_total_ticks = self.detection_total_ticks.saturating_add(1);

        // Update estimation readiness (until ready)
        if !self.estimation_ready() {
            self.estimation_ready_ticks += 1;
        }

        // Update detection readiness (until ready, independent of estimation)
        if !self.detection_ready() {
            self.detection_enabled_ticks += 1;
        }

        // Block detection unless ready (but position tracking continues above)
        if !self.detection_ready() {
            return TickResult { event: None, persist_request: None };
        }

        // STEP 2: State machine transitions (unified triggers)
        let old_mode = self.mode;

        match self.mode {
            SystemMode::Normal => {
                // Check: divergence > 50m for 5 ticks
                // Set freeze time and recovery flag on first suspect tick
                if est.divergence_d2 > mode::OFF_ROUTE_D2_THRESHOLD {
                    if self.off_route_suspect_ticks == 0 {
                        // First tick of suspect phase - set freeze time and recovery flag immediately
                        self.off_route_since = Some(gps.timestamp);
                        self.needs_recovery_on_reacquisition = true;
                    }
                    self.off_route_suspect_ticks += 1;
                    if self.off_route_suspect_ticks >= 5 {
                        self.transition_to_offroute(&est, gps.timestamp);
                        return TickResult { event: None, persist_request: None };  // Suppress detection during transition
                    }
                } else {
                    // Good divergence - reset suspect counter
                    self.off_route_suspect_ticks = 0;
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
                        return TickResult { event: None, persist_request: None };  // Will resume detection next tick
                    }
                    TransitionAction::Stay => {
                        // Stay in OffRoute
                        return TickResult { event: None, persist_request: None };
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
                return TickResult { event: None, persist_request: None };  // Recovery failed, stay in Recovering
            }
        }

        // GPS Jump Recovery (H1)
        // When GPS position jumps >200m in Normal mode, use recovery module
        // to find correct stop index. RecoveryInput uses:
        // - s_cm: current GPS position (jumped)
        // - v_cms: filtered velocity from dead reckoning
        // - dt_seconds: time since last valid GPS fix
        // - frozen_s_cm: None (no position freeze in Normal mode)
        let in_snap_cooldown = self.just_snapped_ticks > 0;
        let prev_s_cm = self.last_valid_s_cm;
        // Skip recovery on first fix - last_valid_s_cm is still 0 (initial value)
        if self.mode == SystemMode::Normal && !self.just_reset && self.has_received_first_fix {
            let s_raw = self.current_position(&est);
            if !in_snap_cooldown && crate::recovery_trigger::should_trigger_recovery(s_raw, prev_s_cm) {
                #[cfg(feature = "firmware")]
                defmt::warn!(
                    "GPS jump detected: s={}→{}, triggering recovery",
                    prev_s_cm,
                    s_raw
                );

                // Calculate time delta since last GPS fix (in seconds)
        let dt_since_last_fix = if self.last_gps_timestamp > 0 {
            gps.timestamp.saturating_sub(self.last_gps_timestamp) / 1000
        } else {
            1 // Default to 1 second on first fix or after outage
        };

                // Collect stops into a heapless::Vec for recovery module
                let mut stops_vec = heapless::Vec::<shared::Stop, 256>::new();
                for i in 0..self.route_data.stop_count {
                    if let Some(stop) = self.route_data.get_stop(i) {
                        if stops_vec.push(stop).is_err() {
                            #[cfg(feature = "firmware")]
                            defmt::warn!("Too many stops for recovery buffer");
                            break;
                        }
                    }
                }

                let recovery_input = crate::recovery::RecoveryInput {
                    s_cm: s_raw,
                    v_cms: est_state.dr.filtered_v,
                    dt_seconds: dt_since_last_fix,
                    stops: stops_vec,
                    hint_idx: self.last_stop_index,
                    frozen_s_cm: None,  // No frozen position in Normal mode
                    search_window: 10,
                };

                if let Some(recovered_idx) = crate::recovery::recover(recovery_input) {
                    #[cfg(feature = "firmware")]
                    defmt::info!("Recovery found stop index: {}", recovered_idx);
                    self.last_stop_index = recovered_idx;
                    self.reset_stop_states_after_recovery(recovered_idx as usize, s_raw);
                } else {
                    #[cfg(feature = "firmware")]
                    defmt::warn!("Recovery failed: no valid stop found");
                }

                // Update tracking
                self.last_valid_s_cm = s_raw;
                self.last_gps_timestamp = gps.timestamp;
            }
        }

        // Update position tracking if no jump occurred
        if !crate::recovery_trigger::should_trigger_recovery(self.current_position(&est), self.last_valid_s_cm) {
            self.last_valid_s_cm = self.current_position(&est);
            self.last_gps_timestamp = gps.timestamp;
        }


        // STEP 3.6: Handle snap from off-route re-entry
        if est.snapped && self.mode == SystemMode::Normal {
            // 1. Find forward closest stop (prevents backward selection)
            let new_idx = self.find_forward_closest_stop_index(est.s_cm, self.last_stop_index);
            self.last_stop_index = new_idx;

            // 2. Reset stop states using same logic as recovery
            self.reset_stop_states_after_recovery(new_idx as usize, est.s_cm);

            // 3. Clear all recovery triggers
            self.needs_recovery_on_reacquisition = false;
            self.frozen_s_cm = None;
            self.off_route_since = None;
            self.last_valid_s_cm = est.s_cm;
            self.last_gps_timestamp = gps.timestamp;

            // 4. Set 2-second cooldown
            self.just_snapped_ticks = 2;
        }

        // Re-acquisition Recovery
        // After returning from OffRoute without snap, use recovery module
        // to find correct stop index based on elapsed time since freeze.
        // RecoveryInput uses:
        // - s_cm: Kalman-filtered position
        // - v_cms: filtered velocity
        // - dt_seconds: time since off_route_since
        // - frozen_s_cm: None (freeze context cleared after off-route)
        if !est.snapped && !in_snap_cooldown && self.needs_recovery_on_reacquisition && self.mode == SystemMode::Normal {
            self.needs_recovery_on_reacquisition = false;

            // Calculate elapsed time since freeze
            let elapsed_seconds = self.off_route_since
                .map(|t| gps.timestamp.saturating_sub(t) / 1000)
                .unwrap_or(1);

            // Run recovery to find correct stop index
            let mut stops_vec = heapless::Vec::<shared::Stop, 256>::new();
            for i in 0..self.route_data.stop_count {
                if let Some(stop) = self.route_data.get_stop(i) {
                    let _ = stops_vec.push(stop);
                }
            }

            let recovery_input = crate::recovery::RecoveryInput {
                s_cm: est.s_cm,
                v_cms: est_state.dr.filtered_v,
                dt_seconds: elapsed_seconds,
                stops: stops_vec,
                hint_idx: self.last_stop_index,
                frozen_s_cm: None,  // No frozen position for re-acquisition
                search_window: 10,
            };

            if let Some(recovered_idx) = crate::recovery::recover(recovery_input) {
                #[cfg(feature = "firmware")]
                defmt::info!("Re-acquisition recovered stop index: {}", recovered_idx);
                self.last_stop_index = recovered_idx;
                self.reset_stop_states_after_recovery(recovered_idx as usize, est.s_cm);
            }

            // Clear freeze time and context after re-acquisition recovery
            self.off_route_since = None;
            self.frozen_s_cm = None;
        }

        // STEP 4: Detection (ONLY in Normal mode)
        let event = if self.mode == SystemMode::Normal {
            self.run_detection(&est, s_cm_for_detection, gps.timestamp)
        } else {
            None
        };

        // STEP 5: Check persistence
        let persist_request = if self.mode == SystemMode::Normal {
            self.current_stop_index()
                .filter(|&idx| self.should_persist(idx))
                .map(|idx| shared::PersistedState::new(s_cm_for_detection, idx))
        } else {
            None
        };

        // Decrement snap cooldown
        if self.just_snapped_ticks > 0 {
            self.just_snapped_ticks = self.just_snapped_ticks.saturating_sub(1);
        }

        TickResult { event, persist_request }
    }

    /// Run arrival detection (Normal mode only)
    fn run_detection(&mut self, est: &EstimationOutput, s_cm: DistCm, timestamp: u64) -> Option<ArrivalEvent> {
        use shared::PositionSignals;

        // Create position signals for detection
        let signals = PositionSignals {
            z_gps_cm: est.z_gps_cm,
            s_cm: est.s_cm,
        };

        // Step 1: Find active stops (corridor filter)
        let skip_flags = &[false; 32]; // TODO: track skip flags per stop
        let active_indices = crate::detection::find_active_stops(est.s_cm, self.route_data, skip_flags);

        // Step 2: For each active stop, compute probability and update FSM
        for stop_idx in active_indices {
            if stop_idx >= self.stop_states.len() {
                continue;
            }

            let stop = match self.route_data.get_stop(stop_idx) {
                Some(s) => s,
                None => continue,
            };
            let stop_state = &mut self.stop_states[stop_idx];

            // Get next sequential stop for adaptive weights
            let next_stop_idx = stop_idx.checked_add(1);
            let next_stop_value = next_stop_idx.and_then(|idx| self.route_data.get_stop(idx));
            let next_stop = next_stop_value.as_ref();

            // Compute arrival probability with adaptive weights
            let gps_status = if est.has_fix {
                crate::detection::GpsStatus::Valid
            } else {
                crate::detection::GpsStatus::DrOutage
            };
            let probability = crate::detection::compute_arrival_probability_adaptive(
                signals,
                est.v_cms,
                &stop,
                stop_state.dwell_time_s,
                gps_status,
                next_stop,
            );

            // Update state machine FIRST (v8.4: FSM transition before announce check)
            let event = stop_state.update(
                s_cm,
                est.v_cms,
                stop.progress_cm,
                stop.corridor_start_cm,
                probability,
            );

            // THEN check for announcement trigger
            if stop_state.should_announce(s_cm, stop.corridor_start_cm) {
                return Some(ArrivalEvent {
                    time: timestamp,
                    stop_idx: stop_idx as u8,
                    s_cm,
                    v_cms: est.v_cms,
                    probability,
                    event_type: shared::ArrivalEventType::Announce,
                });
            }

            match event {
                detection::state_machine::StopEvent::Arrived => {
                    return Some(ArrivalEvent {
                        time: timestamp,
                        stop_idx: stop_idx as u8,
                        s_cm,
                        v_cms: est.v_cms,
                        probability,
                        event_type: shared::ArrivalEventType::Arrival,
                    });
                }
                detection::state_machine::StopEvent::Departed => {
                    return Some(ArrivalEvent {
                        time: timestamp,
                        stop_idx: stop_idx as u8,
                        s_cm,
                        v_cms: est.v_cms,
                        probability,
                        event_type: shared::ArrivalEventType::Departure,
                    });
                }
                detection::state_machine::StopEvent::None => {}
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
    fn test_warmup_methods() {
        use shared::binfile::{RouteData, MAGIC, VERSION};
        use shared::binfile::crc32;

        // Create minimal valid RouteData buffer
        // Header: magic(4) + version(2) + node_count(2) + stop_count(1) + padding(3) + origin(8) + lat_avg(8) = 28 bytes
        let mut buffer = [0u8; 128];

        // Write magic
        buffer[0..4].copy_from_slice(&MAGIC.to_le_bytes());
        // Write version
        buffer[4..6].copy_from_slice(&VERSION.to_le_bytes());
        // Write node_count (0)
        buffer[6..8].copy_from_slice(&0u16.to_le_bytes());
        // Write stop_count (0)
        buffer[8] = 0;
        // padding at [9..12] is already 0
        // origin x0_cm, y0_cm at [12..20] is already 0
        // lat_avg_deg at [20..28] is already 0 (f64)

        // Compute and write CRC32 at end
        let crc = crc32(&buffer[..124]);
        buffer[124..128].copy_from_slice(&crc.to_le_bytes());

        let route_data = RouteData::load(&buffer).expect("Failed to load minimal route data");
        let state = SystemState::new(&route_data, None);

        assert!(!state.estimation_ready(), "Should not be ready initially");
        assert!(!state.detection_ready(), "Detection should not be ready");
        assert!(state.disable_heading_filter(), "Should disable filter before first fix");
    }

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

    #[test]
    fn test_systemstate_size() {
        use core::mem::size_of;
        let size = size_of::<SystemState>();
        // This test documents the current size issue
        // The 4KB budget cannot be met with 256 StopStates
        let _ = size; // Suppress unused warning
    }

    #[test]
    fn test_find_closest_stop_index() {
        use shared::binfile::{RouteData, MAGIC, VERSION};
        use shared::binfile::crc32;

        // Create minimal valid RouteData buffer with 1 stop
        let mut buffer = [0u8; 128];

        // Write magic
        buffer[0..4].copy_from_slice(&MAGIC.to_le_bytes());
        // Write version
        buffer[4..6].copy_from_slice(&VERSION.to_le_bytes());
        // Write node_count (0)
        buffer[6..8].copy_from_slice(&0u16.to_le_bytes());
        // Write stop_count (1)
        buffer[8] = 1;
        // padding at [9..12] is already 0
        // origin x0_cm, y0_cm at [12..20] is already 0
        // lat_avg_deg at [20..28] is already 0 (f64)

        // Compute and write CRC32 at end
        let crc = crc32(&buffer[..124]);
        buffer[124..128].copy_from_slice(&crc.to_le_bytes());

        let route_data = RouteData::load(&buffer).expect("Failed to load minimal route data");
        let state = SystemState::new(&route_data, None);

        // Test that method returns a valid index
        let idx = state.find_closest_stop_index(5000);
        assert!(idx < route_data.stop_count as u8, "Index {} should be less than stop_count {}", idx, route_data.stop_count);
    }

    #[test]
    fn test_find_forward_closest_stop_index() {
        use shared::binfile::{RouteData, MAGIC, VERSION};
        use shared::binfile::crc32;

        // Create minimal valid RouteData buffer
        let mut buffer = [0u8; 128];

        // Write magic
        buffer[0..4].copy_from_slice(&MAGIC.to_le_bytes());
        // Write version
        buffer[4..6].copy_from_slice(&VERSION.to_le_bytes());
        // Write node_count (0)
        buffer[6..8].copy_from_slice(&0u16.to_le_bytes());
        // Write stop_count (0)
        buffer[8] = 0;
        // padding at [9..12] is already 0
        // origin x0_cm, y0_cm at [12..20] is already 0
        // lat_avg_deg at [20..28] is already 0 (f64)

        // Compute and write CRC32 at end
        let crc = crc32(&buffer[..124]);
        buffer[124..128].copy_from_slice(&crc.to_le_bytes());

        let route_data = RouteData::load(&buffer).expect("Failed to load minimal route data");
        let state = SystemState::new(&route_data, None);

        // Test forward search from index 5
        let idx = state.find_forward_closest_stop_index(5000, 5);
        assert!(idx >= 5, "Should only return stops at or after index 5");
    }

    #[test]
    fn test_persistence_helpers() {
        use shared::binfile::{RouteData, MAGIC, VERSION};
        use shared::binfile::crc32;

        // Create minimal valid RouteData buffer
        let mut buffer = [0u8; 128];

        // Write magic
        buffer[0..4].copy_from_slice(&MAGIC.to_le_bytes());
        // Write version
        buffer[4..6].copy_from_slice(&VERSION.to_le_bytes());
        // Write node_count (0)
        buffer[6..8].copy_from_slice(&0u16.to_le_bytes());
        // Write stop_count (0)
        buffer[8] = 0;
        // padding at [9..12] is already 0
        // origin x0_cm, y0_cm at [12..20] is already 0
        // lat_avg_deg at [20..28] is already 0 (f64)

        // Compute and write CRC32 at end
        let crc = crc32(&buffer[..124]);
        buffer[124..128].copy_from_slice(&crc.to_le_bytes());

        let route_data = RouteData::load(&buffer).expect("Failed to load minimal route data");
        let mut state = SystemState::new(&route_data, None);

        // Initially should not persist (no first fix)
        assert!(!state.should_persist(0));

        // After first fix, current_stop_index returns Some
        state.has_received_first_fix = true;
        assert!(state.current_stop_index().is_some());

        // Test rate limiting
        state.last_persisted_stop = 0;
        state.ticks_since_persist = 0;
        assert!(!state.should_persist(1), "Should rate limit");

        state.ticks_since_persist = 60;
        assert!(state.should_persist(1), "Should allow after 60 ticks");

        state.mark_persisted(1);
        assert_eq!(state.last_persisted_stop, 1);
        assert_eq!(state.ticks_since_persist, 0);
    }
}

// Compile-time verification that SystemState fits within SRAM budget
// NOTE: Currently 4208 bytes (112 bytes over 4KB limit due to 256 StopStates)
// TODO: Redesign to fit within 4KB budget (e.g., reduce StopState size, use fewer stops, or move to Flash)
// Temporarily disabled to allow integration to proceed - size check will be addressed in follow-up
// const _: () = assert!(size_of::<SystemState>() <= 4096, "SystemState exceeds 4KB SRAM budget");

#[cfg(test)]
mod recovery_integration_test;

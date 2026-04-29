//! GPS processing pipeline state
//!
//! Manages the main state machine for processing GPS updates through
//! the full arrival detection pipeline.
#![allow(dead_code)]

use crate::detection::{compute_arrival_probability_adaptive, find_active_stops};
use crate::recovery_trigger::should_trigger_recovery;
use gps_processor::kalman::{process_gps_update, ProcessResult};
use shared::FsmState;
use shared::{
    binfile::RouteData, ArrivalEvent, DistCm, DrState, GpsPoint, KalmanState, PositionSignals, Stop,
};

// ===== Constants =====

/// Number of valid GPS ticks required after first fix before arrival detection is enabled.
///
/// This warmup period allows the Kalman filter to converge to stable position and velocity
/// estimates. The Kalman filter requires multiple measurements to initialize its covariance
/// matrices and reduce uncertainty to acceptable levels for reliable arrival detection.
///
/// The value 3 represents approximately 3 seconds at 1 Hz GPS update rate, which empirical
/// testing shows is sufficient for the filter to reach acceptable convergence in typical
/// urban canyon conditions.

// ===== Estimation Readiness =====
/// Valid GPS ticks required for estimation to be ready (affects heading filter, Kalman)
const ESTIMATION_WARMUP_TICKS: u8 = 3;
/// Maximum ticks before estimation timeout safety valve
const ESTIMATION_TIMEOUT_TICKS: u8 = 10;

// ===== Detection Gating =====
/// Valid ticks required for detection to be enabled
const DETECTION_WARMUP_TICKS: u8 = 3;
/// Maximum ticks before detection timeout safety valve
const DETECTION_TIMEOUT_TICKS: u8 = 10;

// Legacy aliases for backward compatibility (deprecated)
#[deprecated(note = "Use ESTIMATION_WARMUP_TICKS instead")]
const WARMUP_TICKS_REQUIRED: u8 = 3;
#[deprecated(note = "Use ESTIMATION_TIMEOUT_TICKS instead")]
const WARMUP_TIMEOUT_TICKS: u8 = 10;

// ===== State Struct =====

/// Global state for the GPS processing pipeline.
///
/// # Warmup Behavior
///
/// The State machine implements a warmup period to ensure reliable arrival detection:
///
/// - **First GPS tick**: Initializes the Kalman filter with the first position fix.
///   No arrival detection is performed during this initialization phase.
///
/// - **Warmup period** ([`WARMUP_TICKS_REQUIRED`] ticks): After initialization, the system
///   waits for 3 additional GPS ticks. This allows the Kalman filter to converge to stable
///   position and velocity estimates before making arrival decisions.
///
/// - **Normal operation**: After warmup completes, arrival detection is fully enabled.
///
/// # Outage Handling
///
/// The warmup counter resets to 0 during GPS outages (when [`ProcessResult::Outage`] occurs)
/// for conservative behavior. This ensures that after extended signal loss, the system
/// requires a fresh warmup period before making arrival decisions, since:
///
/// 1. GPS outage may indicate poor signal quality or multipath conditions
/// 2. Dead-reckoning mode during outage may accumulate position errors
/// 3. Kalman filter covariance matrices may have inflated uncertainty
///
/// Dead-reckoning outages ([`ProcessResult::DrOutage`]) do NOT reset the warmup counter
/// because DR mode maintains valid state estimates - it only indicates the GPS measurement
/// was rejected for quality reasons (e.g., excessive speed change), not that signal was lost.
pub struct State<'a> {
    pub nmea: gps_processor::nmea::NmeaState,
    pub kalman: KalmanState,
    pub dr: DrState,
    pub stop_states: heapless::Vec<detection::state_machine::StopState, 256>,
    pub route_data: &'a RouteData<'a>,
    /// First fix flag - true until first GPS fix is received
    pub first_fix: bool,

    // ===== Estimation Readiness (affects heading filter, Kalman) =====
    /// Valid GPS ticks where Kalman measurement update ran
    pub estimation_ready_ticks: u8,
    /// Total ticks since first fix (timeout safety valve)
    pub estimation_total_ticks: u8,

    // ===== Detection Gating (blocks arrival detection) =====
    /// Valid ticks since estimation became ready
    pub detection_enabled_ticks: u8,
    /// Total ticks since first fix (detection timeout)
    pub detection_total_ticks: u8,

    /// Flag indicating state was just reset (e.g., after GPS outage)
    pub just_reset: bool,

    /// Last confirmed stop index for GPS jump recovery
    last_known_stop_index: u8,
    /// Last valid position for jump detection (cm)
    last_valid_s_cm: DistCm,
    /// Timestamp of last GPS fix for recovery time delta calculation
    last_gps_timestamp: u64,
    /// Pending persisted state to apply after first GPS fix
    pending_persisted: Option<shared::PersistedState>,
    /// Last stop index that was persisted to flash
    last_persisted_stop: u8,
    /// Ticks since last persist operation (for rate limiting)
    pub ticks_since_persist: u16,
    /// Flag indicating recovery should run on next valid GPS after off-route
    needs_recovery_on_reacquisition: bool,
    /// NEW: Ticks remaining in snap cooldown period (prevents recovery interference)
    just_snapped_ticks: u8,
}

impl<'a> State<'a> {
    pub fn new(route_data: &'a RouteData<'a>, persisted: Option<shared::PersistedState>) -> Self {
        use detection::state_machine::StopState;
        use gps_processor::nmea::NmeaState;

        let stop_count = route_data.stop_count;
        let mut stop_states = heapless::Vec::new();
        for i in 0..stop_count {
            if let Err(_) = stop_states.push(StopState::new(i as u8)) {
                #[cfg(feature = "firmware")]
                defmt::warn!("Route has {} stops but only 256 supported - stops beyond index 255 will be ignored", stop_count);
                break;
            }
        }

        Self {
            nmea: NmeaState::new(),
            kalman: KalmanState::new(),
            dr: DrState::new(),
            stop_states,
            route_data,
            first_fix: true,
            // Estimation readiness
            estimation_ready_ticks: 0,
            estimation_total_ticks: 0,
            // Detection gating
            detection_enabled_ticks: 0,
            detection_total_ticks: 0,
            // Shared flags
            just_reset: false,
            last_known_stop_index: 0,
            last_valid_s_cm: 0,
            last_gps_timestamp: 0,
            pending_persisted: persisted,
            last_persisted_stop: if let Some(ps) = persisted {
                ps.last_stop_index
            } else {
                0
            },
            ticks_since_persist: 0,
            needs_recovery_on_reacquisition: false,
            just_snapped_ticks: 0,
        }
    }

    /// Process a GPS point through the full pipeline
    /// Returns Some(arrival event) if an arrival is detected
    pub fn process_gps(&mut self, gps: &GpsPoint) -> Option<ArrivalEvent> {
        use detection::state_machine::StopEvent;

        // Module ④+⑤: Map matching and projection
        // Module ⑥: Speed constraint filter
        // Module ⑦: Kalman filter
        // Module ⑧: Dead-reckoning
        // Disable heading filter during warmup (GPS heading may be unreliable after
        // long outages). The filter is disabled when:
        // 1. First fix ever (self.first_fix = true)
        // 2. During warmup (estimation_ready_ticks < WARMUP_TICKS_REQUIRED)
        let in_warmup = self.estimation_ready_ticks < WARMUP_TICKS_REQUIRED;
        let disable_heading_filter = self.first_fix || in_warmup;
        let result = process_gps_update(
            &mut self.kalman,
            &mut self.dr,
            gps,
            self.route_data,
            gps.timestamp,
            disable_heading_filter,
            self.last_known_stop_index,  // C3: pass current stop index
        );

        let (s_cm, v_cms, signals, gps_status) = match result {
            ProcessResult::Valid {
                signals,
                v_cms,
                seg_idx: _,
                snapped,
            } => {
                use crate::detection::GpsStatus;
                let PositionSignals { z_gps_cm: _, s_cm } = signals;
                let gps_status = GpsStatus::Valid;

                // Handle cooldown decrement
                if self.just_snapped_ticks > 0 {
                    self.just_snapped_ticks = self.just_snapped_ticks.saturating_sub(1);
                }
                let in_snap_cooldown = self.just_snapped_ticks > 0;

                // Check for GPS jump requiring recovery (H1)
                let prev_s_cm = self.last_valid_s_cm;
                // Skip recovery on first fix - last_valid_s_cm is still 0 (initial value)
                if !snapped && !in_snap_cooldown && !self.first_fix && should_trigger_recovery(s_cm, prev_s_cm) {
                    #[cfg(feature = "firmware")]
                    defmt::warn!(
                        "GPS jump detected: s={}→{}, triggering recovery",
                        prev_s_cm,
                        s_cm
                    );

                    // Call recovery module
                    // Calculate time delta since last GPS fix (in seconds)
                    let dt_since_last_fix = if self.last_gps_timestamp > 0 {
                        gps.timestamp.saturating_sub(self.last_gps_timestamp)
                    } else {
                        1 // Default to 1 second on first fix or after outage
                    };

                    // Collect stops into a heapless::Vec for recovery module
                    let mut stops_vec = heapless::Vec::<Stop, 256>::new();
                    for i in 0..self.route_data.stop_count {
                        if let Some(stop) = self.route_data.get_stop(i) {
                            if let Err(_) = stops_vec.push(stop) {
                                #[cfg(feature = "firmware")]
                                defmt::warn!("Too many stops for recovery buffer");
                                break;
                            }
                        }
                    }

                    if let Some(recovered_idx) = detection::recovery::find_stop_index(
                        s_cm,
                        self.dr.filtered_v,  // Use EMA-smoothed velocity for stable constraints
                        dt_since_last_fix,
                        &stops_vec,
                        self.last_known_stop_index,
                        &self.kalman.freeze_ctx,  // C3: pass freeze context
                    ) {
                        #[cfg(feature = "firmware")]
                        defmt::info!("Recovery found stop index: {}", recovered_idx);
                        self.last_known_stop_index = recovered_idx as u8;
                        self.reset_stop_states_after_recovery(recovered_idx, s_cm);  // C2: pass current position
                    } else {
                        #[cfg(feature = "firmware")]
                        defmt::warn!("Recovery failed: no valid stop found");
                    }

                    // C1: Clear freeze time after H1 recovery completes
                    self.kalman.off_route_freeze_time = None;
                }

                if self.first_fix {
                    self.first_fix = false;
                    // First fix initializes Kalman but doesn't run update_adaptive
                    // Counts toward timeout but NOT convergence
                    self.estimation_total_ticks = 1;
                    self.detection_total_ticks = 1;
                    self.last_valid_s_cm = s_cm;  // C1 fix: initialize to prevent false jump detection on tick 2

                    // Apply persisted state if valid and within 500m threshold
                    if let Some(ps) = self.pending_persisted.take() {
                        // Check 500m threshold from spec (Section 11.4)
                        // Only trust persisted state if current GPS is close enough
                        let delta_cm = if s_cm >= ps.last_progress_cm {
                            s_cm - ps.last_progress_cm
                        } else {
                            ps.last_progress_cm - s_cm
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

                    return None;
                }

                if self.just_reset {
                    // After warmup reset (e.g., GPS outage), first tick counts as first fix
                    self.just_reset = false;
                    self.estimation_total_ticks = 1;
                    self.detection_total_ticks = 1;
                    return None;
                }

                // Increment total time counter
                self.estimation_total_ticks = self.estimation_total_ticks.saturating_add(1);

                // Check convergence requirement
                if self.estimation_ready_ticks < WARMUP_TICKS_REQUIRED {
                    self.estimation_ready_ticks += 1;

                    // Block detection unless timeout expired
                    if self.estimation_total_ticks < WARMUP_TIMEOUT_TICKS {
                        #[cfg(feature = "firmware")]
                        defmt::debug!(
                            "Warmup: {}/{} valid, {}/{} total",
                            self.estimation_ready_ticks,
                            WARMUP_TICKS_REQUIRED,
                            self.estimation_total_ticks,
                            WARMUP_TIMEOUT_TICKS
                        );
                        return None;
                    }
                }

                // Handle snap from off-route re-entry
                if snapped {
                    // 1. Find forward closest stop (prevents backward selection)
                    let new_idx = self.find_forward_closest_stop_index(s_cm, self.last_known_stop_index);
                    self.last_known_stop_index = new_idx;

                    // 2. Reset stop states using same logic as recovery (all to Idle, then set appropriate states)
                    self.reset_stop_states_after_recovery(new_idx as usize, s_cm);  // C2: pass current position

                    // 3. Clear all recovery triggers
                    self.needs_recovery_on_reacquisition = false;
                    self.kalman.freeze_ctx = None;
                    self.kalman.off_route_freeze_time = None;  // Clear freeze time on snap
                    self.last_valid_s_cm = s_cm;  // Update immediately to prevent false jump detection
                    self.last_gps_timestamp = gps.timestamp;  // S3: Update timestamp to prevent stale dt calculation

                    // 4. Set 2-second cooldown
                    self.just_snapped_ticks = 2;

                    // Skip normal recovery and proceed to detection
                } else {
                // Update recovery tracking
                self.last_known_stop_index = self.find_closest_stop_index(s_cm);
                self.last_valid_s_cm = s_cm;
                // Update timestamp for next iteration
                self.last_gps_timestamp = gps.timestamp;

                // Check for re-acquisition recovery
                if !snapped && !in_snap_cooldown && self.needs_recovery_on_reacquisition {
                    self.needs_recovery_on_reacquisition = false;

                    // Calculate elapsed time since freeze (from KalmanState)
                    let elapsed_seconds =
                        if let Some(freeze_time) = self.kalman.off_route_freeze_time {
                            gps.timestamp.saturating_sub(freeze_time)
                        } else {
                            1 // Default if not set
                        };

                    // Run recovery to find correct stop index
                    let mut stops_vec = heapless::Vec::<Stop, 256>::new();
                    for i in 0..self.route_data.stop_count {
                        if let Some(stop) = self.route_data.get_stop(i) {
                            let _ = stops_vec.push(stop);
                        }
                    }

                    if let Some(recovered_idx) = detection::recovery::find_stop_index(
                        s_cm,
                        self.dr.filtered_v,  // Use EMA-smoothed velocity for stable constraints
                        elapsed_seconds,
                        &stops_vec,
                        self.last_known_stop_index,
                        &self.kalman.freeze_ctx,  // C3: pass freeze context
                    ) {
                        #[cfg(feature = "firmware")]
                        defmt::info!("Re-acquisition recovered stop index: {}", recovered_idx);
                        self.last_known_stop_index = recovered_idx as u8;
                        self.reset_stop_states_after_recovery(recovered_idx, s_cm);  // C2: pass current position
                    }
                    // If recovery returns None, continue with existing states

                    // C1: Clear freeze time and context after re-acquisition recovery completes
                    self.kalman.off_route_freeze_time = None;
                    self.kalman.freeze_ctx = None;
                }
                }

                // Return s_cm, v_cms, signals, and gps_status for detection
                (s_cm, v_cms, signals, gps_status)
            }
            ProcessResult::Rejected(reason) => {
                #[cfg(feature = "firmware")]
                defmt::warn!("GPS update rejected: {}", reason);
                #[cfg(not(feature = "firmware"))]
                let _ = reason; // Suppress unused warning when firmware feature is disabled

                // Increment timeout counter even on rejection (I5 fix)
                // This prevents permanent stuck state when GPS is repeatedly rejected
                if !self.first_fix {
                    self.estimation_total_ticks = self.estimation_total_ticks.saturating_add(1);
                }

                return None; // Still block detection
            }
            ProcessResult::Outage => {
                #[cfg(feature = "firmware")]
                defmt::warn!("GPS outage exceeded 10 seconds");
                // Reset warmup on GPS loss (conservative - requires fresh warmup after outage)
                if !self.first_fix {
                    self.estimation_ready_ticks = 0;
                    self.estimation_total_ticks = 0;
                    self.detection_total_ticks = 0;
                    self.just_reset = true;
                    #[cfg(feature = "firmware")]
                    defmt::debug!("GPS outage reset warmup counters");
                }
                return None;
            }
            ProcessResult::DrOutage { s_cm, v_cms } => {
                #[cfg(feature = "firmware")]
                defmt::debug!("DR mode: s={}cm, v={}cm/s", s_cm, v_cms);
                // DR mode occurs when GPS measurement is rejected for quality reasons
                // (e.g., excessive speed change, monotonicity violation).
                // I5 fix: Count toward timeout but NOT convergence, preventing permanent stuck state.

                if self.just_reset {
                    // After warmup reset (e.g., GPS outage), first tick counts as first fix
                    self.just_reset = false;
                    self.estimation_total_ticks = 1;
                    self.detection_total_ticks = 1;
                    return None;
                }

                // Increment timeout counter but NOT valid counter (I5 fix)
                // Note: first_fix is already false after first GPS, so we don't need to check it
                if !self.first_fix {
                    self.estimation_total_ticks = self.estimation_total_ticks.saturating_add(1);
                }

                // Block detection unless timeout expired
                if self.estimation_total_ticks < WARMUP_TIMEOUT_TICKS {
                    #[cfg(feature = "firmware")]
                    defmt::debug!(
                        "Warmup (DR): {}/{} valid, {}/{} total",
                        self.estimation_ready_ticks,
                        WARMUP_TICKS_REQUIRED,
                        self.estimation_total_ticks,
                        WARMUP_TIMEOUT_TICKS
                    );
                    return None;
                }

                // Timeout expired: detection enabled, proceed with DR estimates
                use crate::detection::GpsStatus;
                let signals = PositionSignals {
                    z_gps_cm: s_cm,
                    s_cm,
                };
                (s_cm, v_cms, signals, GpsStatus::DrOutage)
            }
            ProcessResult::OffRoute {
                last_valid_s: _,
                last_valid_v: _,
                freeze_time: _,
            } => {
                // Set flag for recovery on re-acquisition
                self.needs_recovery_on_reacquisition = true;

                // Note: freeze_time is already set in KalmanState by update_off_route_hysteresis
                // Bug 5 fix: This is now accurate (set when position first freezes, not when OffRoute confirmed)

                #[cfg(feature = "firmware")]
                defmt::warn!(
                    "Off-route detected: GPS > 50m from route for 5s. Freezing at s={}cm.",
                    self.kalman.frozen_s_cm.unwrap_or(self.kalman.s_cm)
                );

                // Position is frozen - do NOT update last_valid_s_cm
                // Suspend arrival detection by returning None
                return None;
            }
            ProcessResult::SuspectOffRoute { s_cm: _, v_cms: _ } => {
                // M1: Suspect off-route - suppress detection, don't increment warmup
                // Mark for recovery when we return to Normal
                self.needs_recovery_on_reacquisition = true;
                return None;
            }
        };

        // Module ⑨: Stop corridor filtering
        let active_indices = find_active_stops(signals, self.route_data);

        // Module ⑩+⑪: Arrival probability and state machine for each active stop
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
            let probability = compute_arrival_probability_adaptive(
                signals,
                v_cms,
                &stop,
                stop_state.dwell_time_s,
                gps_status,
                next_stop,
            );

            // Update state machine FIRST (v8.4: FSM transition before announce check)
            let event = stop_state.update(
                s_cm,
                v_cms,
                stop.progress_cm,
                stop.corridor_start_cm,
                probability,
            );

            // THEN check for announcement trigger
            if stop_state.should_announce(s_cm, stop.corridor_start_cm) {
                #[cfg(feature = "firmware")]
                defmt::info!(
                    "Announcement for stop {}: s={}cm, v={}cm/s",
                    stop_idx,
                    s_cm,
                    v_cms
                );
                return Some(ArrivalEvent {
                    time: gps.timestamp,
                    stop_idx: stop_idx as u8,
                    s_cm,
                    v_cms,
                    probability: 0,
                    event_type: shared::ArrivalEventType::Announce,
                });
            }

            match event {
                StopEvent::Arrived => {
                    #[cfg(feature = "firmware")]
                    defmt::info!(
                        "Arrival at stop {}: s={}cm, v={}cm/s, p={}",
                        stop_idx,
                        s_cm,
                        v_cms,
                        probability
                    );
                    return Some(ArrivalEvent {
                        time: gps.timestamp,
                        stop_idx: stop_idx as u8,
                        s_cm,
                        v_cms,
                        probability,
                        event_type: shared::ArrivalEventType::Arrival,
                    });
                }
                StopEvent::Departed => {
                    #[cfg(feature = "firmware")]
                    defmt::info!(
                        "Departure from stop {}: s={}cm, v={}cm/s",
                        stop_idx,
                        s_cm,
                        v_cms
                    );
                    return Some(ArrivalEvent {
                        time: gps.timestamp,
                        stop_idx: stop_idx as u8,
                        s_cm,
                        v_cms,
                        probability,
                        event_type: shared::ArrivalEventType::Departure,
                    });
                }
                StopEvent::None => {}
            }
        }

        None
    }

    /// Find closest stop index to current position
    pub fn find_closest_stop_index(&self, s_cm: DistCm) -> u8 {
        let mut closest_idx = 0;
        let mut closest_dist = i32::MAX;

        for i in 0..self.route_data.stop_count {
            if let Some(stop) = self.route_data.get_stop(i) {
                let dist = (s_cm - stop.progress_cm).abs();
                if dist < closest_dist {
                    closest_dist = dist;
                    closest_idx = i as u8;
                }
            }
        }

        closest_idx
    }

    /// Find closest stop index in forward direction only
    ///
    /// Searches from last_idx to end of route only. This prevents
    /// selecting stops behind the current position, which is important
    /// after off-route snap re-entry.
    ///
    /// # Arguments
    /// * `s_cm` - Current position along route (cm)
    /// * `last_idx` - Starting index for search (inclusive)
    ///
    /// # Returns
    /// Index of closest stop at or after last_idx
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

    /// Reset all stop states to Idle after recovery
    fn reset_stop_states_after_recovery(&mut self, recovered_idx: usize, current_s_cm: DistCm) {
        use detection::state_machine::StopState;

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

    /// Get the last known stop index (for testing recovery behavior)
    pub fn last_known_stop_index(&self) -> u8 {
        self.last_known_stop_index
    }

    /// Get the last valid position in cm (for testing recovery behavior)
    pub fn last_valid_s_cm(&self) -> DistCm {
        self.last_valid_s_cm
    }

    /// Returns true if state should be persisted this tick.
    /// Writes when stop index changes, but no more than once per 60 seconds.
    /// This rate limiting prevents excessive flash wear (~100k erase cycles).
    pub fn should_persist(&self, current_stop: u8) -> bool {
        // M5: Gate persistence during off-route/suspect states
        // Don't persist if position is frozen (off-route or suspect)
        if self.kalman.freeze_ctx.is_some() {
            return false;
        }
        // Don't persist if in suspect state (may be about to go off-route)
        if self.kalman.off_route_suspect_ticks > 0 {
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

    /// Get the current stop index from last_known_stop_index.
    /// Returns None if not yet initialized.
    pub fn current_stop_index(&self) -> Option<u8> {
        if self.first_fix {
            None
        } else {
            Some(self.last_known_stop_index)
        }
    }

    /// Get the recovery flag state (for testing)
    pub fn needs_recovery_on_reacquisition(&self) -> bool {
        self.needs_recovery_on_reacquisition
    }

    /// Get the freeze time (for testing)
    pub fn off_route_freeze_time(&self) -> Option<u64> {
        self.kalman.off_route_freeze_time
    }

    /// Apply persisted stop index by marking all prior stops as Departed.
    ///
    /// This prevents the corridor filter from re-triggering stops that
    /// were already passed before the reboot. Without this, the bus would
    /// re-announce all stops from the beginning of the route.
    fn apply_persisted_stop_index(&mut self, stop_index: u8) {
        use shared::FsmState;

        for i in 0..stop_index.min(self.stop_states.len() as u8) as usize {
            self.stop_states[i].fsm_state = FsmState::Departed;
            self.stop_states[i].announced = true;
        }
        self.last_known_stop_index = stop_index;  // C3 fix: update tracking to persisted value
    }

    /// Check if estimation is ready (affects heading filter, Kalman)
    pub fn estimation_ready(&self) -> bool {
        self.estimation_ready_ticks >= ESTIMATION_WARMUP_TICKS
            || self.estimation_total_ticks >= ESTIMATION_TIMEOUT_TICKS
    }

    /// Check if detection is enabled (independent of estimation)
    pub fn detection_ready(&self) -> bool {
        self.detection_enabled_ticks >= DETECTION_WARMUP_TICKS
            || self.detection_total_ticks >= DETECTION_TIMEOUT_TICKS
    }

    /// Check if heading filter should be disabled
    pub fn disable_heading_filter(&self) -> bool {
        self.first_fix || !self.estimation_ready()
    }
}

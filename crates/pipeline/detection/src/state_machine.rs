//! Stop state machine with skip-stop protection

use shared::{DistCm, FsmState, Prob8, SpeedCms};

/// Event type returned by state machine update
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopEvent {
    /// Bus has arrived at this stop
    Arrived,
    /// Bus has departed from this stop
    Departed,
    /// No event this update
    None,
}

/// Runtime state for a single stop
pub struct StopState {
    /// Stop index in route
    pub index: u8,
    /// Current FSM state
    pub fsm_state: FsmState,
    /// Time spent in corridor (seconds)
    pub dwell_time_s: u16,
    /// Last computed arrival probability
    pub last_probability: Prob8,
    /// Last announced stop (v8.4: announcement tracking)
    /// Uses u8::MAX (255) as uninitialized value
    pub last_announced_stop: u8,
    /// Whether this stop has been announced in this trip (v8.6: one-time announcement)
    /// Once true, this stop can never be announced again in the same trip
    pub announced: bool,
}

impl StopState {
    pub fn new(index: u8) -> Self {
        StopState {
            index,
            fsm_state: FsmState::Idle,
            dwell_time_s: 0,
            last_probability: 0,
            last_announced_stop: u8::MAX,
            announced: false,
        }
    }

    /// Update state and return any event (arrival or departure)
    ///
    /// Arrival is triggered when:
    /// - Distance to stop < 50m (5000 cm)
    /// - Probability > THETA_ARRIVAL (191)
    ///
    /// Departure is triggered when:
    /// - Distance to stop > 40m (4000 cm)
    /// - Bus has moved past the stop (s_cm > stop_progress)
    /// - Currently in AtStop state
    ///
    /// Note: Speed threshold was removed to accommodate buses that stop
    /// slightly past the stop location due to GPS noise or urban constraints.
    ///
    /// v8.4: Also returns true if corridor entry (first time) for announcement
    pub fn update(
        &mut self,
        s_cm: DistCm,
        _v_cms: SpeedCms,
        stop_progress: DistCm,
        corridor_start_cm: DistCm,
        probability: Prob8,
    ) -> StopEvent {
        let d_to_stop = (s_cm - stop_progress).abs();

        match self.fsm_state {
            FsmState::Idle => {
                // Transition to Approaching when entering corridor
                if s_cm >= corridor_start_cm {
                    self.fsm_state = FsmState::Approaching;
                }
                // Don't increment dwell_time when idle
            }
            FsmState::Approaching => {
                if d_to_stop < 5000 {
                    self.fsm_state = FsmState::Arriving;
                }
                // Can exit corridor back to Idle if we leave the corridor
                if s_cm < corridor_start_cm {
                    self.fsm_state = FsmState::Idle;
                    self.dwell_time_s = 0; // Reset dwell time when leaving corridor
                } else {
                    // Update dwell time when in corridor
                    self.dwell_time_s += 1;
                }
            }
            FsmState::Arriving => {
                if d_to_stop < 5000 && probability > crate::probability::THETA_ARRIVAL {
                    self.fsm_state = FsmState::AtStop;
                    self.dwell_time_s += 1;
                    self.last_probability = probability;
                    self.announced = true;  // Mark as announced - one-time announcement rule
                    return StopEvent::Arrived; // Just arrived!
                }
                if d_to_stop > 4000 && s_cm > stop_progress {
                    self.fsm_state = FsmState::Departed;
                    self.last_probability = probability;
                    return StopEvent::Departed; // Departed from Arriving state
                }
                self.dwell_time_s += 1;
            }
            FsmState::AtStop => {
                if d_to_stop > 4000 && s_cm > stop_progress {
                    self.fsm_state = FsmState::Departed;
                    self.last_probability = probability;
                    return StopEvent::Departed; // Just departed!
                }
                // Don't increment dwell_time after departure
            }
            FsmState::Departed => {
                // Stay departed - dwell_time no longer accumulates
            }
            FsmState::TripComplete => {
                // Terminal state - no further transitions
            }
        }

        self.last_probability = probability;
        StopEvent::None
    }

    /// Check if announcement should trigger (v8.4 corridor entry announcement)
    ///
    /// Triggers when:
    /// - Any FSM state is active (Approaching/Arriving/AtStop)
    /// - Not yet announced for this stop
    /// - Just entered corridor (s_cm >= corridor_start_cm)
    ///
    /// Returns true if announcement should be made
    pub fn should_announce(&mut self, s_cm: DistCm, corridor_start_cm: DistCm) -> bool {
        // Check if already in corridor and not yet announced
        if s_cm >= corridor_start_cm && self.last_announced_stop != self.index {
            // Check if we're in an active FSM state
            let is_active = matches!(
                self.fsm_state,
                FsmState::Approaching | FsmState::Arriving | FsmState::AtStop
            );

            if is_active {
                self.last_announced_stop = self.index;
                return true;
            }
        }

        false
    }

    /// Check if stop can be re-activated (after departure)
    ///
    /// v8.6: Always returns false - one-time announcement rule.
    /// Once a stop has been announced, it can never be announced again in the same trip.
    /// This prevents duplicate arrivals caused by GPS noise or route loops.
    #[allow(dead_code)]
    pub fn can_reactivate(&self, _s_cm: DistCm, _stop_progress: DistCm) -> bool {
        false  // Never allow reactivation - one-time announcement per trip
    }

    /// Check if this is the terminal trip-completed state
    pub fn is_trip_complete(&self) -> bool {
        matches!(self.fsm_state, FsmState::TripComplete)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fsm_transitions() {
        let mut state = StopState::new(0);
        let stop_progress = 10000;
        let corridor_start_cm = 2000; // 80m before stop

        // Start in Idle state
        assert_eq!(state.fsm_state, FsmState::Idle);

        // Enter corridor -> Approaching
        state.update(2000, 100, stop_progress, corridor_start_cm, 0);
        assert_eq!(state.fsm_state, FsmState::Approaching);

        // Entering Arriving zone
        state.update(6000, 100, stop_progress, corridor_start_cm, 100);
        assert_eq!(state.fsm_state, FsmState::Arriving);

        // At stop (trigger!) - distance < 50m, probability > 191
        let event = state.update(14050, 100, stop_progress, corridor_start_cm, 200);
        assert_eq!(event, StopEvent::Arrived);
        assert_eq!(state.fsm_state, FsmState::AtStop);

        // Departing
        let event = state.update(15000, 500, stop_progress, corridor_start_cm, 10);
        assert_eq!(event, StopEvent::Departed);
        assert_eq!(state.fsm_state, FsmState::Departed);
    }

    #[test]
    fn test_trip_complete_state() {
        let mut state = StopState::new(255); // Last stop (u8::MAX)
        state.fsm_state = FsmState::Departed;
        assert!(!state.is_trip_complete());

        // Transition to TripComplete
        state.fsm_state = FsmState::TripComplete;
        assert!(state.is_trip_complete());

        // v8.6: can_reactivate always returns false (one-time announcement rule)
        assert!(!state.can_reactivate(10000, 10000));
    }

    #[test]
    fn test_trip_complete_is_terminal_state() {
        // v8.5: TripComplete is a terminal state - no further transitions
        let mut state = StopState::new(255);
        state.fsm_state = FsmState::TripComplete;
        let stop_progress = 100000;

        // Try to update from TripComplete state
        // Even if GPS position changes, state should remain TripComplete
        let corridor_start_cm = stop_progress - 8000;
        let event = state.update(200000, 500, stop_progress, corridor_start_cm, 255);

        // Should not trigger arrival (already at terminal state)
        assert_eq!(event, StopEvent::None, "TripComplete should not trigger arrival");
        assert_eq!(state.fsm_state, FsmState::TripComplete);
    }

    #[test]
    fn test_trip_complete_dwell_time_does_not_accumulate() {
        // v8.5: In TripComplete, dwell_time should not change
        let mut state = StopState::new(255);
        state.fsm_state = FsmState::TripComplete;
        state.dwell_time_s = 100;

        // Update multiple times
        let corridor_start_cm = 100000 - 8000;
        for _ in 0..10 {
            state.update(100000, 0, 100000, corridor_start_cm, 0);
        }

        // dwell_time should remain unchanged in TripComplete
        assert_eq!(
            state.dwell_time_s, 100,
            "dwell_time should not accumulate in TripComplete"
        );
    }

    #[test]
    fn test_departed_state_prevents_reactivation() {
        // v8.6: Departed state CANNOT be reactivated (one-time announcement rule)
        // Once a stop has been announced, it can never be announced again in the same trip
        let mut state = StopState::new(10);
        state.fsm_state = FsmState::Departed;
        let stop_progress = 10000;

        // Departed should NOT allow reactivation - one-time announcement per trip
        assert!(!state.can_reactivate(stop_progress - 8000, stop_progress));
        assert!(!state.can_reactivate(stop_progress, stop_progress));
        assert!(!state.can_reactivate(stop_progress + 4000, stop_progress));
    }

    #[test]
    fn test_fsm_handles_all_states() {
        // v8.5: Ensure update() handles all FSM states without panic
        let stop_progress = 10000;
        let corridor_start_cm = 2000;
        let states = [
            FsmState::Idle,
            FsmState::Approaching,
            FsmState::Arriving,
            FsmState::AtStop,
            FsmState::Departed,
            FsmState::TripComplete,
        ];

        for fsm_state in states {
            let mut state = StopState::new(0);
            state.fsm_state = fsm_state;

            // Should not panic for any state
            state.update(15000, 100, stop_progress, corridor_start_cm, 100);
        }
    }

    #[test]
    fn test_idle_state_initialization() {
        // v8.5: New states should initialize to Idle, not Approaching
        let state = StopState::new(5);
        assert_eq!(state.fsm_state, FsmState::Idle);
        assert_eq!(state.index, 5);
        assert_eq!(state.dwell_time_s, 0);
    }

    #[test]
    fn test_idle_to_approaching_on_corridor_entry() {
        // v8.5: Idle -> Approaching when entering corridor (s_cm >= corridor_start_cm)
        let mut state = StopState::new(0);
        let stop_progress = 10000;
        let corridor_start_cm = 2000;

        // Before corridor: should stay Idle
        state.update(1000, 100, stop_progress, corridor_start_cm, 0);
        assert_eq!(state.fsm_state, FsmState::Idle);
        assert_eq!(state.dwell_time_s, 0);

        // Enter corridor: should transition to Approaching
        state.update(2000, 100, stop_progress, corridor_start_cm, 0);
        assert_eq!(state.fsm_state, FsmState::Approaching);
        assert_eq!(state.dwell_time_s, 0); // No increment on transition tick
    }

    #[test]
    fn test_approaching_to_idle_on_corridor_exit() {
        // v8.5: Approaching -> Idle when exiting corridor (s_cm < corridor_start_cm)
        let mut state = StopState::new(0);
        let stop_progress = 10000;
        let corridor_start_cm = 2000;

        // Enter corridor and stay for a few ticks
        state.update(5000, 100, stop_progress, corridor_start_cm, 0);
        assert_eq!(state.fsm_state, FsmState::Approaching);

        state.update(5000, 100, stop_progress, corridor_start_cm, 0);
        assert_eq!(state.fsm_state, FsmState::Approaching);
        assert_eq!(state.dwell_time_s, 1);

        state.update(5000, 100, stop_progress, corridor_start_cm, 0);
        assert_eq!(state.dwell_time_s, 2);

        // Exit corridor: should transition to Idle and reset dwell_time
        state.update(1000, 100, stop_progress, corridor_start_cm, 0);
        assert_eq!(state.fsm_state, FsmState::Idle);
        assert_eq!(state.dwell_time_s, 0); // Reset on exit
    }

    #[test]
    fn test_dwell_time_only_counts_in_corridor() {
        // v8.5: dwell_time_s should only increment while in Approaching state
        let mut state = StopState::new(0);
        let stop_progress = 10000;
        let corridor_start_cm = 2000;

        // Outside corridor: no dwell_time increment
        for _ in 0..10 {
            state.update(1000, 100, stop_progress, corridor_start_cm, 0);
        }
        assert_eq!(state.fsm_state, FsmState::Idle);
        assert_eq!(state.dwell_time_s, 0);

        // Enter corridor: first tick transitions, no increment
        state.update(5000, 100, stop_progress, corridor_start_cm, 0);
        assert_eq!(state.fsm_state, FsmState::Approaching);
        assert_eq!(state.dwell_time_s, 0);

        // Subsequent ticks in corridor: dwell_time increments
        for _ in 0..5 {
            state.update(5000, 100, stop_progress, corridor_start_cm, 0);
        }
        assert_eq!(state.dwell_time_s, 5);

        // Exit corridor: resets to Idle
        state.update(1000, 100, stop_progress, corridor_start_cm, 0);
        assert_eq!(state.fsm_state, FsmState::Idle);
        assert_eq!(state.dwell_time_s, 0);
    }

    #[test]
    fn test_one_time_announcement_rule() {
        // v8.6: A stop can only be announced once per trip
        let mut state = StopState::new(0);
        let stop_progress = 10000;
        let corridor_start_cm = 2000;

        // Initially not announced
        assert!(!state.announced);

        // Enter corridor (Approaching)
        let event = state.update(2000, 100, stop_progress, corridor_start_cm, 0);
        assert_eq!(event, StopEvent::None);
        assert!(!state.announced);

        // Move to Arriving zone
        let event = state.update(6000, 100, stop_progress, corridor_start_cm, 100);
        assert_eq!(event, StopEvent::None);
        assert!(!state.announced);

        // First arrival should set announced flag
        let event = state.update(14050, 100, stop_progress, corridor_start_cm, 200);
        assert_eq!(event, StopEvent::Arrived);
        assert!(state.announced, "announced flag should be set after arrival");

        // Depart from stop
        let event = state.update(15000, 500, stop_progress, corridor_start_cm, 10);
        assert_eq!(event, StopEvent::Departed);
        assert!(state.announced, "announced flag should remain true after departure");

        // Even if we re-enter the corridor, can_reactivate returns false
        assert!(!state.can_reactivate(stop_progress, stop_progress));
    }
}

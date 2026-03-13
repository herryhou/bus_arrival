//! Stop state machine with skip-stop protection

use shared::{DistCm, SpeedCms, Prob8, FsmState};

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
}

impl StopState {
    pub fn new(index: u8) -> Self {
        StopState {
            index,
            fsm_state: FsmState::Approaching,
            dwell_time_s: 0,
            last_probability: 0,
        }
    }

    /// Reset state for re-entry into corridor (after departure)
    pub fn reset(&mut self) {
        self.fsm_state = FsmState::Approaching;
        self.dwell_time_s = 0;
        self.last_probability = 0;
    }

    /// Update state and return true if just arrived
    pub fn update(
        &mut self,
        s_cm: DistCm,
        v_cms: SpeedCms,
        stop_progress: DistCm,
        probability: Prob8,
    ) -> bool {
        let d_to_stop = (s_cm - stop_progress).abs();

        match self.fsm_state {
            FsmState::Approaching => {
                if d_to_stop < 5000 {
                    self.fsm_state = FsmState::Arriving;
                }
                // Update dwell time when in corridor
                self.dwell_time_s += 1;
            }
            FsmState::Arriving => {
                if d_to_stop < 3000 && v_cms < 56 && probability > 191 {
                    self.fsm_state = FsmState::AtStop;
                    self.dwell_time_s += 1;
                    return true;  // Just arrived!
                }
                self.dwell_time_s += 1;
            }
            FsmState::AtStop => {
                if d_to_stop > 4000 && s_cm > stop_progress {
                    self.fsm_state = FsmState::Departed;
                }
                // Don't increment dwell_time after departure
            }
            FsmState::Departed => {
                // Stay departed - dwell_time no longer accumulates
            }
        }

        self.last_probability = probability;
        false
    }

    /// Check if stop can be re-activated (after departure)
    pub fn can_reactivate(&self, s_cm: DistCm, stop_progress: DistCm) -> bool {
        matches!(self.fsm_state, FsmState::Departed)
            && s_cm >= stop_progress - 8000  // Back in corridor
            && s_cm <= stop_progress + 4000
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fsm_transitions() {
        let mut state = StopState::new(0);
        let stop_progress = 10000;
        
        // Far away
        state.update(2000, 100, stop_progress, 0);
        assert_eq!(state.fsm_state, FsmState::Approaching);

        // Entering Arriving zone
        state.update(6000, 100, stop_progress, 100);
        assert_eq!(state.fsm_state, FsmState::Arriving);

        // At stop (trigger!)
        let arrived = state.update(10050, 20, stop_progress, 200);
        assert!(arrived);
        assert_eq!(state.fsm_state, FsmState::AtStop);

        // Departing
        state.update(15000, 500, stop_progress, 10);
        assert_eq!(state.fsm_state, FsmState::Departed);
    }
}

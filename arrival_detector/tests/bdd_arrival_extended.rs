use arrival_detector::probability::{build_gaussian_lut, build_logistic_lut, arrival_probability, THETA_ARRIVAL};
use arrival_detector::state_machine::StopState;
use shared::{Stop, FsmState};

#[test]
fn scenario_premature_departure() {
    let g_lut = build_gaussian_lut();
    let l_lut = build_logistic_lut();
    let stop = Stop {
        progress_cm: 10000,
        corridor_start_cm: 2000,
        corridor_end_cm: 14000,
    };
    let mut state = StopState::new(0);

    // Given: bus enters arrival zone
    state.update(8000, 500, stop.progress_cm, 0);
    assert_eq!(state.fsm_state, FsmState::Arriving);

    // When: bus moves past the stop without probability triggering arrival
    // (e.g. speed remains too high)
    let prob = arrival_probability(14100, 1000, &stop, state.dwell_time_s, &g_lut, &l_lut);
    assert!(prob < THETA_ARRIVAL);
    
    let arrived = state.update(14100, 1000, stop.progress_cm, prob);
    
    // Then: state should transition to Departed without triggering arrival
    assert!(!arrived);
    assert_eq!(state.fsm_state, FsmState::Departed);
}

#[test]
fn scenario_stop_reactivation() {
    let stop = Stop {
        progress_cm: 10000,
        corridor_start_cm: 2000,
        corridor_end_cm: 14000,
    };
    let mut state = StopState::new(0);

    // Given: stop is already in Departed state
    state.fsm_state = FsmState::Departed;

    // When: bus moves back into the corridor (e.g. after a loop)
    // d = |5000 - 10000| = 5000. 
    // can_reactivate check: s_cm >= 10000 - 8000 = 2000 and s_cm <= 10000 + 4000 = 14000
    let can_reset = state.can_reactivate(5000, stop.progress_cm);
    assert!(can_reset);

    if can_reset {
        state.reset();
    }

    // Then: state should be Approaching again
    assert_eq!(state.fsm_state, FsmState::Approaching);
    assert_eq!(state.dwell_time_s, 0);
}

#[test]
fn scenario_stationary_but_far_from_stop() {
    let g_lut = build_gaussian_lut();
    let l_lut = build_logistic_lut();
    let stop = Stop {
        progress_cm: 10000,
        corridor_start_cm: 2000,
        corridor_end_cm: 14000,
    };
    
    // When: bus is stationary but 70m away from stop
    // s_cm = 3000. d = |3000 - 10000| = 7000.
    let dwell_time = 30; // 30 seconds
    let prob = arrival_probability(3000, 0, &stop, dwell_time, &g_lut, &l_lut);
    
    // Then: probability should be low due to distance penalty
    // sigma_d = 2750. 7000/2750 = 2.54 sigma. 
    // p1 (distance) will be low.
    assert!(prob < THETA_ARRIVAL, "Probability {} should be < {} for 70m distance", prob, THETA_ARRIVAL);
}

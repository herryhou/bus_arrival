use arrival_detector::probability::{build_gaussian_lut, build_logistic_lut, arrival_probability, THETA_ARRIVAL};
use arrival_detector::state_machine::StopState;
use shared::{Stop, FsmState};

#[test]
fn test_math_level_lut_verification() {
    let g_lut = build_gaussian_lut();
    let l_lut = build_logistic_lut();

    // Verify Gaussian LUT at key points
    // i = (x/sigma) * 64. 
    // x=0 -> i=0. exp(0)=1.0 -> 255.
    assert_eq!(g_lut[0], 255);
    // x=sigma -> i=64. exp(-0.5)=0.606 -> 155.
    assert!(g_lut[64] >= 154 && g_lut[64] <= 156);
    // x=2*sigma -> i=128. exp(-2)=0.135 -> 34.
    assert!(g_lut[128] >= 34 && g_lut[128] <= 35);
    // x=4*sigma -> i=256(255). exp(-8)=0.0003 -> 0.
    assert_eq!(g_lut[255], 0);

    // Verify Logistic LUT at key points
    // i = v/10. v_stop = 200 -> i=20.
    // v=200 -> i=20. 1/(1+exp(0))=0.5 -> 127.
    assert_eq!(l_lut[20], 127);
    // v=0 -> i=0. 1/(1+exp(-2))=0.88 -> 224. (using k=0.01, v_stop=200 -> exp(0.01*-200)=exp(-2))
    assert!(l_lut[0] >= 224 && l_lut[0] <= 225);
}

#[test]
fn scenario_successful_arrival_at_standard_stop() {
    let g_lut = build_gaussian_lut();
    let l_lut = build_logistic_lut();
    
    // Given: a route with a stop at progress 10000 cm
    let stop = Stop {
        progress_cm: 10000,
        corridor_start_cm: 2000,
        corridor_end_cm: 14000,
    };
    
    // And: the bus is in the Idle state
    let mut state = StopState::new(0);
    assert_eq!(state.fsm_state, FsmState::Idle);

    // When: the bus enters the corridor (s_cm >= corridor_start_cm)
    state.update(6000, 500, stop.progress_cm, stop.corridor_start_cm, 0);
    assert_eq!(state.fsm_state, FsmState::Approaching);

    // And: the bus enters the arrival zone (d < 5000cm)
    // d = |6000 - 10000| = 4000.
    state.update(6000, 500, stop.progress_cm, stop.corridor_start_cm, 0);
    assert_eq!(state.fsm_state, FsmState::Arriving);

    // And: the speed drops to 20 cm/s
    // And: the bus remains stationary for 10 seconds (simulated by multiple updates)
    state.dwell_time_s = 10;
    
    // Calculate probability
    let prob = arrival_probability(10050, 20, &stop, state.dwell_time_s, &g_lut, &l_lut);
    
    // Then: the probability should exceed 191
    assert!(prob > THETA_ARRIVAL, "Probability {} should be > {}", prob, THETA_ARRIVAL);

    // And: an ArrivalEvent for that stop must be emitted
    let arrived = state.update(10050, 20, stop.progress_cm, stop.corridor_start_cm, prob);
    assert!(arrived);
    assert_eq!(state.fsm_state, FsmState::AtStop);
}

#[test]
fn scenario_skip_stop_protection_high_speed() {
    let g_lut = build_gaussian_lut();
    let l_lut = build_logistic_lut();
    let stop = Stop {
        progress_cm: 10000,
        corridor_start_cm: 2000,
        corridor_end_cm: 14000,
    };
    let mut state = StopState::new(0);

    // When: the bus passes the stop at 1111 cm/s (40 km/h)
    // Progress: 9000 -> 10111 -> 11222

    // At 9000: Entering corridor -> Approaching, then to Arriving zone
    state.update(9000, 1111, stop.progress_cm, stop.corridor_start_cm, 0);
    assert_eq!(state.fsm_state, FsmState::Approaching);
    state.update(9000, 1111, stop.progress_cm, stop.corridor_start_cm, 0);
    assert_eq!(state.fsm_state, FsmState::Arriving);

    // At 10111: Passing stop
    let prob = arrival_probability(10111, 1111, &stop, state.dwell_time_s, &g_lut, &l_lut);

    // Then: Speed likelihood (p2) should be low, and prob should be low
    // v=1111 -> i=111. 1/(1+exp(0.01*(1111-200))) = 1/(1+exp(9.11)) -> near 0.
    assert!(prob < THETA_ARRIVAL);

    let arrived = state.update(10111, 1111, stop.progress_cm, stop.corridor_start_cm, prob);
    assert!(!arrived);

    // At 14100: Departing
    state.update(14100, 1111, stop.progress_cm, stop.corridor_start_cm, 0);
    assert_eq!(state.fsm_state, FsmState::Departed);
}

#[test]
fn scenario_gps_jump_recovery() {
    let stops = vec![
        Stop { progress_cm: 10000, corridor_start_cm: 2000, corridor_end_cm: 18000 },
        Stop { progress_cm: 50000, corridor_start_cm: 42000, corridor_end_cm: 58000 },
        Stop { progress_cm: 90000, corridor_start_cm: 82000, corridor_end_cm: 98000 },
    ];

    // Given: the current stop index is 0
    let last_index = 0u8;

    // When: a GPS jump occurs to progress 50100 cm (400m jump)
    let jump_progress = 50100;
    
    // Then: find_stop_index must trigger and find the best stop
    let recovered = arrival_detector::recovery::find_stop_index(jump_progress, &stops, last_index);
    assert_eq!(recovered, Some(1));

    // When: a GPS jump occurs to progress 89900 cm (800m jump)
    let jump_progress = 89900;
    let recovered = arrival_detector::recovery::find_stop_index(jump_progress, &stops, 1);
    assert_eq!(recovered, Some(2));

    // When: a jump backwards occurs, it's penalized
    // At s=49900, but last_index=2.
    // Index 1 (50000): dist 100, penalty 5000 -> score 5100
    // Index 2 (90000): dist 40100, penalty 0 -> score 40100
    let recovered = arrival_detector::recovery::find_stop_index(49900, &stops, 2);
    assert_eq!(recovered, Some(1));
}

#[test]
fn scenario_close_stop_discrimination() {
    let stop_a = Stop {
        progress_cm: 5000,
        corridor_start_cm: 0,
        corridor_end_cm: 7000, // 20m post-stop
    };
    let stop_b = Stop {
        progress_cm: 13000,
        corridor_start_cm: 7000, // Starts where A ends
        corridor_end_cm: 17000,
    };
    let stops = vec![stop_a, stop_b];

    // When: the bus is at 5100 cm
    let active = arrival_detector::corridor::find_active_stops(5100, &stops);
    
    // Then: only Stop A's Corridor Filter should be active
    assert_eq!(active, vec![0]);

    // When: the bus is at 7100 cm
    let active = arrival_detector::corridor::find_active_stops(7100, &stops);
    
    // Then: only Stop B's Corridor Filter should be active
    assert_eq!(active, vec![1]);
}

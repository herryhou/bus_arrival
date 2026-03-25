use arrival_detector::corridor::find_active_stops;
use arrival_detector::probability::THETA_ARRIVAL;
use arrival_detector::state_machine::{StopState, StopEvent};
use shared::{Stop, FsmState};

#[test]
fn scenario_simultaneous_overlapping_corridors() {
    // Given: two stops have overlapping corridors
    // Stop 0: [9200, 10400]
    // Stop 1: [9700, 10900]
    let stops = vec![
        Stop { progress_cm: 10000, corridor_start_cm: 9200, corridor_end_cm: 10400 },
        Stop { progress_cm: 10500, corridor_start_cm: 9700, corridor_end_cm: 10900 },
    ];

    // When: the bus is at 10000m (in the overlap region)
    let active = find_active_stops(10000, &stops);

    // Then: both stops should be returned as active
    assert_eq!(active.len(), 2);
    assert_eq!(active, vec![0, 1]);
}

#[test]
fn scenario_corridor_boundary_exact_start_and_end() {
    let stops = vec![
        Stop { progress_cm: 10000, corridor_start_cm: 2000, corridor_end_cm: 14000 },
    ];

    // When: the bus progress is exactly at the start boundary
    let active_start = find_active_stops(2000, &stops);
    // Then: the stop should be identified as active
    assert_eq!(active_start, vec![0]);

    // When: the bus progress is exactly at the end boundary
    let active_end = find_active_stops(14000, &stops);
    // Then: the stop should be identified as active
    assert_eq!(active_end, vec![0]);
}

#[test]
fn scenario_probability_threshold_edge_case() {
    let mut state = StopState::new(0);
    let stop_progress = 10000;

    // Given: the arrival probability equals exactly THETA_ARRIVAL (191)
    let probability = THETA_ARRIVAL;

    // When: the state update is processed in corridor and Arriving zone
    let corridor_start_cm = stop_progress - 8000; // 2000
    state.update(9000, 100, stop_progress, corridor_start_cm, 0); // Enter corridor and Arriving zone
    let event_at_threshold = state.update(10000, 100, stop_progress, corridor_start_cm, probability);

    // Then: arrival should NOT be triggered (must be > threshold)
    assert_eq!(event_at_threshold, StopEvent::None, "Arrival should NOT trigger at exactly THETA_ARRIVAL");

    // When: probability is 192
    let event_above_threshold = state.update(10000, 100, stop_progress, corridor_start_cm, THETA_ARRIVAL + 1);

    // Then: arrival SHOULD trigger
    assert_eq!(event_above_threshold, StopEvent::Arrived, "Arrival should trigger at THETA_ARRIVAL + 1");
}

#[test]
fn scenario_dwell_time_progression() {
    let mut state = StopState::new(0);
    let stop_progress = 10000;

    // Given: a bus is stationary at a stop (speed = 0 cm/s)
    let corridor_start_cm = stop_progress - 8000; // 2000

    // First update transitions from Idle to Approaching (no dwell_time increment)
    state.update(10000, 0, stop_progress, corridor_start_cm, 0);
    assert_eq!(state.fsm_state, FsmState::Approaching);
    assert_eq!(state.dwell_time_s, 0);

    // When: 5 more updates occur (simulating 5 seconds/updates in corridor)
    for _ in 0..5 {
        state.update(10000, 0, stop_progress, corridor_start_cm, 0);
    }

    // Then: the dwell_time_s should be 5 (one for each update while in Approaching)
    assert_eq!(state.dwell_time_s, 5);
}

#[test]
fn scenario_gps_jump_over_entire_corridor() {
    // Given: a stop with corridor [2000, 14000]
    let stops = vec![
        Stop { progress_cm: 10000, corridor_start_cm: 2000, corridor_end_cm: 14000 },
    ];

    // When: checking progress before corridor
    let active_before = find_active_stops(1000, &stops);
    assert_eq!(active_before.len(), 0, "Should not be active before corridor");

    // When: GPS jumps to 15000 (skipping the corridor entirely)
    let active_after = find_active_stops(15000, &stops);

    // Then: the stop should never be marked as active
    assert_eq!(active_after.len(), 0, "Should not be active after jumping over corridor");
}

#[test]
fn scenario_dense_stops_adjacent_corridors() {
    // Given: multiple stops with corridors that touch but don't overlap
    // Stop 0: [0, 10000]
    // Stop 1: [10000, 20000] - starts where Stop 0 ends
    // Stop 2: [20000, 30000] - starts where Stop 1 ends
    let stops = vec![
        Stop { progress_cm: 5000, corridor_start_cm: 0, corridor_end_cm: 10000 },
        Stop { progress_cm: 15000, corridor_start_cm: 10000, corridor_end_cm: 20000 },
        Stop { progress_cm: 25000, corridor_start_cm: 20000, corridor_end_cm: 30000 },
    ];

    // When: checking at boundary between Stop 0 and Stop 1
    let active_at_boundary = find_active_stops(10000, &stops);

    // Then: both stops should be active (at exact boundary)
    assert_eq!(active_at_boundary.len(), 2, "Both stops should be active at boundary");

    // When: checking in middle of Stop 1's corridor
    let active_middle = find_active_stops(15000, &stops);

    // Then: only Stop 1 should be active
    assert_eq!(active_middle, vec![1], "Only Stop 1 should be active");

    // Verify no gaps: every point should have at least one active stop
    for progress in [0, 5000, 10000, 15000, 20000, 25000, 30000].iter() {
        let active = find_active_stops(*progress, &stops);
        assert!(active.len() >= 1, "Should have at least one active stop at progress {}", progress);
    }
}

#[test]
fn scenario_stop_reactivation_after_loop() {
    use shared::FsmState;

    // Given: a stop with corridor [2000, 14000]
    let stop = Stop {
        progress_cm: 10000,
        corridor_start_cm: 2000,
        corridor_end_cm: 14000,
    };
    let mut state = StopState::new(0);

    // When: bus approaches and enters corridor and Arriving zone
    state.update(6000, 100, stop.progress_cm, stop.corridor_start_cm, 0);
    assert_eq!(state.fsm_state, FsmState::Approaching);
    state.update(6000, 100, stop.progress_cm, stop.corridor_start_cm, 0);
    assert_eq!(state.fsm_state, FsmState::Arriving);

    // When: bus arrives at stop
    state.update(10000, 0, stop.progress_cm, stop.corridor_start_cm, 200); // High probability triggers arrival
    assert_eq!(state.fsm_state, FsmState::AtStop);

    // When: bus departs (moves past stop)
    state.update(15000, 500, stop.progress_cm, stop.corridor_start_cm, 0);
    assert_eq!(state.fsm_state, FsmState::Departed);

    // When: bus loops back and enters corridor again (e.g., circular route)
    let can_reset = state.can_reactivate(5000, stop.progress_cm);
    assert!(can_reset, "Should be able to re-enter corridor after loop");

    state.reset();
    assert_eq!(state.fsm_state, FsmState::Idle);
    assert_eq!(state.dwell_time_s, 0, "Dwell time should reset");
}

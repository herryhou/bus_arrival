//! Announcement integration test
//!
//! This test verifies that the should_announce() integration correctly triggers
//! announcement events when a bus enters a stop corridor for the first time.
//!
//! **v8.4 Corridor Entry Announcement Feature:**
//! - Announcement triggers when bus enters corridor (s_cm >= corridor_start_cm)
//! - Only triggers once per stop (one-time announcement rule)
//! - Must check AFTER FSM state update (spec requirement)
//!
//! Run with: cargo test -p detection test_announcement

use detection::state_machine::StopState;
use shared::FsmState;

#[test]
fn test_announcement_triggers_on_corridor_entry() {
    // Test that announcement triggers when entering corridor
    let mut state = StopState::new(0);
    let stop_progress = 10000;
    let corridor_start_cm = 2000;

    // Before corridor: no announcement
    assert!(!state.should_announce(1000, corridor_start_cm));

    // Enter corridor: should trigger announcement
    // First, update to transition to Approaching state
    state.update(2000, 100, stop_progress, corridor_start_cm, 0);
    assert_eq!(state.fsm_state, FsmState::Approaching);

    // Now check for announcement
    let announced = state.should_announce(2000, corridor_start_cm);
    assert!(announced, "Should announce on corridor entry");
    assert_eq!(state.last_announced_stop, 0, "Should record announcement");

    // Second check: should not announce again (one-time rule)
    let announced_again = state.should_announce(2000, corridor_start_cm);
    assert!(!announced_again, "Should not announce twice for same stop");
}

#[test]
fn test_announcement_only_in_active_states() {
    // Test that announcement only triggers in active FSM states
    let mut state = StopState::new(1);
    let stop_progress = 10000;
    let corridor_start_cm = 2000;

    // Idle state (before entering corridor): no announcement
    assert_eq!(state.fsm_state, FsmState::Idle);
    assert!(!state.should_announce(1000, corridor_start_cm));

    // Enter corridor: transition to Approaching
    state.update(2000, 100, stop_progress, corridor_start_cm, 0);
    assert_eq!(state.fsm_state, FsmState::Approaching);

    // Now in active state: should announce
    assert!(state.should_announce(2000, corridor_start_cm));

    // Move to Arriving zone
    state.update(6000, 100, stop_progress, corridor_start_cm, 100);
    assert_eq!(state.fsm_state, FsmState::Arriving);

    // Already announced, should not announce again
    assert!(!state.should_announce(6000, corridor_start_cm));
}

#[test]
fn test_announcement_one_time_per_stop() {
    // Test the one-time announcement rule (v8.6)
    let mut state = StopState::new(5);
    let stop_progress = 50000;
    let corridor_start_cm = 42000;

    // Enter and get announced
    state.update(42000, 100, stop_progress, corridor_start_cm, 0);
    assert!(state.should_announce(42000, corridor_start_cm));

    // Mark as announced via arrival
    state.update(50050, 0, stop_progress, corridor_start_cm, 200);

    // Depart
    state.update(55000, 500, stop_progress, corridor_start_cm, 10);

    // Even if we could re-enter (which we can't due to one-time rule),
    // should not announce again
    assert!(!state.should_announce(42000, corridor_start_cm));
}

#[test]
fn test_announcement_state_tracking() {
    // Test that last_announced_stop is correctly tracked
    let mut state0 = StopState::new(0);
    let mut state1 = StopState::new(1);
    let corridor_start_cm = 2000;

    // Initially uninitialized (u8::MAX)
    assert_eq!(state0.last_announced_stop, u8::MAX);
    assert_eq!(state1.last_announced_stop, u8::MAX);

    // Announce stop 0
    state0.update(2000, 100, 10000, corridor_start_cm, 0);
    assert!(state0.should_announce(2000, corridor_start_cm));
    assert_eq!(state0.last_announced_stop, 0);

    // Announce stop 1
    state1.update(2000, 100, 20000, corridor_start_cm, 0);
    assert!(state1.should_announce(2000, corridor_start_cm));
    assert_eq!(state1.last_announced_stop, 1);

    // Each stop tracks independently
    assert_eq!(state0.last_announced_stop, 0);
    assert_eq!(state1.last_announced_stop, 1);
}

#[test]
fn test_announcement_before_fsm_update() {
    // Test that announcement check happens AFTER FSM update
    // This is critical for v8.4 spec compliance
    let mut state = StopState::new(0);
    let stop_progress = 10000;
    let corridor_start_cm = 2000;

    // Before update: in Idle state, no announcement
    assert_eq!(state.fsm_state, FsmState::Idle);
    assert!(!state.should_announce(2000, corridor_start_cm));

    // After update: in Approaching state, announcement possible
    state.update(2000, 100, stop_progress, corridor_start_cm, 0);
    assert_eq!(state.fsm_state, FsmState::Approaching);
    assert!(state.should_announce(2000, corridor_start_cm));
}

#[test]
fn test_announcement_not_in_idle_state() {
    // Test that Idle state never triggers announcement
    let mut state = StopState::new(0);
    let stop_progress = 10000;
    let corridor_start_cm = 2000;

    // Force state to Idle
    state.fsm_state = FsmState::Idle;

    // Even if s_cm >= corridor_start_cm, Idle state should not announce
    assert!(!state.should_announce(2000, corridor_start_cm));
    assert!(!state.should_announce(5000, corridor_start_cm));

    // Transition to Approaching
    state.update(2000, 100, stop_progress, corridor_start_cm, 0);
    assert_eq!(state.fsm_state, FsmState::Approaching);

    // Now should announce
    assert!(state.should_announce(2000, corridor_start_cm));
}

#[test]
fn test_announcement_corridor_boundaries() {
    // Test announcement behavior at corridor boundaries
    let mut state = StopState::new(0);
    let stop_progress = 10000;
    let corridor_start_cm = 2000;

    // Just before corridor: no announcement
    state.update(1999, 100, stop_progress, corridor_start_cm, 0);
    assert_eq!(state.fsm_state, FsmState::Idle);
    assert!(!state.should_announce(1999, corridor_start_cm));

    // Exactly at corridor start: should announce
    state.update(2000, 100, stop_progress, corridor_start_cm, 0);
    assert_eq!(state.fsm_state, FsmState::Approaching);
    assert!(state.should_announce(2000, corridor_start_cm));

    // Well inside corridor: already announced
    assert!(!state.should_announce(5000, corridor_start_cm));
}

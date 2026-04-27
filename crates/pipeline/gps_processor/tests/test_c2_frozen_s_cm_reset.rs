//! Test C2: reset_off_route_state must clear frozen_s_cm
//!
//! Bug: reset_off_route_state cleared off_route_freeze_time but NOT frozen_s_cm,
//! causing spurious re-entry snaps after GPS outage during Suspect state.

use shared::KalmanState;
use gps_processor::kalman::reset_off_route_state;

#[test]
fn test_reset_off_route_state_clears_frozen_s_cm() {
    let mut state = KalmanState {
        s_cm: 10000,
        v_cms: 500,
        frozen_s_cm: Some(10000),
        off_route_suspect_ticks: 3,
        off_route_clear_ticks: 0,
        off_route_freeze_time: Some(12345),
        freeze_ctx: None,
        last_seg_idx: 0,
    };

    reset_off_route_state(&mut state);

    // All off-route fields should be cleared
    assert_eq!(state.off_route_suspect_ticks, 0, "suspect_ticks should be 0");
    assert_eq!(state.off_route_clear_ticks, 0, "clear_ticks should be 0");
    assert_eq!(state.off_route_freeze_time, None, "freeze_time should be None");
    assert_eq!(state.frozen_s_cm, None, "frozen_s_cm should be None (C2 fix)");
}

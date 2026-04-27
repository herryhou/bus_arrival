//! Test geometry-based FSM reset after snap
//! Run with: cargo test -p pico2-firmware --features dev reset_stop_states_after_snap

use pico2_firmware::state::State;
use shared::binfile::RouteData;
use shared::FsmState;
use std::path::Path;

#[test]
fn test_reset_stop_states_after_snap_before_current() {
    // Test that stops before current_idx are marked Departed
    let test_data_path = Path::new("../../tools/data/ty225_normal.bin");
    if !test_data_path.exists() {
        return;
    }

    let route_data_bytes = match std::fs::read(test_data_path) {
        Ok(bytes) => bytes,
        Err(_) => return,
    };

    let route_data = match RouteData::load(&route_data_bytes) {
        Ok(data) => data,
        Err(_) => return,
    };

    let mut state = State::new(&route_data, None);

    // Reset with current_idx=5, position at stop 7
    let current_idx = 5u8;
    if let Some(stop_7) = route_data.get_stop(7) {
        state.reset_stop_states_after_snap(current_idx, stop_7.progress_cm);

        // Stops 0-4 should be Departed
        for i in 0..5 {
            assert_eq!(
                state.stop_states[i].fsm_state,
                FsmState::Departed,
                "Stop {} should be Departed",
                i
            );
            assert!(
                state.stop_states[i].announced,
                "Stop {} should be marked announced",
                i
            );
            assert_eq!(
                state.stop_states[i].last_announced_stop, i as u8,
                "Stop {} last_announced_stop should be {}",
                i, i
            );
        }
    }
}

#[test]
fn test_reset_stop_states_after_snap_at_stop() {
    // Test current stop geometry: within 50m → AtStop
    let test_data_path = Path::new("../../tools/data/ty225_normal.bin");
    if !test_data_path.exists() {
        return;
    }

    let route_data_bytes = match std::fs::read(test_data_path) {
        Ok(bytes) => bytes,
        Err(_) => return,
    };

    let route_data = match RouteData::load(&route_data_bytes) {
        Ok(data) => data,
        Err(_) => return,
    };

    let mut state = State::new(&route_data, None);

    // Reset with current_idx=5, position exactly at stop 5 (within 50m)
    let current_idx = 5u8;
    if let Some(stop_5) = route_data.get_stop(5) {
        state.reset_stop_states_after_snap(current_idx, stop_5.progress_cm);

        // Current stop (5) should be AtStop (within 50m)
        assert_eq!(
            state.stop_states[5].fsm_state,
            FsmState::AtStop,
            "Current stop should be AtStop when position is within 50m"
        );
        assert!(
            state.stop_states[5].announced,
            "Current stop should be marked announced to prevent re-announcement"
        );
        assert_eq!(
            state.stop_states[5].dwell_time_s, 0,
            "Dwell time should be reset"
        );
        assert_eq!(
            state.stop_states[5].previous_distance_cm, None,
            "Previous distance should be reset"
        );
    }
}

#[test]
fn test_reset_stop_states_after_snap_past_stop() {
    // Test current stop geometry: >40m past → Departed
    let test_data_path = Path::new("../../tools/data/ty225_normal.bin");
    if !test_data_path.exists() {
        return;
    }

    let route_data_bytes = match std::fs::read(test_data_path) {
        Ok(bytes) => bytes,
        Err(_) => return,
    };

    let route_data = match RouteData::load(&route_data_bytes) {
        Ok(data) => data,
        Err(_) => return,
    };

    let mut state = State::new(&route_data, None);

    // Reset with current_idx=5, position 50m past stop 5
    let current_idx = 5u8;
    if let Some(stop_5) = route_data.get_stop(5) {
        let s_cm_past = stop_5.progress_cm + 5000; // 50m past
        state.reset_stop_states_after_snap(current_idx, s_cm_past);

        // Current stop (5) should be Departed (>40m past)
        assert_eq!(
            state.stop_states[5].fsm_state,
            FsmState::Departed,
            "Current stop should be Departed when position is >40m past"
        );
        assert!(
            state.stop_states[5].announced,
            "Current stop should be marked announced"
        );
    }
}

#[test]
fn test_reset_stop_states_after_snap_approaching() {
    // Test current stop geometry: not at stop, not past → Approaching
    let test_data_path = Path::new("../../tools/data/ty225_normal.bin");
    if !test_data_path.exists() {
        return;
    }

    let route_data_bytes = match std::fs::read(test_data_path) {
        Ok(bytes) => bytes,
        Err(_) => return,
    };

    let route_data = match RouteData::load(&route_data_bytes) {
        Ok(data) => data,
        Err(_) => return,
    };

    let mut state = State::new(&route_data, None);

    // Reset with current_idx=5, position 30m before stop 5
    let current_idx = 5u8;
    if let Some(stop_5) = route_data.get_stop(5) {
        let s_cm_before = stop_5.progress_cm - 3000; // 30m before
        state.reset_stop_states_after_snap(current_idx, s_cm_before);

        // Current stop (5) should be Approaching (not at stop, not past)
        assert_eq!(
            state.stop_states[5].fsm_state,
            FsmState::Approaching,
            "Current stop should be Approaching when position is before but not at stop"
        );
        assert!(
            !state.stop_states[5].announced,
            "Current stop should not be marked announced when Approaching"
        );
    }
}

#[test]
fn test_reset_stop_states_after_snap_future_stops() {
    // Test that future stops remain Idle
    let test_data_path = Path::new("../../tools/data/ty225_normal.bin");
    if !test_data_path.exists() {
        return;
    }

    let route_data_bytes = match std::fs::read(test_data_path) {
        Ok(bytes) => bytes,
        Err(_) => return,
    };

    let route_data = match RouteData::load(&route_data_bytes) {
        Ok(data) => data,
        Err(_) => return,
    };

    let mut state = State::new(&route_data, None);

    // Reset with current_idx=5
    let current_idx = 5u8;
    if let Some(stop_5) = route_data.get_stop(5) {
        state.reset_stop_states_after_snap(current_idx, stop_5.progress_cm);

        // Stops 6+ should be Idle
        for i in 6..state.stop_states.len().min(10) {
            assert_eq!(
                state.stop_states[i].fsm_state,
                FsmState::Idle,
                "Stop {} should be Idle",
                i
            );
            assert!(
                !state.stop_states[i].announced,
                "Stop {} should not be marked announced",
                i
            );
            assert_eq!(
                state.stop_states[i].last_announced_stop, u8::MAX,
                "Stop {} last_announced_stop should be u8::MAX",
                i
            );
        }
    }
}

#[test]
fn test_reset_stop_states_after_snap_boundary_50m() {
    // Test exact 50m boundary for AtStop
    let test_data_path = Path::new("../../tools/data/ty225_normal.bin");
    if !test_data_path.exists() {
        return;
    }

    let route_data_bytes = match std::fs::read(test_data_path) {
        Ok(bytes) => bytes,
        Err(_) => return,
    };

    let route_data = match RouteData::load(&route_data_bytes) {
        Ok(data) => data,
        Err(_) => return,
    };

    let mut state = State::new(&route_data, None);

    let current_idx = 5u8;
    if let Some(stop_5) = route_data.get_stop(5) {
        // Exactly 50m away (5000 cm) → should be AtStop
        let s_cm_50m = stop_5.progress_cm + 5000;
        state.reset_stop_states_after_snap(current_idx, s_cm_50m);

        assert_eq!(
            state.stop_states[5].fsm_state,
            FsmState::AtStop,
            "Should be AtStop at exactly 50m"
        );

        // Just over 50m (5001 cm) → should NOT be AtStop
        let mut state2 = State::new(&route_data, None);
        let s_cm_50m_plus_1 = stop_5.progress_cm + 5001;
        state2.reset_stop_states_after_snap(current_idx, s_cm_50m_plus_1);

        assert_ne!(
            state2.stop_states[5].fsm_state,
            FsmState::AtStop,
            "Should not be AtStop just over 50m"
        );
    }
}

#[test]
fn test_reset_stop_states_after_snap_boundary_40m_past() {
    // Test exact 40m past boundary for Departed
    let test_data_path = Path::new("../../tools/data/ty225_normal.bin");
    if !test_data_path.exists() {
        return;
    }

    let route_data_bytes = match std::fs::read(test_data_path) {
        Ok(bytes) => bytes,
        Err(_) => return,
    };

    let route_data = match RouteData::load(&route_data_bytes) {
        Ok(data) => data,
        Err(_) => return,
    };

    let mut state = State::new(&route_data, None);

    let current_idx = 5u8;
    if let Some(stop_5) = route_data.get_stop(5) {
        // Exactly 40m past (4000 cm) → should NOT be Departed (Approaching instead)
        let s_cm_40m = stop_5.progress_cm + 4000;
        state.reset_stop_states_after_snap(current_idx, s_cm_40m);

        assert_eq!(
            state.stop_states[5].fsm_state,
            FsmState::Approaching,
            "Should be Approaching at exactly 40m past (not >40m)"
        );

        // Just over 40m (4001 cm) → should be Departed
        let mut state2 = State::new(&route_data, None);
        let s_cm_40m_plus_1 = stop_5.progress_cm + 4001;
        state2.reset_stop_states_after_snap(current_idx, s_cm_40m_plus_1);

        assert_eq!(
            state2.stop_states[5].fsm_state,
            FsmState::Departed,
            "Should be Departed just over 40m past"
        );
    }
}

#[test]
fn test_reset_stop_states_after_snap_all_resets() {
    // Test that all reset fields are properly cleared
    let test_data_path = Path::new("../../tools/data/ty225_normal.bin");
    if !test_data_path.exists() {
        return;
    }

    let route_data_bytes = match std::fs::read(test_data_path) {
        Ok(bytes) => bytes,
        Err(_) => return,
    };

    let route_data = match RouteData::load(&route_data_bytes) {
        Ok(data) => data,
        Err(_) => return,
    };

    let mut state = State::new(&route_data, None);

    // First, set some non-default values
    for i in 0..state.stop_states.len().min(10) {
        state.stop_states[i].dwell_time_s = 10;
        state.stop_states[i].previous_distance_cm = Some(1000);
    }

    // Reset with current_idx=5
    let current_idx = 5u8;
    if let Some(stop_5) = route_data.get_stop(5) {
        state.reset_stop_states_after_snap(current_idx, stop_5.progress_cm);

        // All stops should have reset dwell_time and previous_distance
        for i in 0..state.stop_states.len().min(10) {
            assert_eq!(
                state.stop_states[i].dwell_time_s, 0,
                "Stop {} dwell_time_s should be reset to 0",
                i
            );
            assert_eq!(
                state.stop_states[i].previous_distance_cm, None,
                "Stop {} previous_distance_cm should be reset to None",
                i
            );
        }
    }
}

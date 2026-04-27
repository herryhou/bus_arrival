//! Test forward closest stop index search
//! Run with: cargo test -p pico2-firmware --features dev test_find_forward_closest_stop_index

use pico2_firmware::state::State;
use shared::binfile::RouteData;
use std::path::Path;

#[test]
fn test_find_forward_closest_stop_index_basic() {
    // Test basic forward search from middle of route
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

    let state = State::new(&route_data, None);

    // Forward search from stop 5, position at stop 7 → should return 7
    if let Some(stop_7) = route_data.get_stop(7) {
        let result = state.find_forward_closest_stop_index(stop_7.progress_cm, 5);
        assert_eq!(result, 7, "Should select stop 7 when searching forward from stop 5");
    }
}

#[test]
fn test_find_forward_closest_stop_index_edge_case() {
    // Test edge case: exact position at last_idx
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

    let state = State::new(&route_data, None);

    // When position is exactly at last_idx, should return last_idx
    if let Some(stop_10) = route_data.get_stop(10) {
        let result = state.find_forward_closest_stop_index(stop_10.progress_cm, 10);
        assert_eq!(result, 10, "Should return stop 10 when position is exactly at stop 10");
    }
}

#[test]
fn test_find_forward_closest_stop_index_last_stop() {
    // Test last stop boundary condition
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

    let state = State::new(&route_data, None);

    let last_idx = (route_data.stop_count as u8).saturating_sub(1);
    if let Some(last_stop) = route_data.get_stop(last_idx as usize) {
        let result = state.find_forward_closest_stop_index(last_stop.progress_cm, last_idx);
        assert_eq!(result, last_idx, "Should return last stop when searching from last stop");
    }
}

#[test]
fn test_find_forward_closest_prevents_backward_selection() {
    // Critical test: verify forward search NEVER selects stops before last_idx
    // This is the key difference from find_closest_stop_index
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

    let state = State::new(&route_data, None);

    // When last_idx=10 and position is at stop 8 (behind), should still return 10+
    if let Some(stop_8) = route_data.get_stop(8) {
        let result = state.find_forward_closest_stop_index(stop_8.progress_cm, 10);
        assert!(result >= 10, "Should never select stop before last_idx (10), got {}", result);
    }

    // When last_idx=5 and position is at stop 3 (way behind), should still return 5+
    if let Some(stop_3) = route_data.get_stop(3) {
        let result = state.find_forward_closest_stop_index(stop_3.progress_cm, 5);
        assert!(result >= 5, "Should never select stop before last_idx (5), got {}", result);
    }
}

#[test]
fn test_find_forward_closest_vs_full_search() {
    // Verify forward search behaves differently from full search
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

    let state = State::new(&route_data, None);

    // When position is at stop 5 and last_idx=10:
    // - Forward search: returns 10 (or later, whichever is closest from 10 onward)
    // - Full search: returns 5 (closest overall)
    if let Some(stop_5) = route_data.get_stop(5) {
        let forward_result = state.find_forward_closest_stop_index(stop_5.progress_cm, 10);
        let full_result = state.find_closest_stop_index(stop_5.progress_cm);

        assert_eq!(full_result, 5, "Full search should return stop 5 (closest overall)");
        assert!(forward_result >= 10, "Forward search should return stop 10 or later, got {}", forward_result);
    }
}

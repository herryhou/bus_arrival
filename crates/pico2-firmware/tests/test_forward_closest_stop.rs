//! Test forward closest stop index search
//! Run with: cargo test -p pico2-firmware --features dev test_find_forward_closest_stop_index

use pico2_firmware::state::State;
use shared::binfile::RouteData;
use std::path::Path;

#[test]
fn test_find_forward_closest_stop_index() {
    // Load actual route data for realistic testing
    let test_data_path = Path::new("../../tools/data/ty225_normal.bin");
    if !test_data_path.exists() {
        return; // Skip test if route data not found
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

    // Test 1: Forward search from middle of route
    // If we're at stop 5 and position is at stop 7, should return 7
    if let Some(stop_7) = route_data.get_stop(7) {
        let result = state.find_forward_closest_stop_index(stop_7.progress_cm, 5);
        assert_eq!(result, 7, "Should select stop 7 when searching forward from stop 5");
    }

    // Test 2: Forward search prevents backward selection
    // If we're at stop 10 and position is at stop 8 (behind), should still select stop 10 or later
    if let Some(stop_10) = route_data.get_stop(10) {
        if let Some(stop_8) = route_data.get_stop(8) {
            let result = state.find_forward_closest_stop_index(stop_8.progress_cm, 10);
            assert!(result >= 10, "Should never select stop before last_idx (10), got {}", result);
        }
    }

    // Test 3: Forward search at last stop
    let last_idx = (route_data.stop_count as u8).saturating_sub(1);
    if let Some(last_stop) = route_data.get_stop(last_idx as usize) {
        let result = state.find_forward_closest_stop_index(last_stop.progress_cm, last_idx);
        assert_eq!(result, last_idx, "Should return last stop when searching from last stop");
    }
}

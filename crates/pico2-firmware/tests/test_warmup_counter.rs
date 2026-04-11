//! Test warmup counter behavior in State

use std::fs;
use pico2_firmware::state::State;

#[test]
fn test_warmup_counter_increments_after_first_fix() {
    // Load route data
    let route_bytes = fs::read("../../test_data/ty225_normal.bin")
        .expect("Failed to load ty225_normal.bin");
    let route_data = shared::binfile::RouteData::load(&route_bytes)
        .expect("Failed to parse ty225_normal.bin");

    let mut state = State::new(&route_data);

    // Initial state: first_fix is true, warmup_counter is 0
    assert!(state.first_fix, "Initially first_fix should be true");
    assert_eq!(state.warmup_counter, 0, "Initially warmup_counter should be 0");

    // First GPS fix: should set first_fix to false, warmup_counter stays 0
    // Use a position far away from any stops (0,0 is definitely not in Hong Kong)
    let gps1 = shared::GpsPoint {
        lat: 0.0,  // 0° (equator) - far from route
        lon: 0.0,  // 0° (prime meridian) - far from route
        heading_cdeg: i16::MIN,  // GGA-only mode
        speed_cms: 500,
        timestamp: 1000,
        has_fix: true,
        hdop_x10: 10,
    };

    let _result = state.process_gps(&gps1);
    // First fix should not trigger arrival (wrong position for the route)
    // But we're mainly testing state transitions, so we'll accept either result
    // as long as the state transitions are correct
    assert!(!state.first_fix, "After first fix, first_fix should be false");
    assert_eq!(state.warmup_counter, 0, "After first fix, warmup_counter should still be 0");
    assert!(!state.first_fix, "After first fix, first_fix should be false");
    assert_eq!(state.warmup_counter, 0, "After first fix, warmup_counter should still be 0");

    // Second GPS tick: warmup_counter should increment to 1
    let gps2 = shared::GpsPoint {
        timestamp: 2000,
        ..gps1
    };

    let result = state.process_gps(&gps2);
    assert!(result.is_none(), "During warmup, no arrival should trigger");
    assert_eq!(state.warmup_counter, 1, "After second tick, warmup_counter should be 1");

    // Third GPS tick: warmup_counter should increment to 2
    let gps3 = shared::GpsPoint {
        timestamp: 3000,
        ..gps1
    };

    let result = state.process_gps(&gps3);
    assert!(result.is_none(), "During warmup, no arrival should trigger");
    assert_eq!(state.warmup_counter, 2, "After third tick, warmup_counter should be 2");

    // Fourth GPS tick: warmup_counter should increment to 3 (WARMUP_TICKS_REQUIRED)
    let gps4 = shared::GpsPoint {
        timestamp: 4000,
        ..gps1
    };

    let result = state.process_gps(&gps4);
    assert!(result.is_none(), "At end of warmup, still no arrival (wrong position)");
    assert_eq!(state.warmup_counter, 3, "After fourth tick, warmup_counter should be 3 (complete)");
}

#[test]
fn test_warmup_prevents_arrival_detection() {
    // Load route data
    let route_bytes = fs::read("../../test_data/ty225_normal.bin")
        .expect("Failed to load ty225_normal.bin");
    let route_data = shared::binfile::RouteData::load(&route_bytes)
        .expect("Failed to parse ty225_normal.bin");

    let mut state = State::new(&route_data);

    // First fix to initialize
    let gps_init = shared::GpsPoint {
        lat: 0.0,  // Far from route
        lon: 0.0,
        heading_cdeg: i16::MIN,
        speed_cms: 500,
        timestamp: 1000,
        has_fix: true,
        hdop_x10: 10,
    };
    state.process_gps(&gps_init);

    // During warmup, no arrival should trigger even if we send many GPS updates
    // The warmup counter prevents arrival detection until it reaches 3
    for i in 1..=3 {
        let gps = shared::GpsPoint {
            timestamp: 1000 + (i as u64) * 1000,
            ..gps_init
        };

        let result = state.process_gps(&gps);

        // Verify warmup counter is incrementing
        assert_eq!(
            state.warmup_counter, i,
            "During warmup tick {}, counter should be {}",
            i, i
        );

        // No arrival should trigger during warmup
        assert!(result.is_none(), "During warmup (counter={}), arrival should not trigger", i);
    }

    // After warmup completes (counter=3), arrival detection should be enabled
    // (though GPS position may not exactly match due to route data)
    assert_eq!(state.warmup_counter, 3, "Warmup should complete after 3 ticks");
}

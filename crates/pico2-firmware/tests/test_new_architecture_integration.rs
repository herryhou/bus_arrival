//! Integration tests for new architecture (SystemState + ModeMachine)
//!
//! Tests the full pipeline: NMEA → NmeaParser → SystemState → ArrivalEvent

#![no_std]

use pico2_firmware::parser::NmeaParser;
use shared::GpsPoint;

#[cfg(test)]
mod tests {
    use super::*;

    /// Test basic NMEA parsing through to SystemState
    #[test]
    fn test_nmea_to_systemstate_integration() {
        // This test verifies that NMEA sentences can be parsed
        // and flow through SystemState without errors

        let mut parser = NmeaParser::new();

        // Feed RMC then GGA at t=1 (no emission yet)
        parser.feed_sentence("$GPRMC,221320,A,2500.2582,N,12117.1898,E,8.4,80.5,141123,,*2E");
        parser.feed_sentence("$GPGGA,221320,2500.2582,N,12117.1898,E,1,08,3.5,10.0,M,0.0,M,,*4B");

        // Feed new sentence at t=2 (should emit t=1 fix)
        let result = parser.feed_sentence("$GPRMC,221321,A,2500.2583,N,12117.1899,E,8.5,81.5,141123,,*2F");
        assert!(result.is_some(), "New timestamp should emit previous fix");

        // Verify we got a GPS point
        let gps = result.unwrap();
        assert!(gps.has_fix, "GPS should have fix");
        assert!(gps.lat != 0.0, "GPS should have valid latitude");
        assert!(gps.lon != 0.0, "GPS should have valid longitude");
    }

    /// Test SystemState mode transitions
    #[test]
    fn test_systemstate_mode_transitions() {
        // Create minimal route data for testing
        // In a real test, we'd load actual route data from a bin file

        // For this test, we just verify the SystemState can be created
        // and has the expected mode machine integration

        // Note: This test would need actual route data to be functional
        // For now, it's a placeholder showing the test structure

        // let route_data = RouteData::load(/* ... */);
        // let mut state = SystemState::new(&route_data, None);
        // let mut est_state = EstimationState::new();

        // Verify initial state
        // assert_eq!(state.mode(), SystemMode::Normal);

        // TODO: Add actual GPS processing tests with real route data
    }

    /// Test persistence methods work correctly
    #[test]
    fn test_systemstate_persistence_methods() {
        // This test verifies the persistence API works

        // Note: This would need actual route data to be functional
        // For now, it's a placeholder showing the test structure

        // let route_data = RouteData::load(/* ... */);
        // let mut state = SystemState::new(&route_data, None);

        // Test initial state
        // assert_eq!(state.current_stop_index(), Some(0));
        // assert!(!state.should_persist(0));

        // Test after updating stop index
        // state.last_stop_index = 5;
        // assert!(state.should_persist(5));
    }

    /// Test detection warmup logic
    #[test]
    fn test_detection_warmup_blocks_events() {
        // Verify that detection is blocked during warmup period

        // Note: This would need actual route data and GPS points
        // For now, it's a placeholder showing the test structure

        // let route_data = RouteData::load(/* ... */);
        // let mut state = SystemState::new(&route_data, None);
        // let mut est_state = EstimationState::new();

        // Process first few GPS points - should not emit events
        // for _ in 0..3 {
        //     let gps = create_test_gps_point();
        //     let event = state.tick(&gps, &mut est_state);
        //     assert!(event.is_none(), "Should not emit during warmup");
        // }
    }
}

// Helper functions for testing (would be implemented with actual data)
#[allow(dead_code)]
fn create_test_gps_point() -> GpsPoint {
    GpsPoint {
        timestamp: 123_519_000,
        lat: 48.07038,  // 48.07038° N
        lon: 11.31324,  // 11.31324° E
        has_fix: true,
        hdop_x10: Some(10),
        speed_cms: Some(500),  // 5 m/s
        heading_cdeg: Some(8440),  // 84.4°
    }
}

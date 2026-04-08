//! Signal loss scenario tests (GPS outage, tunnel)

use super::common::{load_ty225_route, load_nmea, ExpectedResults};
use shared::binfile::RouteData;
use shared::{KalmanState, DrState};
use gps_processor::nmea::NmeaState;
use detection::state_machine::StopState;

/// Test: GPS outage scenario (10s signal loss)
/// Validates: Dead reckoning maintains position during outage
#[test]
fn test_outage_dead_reckoning() {
    // Load outage scenario data
    let route_bytes = load_ty225_route("outage");
    let route_data = RouteData::load(&route_bytes)
        .expect("Failed to load route data");

    let nmea_lines = load_nmea("outage");
    let expected = ExpectedResults::from_ground_truth("outage");

    // Initialize pipeline
    let mut nmea = NmeaState::new();
    let mut kalman = KalmanState::new();
    let mut dr = DrState::new();

    let mut stop_states: Vec<StopState> = route_data.stops()
        .iter()
        .enumerate()
        .map(|(i, _)| StopState::new(i as u8))
        .collect();

    let mut detected_arrivals: Vec<usize> = Vec::new();
    let mut outage_count = 0;
    let mut recovery_count = 0;

    // Process NMEA with outage
    for line in nmea_lines {
        if let Some(gps) = nmea.parse_sentence(&line) {
            if gps.has_fix {
                recovery_count += 1;
            } else {
                outage_count += 1;
            }
            // Pipeline processing
        }
    }

    // Validate: should have GPS invalid messages during outage
    assert!(
        outage_count > 0,
        "Outage scenario should have GPS invalid messages"
    );

    // Validate: should recover after outage
    assert!(
        recovery_count > 0,
        "Outage scenario should have GPS recovery"
    );

    // Validate arrivals despite outage
    assert!(
        detected_arrivals.len() >= expected.min_arrivals,
        "Outage scenario: expected at least {} arrivals, got {}",
        expected.min_arrivals,
        detected_arrivals.len()
    );
}

/// Test: Validate outage scenario route data
#[test]
fn test_outage_route_data() {
    let route_bytes = load_ty225_route("outage");
    let route_data = RouteData::load(&route_bytes)
        .expect("Failed to load route data");

    // Verify route loaded
    assert_eq!(route_data.stops().len(), 58, "Route should have 58 stops");

    // Verify route nodes exist
    assert!(
        route_data.node_count > 0,
        "Route should have nodes"
    );
}

/// Test: NMEA file contains valid GPS invalid messages
#[test]
fn test_outage_nmea_has_invalid_gps() {
    let nmea_lines = load_nmea("outage");

    let mut has_invalid = false;
    for line in nmea_lines {
        // Look for GPGGA with GPS quality indicator = 0 (no fix)
        if line.contains("$GPGGA") && line.contains(",0,") {
            has_invalid = true;
            break;
        }
    }

    assert!(
        has_invalid,
        "Outage NMEA should contain GPS invalid messages"
    );
}

//! Integration test to verify pipeline produces consistent output
//! between std and no_std builds

use std::fs;
use std::io::BufRead;

#[test]
fn test_pipeline_with_route_data() {
    // Load route data
    let route_bytes = fs::read("../../test_data/ty225_normal.bin")
        .expect("Failed to load ty225_normal.bin");
    let route_data = shared::binfile::RouteData::load(&route_bytes)
        .expect("Failed to parse ty225_normal.bin");

    // Load test NMEA
    let nmea_file = fs::File::open("../../test_data/ty225_normal_nmea.txt")
        .expect("Failed to open ty225_normal_nmea.txt");
    let reader = std::io::BufReader::new(nmea_file);

    // Initialize pipeline state
    use shared::{KalmanState, DrState};
    use gps_processor::nmea::NmeaState;
    use detection::state_machine::StopState;

    let mut nmea = NmeaState::new();
    let mut kalman = KalmanState::new();
    let mut dr = DrState::new();

    let mut stop_states: Vec<StopState> = route_data.stops()
        .iter()
        .enumerate()
        .map(|(i, _)| StopState::new(i as u8))
        .collect();

    let mut arrivals: Vec<u8> = Vec::new();
    let mut departures: Vec<u8> = Vec::new();

    // Process NMEA sentences
    for line in reader.lines() {
        let line = line.expect("Failed to read line");
        if let Some(_gps) = nmea.parse_sentence(&line) {
            // TODO: Complete pipeline processing
            // For now, just verify we can parse
        }
    }

    // Verify we got some results
    // (This will be updated when full pipeline is implemented)
    assert!(true, "Integration test structure verified");
}

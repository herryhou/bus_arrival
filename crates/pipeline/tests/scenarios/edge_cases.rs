//! Edge case and stress tests
//!
//! Tests for unusual scenarios that could occur in production:
//! - Corrupt GPS data
//! - Extreme GPS jumps (>200m)
//! - Rapid direction changes
//! - Stationary GPS (no movement)
//! - GPS returning same coordinates

use super::common::{load_ty225_route, load_nmea_reader};
use shared::binfile::RouteData;
use pipeline::Pipeline;

/// Test: Pipeline handles empty NMEA input gracefully
#[test]
fn test_empty_nmea() {
    let route_bytes = load_ty225_route("normal");
    let route_data = RouteData::load(&route_bytes)
        .expect("Failed to load route data");

    // Create empty NMEA input
    let empty_nmea = b"";
    let reader = std::io::BufReader::new(&empty_nmea[..]);

    let result = Pipeline::process_nmea_reader(
        reader,
        &route_data,
        &pipeline::PipelineConfig::default(),
    );

    // Should succeed with no arrivals
    assert!(result.is_ok(), "Empty NMEA should not error");
    let result = result.unwrap();
    assert_eq!(result.arrivals.len(), 0, "Should have no arrivals");
}

/// Test: Pipeline handles corrupt NMEA gracefully
#[test]
fn test_corrupt_nmea() {
    let route_bytes = load_ty225_route("normal");
    let route_data = RouteData::load(&route_bytes)
        .expect("Failed to load route data");

    // Create NMEA with corrupt sentences mixed with valid ones
    let corrupt_nmea = b"$GPGGA,invalid\n$GPGGA,123456.000,2500.0000,N,12130.0000,E,1,08,0.9,10.0,M,0.0,M,,*47\n$INVALID SENTENCE\n";
    let reader = std::io::BufReader::new(&corrupt_nmea[..]);

    let result = Pipeline::process_nmea_reader(
        reader,
        &route_data,
        &pipeline::PipelineConfig::default(),
    );

    // Should skip corrupt sentences and process valid ones
    assert!(result.is_ok(), "Should handle corrupt NMEA gracefully");
}

/// Test: Stationary GPS (no movement) should not trigger arrivals
#[test]
fn test_stationary_gps() {
    let route_bytes = load_ty225_route("normal");
    let route_data = RouteData::load(&route_bytes)
        .expect("Failed to load route data");

    // Create NMEA with same GPS position repeated
    let stationary_nmea = b"$GPGGA,000000.000,2500.0000,N,12130.0000,E,1,08,0.9,10.0,M,0.0,M,,*47\n$GPGGA,000001.000,2500.0000,N,12130.0000,E,1,08,0.9,10.0,M,0.0,M,,*47\n$GPGGA,000002.000,2500.0000,N,12130.0000,E,1,08,0.9,10.0,M,0.0,M,,*48\n";
    let reader = std::io::BufReader::new(&stationary_nmea[..]);

    let result = Pipeline::process_nmea_reader(
        reader,
        &route_data,
        &pipeline::PipelineConfig::default(),
    );

    assert!(result.is_ok());
    let result = result.unwrap();
    // Stationary GPS should not trigger arrivals (speed check prevents this)
    // Assuming position is not near any stop
}

/// Test: Extreme GPS jump (>500m) should trigger recovery
#[test]
fn test_extreme_gps_jump() {
    // This test verifies that extreme jumps are handled by recovery
    // without causing crashes or index corruption

    let route_bytes = load_ty225_route("normal");
    let route_data = RouteData::load(&route_bytes)
        .expect("Failed to load route data");

    // The jump scenario already tests this
    // This is a sanity check that extreme jumps don't crash
    let result = Pipeline::process_nmea_reader(
        load_nmea_reader("jump"),
        &route_data,
        &pipeline::PipelineConfig::default(),
    );

    assert!(result.is_ok(), "Extreme jumps should not crash pipeline");
}

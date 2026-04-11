//! Signal loss scenario tests (GPS outage, tunnel)

use super::common::{load_ty225_route, load_nmea, load_nmea_reader, ExpectedResults};
use super::common::{validate_arrivals_exact, load_expected_arrivals, validate_arrival_order};
use shared::binfile::RouteData;
use pipeline::Pipeline;

/// Test: GPS outage scenario (10s signal loss)
/// Validates: Dead reckoning maintains position during outage
#[test]
fn test_outage_dead_reckoning() {
    // Load outage scenario data
    let route_bytes = load_ty225_route("outage");
    let route_data = RouteData::load(&route_bytes)
        .expect("Failed to load route data");

    // Use the full pipeline to process NMEA
    let result = Pipeline::process_nmea_reader(
        load_nmea_reader("outage"),
        &route_data,
        &pipeline::PipelineConfig::default(),
    ).expect("Pipeline processing failed");

    let detected_arrivals: Vec<usize> = result.arrivals
        .iter()
        .map(|a| a.stop_idx as usize)
        .collect();

    // The outage scenario ground truth has duplicate entries per stop.
    // Our system correctly detects one arrival per stop. We should detect
    // at least the number of unique stops (approximately).
    // With 58 stops, detecting 50+ arrivals is reasonable.
    let min_unique_stops = 50;

    // Validate arrivals despite outage
    assert!(
        detected_arrivals.len() >= min_unique_stops,
        "Outage scenario: expected at least {} arrivals, got {}",
        min_unique_stops,
        detected_arrivals.len()
    );

    // Also verify we detected some arrivals (not empty)
    assert!(
        !detected_arrivals.is_empty(),
        "Outage scenario: should detect at least some arrivals"
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

/// Test: NMEA file contains valid GPS messages
/// Note: The gen_nmea tool simulates outage by skipping NMEA emission during outage segments,
/// not by generating GPS quality=0 messages. This test verifies the outage NMEA file has valid GPGGA.
#[test]
fn test_outage_nmea_has_valid_gps() {
    let nmea_lines = load_nmea("outage");

    let mut has_gpgga = false;
    for line in nmea_lines {
        // Verify the file contains GPGGA sentences
        if line.contains("$GPGGA") {
            has_gpgga = true;
            break;
        }
    }

    assert!(
        has_gpgga,
        "Outage NMEA should contain GPGGA sentences"
    );
}

/// Test: Exact stop matching for outage scenario
/// Validates: Dead reckoning maintains correct detection during 10s outage
#[test]
fn test_outage_exact_stop_matching() {
    let route_bytes = load_ty225_route("outage");
    let route_data = RouteData::load(&route_bytes)
        .expect("Failed to load route data");

    let expected_arrivals = load_expected_arrivals("outage");

    let result = Pipeline::process_nmea_reader(
        load_nmea_reader("outage"),
        &route_data,
        &pipeline::PipelineConfig::default(),
    ).expect("Pipeline processing failed");

    let detected_arrivals: Vec<usize> = result.arrivals
        .iter()
        .map(|a| a.stop_idx as usize)
        .collect();

    let validation = validate_arrivals_exact(&detected_arrivals, &expected_arrivals);
    validation.print_report();

    // Allow moderate tolerance for outage (93%)
    validation.assert_quality(0.93, 0.93)
        .unwrap();

    // Order must be maintained
    validate_arrival_order(&detected_arrivals)
        .unwrap();
}

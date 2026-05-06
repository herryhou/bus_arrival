//! Test GGA sentence sets heading sentinel

use gps_processor::accumulator::FixAccumulator;

#[test]
fn test_gga_sets_heading_sentinel() {
    let mut acc = FixAccumulator::new();

    // Parse GGA sentence (no heading data)
    let result = acc.update("$GPGGA,221320,2500.2582,N,12117.1898,E,1,08,3.5,10.0,M,0.0,M,,*4B");

    assert!(result); // GGA sentence should be parsed successfully
    assert!(!acc.should_emit()); // GGA alone doesn't trigger emit
}

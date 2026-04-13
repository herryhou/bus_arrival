//! Test GGA sentence sets heading sentinel

use gps_processor::nmea::NmeaState;

#[test]
fn test_gga_sets_heading_sentinel() {
    let mut state = NmeaState::new();

    // Parse GGA sentence (no heading data)
    let result =
        state.parse_sentence("$GPGGA,221320,2500.2582,N,12117.1898,E,1,08,3.5,10.0,M,0.0,M,,*4B");

    assert!(result.is_some());
    let point = result.unwrap();
    assert_eq!(point.heading_cdeg, i16::MIN); // Sentinel value
    assert_eq!(point.speed_cms, 0); // GGA doesn't provide speed
}

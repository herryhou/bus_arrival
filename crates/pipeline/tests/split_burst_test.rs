//! Test that split bursts (UART fragmentation) are handled correctly.
//! Ensures accumulator preserves partial data across reads.

use gps_processor::FixAccumulator;

#[test]
fn test_split_burst_emits_once() {
    let mut acc = FixAccumulator::new();

    // Simulate split burst: RMC arrives, then delay, then GGA
    acc.update("$GPRMC,221320,A,2500.2582,N,12117.1898,E,8.4,80.5,141123,,*2E");
    assert!(!acc.should_emit()); // Still accumulating t=01

    // Simulate UART delay...
    acc.update("$GPGGA,221320,2500.2582,N,12117.1898,E,1,08,3.5,10.0,M,0.0,M,,*4B");
    assert!(!acc.should_emit()); // Still same timestamp

    // New timestamp arrives - triggers emission of t=01
    acc.update("$GPRMC,221321,A,2500.2583,N,12117.1899,E,8.5,81.5,141123,,*2F");
    assert!(acc.should_emit());

    let (gps, _) = acc.build().unwrap();
    assert_eq!(gps.timestamp, 22 * 3600 + 13 * 60 + 21); // t=02 (current timestamp)
    assert!(gps.speed_cms.is_some()); // Has RMC data
    assert!(gps.hdop_x10.is_some());  // Has GGA data (from previous burst)
}

#[test]
fn test_timestamp_jump() {
    let mut acc = FixAccumulator::new();

    // t=01
    acc.update("$GPRMC,221320,A,2500.2582,N,12117.1898,E,8.4,80.5,141123,,*2E");
    assert!(!acc.should_emit()); // First timestamp - don't emit

    acc.update("$GPGGA,221320,2500.2582,N,12117.1898,E,1,08,3.5,10.0,M,0.0,M,,*4B");
    assert!(!acc.should_emit()); // Same timestamp - don't emit

    // t=03 (t=02 was lost)
    acc.update("$GPRMC,221323,A,2500.2584,N,12117.1900,E,8.6,82.5,141123,,*2B");
    assert!(acc.should_emit()); // New timestamp - emit

    let (gps, _) = acc.build().unwrap();
    assert_eq!(gps.timestamp, 22 * 3600 + 13 * 60 + 23); // t=03 (current timestamp)

    // Reset and process t=03
    acc.reset();
    acc.update("$GPRMC,221323,A,2500.2584,N,12117.1900,E,8.6,82.5,141123,,*2B");

    // Next timestamp would trigger emission of t=03
}

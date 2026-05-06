//! Regression test using real NMEA data.
//! Verifies exactly one GPS point per second.

use gps_processor::FixAccumulator;
use std::fs::read_to_string;

#[test]
fn test_real_nmea_burst_one_tick_per_second() {
    let nmea_data = read_to_string("../../test_data/tpF805_normal_nmea.txt")
        .expect("Test NMEA file not found");

    let mut acc = FixAccumulator::new();
    let mut emit_count = 0;
    let mut ticks_per_second = std::collections::HashMap::new();

    for line in nmea_data.lines() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with('$') {
            continue;
        }

        acc.update(&line);

        if acc.should_emit() {
            emit_count += 1;
            if let Some((gps, _)) = acc.build() {
                let ts = gps.timestamp;
                *ticks_per_second.entry(ts).or_insert(0) += 1;
                acc.reset();
            }
        }
    }

    // Verify: exactly one tick per second
    for (ts, count) in &ticks_per_second {
        assert_eq!(
            *count, 1,
            "Timestamp {} emitted {} times, expected 1",
            ts, count
        );
    }

    // Verify: processed multiple seconds
    assert!(emit_count > 10, "Should process at least 10 seconds");
}

// Test consistency between compute_arrival_probability and _adaptive
// Run with: cargo test --package pico2-firmware --features dev

use pico2_firmware::detection::{
    compute_arrival_probability, compute_arrival_probability_adaptive, GpsStatus,
};
use shared::{PositionSignals, Stop};

#[test]
fn test_consistency_standard_weights() {
    let stop = Stop {
        progress_cm: 100_000,
        corridor_start_cm: 90_000,
        corridor_end_cm: 110_000,
    };

    let test_cases = [
        (100_000, 0, 10),   // At stop, zero speed, 10s dwell
        (100_000, 200, 5),  // At stop, moderate speed, 5s dwell
        (95_000, 500, 0),   // Approaching, fast speed, 0s dwell
        (105_000, 100, 15), // Past stop, slow speed, 15s dwell
        (90_000, 800, 3),   // At corridor start, fast
        (110_000, 50, 20),  // At corridor end, very slow
    ];

    for (s_cm, v_cms, dwell_time_s) in test_cases {
        let signals = PositionSignals {
            z_gps_cm: s_cm,
            s_cm,
        };
        let prob_standard =
            compute_arrival_probability(signals, v_cms, &stop, dwell_time_s, GpsStatus::Valid);

        // When next_stop is None or far (>12m), adaptive should use standard weights
        let prob_adaptive_none = compute_arrival_probability_adaptive(
            signals,
            v_cms,
            &stop,
            dwell_time_s,
            GpsStatus::Valid,
            None,
        );

        let far_stop = Stop {
            progress_cm: 150_000, // 50m away (>12m threshold)
            corridor_start_cm: 140_000,
            corridor_end_cm: 160_000,
        };
        let prob_adaptive_far = compute_arrival_probability_adaptive(
            signals,
            v_cms,
            &stop,
            dwell_time_s,
            GpsStatus::Valid,
            Some(&far_stop),
        );

        assert_eq!(
            prob_standard, prob_adaptive_none,
            "Standard vs Adaptive(None) mismatch: s={}, v={}, dwell={}",
            s_cm, v_cms, dwell_time_s
        );

        assert_eq!(
            prob_standard, prob_adaptive_far,
            "Standard vs Adaptive(far) mismatch: s={}, v={}, dwell={}",
            s_cm, v_cms, dwell_time_s
        );
    }
}

#[test]
fn test_consistency_close_stop_differs() {
    let stop = Stop {
        progress_cm: 100_000,
        corridor_start_cm: 90_000,
        corridor_end_cm: 110_000,
    };

    let close_stop = Stop {
        progress_cm: 105_000, // 5m away (<12m threshold)
        corridor_start_cm: 95_000,
        corridor_end_cm: 115_000,
    };

    let test_cases = [
        (100_000, 0, 10),  // At stop, zero speed, 10s dwell
        (100_000, 200, 5), // At stop, moderate speed, 5s dwell
        (95_000, 500, 0),  // Approaching, fast speed, 0s dwell
    ];

    for (s_cm, v_cms, dwell_time_s) in test_cases {
        let signals = PositionSignals {
            z_gps_cm: s_cm,
            s_cm,
        };
        let prob_standard =
            compute_arrival_probability(signals, v_cms, &stop, dwell_time_s, GpsStatus::Valid);
        let prob_adaptive_close = compute_arrival_probability_adaptive(
            signals,
            v_cms,
            &stop,
            dwell_time_s,
            GpsStatus::Valid,
            Some(&close_stop),
        );

        // When next stop is close, adaptive should use different weights (14,7,11,0)
        // so results should generally differ from standard (13,6,10,3)
        // The only exception is when dwell_time_s = 0, where p4=0 anyway
        if dwell_time_s > 0 {
            assert_ne!(
                prob_standard, prob_adaptive_close,
                "Standard vs Adaptive(close) should differ when dwell_time={}: s={}, v={}",
                dwell_time_s, s_cm, v_cms
            );
        }
    }
}

// Tests for adaptive weights in arrival probability computation
// Run with: cargo test --package pico2-firmware --features dev

use pico2_firmware::detection::compute_arrival_probability_adaptive;
use shared::Stop;

#[test]
fn test_adaptive_weights_close_stop() {
    let stop_current = Stop {
        progress_cm: 100_000,
        corridor_start_cm: 90_000,
        corridor_end_cm: 110_000,
    };

    let stop_next = Stop {
        progress_cm: 108_000, // 8,000cm away (<12,000 threshold)
        corridor_start_cm: 98_000,
        corridor_end_cm: 118_000,
    };

    let prob = compute_arrival_probability_adaptive(
        100_000,  // s_cm (at stop)
        600,      // v_cms (approaching)
        &stop_current,
        5,        // dwell_time_s
        Some(&stop_next),
    );

    assert!(prob > 190, "Expected probability > 190 for close stop, got {}", prob);
}

#[test]
fn test_adaptive_weights_normal_stop() {
    let stop_current = Stop {
        progress_cm: 100_000,
        corridor_start_cm: 90_000,
        corridor_end_cm: 110_000,
    };

    let stop_next = Stop {
        progress_cm: 125_000, // 25,000cm away (>12,000 threshold)
        corridor_start_cm: 115_000,
        corridor_end_cm: 135_000,
    };

    let prob = compute_arrival_probability_adaptive(
        100_000, 600, &stop_current, 5, Some(&stop_next)
    );

    // Normal stop: verify computation succeeds
    assert!(prob < 255); // High probability but not max (not at stop with zero speed)
}

#[test]
fn test_adaptive_weights_last_stop() {
    let stop = Stop {
        progress_cm: 100_000,
        corridor_start_cm: 90_000,
        corridor_end_cm: 110_000,
    };

    let prob = compute_arrival_probability_adaptive(
        100_000, 0, &stop, 10, None
    );

    // Last stop: at stop with zero speed and 10s dwell should have high probability
    assert!(prob > 150);
}

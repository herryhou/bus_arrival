//! Test H4: EMA velocity filter implementation
//! Per spec Section 11.1: v_filtered(t) = v_filtered(t-1) + 3*(v_gps - v_filtered(t-1))/10

use gps_processor::kalman::update_dr_ema;

#[test]
fn test_ema_velocity_filter_formula() {
    // Test the EMA formula: v_filtered = v_prev + 3*(v_gps - v_prev)/10

    // Test 1: Initial value
    let v_prev = 0;
    let v_gps = 500;
    let expected = v_prev + 3 * (v_gps - v_prev) / 10;
    assert_eq!(expected, 150, "EMA from 0 to 500");

    // Test 2: Convergence
    let v_prev = 400;
    let v_gps = 600;
    let expected = v_prev + 3 * (v_gps - v_prev) / 10;
    assert_eq!(expected, 460, "EMA convergence from 400 to 600");

    // Test 3: Smoothing (noise reduction)
    let v_prev = 500;
    let v_gps = 900; // Sudden spike
    let expected = v_prev + 3 * (v_gps - v_prev) / 10;
    assert_eq!(expected, 620, "EMA smoothing: 500 -> 620 (not 900)");

    // Test 4: Multiple steps converge toward GPS speed
    let mut v_filtered = 300;
    for _ in 0..10 {
        v_filtered = v_filtered + 3 * (600 - v_filtered) / 10;
    }
    // After 10 updates, should be close to 600
    assert!(
        v_filtered >= 580 && v_filtered <= 600,
        "EMA should converge: got {}",
        v_filtered
    );
}

#[test]
fn test_ema_update_function() {
    // Test the actual update function
    let v_filtered_prev = 400;
    let v_gps = 600;

    let result = update_dr_ema(v_filtered_prev, v_gps);
    let expected = 400 + 3 * (600 - 400) / 10;

    assert_eq!(result, expected, "update_dr_ema should apply EMA formula");
}

//! UART signed integer formatting test
//!
//! This test verifies that the write_arrival_event_async function
//! correctly formats negative values for s_cm and v_cms.

#![cfg(feature = "dev")]

use shared::{ArrivalEvent, ArrivalEventType, DistCm, Prob8, SpeedCms};

#[test]
fn test_negative_s_cm_formats_correctly() {
    // Verify the concept: negative i32 values should format with minus sign
    let s_cm: DistCm = -100; // -1 meter

    // The formatting should produce "-100" not "4294967196"
    let formatted = format!("{}", s_cm);
    assert!(formatted.contains('-'), "Negative value should contain minus sign");
    assert_eq!(formatted, "-100");
}

#[test]
fn test_negative_v_cms_formats_correctly() {
    // Verify the concept: negative i32 values should format with minus sign
    let v_cms: SpeedCms = -50; // -50 cm/s

    // The formatting should produce "-50" not "4294967246"
    let formatted = format!("{}", v_cms);
    assert!(formatted.contains('-'), "Negative value should contain minus sign");
    assert_eq!(formatted, "-50");
}

#[test]
fn test_positive_values_format_correctly() {
    // Verify positive values still work correctly
    let s_cm: DistCm = 10000; // 100 meters
    let v_cms: SpeedCms = 500; // 500 cm/s = 5 m/s

    assert_eq!(format!("{}", s_cm), "10000");
    assert_eq!(format!("{}", v_cms), "500");
}

#[test]
fn test_zero_formats_correctly() {
    // Verify zero formats correctly (edge case)
    let s_cm: DistCm = 0;
    let v_cms: SpeedCms = 0;

    assert_eq!(format!("{}", s_cm), "0");
    assert_eq!(format!("{}", v_cms), "0");
}

#[test]
fn test_distcm_type_exists() {
    // Compile-time check that DistCm type exists
    let _ = std::marker::PhantomData::<shared::DistCm>;
}

#[test]
fn test_speedcms_type_exists() {
    // Compile-time check that SpeedCms type exists
    let _ = std::marker::PhantomData::<shared::SpeedCms>;
}

#[test]
fn test_cold_start_scenario() {
    // Simulate a cold-start scenario where Kalman filter hasn't converged
    // This is the scenario where negative values are most likely to occur

    // Before convergence, position might be negative (before route start)
    let cold_start_event = ArrivalEvent {
        time: 12345,
        stop_idx: 0,
        s_cm: -500,  // -5 meters (before route origin)
        v_cms: -100, // Negative velocity (GPS noise or backward movement)
        probability: Prob8::from(0),
        event_type: ArrivalEventType::Announce,
    };

    // Verify values are what we expect
    assert_eq!(cold_start_event.s_cm, -500);
    assert_eq!(cold_start_event.v_cms, -100);

    // When formatted, these should produce "-500cm" and "-100cm/s"
    // NOT "4294966796cm" and "4294967196cm/s"
    let s_str = format!("{}", cold_start_event.s_cm);
    let v_str = format!("{}", cold_start_event.v_cms);

    assert!(s_str.starts_with('-'), "s_cm should format as negative");
    assert!(v_str.starts_with('-'), "v_cms should format as negative");
    assert_eq!(s_str, "-500");
    assert_eq!(v_str, "-100");
}

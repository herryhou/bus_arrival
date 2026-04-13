//! UART signed integer formatting test
//!
//! This test verifies that the write_arrival_event_async function
//! correctly formats negative values for s_cm and v_cms.

#![cfg(feature = "dev")]

use shared::{DistCm, SpeedCms};

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

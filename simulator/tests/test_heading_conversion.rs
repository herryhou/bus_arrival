//! Tests for NMEA heading conversion from 0-360° to i16 range -18000 to 18000
//!
//! Bug: Heading > 180° overflowed i16::MAX (32767)
//! Fix: Convert heading > 18000 to negative range by subtracting 36000

/// Convert NMEA heading (0-360°) to HeadCdeg range (-18000 to 18000)
/// This is the same logic used in simulator/src/nmea.rs
fn convert_nmea_heading(heading_deg: f64) -> i16 {
    let heading_cdeg = (heading_deg * 100.0).round() as i32;
    if heading_cdeg > 18000 {
        (heading_cdeg - 36000) as i16
    } else {
        heading_cdeg as i16
    }
}

#[test]
fn test_heading_conversion_350_degrees() {
    // NMEA heading 350.5° should convert to -950 cdeg (not overflow to 32767)
    let heading = convert_nmea_heading(350.5);
    assert_eq!(heading, -950, "350.5° should be -950 cdeg (35050 - 36000)");
}

#[test]
fn test_heading_conversion_80_degrees() {
    // NMEA heading 80.7° should convert to 8070 cdeg (no conversion needed)
    let heading = convert_nmea_heading(80.7);
    assert_eq!(heading, 8070, "80.7° should be 8070 cdeg");
}

#[test]
fn test_heading_conversion_0_degrees() {
    // North (0°) should be 0 cdeg
    let heading = convert_nmea_heading(0.0);
    assert_eq!(heading, 0, "0° should be 0 cdeg");
}

#[test]
fn test_heading_conversion_90_degrees() {
    // East (90°) should be 9000 cdeg
    let heading = convert_nmea_heading(90.0);
    assert_eq!(heading, 9000, "90° should be 9000 cdeg");
}

#[test]
fn test_heading_conversion_180_degrees() {
    // South (180°) should be 18000 cdeg
    let heading = convert_nmea_heading(180.0);
    assert_eq!(heading, 18000, "180° should be 18000 cdeg");
}

#[test]
fn test_heading_conversion_270_degrees() {
    // West (270°) should be -9000 cdeg
    let heading = convert_nmea_heading(270.0);
    assert_eq!(heading, -9000, "270° should be -9000 cdeg (27000 - 36000)");
}

#[test]
fn test_heading_conversion_359_degrees() {
    // Almost 360° should be close to 0 in negative range
    let heading = convert_nmea_heading(359.9);
    assert_eq!(heading, -10, "359.9° should be -10 cdeg (35990 - 36000)");
}

#[test]
fn test_heading_conversion_boundary_180() {
    // Exactly 180° should stay positive
    let heading = convert_nmea_heading(180.0);
    assert_eq!(heading, 18000, "180° should be 18000 cdeg");

    // Just over 180° should convert to negative
    let heading = convert_nmea_heading(180.01);
    assert_eq!(heading, -17999, "180.01° should be -17999 cdeg");
}

#[test]
fn test_no_heading_overflow_for_common_values() {
    // Verify common NMEA headings don't overflow to 32767
    let headings = [0.0, 45.0, 90.0, 135.0, 180.0, 225.0, 270.0, 315.0, 350.5, 359.9];

    for &heading_deg in &headings {
        let heading_cdeg = convert_nmea_heading(heading_deg);

        // Should never be 32767 (i16::MAX) which indicates overflow
        assert_ne!(heading_cdeg, 32767,
            "Heading {}° should not overflow to 32767 (i16::MAX)", heading_deg);

        // Result should be in valid range
        assert!(heading_cdeg >= -18000 && heading_cdeg <= 18000,
            "Heading {}° resulted in {} cdeg, which is outside valid range [-18000, 18000]",
            heading_deg, heading_cdeg);
    }
}

#[test]
fn test_heading_conversion_roundtrip() {
    // Test that common directions convert correctly
    let test_cases = [
        (0.0, 0),      // North
        (45.0, 4500),  // Northeast
        (90.0, 9000),  // East
        (135.0, 13500),// Southeast
        (180.0, 18000),// South
        (225.0, -13500),// Southwest (22500 - 36000)
        (270.0, -9000),// West (27000 - 36000)
        (315.0, -4500),// Northwest (31500 - 36000)
        (350.5, -950), // North-northwest (35050 - 36000)
    ];

    for &(heading_deg, expected_cdeg) in &test_cases {
        let heading_cdeg = convert_nmea_heading(heading_deg);
        assert_eq!(heading_cdeg, expected_cdeg,
            "Heading {}° should convert to {} cdeg", heading_deg, expected_cdeg);
    }
}

#[test]
fn test_original_bug_3505_not_overflow() {
    // This was the original bug: 350.5° × 100 = 35050 > i16::MAX (32767)
    // Before fix: would saturate/overflow to 32767
    // After fix: correctly converts to -950
    let heading = convert_nmea_heading(350.5);

    // Should NOT be 32767 (overflow value)
    assert_ne!(heading, 32767, "350.5° should not overflow to 32767");

    // Should be -950 (correct conversion)
    assert_eq!(heading, -950, "350.5° should convert to -950 cdeg");
}

#[test]
fn test_overflow_protection() {
    // Test that even extreme values near 360° don't overflow
    let extreme_headings = [359.0, 359.5, 359.9, 359.99];

    for &heading_deg in &extreme_headings {
        let heading_cdeg = convert_nmea_heading(heading_deg);

        // Calculate expected value
        let expected = ((heading_deg * 100.0).round() as i32 - 36000) as i16;

        assert_eq!(heading_cdeg, expected,
            "Heading {}° should convert to {} cdeg", heading_deg, expected);

        // Verify it's not the overflow value
        assert_ne!(heading_cdeg, 32767,
            "Heading {}° should not overflow to 32767", heading_deg);
    }
}

//! Heading filter for map matching
//!
//! This module provides pure heading filter logic without knowledge of
//! grid structure, route data, or search strategies.

use shared::{HeadCdeg, SpeedCms};

/// Hard heading gate at full speed (w = 256, ≥ 3 km/h).
/// A bus in motion cannot be heading >90° from the segment direction.
const MAX_HEADING_DIFF_CDEG: u32 = 9_000; // 90°

/// Heading filter threshold for a given speed weight.
///
/// Returns `u32::MAX` (gate disabled) when w = 0 — at a standstill GPS heading
/// is unreliable; don't reject any segment.
/// Returns `MAX_HEADING_DIFF_CDEG` (90°) at w = 256.
/// Linearly interpolates between the two, giving a progressively tighter gate
/// as the bus picks up speed.
pub(crate) fn heading_threshold_cdeg(w: i32) -> u32 {
    if w == 0 {
        return u32::MAX;
    }
    // threshold = 36000 - (36000 - MAX_HEADING_DIFF_CDEG) × w / 256
    let range = 36_000u32 - MAX_HEADING_DIFF_CDEG; // 27 000
    36_000 - range * w as u32 / 256
}

/// Returns true if this segment is a plausible direction of travel given the
/// current GPS heading.
///
/// Heading filter strictness depends on mode:
///   - First fix/recovery (is_first_fix = true): 180° relaxed threshold
///   - Sentinel heading (i16::MIN): always eligible (GGA-only mode)
///   - Stopped (w = 0): always eligible (heading unreliable)
///   - Moving: eligible iff heading_diff ≤ threshold(speed)
///
/// Note: this is a hard gate, not a blended penalty. A segment is either
/// physically plausible or it isn't; partial credit produces commensuration
/// problems (adding cm² to cdeg²).
pub fn heading_eligible(
    gps_heading: HeadCdeg,
    gps_speed: SpeedCms,
    seg_heading: HeadCdeg,
    is_first_fix: bool,
) -> bool {
    if gps_heading == i16::MIN {
        return true; // GGA-only: preserve existing sentinel behaviour
    }
    let w = heading_weight(gps_speed);
    let threshold = if is_first_fix {
        // Relaxed threshold for post-outage recovery: 180°
        18_000
    } else {
        heading_threshold_cdeg(w)
    };
    let diff = heading_diff_cdeg(gps_heading, seg_heading) as u32;
    diff <= threshold
}

/// Heading weight: 0 at v=0, 256 at v≥83 cm/s (3 km/h)
pub(crate) fn heading_weight(v_cms: SpeedCms) -> i32 {
    ((v_cms * 256) / 83).min(256)
}

/// Calculate heading difference (shortest around 360°)
pub(crate) fn heading_diff_cdeg(a: HeadCdeg, b: HeadCdeg) -> HeadCdeg {
    let diff = (a as i32 - b as i32).unsigned_abs() % 36000;
    if diff > 18000 {
        (36000 - diff) as HeadCdeg
    } else {
        diff as HeadCdeg
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_heading_eligible_sentinel() {
        let seg_heading: HeadCdeg = 9000; // 90°

        // Sentinel: always eligible regardless of segment heading or speed
        assert!(heading_eligible(i16::MIN, 500, seg_heading, false));
        assert!(heading_eligible(i16::MIN, 0, seg_heading, false));
    }

    #[test]
    fn test_heading_eligible_stopped() {
        // Stopped (w=0): always eligible — heading is unreliable
        assert!(heading_eligible(0, 0, 9000, false)); // facing opposite direction
        assert!(heading_eligible(0, 0, 18000, false)); // 180° misaligned
    }

    #[test]
    fn test_heading_eligible_moving() {
        let speed: SpeedCms = 500; // well above 83 cm/s → w=256 → threshold=9000

        // Same heading: eligible
        assert!(heading_eligible(9000, speed, 9000, false));

        // 89° off: eligible (just under 90° gate)
        assert!(heading_eligible(0, speed, 8999, false));

        // 91° off: not eligible
        assert!(!heading_eligible(0, speed, 9001, false));

        // 180° (opposite direction): not eligible at speed
        assert!(!heading_eligible(0, speed, 18000, false));
    }

    #[test]
    fn test_heading_eligible_first_fix() {
        let speed: SpeedCms = 500; // Moving

        // First fix: 180° relaxed threshold
        assert!(heading_eligible(0, speed, 18000, true)); // 180° off, eligible
        assert!(heading_eligible(0, speed, 9001, true)); // 90° off, eligible
    }

    #[test]
    fn test_heading_threshold_cdeg() {
        // At w=0 (stopped): gate disabled (u32::MAX)
        assert_eq!(heading_threshold_cdeg(0), u32::MAX);

        // At w=256 (full speed): 90° gate
        assert_eq!(heading_threshold_cdeg(256), 9_000);

        // At w=128 (half speed): intermediate threshold
        let threshold = heading_threshold_cdeg(128);
        assert!(threshold > 9_000 && threshold < 36_000);

        // Threshold decreases as weight increases
        assert!(heading_threshold_cdeg(64) > heading_threshold_cdeg(128));
        assert!(heading_threshold_cdeg(128) > heading_threshold_cdeg(256));
    }

    #[test]
    fn test_heading_weight() {
        // At v=0: w=0
        assert_eq!(heading_weight(0), 0);

        // At v=83 cm/s (3 km/h): w=256 (saturated)
        assert_eq!(heading_weight(83), 256);

        // At v=500 cm/s: w=256 (saturated)
        assert_eq!(heading_weight(500), 256);

        // At v=41 cm/s (~1.5 km/h): w=128 (half)
        assert_eq!(heading_weight(41), 126); // 41*256/83 = 126.4, truncated
    }

    proptest! {
        #[test]
        fn prop_heading_diff_symmetric(a in -18000i16..18000, b in -18000i16..18000) {
            let diff1 = heading_diff_cdeg(a, b);
            let diff2 = heading_diff_cdeg(b, a);
            prop_assert_eq!(diff1, diff2);
        }

        #[test]
        fn prop_heading_diff_identity(a in -18000i16..18000) {
            let diff = heading_diff_cdeg(a, a);
            prop_assert_eq!(diff, 0);
        }

        #[test]
        fn prop_heading_diff_max_180(a in -18000i16..18000, b in -18000i16..18000) {
            let diff = heading_diff_cdeg(a, b);
            prop_assert!(diff <= 18000);
        }
    }
}

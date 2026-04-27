//! GPS jump detection for triggering stop index recovery
//!
//! Implements trigger condition from spec Section 15.1:
//! - GPS jump > 200 m (during operation)
//! - Restart mismatch > 500 m (after restart, requires Flash storage - deferred)
//!
//! For now, implements GPS jump detection using current and previous position.

use shared::DistCm;

/// Check if GPS jump conditions warrant recovery
///
/// Returns true if GPS jump > 200 m (per spec Section 15.1)
/// This detects position jumps during operation.
///
/// Note: Restart mismatch (> 500 m) requires stored position from Flash,
/// which will be implemented later (H2 from code review).
pub fn should_trigger_recovery(s_cm: DistCm, prev_s_cm: DistCm) -> bool {
    // GPS jump > 200 m (per spec Section 15.1)
    // Triggers recovery when position jumps > 200 m between consecutive fixes
    let jump_distance = s_cm.abs_diff(prev_s_cm) as u32;
    jump_distance > 20000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recovery_triggered_by_200m_jump() {
        // Exactly 200 m should NOT trigger
        assert!(!should_trigger_recovery(10000, 30000));
        // 201 m should trigger
        assert!(should_trigger_recovery(10000, 30100));
    }

    #[test]
    fn test_no_recovery_for_small_gps_noise() {
        // Small GPS noise (10 m) should not trigger
        assert!(!should_trigger_recovery(10000, 11000));
    }

    #[test]
    fn test_recovery_boundary_conditions() {
        // 200 m GPS jump triggers (just over the threshold)
        assert!(should_trigger_recovery(10000, 30001));
    }

    #[test]
    fn test_recovery_not_triggered_for_small_jumps() {
        // 199 m should NOT trigger
        assert!(!should_trigger_recovery(10000, 29900));
    }
}

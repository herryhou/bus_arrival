//! Off-route detection using hysteresis to avoid false positives from transient GPS multipath
//!
//! This module implements off-route detection to handle sustained GPS drift where
//! positions consistently don't fit route geometry (distance > 50m for 5+ seconds).
//!
//! This feature detects:
//! - Urban canyon multipath causing GPS drift away from road
//! - Physical deviations (detour, depot, wrong route loaded)
//!
//! LIMITATION: Cannot detect "along-route drift" where GPS stays on the road
//! but advances faster than the bus. This requires external ground truth and
//! is not detectable with a single GPS sensor.
//!
//! The detection uses hysteresis (5 ticks to confirm, 2 to clear) to avoid
//! false positives from transient multipath. Position is frozen during off-route
//! episodes, and recovery re-synchronizes stop indices when GPS returns to route.

use shared::{DistCm, KalmanState};

/// Off-route distance threshold: d=50m → d²=2,500 m² = 25,000,000 cm²
pub const OFF_ROUTE_D2_THRESHOLD: i64 = 25_000_000;

/// Ticks to confirm off-route (avoid false positives from multipath)
pub const OFF_ROUTE_CONFIRM_TICKS: u8 = 5;

/// Ticks to clear off-route (fast re-acquisition)
pub const OFF_ROUTE_CLEAR_TICKS: u8 = 2;

/// Result of off-route hysteresis update
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OffRouteStatus {
    /// Normal operation - GPS is on route
    Normal,
    /// Suspect state - position frozen, awaiting confirmation
    Suspect,
    /// Confirmed off-route - position frozen, recovery needed
    OffRoute,
}

/// Update off-route hysteresis state based on GPS match quality
///
/// Returns the new off-route status after updating the state.
/// Position is frozen immediately on first suspect tick.
pub fn update_off_route_hysteresis(
    state: &mut KalmanState,
    match_d2: i64,
    gps_timestamp: u64,
    current_stop_idx: u8,
) -> OffRouteStatus {
    if match_d2 > OFF_ROUTE_D2_THRESHOLD {
        // Poor GPS match: increment suspect counter
        if state.off_route_suspect_ticks == 0 {
            // First tick of off-route suspect: freeze position immediately
            state.frozen_s_cm = Some(state.s_cm);
            // Record freeze time at the same time position is frozen (Bug 5 fix)
            state.off_route_freeze_time = Some(gps_timestamp);
            // C3: Store freeze context for spatial anchoring
            state.freeze_ctx = Some(shared::FreezeContext {
                frozen_s_cm: state.s_cm,
                frozen_stop_idx: current_stop_idx,
            });
        }
        state.off_route_suspect_ticks = state.off_route_suspect_ticks.saturating_add(1);
        state.off_route_clear_ticks = 0;

        if state.off_route_suspect_ticks >= OFF_ROUTE_CONFIRM_TICKS {
            OffRouteStatus::OffRoute
        } else {
            OffRouteStatus::Suspect
        }
    } else {
        // Good GPS match: increment clear counter
        state.off_route_clear_ticks = state.off_route_clear_ticks.saturating_add(1);

        // Only return Suspect if we're actually in a suspect state
        // (i.e., we had prior suspect ticks or position is frozen)
        let is_actually_suspect = state.off_route_suspect_ticks > 0 || state.frozen_s_cm.is_some();

        // After 2 consecutive good matches, reset suspect counter and return Normal
        // CRITICAL: Do NOT clear frozen_s_cm here - let the snap logic handle it
        // This ensures position stays frozen if snap fails
        if state.off_route_clear_ticks >= OFF_ROUTE_CLEAR_TICKS {
            state.off_route_suspect_ticks = 0;
            // NOTE: frozen_s_cm is NOT cleared here - it will be cleared after successful snap
            // C1: Don't clear off_route_freeze_time here - state.rs needs it for recovery dt calculation
            // It will be cleared after recovery completes in state.rs
            OffRouteStatus::Normal
        } else if is_actually_suspect {
            // Still in suspect state (need more good ticks to clear)
            OffRouteStatus::Suspect
        } else {
            // Never been in suspect state, this is normal operation
            OffRouteStatus::Normal
        }
    }
}

/// Reset all off-route state including suspect/clear ticks, freeze time, and frozen position
/// (called on GPS outage)
pub fn reset_off_route_state(state: &mut KalmanState) {
    state.off_route_suspect_ticks = 0;
    state.off_route_clear_ticks = 0;
    state.off_route_freeze_time = None;
    state.frozen_s_cm = None; // C2 fix: clear frozen position
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== Test Constants =====

    /// Default position for test state (cm)
    const TEST_S_CM: DistCm = 100_000;
    /// Default velocity for test state (cm/s)
    const TEST_V_CMS: i32 = 500;
    /// Good GPS match distance (well below threshold)
    const GOOD_MATCH_D2: i64 = 10_000_000;
    /// Bad GPS match distance (above threshold)
    const BAD_MATCH_D2: i64 = 30_000_000;
    /// Default GPS timestamp for tests
    const TEST_TIMESTAMP: u64 = 1000;

    // ===== Test Helpers =====

    /// Create a fresh KalmanState with default test values
    fn test_state() -> KalmanState {
        KalmanState {
            s_cm: TEST_S_CM,
            v_cms: TEST_V_CMS,
            last_seg_idx: 0,
            off_route_suspect_ticks: 0,
            off_route_clear_ticks: 0,
            frozen_s_cm: None,
            off_route_freeze_time: None,
            freeze_ctx: None,
        }
    }

    /// Create a KalmanState with off-route already confirmed
    fn off_route_state() -> KalmanState {
        KalmanState {
            s_cm: TEST_S_CM,
            v_cms: TEST_V_CMS,
            last_seg_idx: 0,
            off_route_suspect_ticks: 5,
            off_route_clear_ticks: 0,
            frozen_s_cm: Some(TEST_S_CM),
            off_route_freeze_time: Some(1000),
            freeze_ctx: None,
        }
    }

    // ===== Off-Route Hysteresis Tests =====

    #[test]
    fn test_off_route_hysteresis_normal_state() {
        // Good GPS match should stay in Normal state
        let mut state = test_state();

        let status = update_off_route_hysteresis(&mut state, GOOD_MATCH_D2, TEST_TIMESTAMP, 0);

        assert_eq!(status, OffRouteStatus::Normal);
        assert_eq!(state.off_route_suspect_ticks, 0);
        assert_eq!(state.off_route_clear_ticks, 1);
        assert!(state.frozen_s_cm.is_none()); // No freeze on good match
    }

    #[test]
    fn test_off_route_hysteresis_first_suspect_tick() {
        // First poor match should freeze position immediately
        let mut state = test_state();

        let status = update_off_route_hysteresis(&mut state, BAD_MATCH_D2, TEST_TIMESTAMP, 5);

        assert_eq!(status, OffRouteStatus::Suspect);
        assert_eq!(state.off_route_suspect_ticks, 1);
        assert_eq!(state.frozen_s_cm, Some(TEST_S_CM)); // Position frozen
        assert_eq!(state.off_route_freeze_time, Some(TEST_TIMESTAMP)); // Time recorded
        assert_eq!(state.off_route_clear_ticks, 0);
        // Verify freeze context is stored
        assert!(state.freeze_ctx.is_some());
        let ctx = state.freeze_ctx.unwrap();
        assert_eq!(ctx.frozen_s_cm, TEST_S_CM);
        assert_eq!(ctx.frozen_stop_idx, 5);
    }

    #[test]
    fn test_off_route_hysteresis_confirmation() {
        // 5 consecutive poor matches should confirm off-route
        let mut state = test_state();

        // First tick: Suspect
        let status = update_off_route_hysteresis(&mut state, BAD_MATCH_D2, TEST_TIMESTAMP, 0);
        assert_eq!(status, OffRouteStatus::Suspect);
        assert_eq!(state.off_route_suspect_ticks, 1);

        // Ticks 2-4: Still Suspect
        for i in 2..5 {
            let status = update_off_route_hysteresis(&mut state, BAD_MATCH_D2, TEST_TIMESTAMP + i as u64, 0);
            assert_eq!(status, OffRouteStatus::Suspect);
            assert_eq!(state.off_route_suspect_ticks, i);
        }

        // Tick 5: Confirmed OffRoute
        let status = update_off_route_hysteresis(&mut state, BAD_MATCH_D2, TEST_TIMESTAMP + 5, 0);
        assert_eq!(status, OffRouteStatus::OffRoute);
        assert_eq!(state.off_route_suspect_ticks, 5);
    }

    #[test]
    fn test_off_route_hysteresis_clear_after_two_good_matches() {
        // After off-route confirmation, 2 good matches should return to Normal
        let mut state = off_route_state();

        // First good match: Still Suspect (need 2 to clear)
        let status = update_off_route_hysteresis(&mut state, GOOD_MATCH_D2, 2000, 0);
        assert_eq!(status, OffRouteStatus::Suspect);
        assert_eq!(state.off_route_clear_ticks, 1);
        assert_eq!(state.off_route_suspect_ticks, 5); // Not reset yet
        assert!(state.frozen_s_cm.is_some()); // Still frozen

        // Second good match: Normal
        let status = update_off_route_hysteresis(&mut state, GOOD_MATCH_D2, 2001, 0);
        assert_eq!(status, OffRouteStatus::Normal);
        assert_eq!(state.off_route_clear_ticks, 2);
        assert_eq!(state.off_route_suspect_ticks, 0); // Reset
        // NOTE: frozen_s_cm is NOT cleared here - snap logic handles it
        assert!(state.frozen_s_cm.is_some()); // Still frozen until snap succeeds
    }

    #[test]
    fn test_off_route_hysteresis_intermittent_bad_matches() {
        // Intermittent bad matches should reset clear counter
        let mut state = KalmanState {
            s_cm: TEST_S_CM,
            v_cms: TEST_V_CMS,
            last_seg_idx: 0,
            off_route_suspect_ticks: 3,
            off_route_clear_ticks: 0,
            frozen_s_cm: Some(TEST_S_CM),
            off_route_freeze_time: None,
            freeze_ctx: None,
        };

        // Good match: start clearing
        let status = update_off_route_hysteresis(&mut state, GOOD_MATCH_D2, TEST_TIMESTAMP, 0);
        assert_eq!(status, OffRouteStatus::Suspect);
        assert_eq!(state.off_route_clear_ticks, 1);

        // Bad match: clear counter resets, suspect ticks increase
        let status = update_off_route_hysteresis(&mut state, BAD_MATCH_D2, TEST_TIMESTAMP + 1, 0);
        assert_eq!(status, OffRouteStatus::Suspect);
        assert_eq!(state.off_route_clear_ticks, 0); // Reset
        assert_eq!(state.off_route_suspect_ticks, 4); // Increased
    }

    #[test]
    fn test_off_route_hysteresis_at_threshold() {
        // Match distance exactly at threshold should trigger suspect
        let mut state = test_state();

        let status = update_off_route_hysteresis(&mut state, OFF_ROUTE_D2_THRESHOLD, TEST_TIMESTAMP, 0);

        // At threshold, should NOT trigger (condition is >, not >=)
        assert_eq!(status, OffRouteStatus::Normal);
        assert_eq!(state.off_route_suspect_ticks, 0);
    }

    #[test]
    fn test_off_route_hysteresis_just_above_threshold() {
        // Match distance just above threshold should trigger suspect
        let mut state = test_state();

        let status = update_off_route_hysteresis(&mut state, OFF_ROUTE_D2_THRESHOLD + 1, TEST_TIMESTAMP, 0);

        assert_eq!(status, OffRouteStatus::Suspect);
        assert_eq!(state.off_route_suspect_ticks, 1);
        assert!(state.frozen_s_cm.is_some());
    }

    // ===== Reset Off-Route State Tests =====

    #[test]
    fn test_reset_off_route_state_clears_all_counters() {
        let mut state = KalmanState {
            s_cm: TEST_S_CM,
            v_cms: TEST_V_CMS,
            last_seg_idx: 0,
            off_route_suspect_ticks: 3,
            off_route_clear_ticks: 1,
            off_route_freeze_time: Some(TEST_TIMESTAMP),
            frozen_s_cm: Some(50_000),
            freeze_ctx: None,
        };

        reset_off_route_state(&mut state);

        assert_eq!(state.off_route_suspect_ticks, 0);
        assert_eq!(state.off_route_clear_ticks, 0);
        assert_eq!(state.off_route_freeze_time, None);
        assert_eq!(state.frozen_s_cm, None);
    }

    #[test]
    fn test_reset_off_route_state_idempotent() {
        // Resetting already-reset state should be safe
        let mut state = test_state();

        reset_off_route_state(&mut state);
        reset_off_route_state(&mut state);

        assert_eq!(state.off_route_suspect_ticks, 0);
        assert_eq!(state.off_route_clear_ticks, 0);
        assert_eq!(state.off_route_freeze_time, None);
        assert_eq!(state.frozen_s_cm, None);
    }

    // ===== Combined Scenario Tests =====

    #[test]
    fn test_off_route_flow_full_cycle() {
        // Full cycle: Normal → Suspect → OffRoute → Normal
        let mut state = test_state();

        // Start: Normal
        let status = update_off_route_hysteresis(&mut state, GOOD_MATCH_D2, TEST_TIMESTAMP, 0);
        assert_eq!(status, OffRouteStatus::Normal);

        // 5 bad matches: Confirm OffRoute
        for i in 1..=5 {
            let status = update_off_route_hysteresis(&mut state, BAD_MATCH_D2, TEST_TIMESTAMP + i as u64, 0);
            if i < 5 {
                assert_eq!(status, OffRouteStatus::Suspect);
            } else {
                assert_eq!(status, OffRouteStatus::OffRoute);
            }
        }

        // Verify state is frozen
        assert_eq!(state.frozen_s_cm, Some(TEST_S_CM));

        // 2 good matches: Return to Normal
        let status = update_off_route_hysteresis(&mut state, GOOD_MATCH_D2, 2000, 0);
        assert_eq!(status, OffRouteStatus::Suspect); // First good match
        let status = update_off_route_hysteresis(&mut state, GOOD_MATCH_D2, 2001, 0);
        assert_eq!(status, OffRouteStatus::Normal); // Second good match

        // Verify suspect counter reset
        assert_eq!(state.off_route_suspect_ticks, 0);
        // But frozen position remains (until snap logic clears it)
        assert!(state.frozen_s_cm.is_some());
    }
}

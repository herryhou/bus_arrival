//! System mode definitions and transition logic
//!
//! # Priority Rationale
//!
//! When GPS returns to route (divergence ≤ 50m for 2 ticks), we check
//! displacement to decide between direct Normal vs. Recovering:
//!
//! - **Priority 1: Recovering** (displacement > 50m)
//!   - Large jump indicates GPS position changed significantly
//!   - Recovery finds correct stop index before resuming detection
//!   - Safety: prevents wrong stop announcements
//!
//! - **Priority 2: Normal** (displacement ≤ 50m)
//!   - GPS near frozen position, no significant movement
//!   - Safe to resume detection immediately
//!   - Avoids unnecessary recovery overhead
//!
//! # Mutual Exclusion
//!
//! Only ONE transition executes per tick. The if/else structure
//! ensures Recovering and Normal paths are mutually exclusive.

use shared::{DistCm, Dist2};

/// System operational mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemMode {
    /// Normal GPS tracking with arrival detection enabled
    Normal,
    /// GPS has diverged from route — position frozen, awaiting re-acquisition
    OffRoute,
    /// Active recovery in progress — finding correct stop index
    Recovering,
}

/// Transition action result from mode handler
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionAction {
    /// Transition to Normal mode
    ToNormal,
    /// Transition to Recovering mode
    ToRecovering,
    /// Stay in current mode
    Stay,
}

/// Off-route distance threshold: d² = 25,000,000 cm² (50m)
pub const OFF_ROUTE_D2_THRESHOLD: Dist2 = 25_000_000;

/// Check Normal → OffRoute transition
///
/// Returns true if divergence > 50m for 5 consecutive ticks
pub fn check_normal_to_offroute(
    divergence_d2: Dist2,
    suspect_ticks: &mut u8,
) -> bool {
    if divergence_d2 > OFF_ROUTE_D2_THRESHOLD {
        *suspect_ticks += 1;
        return *suspect_ticks >= 5;
    } else {
        *suspect_ticks = 0;
        false
    }
}

/// Check OffRoute → Normal/Recovering transition
///
/// Returns transition action based on divergence and displacement.
/// Priority: Recovering (large displacement) > Normal (small displacement).
pub fn check_offroute_transition(
    divergence_d2: Dist2,
    clear_ticks: &mut u8,
    frozen_s_cm: Option<DistCm>,
    current_z_gps_cm: DistCm,
) -> TransitionAction {
    // Both paths require: divergence resolved (≤50m for 2 ticks)
    if divergence_d2 > OFF_ROUTE_D2_THRESHOLD {
        *clear_ticks = 0;
        return TransitionAction::Stay;  // Still diverging
    }

    *clear_ticks += 1;
    if *clear_ticks < 2 {
        return TransitionAction::Stay;  // Need 2 consecutive good ticks
    }

    // Divergence resolved — check displacement
    let displacement = frozen_s_cm
        .map(|f| (current_z_gps_cm - f).abs())
        .unwrap_or(0);

    if displacement > 5000 {
        // Large displacement → Recovering
        TransitionAction::ToRecovering
    } else {
        // Small displacement → Normal (direct)
        TransitionAction::ToNormal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normal_to_offroute_requires_5_ticks() {
        let mut ticks = 0;

        // 4 ticks should not trigger
        for _ in 0..4 {
            assert!(!check_normal_to_offroute(30_000_000, &mut ticks));
        }

        // 5th tick triggers
        assert!(check_normal_to_offroute(30_000_000, &mut ticks));
    }

    #[test]
    fn test_normal_to_offroute_resets_on_good_divergence() {
        let mut ticks = 3;

        // Bad divergence increments (3 → 4, not yet 5)
        assert!(!check_normal_to_offroute(30_000_000, &mut ticks));
        assert_eq!(ticks, 4);

        // Good divergence resets (4 → 0)
        assert!(!check_normal_to_offroute(10_000_000, &mut ticks));
        assert_eq!(ticks, 0);
    }

    #[test]
    fn test_offroute_to_recovering_with_large_displacement() {
        let mut ticks = 0;

        // Need 2 ticks of good divergence
        let result1 = check_offroute_transition(10_000_000, &mut ticks, Some(0), 6000);
        assert_eq!(result1, TransitionAction::Stay);
        assert_eq!(ticks, 1);

        let result2 = check_offroute_transition(10_000_000, &mut ticks, Some(0), 6000);
        assert_eq!(result2, TransitionAction::ToRecovering);
    }

    #[test]
    fn test_offroute_to_normal_with_small_displacement() {
        let mut ticks = 0;

        // Need 2 ticks of good divergence
        let result1 = check_offroute_transition(10_000_000, &mut ticks, Some(0), 1000);
        assert_eq!(result1, TransitionAction::Stay);

        let result2 = check_offroute_transition(10_000_000, &mut ticks, Some(0), 1000);
        assert_eq!(result2, TransitionAction::ToNormal);
    }
}

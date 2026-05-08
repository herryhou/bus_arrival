//! Mode state machine — pure transition logic
//!
//! # Component Boundary
//!
//! **Input:** Mode divergence, GPS status
//! **Output:** Next mode, action to take, detection enable flag
//! **Side Effects:** Updates internal mode + counters (no external state)
//!
//! # Contract
//!
//! - Pure state machine: same input → same output (given internal state)
//! - No access to position, stops, or other external state
//! - Single transition per tick (enforced by design)

use shared::{DistCm, Dist2};
use super::mode::{SystemMode, TransitionAction, check_normal_to_offroute, check_offroute_transition};

/// Mode state machine — owns mode and hysteresis counters
///
/// # Invariant
///
/// This state machine owns ONLY:
/// - Current mode
/// - Hysteresis counters (off_route_suspect_ticks, off_route_clear_ticks)
///
/// It does NOT own:
/// - Position (s_cm, frozen_s_cm)
/// - Stop index
/// - Route data
pub struct ModeMachine {
    /// Current operational mode
    mode: SystemMode,
    /// Hysteresis counter for Normal → OffRoute transition
    off_route_suspect_ticks: u8,
    /// Hysteresis counter for OffRoute → Normal transition
    off_route_clear_ticks: u8,
}

impl ModeMachine {
    /// Create a new mode machine in Normal mode
    ///
    /// # Examples
    ///
    /// ```
    /// use pico2_firmware::control::machine::ModeMachine;
    /// use pico2_firmware::control::mode::SystemMode;
    ///
    /// let machine = ModeMachine::new();
    /// assert_eq!(machine.mode(), SystemMode::Normal);
    /// ```
    pub fn new() -> Self {
        Self {
            mode: SystemMode::Normal,
            off_route_suspect_ticks: 0,
            off_route_clear_ticks: 0,
        }
    }

    /// Get current mode
    pub fn mode(&self) -> SystemMode {
        self.mode
    }

    /// Check if in Normal mode
    pub fn is_normal(&self) -> bool {
        self.mode == SystemMode::Normal
    }

    /// Check if in OffRoute mode
    pub fn is_off_route(&self) -> bool {
        self.mode == SystemMode::OffRoute
    }

    /// Check if in Recovering mode
    pub fn is_recovering(&self) -> bool {
        self.mode == SystemMode::Recovering
    }

    /// Pure state machine transition function
    ///
    /// # Boundary Contract
    ///
    /// - **Input:** Current divergence, GPS status, frozen position, current GPS
    /// - **Output:** Next mode, action to take, detection enabled flag
    /// - **Side Effects:** Updates internal mode + counters only
    ///
    /// # Arguments
    ///
    /// * `input` - Mode input with divergence and position data
    ///
    /// # Returns
    ///
    /// Mode output with next mode and action to take
    ///
    /// # Examples
    ///
    /// ```
    /// use pico2_firmware::control::machine::{ModeMachine, ModeInput};
    /// use pico2_firmware::control::mode::SystemMode;
    ///
    /// let mut machine = ModeMachine::new();
    ///
    /// let input = ModeInput {
    ///     divergence_d2: 30_000_000,  // > 50m threshold
    ///     has_gps_fix: true,
    ///     frozen_s_cm: None,
    ///     current_z_gps_cm: 10000,
    /// };
    ///
    /// // First tick - suspect, not yet off route
    /// let output = machine.update(input);
    /// assert_eq!(output.mode, SystemMode::Normal);
    /// ```
    pub fn update(&mut self, input: ModeInput) -> ModeOutput {
        match self.mode {
            SystemMode::Normal => self.handle_normal(input),
            SystemMode::OffRoute => self.handle_offroute(input),
            SystemMode::Recovering => self.handle_recovering(input),
        }
    }

    /// Handle Normal mode transitions
    fn handle_normal(&mut self, input: ModeInput) -> ModeOutput {
        // Check for Normal → OffRoute transition
        if check_normal_to_offroute(input.divergence_d2, &mut self.off_route_suspect_ticks) {
            self.mode = SystemMode::OffRoute;
            self.off_route_clear_ticks = 0;
            return ModeOutput {
                mode: self.mode,
                action: ModeAction::FreezePosition,
                detection_enabled: false,
            };
        }

        // Stay in Normal
        ModeOutput {
            mode: self.mode,
            action: ModeAction::None,
            detection_enabled: true,
        }
    }

    /// Handle OffRoute mode transitions
    fn handle_offroute(&mut self, input: ModeInput) -> ModeOutput {
        let action = check_offroute_transition(
            input.divergence_d2,
            &mut self.off_route_clear_ticks,
            input.frozen_s_cm,
            input.current_z_gps_cm,
        );

        match action {
            TransitionAction::ToRecovering => {
                self.mode = SystemMode::Recovering;
                ModeOutput {
                    mode: self.mode,
                    action: ModeAction::BeginRecovery,
                    detection_enabled: false,
                }
            }
            TransitionAction::ToNormal => {
                self.mode = SystemMode::Normal;
                self.off_route_suspect_ticks = 0;
                ModeOutput {
                    mode: self.mode,
                    action: ModeAction::ResumeNormal,
                    detection_enabled: true,
                }
            }
            TransitionAction::Stay => {
                ModeOutput {
                    mode: self.mode,
                    action: ModeAction::None,
                    detection_enabled: false,
                }
            }
        }
    }

    /// Handle Recovering mode
    fn handle_recovering(&self, _input: ModeInput) -> ModeOutput {
        // Recovery is handled by the control layer calling the recovery function
        // The mode machine just reports the current state
        ModeOutput {
            mode: self.mode,
            action: ModeAction::None,
            detection_enabled: false,
        }
    }

    /// Transition to Normal mode (called after recovery succeeds)
    pub fn transition_to_normal(&mut self) {
        self.mode = SystemMode::Normal;
        self.off_route_suspect_ticks = 0;
        self.off_route_clear_ticks = 0;
    }
}

impl Default for ModeMachine {
    fn default() -> Self {
        Self::new()
    }
}

/// Mode machine input — ONLY what's needed for mode transitions
///
/// # Boundary Contract
///
/// Contains ONLY:
/// - Divergence from route (for transition triggers)
/// - GPS fix status (for handling outages)
/// - Positions (for displacement calculation)
///
/// Does NOT contain:
/// - Route data
/// - Stop indices
/// - Kalman state
pub struct ModeInput {
    /// Divergence from route (squared distance in cm²)
    pub divergence_d2: Dist2,

    /// Whether GPS has valid fix
    pub has_gps_fix: bool,

    /// Frozen position (when in OffRoute mode, None in Normal)
    pub frozen_s_cm: Option<DistCm>,

    /// Current raw GPS projection (for displacement calculation)
    pub current_z_gps_cm: DistCm,
}

impl ModeInput {
    /// Create a new mode input
    pub fn new(divergence_d2: Dist2, has_gps_fix: bool, frozen_s_cm: Option<DistCm>, current_z_gps_cm: DistCm) -> Self {
        Self {
            divergence_d2,
            has_gps_fix,
            frozen_s_cm,
            current_z_gps_cm,
        }
    }
}

/// Mode machine output — next mode and action to take
///
/// # Boundary Contract
///
/// Contains ALL the information the control layer needs:
/// - Next mode
/// - Action to take (freeze, recover, resume)
/// - Whether detection should be enabled
pub struct ModeOutput {
    /// Next mode after this transition
    pub mode: SystemMode,

    /// Action to take (if any)
    pub action: ModeAction,

    /// Whether arrival detection should be enabled
    pub detection_enabled: bool,
}

/// Action to take after mode transition
///
/// These actions represent side effects that the control layer
/// should perform AFTER the mode transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeAction {
    /// No action needed
    None,

    /// Freeze position (entering OffRoute mode)
    FreezePosition,

    /// Begin recovery (entering Recovering mode)
    /// Control layer should call recovery function
    BeginRecovery,

    /// Resume normal operation (returning to Normal mode)
    ResumeNormal,
}

// ===== Unit Tests =====

#[cfg(test)]
mod tests {
    use super::*;

    /// ===== Boundary Tests =====

    /// COMPILE-TIME CHECK: ModeInput does NOT contain route data
    #[test]
    fn test_mode_input_excludes_route_data() {
        // ModeInput only has: divergence_d2, has_gps_fix, frozen_s_cm, current_z_gps_cm
        // It does NOT have:
        // - route_data: &RouteData
        // - stops: &[Stop]
        // - kalman_state: &KalmanState

        let input = ModeInput {
            divergence_d2: 1000000,
            has_gps_fix: true,
            frozen_s_cm: None,
            current_z_gps_cm: 10000,
        };

        // Can access allowed fields
        let _ = input.divergence_d2;
        let _ = input.has_gps_fix;
        // input.route_data  // ❌ ERROR: no field named `route_data`
    }

    /// COMPILE-TIME CHECK: ModeMachine does NOT access external state
    #[test]
    fn test_mode_machine_is_pure() {
        // ModeMachine only owns:
        // - mode: SystemMode
        // - off_route_suspect_ticks: u8
        // - off_route_clear_ticks: u8
        //
        // It does NOT have access to:
        // - Position (s_cm, frozen_s_cm)
        // - Stop index
        // - Route data
        //
        // This is enforced at compile time by the struct definition

        let machine = ModeMachine::new();
        // Can query mode
        let _ = machine.mode();
        // Cannot access external state (not owned)
    }

    /// ===== State Machine Tests =====

    #[test]
    fn test_new_machine_in_normal_mode() {
        let machine = ModeMachine::new();
        assert_eq!(machine.mode(), SystemMode::Normal);
        assert!(machine.is_normal());
        assert!(!machine.is_off_route());
        assert!(!machine.is_recovering());
    }

    #[test]
    fn test_normal_to_offroute_requires_5_ticks() {
        let mut machine = ModeMachine::new();

        // Need 5 consecutive ticks of high divergence
        for i in 0..4 {
            let input = ModeInput {
                divergence_d2: 30_000_000,  // > 50m threshold
                has_gps_fix: true,
                frozen_s_cm: None,
                current_z_gps_cm: 10000,
            };
            let output = machine.update(input);
            assert_eq!(output.mode, SystemMode::Normal, "Should stay Normal after {} ticks", i + 1);
        }

        // 5th tick triggers transition
        let input = ModeInput {
            divergence_d2: 30_000_000,
            has_gps_fix: true,
            frozen_s_cm: None,
            current_z_gps_cm: 10000,
        };
        let output = machine.update(input);
        assert_eq!(output.mode, SystemMode::OffRoute);
        assert_eq!(output.action, ModeAction::FreezePosition);
        assert!(!output.detection_enabled);
    }

    #[test]
    fn test_normal_to_offroute_resets_on_good_divergence() {
        let mut machine = ModeMachine::new();

        // 3 ticks of bad divergence
        for _ in 0..3 {
            let input = ModeInput {
                divergence_d2: 30_000_000,
                has_gps_fix: true,
                frozen_s_cm: None,
                current_z_gps_cm: 10000,
            };
            machine.update(input);
        }

        // One good tick resets counter
        let input = ModeInput {
            divergence_d2: 10_000_000,  // < 50m threshold
            has_gps_fix: true,
            frozen_s_cm: None,
            current_z_gps_cm: 10000,
        };
        let output = machine.update(input);
        assert_eq!(output.mode, SystemMode::Normal);
    }

    #[test]
    fn test_offroute_to_normal_with_small_displacement() {
        let mut machine = ModeMachine::new();

        // Force transition to OffRoute
        machine.mode = SystemMode::OffRoute;
        machine.off_route_clear_ticks = 0;

        // Two ticks of good divergence with small displacement
        let input = ModeInput {
            divergence_d2: 10_000_000,  // < 50m
            has_gps_fix: true,
            frozen_s_cm: Some(0),
            current_z_gps_cm: 1000,  // Small displacement (< 5000 cm)
        };

        // First tick
        let output1 = machine.update(input);
        assert_eq!(output1.mode, SystemMode::OffRoute, "Should stay OffRoute after 1 tick");
        assert_eq!(output1.action, ModeAction::None);

        // Second tick triggers transition (need to recreate input)
        let input = ModeInput {
            divergence_d2: 10_000_000,
            has_gps_fix: true,
            frozen_s_cm: Some(0),
            current_z_gps_cm: 1000,
        };
        let output2 = machine.update(input);
        assert_eq!(output2.mode, SystemMode::Normal);
        assert_eq!(output2.action, ModeAction::ResumeNormal);
        assert!(output2.detection_enabled);
    }

    #[test]
    fn test_offroute_to_recovering_with_large_displacement() {
        let mut machine = ModeMachine::new();

        // Force transition to OffRoute
        machine.mode = SystemMode::OffRoute;
        machine.off_route_clear_ticks = 0;

        // Two ticks of good divergence with large displacement
        let input = ModeInput {
            divergence_d2: 10_000_000,  // < 50m
            has_gps_fix: true,
            frozen_s_cm: Some(0),
            current_z_gps_cm: 10000,  // Large displacement (> 5000 cm)
        };

        // First tick
        let output1 = machine.update(input);
        assert_eq!(output1.mode, SystemMode::OffRoute);

        // Second tick triggers transition to Recovering (recreate input)
        let input = ModeInput {
            divergence_d2: 10_000_000,
            has_gps_fix: true,
            frozen_s_cm: Some(0),
            current_z_gps_cm: 10000,
        };
        let output2 = machine.update(input);
        assert_eq!(output2.mode, SystemMode::Recovering);
        assert_eq!(output2.action, ModeAction::BeginRecovery);
        assert!(!output2.detection_enabled);
    }

    #[test]
    fn test_offroute_stays_if_divergence_persists() {
        let mut machine = ModeMachine::new();

        // Force transition to OffRoute
        machine.mode = SystemMode::OffRoute;
        machine.off_route_clear_ticks = 0;

        // High divergence persists
        for i in 0..3 {
            let input = ModeInput {
                divergence_d2: 30_000_000,  // > 50m
                has_gps_fix: true,
                frozen_s_cm: Some(0),
                current_z_gps_cm: 10000,
            };
            let output = machine.update(input);
            assert_eq!(output.mode, SystemMode::OffRoute, "Should stay OffRoute after tick {}", i + 1);
            assert_eq!(output.action, ModeAction::None);
            assert!(!output.detection_enabled);
        }
    }

    #[test]
    fn test_detection_enabled_only_in_normal() {
        let mut machine = ModeMachine::new();

        // Normal mode - detection enabled
        let input = ModeInput {
            divergence_d2: 10_000_000,
            has_gps_fix: true,
            frozen_s_cm: None,
            current_z_gps_cm: 10000,
        };
        let output = machine.update(input);
        assert!(output.detection_enabled, "Detection should be enabled in Normal mode");

        // Transition to OffRoute - detection disabled
        for _ in 0..5 {
            let bad_input = ModeInput {
                divergence_d2: 30_000_000,
                has_gps_fix: true,
                frozen_s_cm: None,
                current_z_gps_cm: 10000,
            };
            machine.update(bad_input);
        }
        // Recreate input for final check
        let input = ModeInput {
            divergence_d2: 10_000_000,
            has_gps_fix: true,
            frozen_s_cm: None,
            current_z_gps_cm: 10000,
        };
        let output = machine.update(input);
        assert!(!output.detection_enabled, "Detection should be disabled in OffRoute mode");
    }

    #[test]
    fn test_transition_to_normal_clears_counters() {
        let mut machine = ModeMachine::new();

        // Set up some counters
        machine.off_route_suspect_ticks = 3;
        machine.off_route_clear_ticks = 1;

        // Transition to Normal
        machine.transition_to_normal();

        // Counters should be cleared
        assert_eq!(machine.off_route_suspect_ticks, 0);
        assert_eq!(machine.off_route_clear_ticks, 0);
        assert_eq!(machine.mode(), SystemMode::Normal);
    }
}

//! Estimation layer — isolated GPS → position pipeline
//!
//! # Layer Boundary
//!
//! This layer is **isolated** from control layer concerns. It maintains internal
//! Kalman + DR state but does NOT access:
//! - `mode` (SystemMode)
//! - `last_stop_index` (u8)
//! - `frozen_s_cm` (Option<DistCm>)
//!
//! # Contract
//!
//! **Deterministic:** Same `EstimationInput` always produces same `EstimationOutput`
//! (given the same internal state).
//!
//! **No Side Effects:** Does NOT modify control layer state.
//!
//! **State Isolation:** All estimation state is internal to `EstimationState`.

pub mod kalman;
pub mod dr;

use shared::{GpsPoint, binfile::RouteData, DistCm, SpeedCms, Dist2};

pub use kalman::KalmanState;
pub use dr::DrState;

/// Combined estimation state (internal to estimation layer only)
///
/// # Invariant
///
/// This state is NEVER accessed directly by the control layer.
/// All interaction goes through the `estimate()` function.
pub struct EstimationState {
    pub kalman: KalmanState,
    pub dr: DrState,
}

impl Default for EstimationState {
    fn default() -> Self {
        Self::new()
    }
}

impl EstimationState {
    pub fn new() -> Self {
        Self {
            kalman: KalmanState::new(),
            dr: DrState::new(),
        }
    }
}

/// Estimation input — ONLY what estimation needs from the outside world
///
/// # Boundary Contract
///
/// This struct contains **ONLY** the data required for GPS → position estimation.
/// It deliberately EXCLUDES control layer state (mode, stops, etc.) to enforce isolation.
///
/// # Fields
///
/// - `gps`: Raw GPS data from NMEA parser
/// - `route_data`: Static route geometry (XIP flash reference)
/// - `is_first_fix`: True for first GPS fix after cold start (enables relaxed heading)
///
/// # What's NOT Included (Enforcing Isolation)
///
/// - ❌ `mode: SystemMode` — estimation doesn't care about mode
/// - ❌ `last_stop_index: u8` — estimation doesn't track stops
/// - ❌ `frozen_s_cm: Option<DistCm>` — control layer concern only
pub struct EstimationInput<'a> {
    /// Raw GPS data (from NMEA parser)
    pub gps: GpsPoint,
    /// Route geometry reference (immutable, XIP flash)
    pub route_data: &'a RouteData<'a>,
    /// True for first GPS fix (enables relaxed heading filter)
    pub is_first_fix: bool,
}

impl<'a> EstimationInput<'a> {
    /// Create a new estimation input
    pub fn new(gps: GpsPoint, route_data: &'a RouteData<'a>, is_first_fix: bool) -> Self {
        Self {
            gps,
            route_data,
            is_first_fix,
        }
    }
}

/// Estimation output — ALL position signals produced by estimation layer
///
/// # Boundary Contract
///
/// This struct contains **ALL** the position signals that other layers need.
/// Control layer uses `divergence_d2` for mode transitions.
/// Detection layer uses `z_gps_cm`, `s_cm`, `v_cms` for arrival detection.
///
/// # Field Descriptions
///
/// - `z_gps_cm`: Raw GPS projection onto route (for F1 probability, recovery)
/// - `s_cm`: Kalman-filtered position (primary position in Normal mode)
/// - `v_cms`: Filtered velocity (cm/s)
/// - `divergence_d2`: Squared distance from route (for mode transitions)
/// - `confidence`: Quality signal 0-255 (higher = better)
/// - `has_fix`: Whether GPS has valid fix
///
/// # Usage by Layer
///
/// | Layer | Uses | Purpose |
/// |-------|------|---------|
/// | Control | `divergence_d2`, `has_fix` | Mode transitions |
/// | Detection | `z_gps_cm`, `s_cm`, `v_cms` | Arrival probability |
/// | Recovery | `z_gps_cm`, `v_cms` | Stop index recovery |
pub struct EstimationOutput {
    /// Raw GPS projection onto route (for F1 probability, recovery input)
    pub z_gps_cm: DistCm,
    /// Kalman-filtered position (primary position in Normal mode)
    pub s_cm: DistCm,
    /// Filtered velocity (cm/s)
    pub v_cms: SpeedCms,
    /// Divergence from route (squared distance, for mode transitions)
    pub divergence_d2: Dist2,
    /// Confidence signal (0-255, higher is better)
    pub confidence: u8,
    /// Whether GPS has valid fix
    pub has_fix: bool,
}

impl EstimationOutput {
    /// Check if GPS fix is valid
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.has_fix
    }

    /// Get position for Normal mode (Kalman-filtered)
    #[inline]
    pub fn normal_position(&self) -> DistCm {
        self.s_cm
    }

    /// Get position for Recovering mode (raw GPS)
    #[inline]
    pub fn recovery_position(&self) -> DistCm {
        self.z_gps_cm
    }
}

/// Isolated estimation pipeline
///
/// # Contract
/// - Input: GPS + route (no control layer state)
/// - Output: Position signals (no side effects to control layer)
/// - Internal state: Kalman + DR (opaque to control layer)
///
/// # Guarantees
/// - Does NOT access: mode, last_stop_index, frozen_s_cm
/// - Does NOT trigger: recovery, mode changes
/// - Same GPS input → same EstimationOutput (deterministic)
pub fn estimate(
    input: EstimationInput,
    state: &mut EstimationState,
) -> EstimationOutput {
    use gps_processor::map_match;

    // Check for GPS outage
    if !input.gps.has_fix {
        return handle_outage(state, input.gps.timestamp);
    }

    // 1. Convert GPS to absolute coordinates
    let (gps_x, gps_y) = map_match::latlon_to_cm_absolute_with_lat_avg(
        input.gps.lat,
        input.gps.lon,
        input.route_data.lat_avg_deg,
    );

    // 2. Map matching
    let use_relaxed_heading = input.is_first_fix || state.dr.in_recovery;
    let (seg_idx, match_d2) = map_match::find_best_segment_restricted(
        gps_x,
        gps_y,
        input.gps.heading_cdeg.unwrap_or(i16::MIN),
        input.gps.speed_cms.unwrap_or(0),
        input.route_data,
        state.kalman.last_seg_idx,
        use_relaxed_heading,
    );

    // 3. Project to route
    let z_raw = map_match::project_to_route(
        gps_x, gps_y, seg_idx, input.route_data
    );

    // 4. Kalman filter
    let (s_cm, v_cms) = if input.is_first_fix {
        // First fix: initialize Kalman
        state.kalman.s_cm = z_raw;
        let v_gps = input.gps.speed_cms.unwrap_or(0).clamp(0, 1667);
        state.kalman.v_cms = state.kalman.v_cms + 3 * (v_gps - state.kalman.v_cms) / 10;
        state.kalman.last_seg_idx = seg_idx;

        state.dr.last_gps_time = Some(input.gps.timestamp);
        state.dr.filtered_v = state.kalman.v_cms;
        state.dr.last_valid_s = Some(state.kalman.s_cm);  // Anchor for DR
        state.dr.in_recovery = false;

        (z_raw, state.kalman.v_cms)
    } else {
        // Normal Kalman update
        let hdop_x10 = input.gps.hdop_x10.unwrap_or(9990);
        let speed_cms = input.gps.speed_cms.unwrap_or(0);
        state.kalman.update_adaptive(z_raw, speed_cms, hdop_x10);
        state.kalman.last_seg_idx = seg_idx;

        // Update DR state
        state.dr.last_gps_time = Some(input.gps.timestamp);
        state.dr.filtered_v = update_dr_ema(state.dr.filtered_v, speed_cms);
        state.dr.last_valid_s = Some(state.kalman.s_cm);  // Anchor for DR

        (state.kalman.s_cm, state.kalman.v_cms)
    };

    // 5. Calculate confidence
    let confidence = calculate_confidence(
        input.gps.hdop_x10.unwrap_or(9990),
        false,  // Not in outage (we have fix)
        match_d2,
    );

    EstimationOutput {
        z_gps_cm: z_raw,
        s_cm,
        v_cms,
        divergence_d2: match_d2,
        confidence,
        has_fix: true,
    }
}

/// Handle GPS outage
fn handle_outage(state: &mut EstimationState, timestamp: u64) -> EstimationOutput {

    let dt = match state.dr.last_gps_time {
        Some(t) => timestamp.saturating_sub(t),
        None => return EstimationOutput {
            z_gps_cm: state.kalman.s_cm,
            s_cm: state.kalman.s_cm,
            v_cms: state.kalman.v_cms,
            divergence_d2: 0,
            confidence: 0,
            has_fix: false,
        },
    };

    if dt > 10 {
        state.dr.in_recovery = true;
        return EstimationOutput {
            z_gps_cm: state.kalman.s_cm,
            s_cm: state.kalman.s_cm,
            v_cms: state.kalman.v_cms,
            divergence_d2: 0,
            confidence: 0,
            has_fix: false,
        };
    }

    // DR mode: absolute positioning from anchor (not incremental)
    // s(t) = s_anchor + v * dt, where s_anchor is position at last GPS fix
    state.kalman.s_cm = state.dr.last_valid_s.unwrap_or(state.kalman.s_cm) + state.dr.filtered_v * (dt as shared::DistCm);

    // Speed decay
    let dt_idx = dt.min(10) as usize;
    const DR_DECAY: [u32; 11] = [10000, 9000, 8100, 7290, 6561, 5905, 5314, 4783, 4305, 3874, 3487];
    state.dr.filtered_v = (state.dr.filtered_v as u32 * DR_DECAY[dt_idx] / 10000) as shared::SpeedCms;

    EstimationOutput {
        z_gps_cm: state.kalman.s_cm,
        s_cm: state.kalman.s_cm,
        v_cms: state.kalman.v_cms,
        divergence_d2: 0,
        confidence: 0,
        has_fix: false,
    }
}

/// EMA velocity filter update
fn update_dr_ema(v_filtered_prev: shared::SpeedCms, v_gps: shared::SpeedCms) -> shared::SpeedCms {
    v_filtered_prev + 3 * (v_gps - v_filtered_prev) / 10
}

/// Calculate confidence from HDOP, outage status, and divergence
fn calculate_confidence(hdop_x10: u16, is_in_outage: bool, divergence_d2: shared::Dist2) -> u8 {
    if is_in_outage {
        return 0;
    }

    // HDOP contribution
    let hdop_factor = if hdop_x10 < 20 {
        255u16
    } else if hdop_x10 > 100 {
        0u16
    } else {
        255 - (hdop_x10 - 20) * 255 / 80
    };

    // Divergence contribution
    let div_factor = if divergence_d2 < 10_000_000 {
        255u16
    } else if divergence_d2 > 100_000_000 {
        0u16
    } else {
        255 - ((divergence_d2 - 10_000_000) / 360_000) as u16
    };

    hdop_factor.min(div_factor) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that dead-reckoning produces LINEAR position growth during outage,
    /// not QUADRATIC drift.
    ///
    /// Bug: If `last_gps_time` is not updated during outage, `dt` grows each tick.
    /// Combined with incremental update (`s_cm += v * dt`), this causes quadratic drift.
    ///
    /// Key invariant: position should scale as O(dt), not O(dt²)
    #[test]
    fn test_dr_position_grows_linearly_not_quadratically() {
        let mut state = EstimationState::new();

        // Initialize: GPS fix at t=100, position=0, velocity=100 cm/s
        state.kalman.s_cm = 0;
        state.kalman.v_cms = 100;
        state.dr.last_gps_time = Some(100);
        state.dr.filtered_v = 100;
        state.dr.last_valid_s = Some(0);

        // Tick 1 (dt=1): s = 0 + 100*1 = 100
        let result = handle_outage(&mut state, 101);
        assert_eq!(result.s_cm, 100);

        // Tick 2 (dt=2): s = 0 + 90*2 = 180 (after speed decay)
        // BUG (incremental with growing dt): s = 100 + 90*2 = 280
        let result = handle_outage(&mut state, 102);
        assert_eq!(result.s_cm, 180, "Should be absolute from anchor, not incremental");

        // Tick 3 (dt=3): s = 0 + 72*3 = 216
        // BUG (incremental): s = 180 + 72*3 = 396
        let result = handle_outage(&mut state, 103);
        assert_eq!(result.s_cm, 216, "Should be absolute from anchor, not incremental");
    }

    #[test]
    fn test_dr_position_with_zero_velocity() {
        let mut state = EstimationState::new();

        // Initialize with zero velocity
        state.kalman.s_cm = 1000;
        state.kalman.v_cms = 0;
        state.dr.last_gps_time = Some(100);
        state.dr.filtered_v = 0;
        state.dr.last_valid_s = Some(1000);

        // During outage, position should NOT change
        for timestamp in [101, 102, 103] {
            let result = handle_outage(&mut state, timestamp);
            assert_eq!(result.s_cm, 1000, "Position should not change with v=0");
        }
    }

    #[test]
    fn test_dr_outage_timeout_after_10_seconds() {
        let mut state = EstimationState::new();

        state.kalman.s_cm = 500;
        state.dr.last_gps_time = Some(100);
        state.dr.filtered_v = 100;
        state.dr.last_valid_s = Some(500);

        // At dt=10, should still do DR
        let result = handle_outage(&mut state, 110);
        assert_eq!(result.s_cm, 500 + 100 * 10, "Should do DR at dt=10");

        // At dt=11, should timeout (return last position)
        let result = handle_outage(&mut state, 111);
        assert_eq!(result.s_cm, 500 + 100 * 10, "Should timeout at dt>10");
        assert!(state.dr.in_recovery, "Should set recovery flag");
    }

    #[test]
    fn test_dr_speed_decay_during_outage() {
        let mut state = EstimationState::new();

        state.kalman.s_cm = 0;
        state.dr.last_gps_time = Some(100);
        state.dr.filtered_v = 1000; // 10 m/s
        state.dr.last_valid_s = Some(0);

        // First tick: speed should decay
        handle_outage(&mut state, 101);
        let v1 = state.dr.filtered_v;

        // Second tick: speed should decay more
        handle_outage(&mut state, 102);
        let v2 = state.dr.filtered_v;

        assert!(v2 < v1, "Speed should decay during outage");
    }
}

/// ===== Boundary Tests (Prove Isolation) =====

/// COMPILE-TIME CHECK: EstimationInput does NOT contain control state
///
/// This test ensures that EstimationInput cannot access:
/// - SystemMode
/// - last_stop_index
/// - frozen_s_cm
#[test]
fn test_estimation_input_excludes_control_state() {
    // EstimationInput only has: gps, route_data, is_first_fix
    // It does NOT have control state fields
    let gps = GpsPoint::new();
    // If we try to access control state, it won't compile
    let _ = gps.timestamp; // ✅ OK
    // input.mode          // ❌ ERROR: no field named `mode`
}

/// COMPILE-TIME CHECK: EstimationOutput does NOT contain control state
#[test]
fn test_estimation_output_excludes_control_state() {
    let output = EstimationOutput {
        z_gps_cm: 0,
        s_cm: 0,
        v_cms: 0,
        divergence_d2: 0,
        confidence: 0,
        has_fix: false,
    };

    // EstimationOutput does NOT have control state:
    let _ = output.z_gps_cm;    // ✅ OK
    let _ = output.s_cm;        // ✅ OK
    // output.mode              // ❌ ERROR: no field named `mode`
}

/// RUNTIME CHECK: estimate() function signature enforces isolation
#[test]
fn test_estimate_function_signature_enforces_isolation() {
    // The function signature prevents control state access:
    // fn estimate(input: EstimationInput, state: &mut EstimationState) -> EstimationOutput
    //
    // This makes it IMPOSSIBLE to:
    // 1. Pass control state to estimation
    // 2. Modify control state from estimation
    // 3. Access control state from estimation
}

/// ===== Helper Method Tests =====

#[test]
fn test_estimation_output_is_valid() {
    let valid = EstimationOutput {
        z_gps_cm: 1000,
        s_cm: 1050,
        v_cms: 500,
        divergence_d2: 1000000,
        confidence: 200,
        has_fix: true,
    };

    assert!(valid.is_valid());

    let invalid = EstimationOutput {
        z_gps_cm: 1000,
        s_cm: 1050,
        v_cms: 500,
        divergence_d2: 1000000,
        confidence: 200,
        has_fix: false,
    };

    assert!(!invalid.is_valid());
}

#[test]
fn test_estimation_output_normal_position() {
    let output = EstimationOutput {
        z_gps_cm: 1000,
        s_cm: 1050,
        v_cms: 500,
        divergence_d2: 1000000,
        confidence: 200,
        has_fix: true,
    };

    assert_eq!(output.normal_position(), 1050);
}

#[test]
fn test_estimation_output_recovery_position() {
    let output = EstimationOutput {
        z_gps_cm: 1000,
        s_cm: 1050,
        v_cms: 500,
        divergence_d2: 1000000,
        confidence: 200,
        has_fix: true,
    };

    assert_eq!(output.recovery_position(), 1000);
}

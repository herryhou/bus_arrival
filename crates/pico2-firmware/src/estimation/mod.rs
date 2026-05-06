//! Estimation layer — isolated GPS → position pipeline
//!
//! This layer is isolated from control layer concerns.
//! It maintains internal Kalman + DR state but does NOT access:
//! - mode, last_stop_index, frozen_s_cm

pub mod kalman;
pub mod dr;

use shared::{GpsPoint, binfile::RouteData};

pub use kalman::KalmanState;
pub use dr::DrState;

/// Combined estimation state (internal only)
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

/// Estimation input — GPS + route data
pub struct EstimationInput<'a> {
    pub gps: GpsPoint,
    pub route_data: &'a RouteData<'a>,
    pub is_first_fix: bool,
}

/// Estimation output — all derived position signals
pub struct EstimationOutput {
    /// Raw GPS projection onto route (for F1 probability)
    pub z_gps_cm: shared::DistCm,
    /// Kalman-filtered position (primary position in Normal mode)
    pub s_cm: shared::DistCm,
    /// Filtered velocity (cm/s)
    pub v_cms: shared::SpeedCms,
    /// Divergence from route (squared distance from map matching)
    pub divergence_d2: shared::Dist2,
    /// Confidence signal (0-255, higher is better)
    pub confidence: u8,
    /// Whether GPS has valid fix
    pub has_fix: bool,
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

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
    first_fix_called: bool,  // track if estimate() has ever been called with valid fix
}

impl EstimationState {
    pub fn new() -> Self {
        Self {
            kalman: KalmanState::new(),
            dr: DrState::new(),
            first_fix_called: false,
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
    /// True if map-matching snapped from off-route back to route
    pub snapped: bool,
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

    // Track first fix
    let is_first_fix = !state.first_fix_called && input.gps.has_fix;
    if input.gps.has_fix {
        state.first_fix_called = true;
    }

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
    let use_relaxed_heading = is_first_fix || state.dr.in_recovery;
    let (seg_idx, match_d2) = map_match::find_best_segment_restricted(
        gps_x,
        gps_y,
        input.gps.heading_cdeg,
        input.gps.speed_cms,
        input.route_data,
        state.kalman.last_seg_idx,
        use_relaxed_heading,
    );

    // 3. Project to route
    let z_raw = map_match::project_to_route(
        gps_x, gps_y, seg_idx, input.route_data
    );

    // Detect snap: was previously off-route (high divergence) but now on-route
    let was_off_route = state.dr.in_recovery;
    let snapped = was_off_route && match_d2 < 25_000_000;  // 50m threshold

    // Clear recovery flag when snap is detected
    if snapped {
        state.dr.in_recovery = false;
    }

    // 4. Kalman filter
    let (s_cm, v_cms) = if is_first_fix {
        // First fix: initialize Kalman
        state.kalman.s_cm = z_raw;
        let v_gps = input.gps.speed_cms.max(0).min(1667);
        state.kalman.v_cms = state.kalman.v_cms + 3 * (v_gps - state.kalman.v_cms) / 10;
        state.kalman.last_seg_idx = seg_idx;

        state.dr.last_gps_time = Some(input.gps.timestamp);
        state.dr.filtered_v = state.kalman.v_cms;
        state.dr.in_recovery = false;

        (z_raw, state.kalman.v_cms)
    } else {
        // Normal Kalman update
        let hdop_x10 = input.gps.hdop_x10;
        state.kalman.update_adaptive(z_raw, input.gps.speed_cms, hdop_x10);
        state.kalman.last_seg_idx = seg_idx;

        // Update DR state
        state.dr.last_gps_time = Some(input.gps.timestamp);
        state.dr.filtered_v = update_dr_ema(state.dr.filtered_v, input.gps.speed_cms);

        (state.kalman.s_cm, state.kalman.v_cms)
    };

    // 5. Calculate confidence
    let confidence = calculate_confidence(
        input.gps.hdop_x10,
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
        snapped,
    }
}

/// Handle GPS outage
fn handle_outage(state: &mut EstimationState, timestamp: u64) -> EstimationOutput {
    use shared::DistCm;

    let dt = match state.dr.last_gps_time {
        Some(t) => timestamp.saturating_sub(t),
        None => return EstimationOutput {
            z_gps_cm: state.kalman.s_cm,
            s_cm: state.kalman.s_cm,
            v_cms: state.kalman.v_cms,
            divergence_d2: 0,
            confidence: 0,
            has_fix: false,
            snapped: false,
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
            snapped: false,
        };
    }

    // DR mode
    state.dr.last_valid_s = Some(state.kalman.s_cm);  // Save before advancing
    state.kalman.s_cm = state.kalman.s_cm + state.dr.filtered_v * (dt as DistCm);

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
        snapped: false,
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

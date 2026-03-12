//! Kalman filter and GPS processing pipeline

use shared::{KalmanState, DrState, GpsPoint, DistCm, SpeedCms};
use crate::route_data::RouteData;

/// Maximum feasible distance in 1 second
pub const D_MAX_CM: DistCm = 3667;  // V_max(1667 cm/s) + σ_gps(2000 cm)

/// ProcessResult from GPS update
pub enum ProcessResult {
    Valid { s_cm: DistCm, v_cms: SpeedCms, seg_idx: usize },
    Rejected(&'static str),
    Outage,
    DrOutage { s_cm: DistCm, v_cms: SpeedCms },
}

/// Main processing pipeline for each GPS update
pub fn process_gps_update(
    state: &mut KalmanState,
    dr: &mut DrState,
    gps: &GpsPoint,
    route_data: &RouteData,
    current_time: u64,
) -> ProcessResult {
    // 1. Check for GPS outage
    if !gps.has_fix {
        return handle_outage(state, dr, current_time);
    }

    // 2. Convert GPS to relative coordinates
    let (gps_x, gps_y) = crate::map_match::latlon_to_cm_relative(
        gps.lat,
        gps.lon,
        route_data.grid_origin.x0_cm as i64,
        route_data.grid_origin.y0_cm as i64,
    );

    // 3. Map matching
    let seg_idx = crate::map_match::find_best_segment(
        gps_x,
        gps_y,
        gps.heading_cdeg,
        gps.speed_cms,
        &route_data.grid,
        &route_data.nodes,
    );

    // 4. Projection
    let z_raw = crate::map_match::project_to_route(
        gps_x,
        gps_y,
        seg_idx,
        &route_data.nodes,
    );

    // 5. Speed constraint filter
    if !check_speed_constraint(z_raw, state.s_cm) {
        return ProcessResult::Rejected("speed constraint");
    }

    // 6. Monotonicity filter
    if !check_monotonic(z_raw, state.s_cm) {
        return ProcessResult::Rejected("monotonicity");
    }

    // 7. Kalman update (HDOP-adaptive)
    state.update_adaptive(z_raw, gps.speed_cms, gps.hdop_x10);

    // 8. Update DR state
    dr.last_gps_time = Some(current_time);
    dr.last_valid_s = state.s_cm;
    dr.filtered_v = state.v_cms;

    ProcessResult::Valid {
        s_cm: state.s_cm,
        v_cms: state.v_cms,
        seg_idx,
    }
}

/// Reject GPS updates that exceed physical limits
fn check_speed_constraint(z_new: DistCm, z_prev: DistCm) -> bool {
    (z_new - z_prev).unsigned_abs() <= D_MAX_CM as u32
}

/// Monotonicity constraint with noise tolerance
fn check_monotonic(z_new: DistCm, z_prev: DistCm) -> bool {
    z_new >= z_prev - 1000  // allow -10m GPS noise
}

/// Handle GPS outage (max 10 seconds)
fn handle_outage(
    state: &mut KalmanState,
    dr: &mut DrState,
    current_time: u64,
) -> ProcessResult {
    let dt = match dr.last_gps_time {
        Some(t) => current_time.saturating_sub(t),
        None => return ProcessResult::Rejected("no previous fix"),
    };

    if dt > 10 {
        return ProcessResult::Outage;
    }

    // Dead-reckoning: ŝ(t) = ŝ(t-1) + v_filtered × dt
    state.s_cm = dr.last_valid_s + dr.filtered_v * (dt as DistCm);
    // Speed decays during outage
    dr.filtered_v = dr.filtered_v * 9 / 10;

    ProcessResult::DrOutage {
        s_cm: state.s_cm,
        v_cms: state.v_cms,
    }
}

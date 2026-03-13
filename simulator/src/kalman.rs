//! Kalman filter and GPS processing pipeline

use crate::route_data::RouteData;
use shared::{DistCm, DrState, GpsPoint, KalmanState, SpeedCms};

/// Maximum bus speed: 108 km/h = 3000 cm/s
pub const V_MAX_CMS: SpeedCms = 3000;
/// GPS noise margin: 50m
pub const SIGMA_GPS_CM: DistCm = 5000;

/// ProcessResult from GPS update
pub enum ProcessResult {
    Valid {
        s_cm: DistCm,
        v_cms: SpeedCms,
        seg_idx: usize,
    },
    Rejected(&'static str),
    Outage,
    DrOutage {
        s_cm: DistCm,
        v_cms: SpeedCms,
    },
}

/// Main processing pipeline for each GPS update
pub fn process_gps_update(
    state: &mut KalmanState,
    dr: &mut DrState,
    gps: &GpsPoint,
    route_data: &RouteData,
    _current_time: u64,
    is_first_fix: bool,
) -> ProcessResult {
    // 1. Check for GPS outage
    if !gps.has_fix {
        return handle_outage(state, dr, gps.timestamp);
    }

    // Calculate time delta since last GPS update
    let dt = match dr.last_gps_time {
        Some(t) => (gps.timestamp.saturating_sub(t)) as i32,
        None => 1, // First fix
    };

    // 2. Convert GPS to relative coordinates
    let (gps_x, gps_y) = crate::map_match::latlon_to_cm_relative(
        gps.lat,
        gps.lon,
        route_data.x0_cm as i64,
        route_data.y0_cm as i64,
    );

    // 3. Map matching
    let seg_idx = crate::map_match::find_best_segment_restricted(
        gps_x,
        gps_y,
        gps.heading_cdeg,
        gps.speed_cms,
        route_data,
        state.last_seg_idx,
    );

    // 4. Projection
    let z_raw = crate::map_match::project_to_route(gps_x, gps_y, seg_idx, route_data);

    if is_first_fix {

        state.s_cm = z_raw;

        state.v_cms = gps.speed_cms;
        state.last_seg_idx = seg_idx;
        dr.last_gps_time = Some(gps.timestamp);
        dr.last_valid_s = state.s_cm;
        dr.filtered_v = state.v_cms;
        return ProcessResult::Valid {
            s_cm: state.s_cm,
            v_cms: state.v_cms,
            seg_idx,
        };
    }

    // 5. Speed constraint filter
    if !check_speed_constraint(z_raw, state.s_cm, dt) {
        return ProcessResult::Rejected("speed constraint");
    }

    // 6. Monotonicity filter
    if !check_monotonic(z_raw, state.s_cm) {
        return ProcessResult::Rejected("monotonicity");
    }

    // 7. Kalman update (HDOP-adaptive)
    state.update_adaptive(z_raw, gps.speed_cms, gps.hdop_x10);
    state.last_seg_idx = seg_idx;

    // 8. Update DR state
    dr.last_gps_time = Some(gps.timestamp);
    dr.last_valid_s = state.s_cm;
    dr.filtered_v = state.v_cms;

    ProcessResult::Valid {
        s_cm: state.s_cm,
        v_cms: state.v_cms,
        seg_idx,
    }
}

/// Reject GPS updates that exceed physical limits
fn check_speed_constraint(z_new: DistCm, z_prev: DistCm, dt: i32) -> bool {
    let dist_abs = (z_new - z_prev).unsigned_abs() as i32;
    let max_dist = V_MAX_CMS * dt.max(1) + SIGMA_GPS_CM;
    dist_abs <= max_dist
}

/// Monotonicity constraint with noise tolerance
fn check_monotonic(z_new: DistCm, z_prev: DistCm) -> bool {
    z_new >= z_prev - 1000 // allow -10m GPS noise
}

/// Handle GPS outage (max 10 seconds)
fn handle_outage(state: &mut KalmanState, dr: &mut DrState, timestamp: u64) -> ProcessResult {
    let dt = match dr.last_gps_time {
        Some(t) => timestamp.saturating_sub(t),
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

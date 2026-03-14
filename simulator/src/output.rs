//! JSON output for localization results

use serde::Serialize;
use std::io::{self, Write};
use shared::{Stop, DistCm, HeadCdeg};
use shared::binfile::RouteData;

#[derive(Serialize)]
struct OutputRecord {
    time: u64,
    lat: f64,
    lon: f64,
    s_cm: i64,
    v_cms: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    heading_cdeg: Option<HeadCdeg>,
    status: String,
    seg_idx: Option<usize>,
    active_stops: Vec<usize>,
    stop_states: Vec<StopTraceState>,
    gps_jump: bool,
    recovery_idx: Option<u8>,
}

/// Simplified stop state for simulator output (no arrival detection)
#[derive(Serialize)]
struct StopTraceState {
    stop_idx: u8,
    distance_cm: i32,
}

/// Find stops whose corridor contains the current route progress
fn find_active_stops(s_cm: DistCm, stops: &[Stop]) -> Vec<usize> {
    stops.iter()
        .enumerate()
        .filter(|(_, stop)| {
            s_cm >= stop.corridor_start_cm && s_cm <= stop.corridor_end_cm
        })
        .map(|(i, _)| i)
        .collect()
}

/// Create stop_states for active stops (simplified, no arrival detection)
fn create_stop_states(s_cm: DistCm, active_stops: &[usize], stops: &[Stop]) -> Vec<StopTraceState> {
    active_stops
        .iter()
        .filter_map(|&idx| {
            stops.get(idx).map(|stop| {
                let distance_cm = stop.progress_cm as i32 - s_cm as i32;
                StopTraceState {
                    stop_idx: idx as u8,
                    distance_cm,
                }
            })
        })
        .collect()
}

pub fn write_output<W: Write>(
    output: &mut W,
    time: u64,
    lat: f64,
    lon: f64,
    heading_cdeg: HeadCdeg,
    result: &super::kalman::ProcessResult,
    route_data: &RouteData,
) -> io::Result<()> {
    // Compute active stops for all result types (even invalid ones we have s_cm)
    let s_cm_for_active = match result {
        super::kalman::ProcessResult::Valid { s_cm, .. } => Some(*s_cm),
        super::kalman::ProcessResult::DrOutage { s_cm, .. } => Some(*s_cm),
        _ => None,
    };

    let stops = route_data.stops();
    let active_stops = s_cm_for_active
        .map(|s| find_active_stops(s, &stops))
        .unwrap_or_default();

    let stop_states = s_cm_for_active
        .map(|s| create_stop_states(s, &active_stops, &stops))
        .unwrap_or_default();

    let record = match result {
        super::kalman::ProcessResult::Valid { s_cm, v_cms, seg_idx } => OutputRecord {
            time,
            lat,
            lon,
            s_cm: *s_cm as i64,
            v_cms: *v_cms as i32,
            heading_cdeg: Some(heading_cdeg),
            status: "valid".to_string(),
            seg_idx: Some(*seg_idx),
            active_stops,
            stop_states,
            gps_jump: false,
            recovery_idx: None,
        },
        super::kalman::ProcessResult::Rejected(reason) => OutputRecord {
            time,
            lat,
            lon,
            s_cm: 0,
            v_cms: 0,
            heading_cdeg: Some(heading_cdeg),
            status: format!("rejected_{}", reason),
            seg_idx: None,
            active_stops,
            stop_states,
            gps_jump: false,
            recovery_idx: None,
        },
        super::kalman::ProcessResult::Outage => OutputRecord {
            time,
            lat,
            lon,
            s_cm: 0,
            v_cms: 0,
            heading_cdeg: None,
            status: "dr_outage".to_string(),
            seg_idx: None,
            active_stops,
            stop_states,
            gps_jump: false,
            recovery_idx: None,
        },
        super::kalman::ProcessResult::DrOutage { s_cm, v_cms } => OutputRecord {
            time,
            lat,
            lon,
            s_cm: *s_cm as i64,
            v_cms: *v_cms as i32,
            heading_cdeg: None,
            status: "dr_outage".to_string(),
            seg_idx: None,
            active_stops,
            stop_states,
            gps_jump: false,
            recovery_idx: None,
        },
    };

    writeln!(output, "{}", serde_json::to_string(&record).unwrap())
}

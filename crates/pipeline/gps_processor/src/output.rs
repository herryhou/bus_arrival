//! JSON output for localization results

use serde::Serialize;
use shared::binfile::RouteData;
use shared::{DistCm, HeadCdeg, Stop};
use std::io::{self, Write};

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

// Re-export find_active_stops from detection::corridor
use detection::corridor::find_active_stops;

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
        super::kalman::ProcessResult::Valid { signals, .. } => Some(signals.s_cm),
        super::kalman::ProcessResult::DrOutage { s_cm, .. } => Some(*s_cm),
        super::kalman::ProcessResult::OffRoute { last_valid_s, .. } => Some(*last_valid_s),
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
        super::kalman::ProcessResult::Valid {
            signals,
            v_cms,
            seg_idx,
        } => OutputRecord {
            time,
            lat,
            lon,
            s_cm: signals.s_cm as i64,
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
        super::kalman::ProcessResult::OffRoute {
            last_valid_s,
            last_valid_v,
        } => OutputRecord {
            time,
            lat,
            lon,
            s_cm: *last_valid_s as i64,
            v_cms: *last_valid_v as i32,
            heading_cdeg: None,
            status: "off_route".to_string(),
            seg_idx: None,
            active_stops,
            stop_states,
            gps_jump: false,
            recovery_idx: None,
        },
    };

    writeln!(output, "{}", serde_json::to_string(&record).unwrap())
}

/// Format arrival event as JSON for output
///
/// Uses manual `format!` instead of `serde_json` for embedded/no-std compatibility.
/// This function may be called in contexts where serde_json is not available
/// (when the `std` feature is not enabled). The manual format is simple and
/// well-tested, avoiding the serde_json dependency for this specific use case.
pub fn format_arrival_event(event: &shared::ArrivalEvent) -> String {
    use shared::ArrivalEventType;

    let event_type_str = match event.event_type {
        ArrivalEventType::Arrival => "arrival",
        ArrivalEventType::Departure => "departure",
        ArrivalEventType::Announce => "announce",
    };

    format!(
        r#"{{"type":"{}","time":{},"stop":{},"s":{},"v":{},"p":{}}}"#,
        event_type_str, event.time, event.stop_idx, event.s_cm, event.v_cms, event.probability
    )
}

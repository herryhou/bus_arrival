//! Trace record emission for debugging visualization

use serde::Serialize;
use shared::{DistCm, SpeedCms, Prob8, FsmState, HeadCdeg};
use std::io::{BufWriter, Write};

/// Trace record for debugging visualization
#[derive(Serialize)]
pub struct TraceRecord {
    /// Input: GPS timestamp (seconds since epoch)
    pub time: u64,

    /// Input: Latitude
    pub lat: f64,

    /// Input: Longitude
    pub lon: f64,

    /// Input: Route progress (cm)
    pub s_cm: DistCm,

    /// Input: Velocity (cm/s)
    pub v_cms: SpeedCms,

    /// Input: Heading (hundredths of degrees, -18000 to 18000)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heading_cdeg: Option<HeadCdeg>,

    /// Corridor filter: which stops are active
    pub active_stops: Vec<u8>,

    /// Per-stop detailed state (only for active stops)
    pub stop_states: Vec<StopTraceState>,

    /// GPS jump detected?
    pub gps_jump: bool,

    /// Recovery: new stop index if jumped
    pub recovery_idx: Option<u8>,
}

#[derive(Serialize)]
pub struct StopTraceState {
    pub stop_idx: u8,

    /// Distance to stop (cm)
    pub distance_cm: DistCm,

    /// FSM state - using FsmState directly lets serde handle serialization
    pub fsm_state: FsmState,

    /// Dwell time (seconds)
    pub dwell_time_s: u16,

    /// Arrival probability (0-255)
    pub probability: Prob8,

    /// Individual feature scores
    pub features: FeatureScores,

    /// Just arrived this frame?
    pub just_arrived: bool,
}

#[derive(Serialize, Clone)]
pub struct FeatureScores {
    pub p1: u8,  // Distance likelihood (Gaussian)
    pub p2: u8,  // Speed likelihood (Logistic)
    pub p3: u8,  // Progress likelihood (Gaussian)
    pub p4: u8,  // Dwell time likelihood (Linear)
}

/// Write a trace record to the output file
pub fn write_trace_record<W: Write>(
    output: &mut BufWriter<W>,
    record: &TraceRecord,
) -> std::io::Result<()> {
    let json = serde_json::to_string(record)?;
    writeln!(output, "{}", json)
}

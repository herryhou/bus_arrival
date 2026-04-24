//! Trace record emission for debugging visualization

use serde::{Deserialize, Serialize, Serializer};
use shared::{DistCm, SpeedCms, Prob8, FsmState, HeadCdeg};
use std::io::{BufWriter, Write};

/// Serialize f64 with at most 6 decimal places
fn serialize_f64_6dec<S>(value: &f64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let formatted = format!("{:.6}", value);
    // Parse back to f64 to avoid serializing as string
    let parsed: f64 = formatted.parse().unwrap_or(*value);
    serializer.serialize_f64(parsed)
}

/// Trace record for debugging visualization
#[derive(Serialize, Deserialize)]
pub struct TraceRecord {
    /// Input: GPS timestamp (seconds since epoch)
    pub time: u64,

    /// Input: Latitude
    #[serde(serialize_with = "serialize_f64_6dec")]
    pub lat: f64,

    /// Input: Longitude
    #[serde(serialize_with = "serialize_f64_6dec")]
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

    // === New: Map matching ===
    /// Which route segment we're matched to (None if off-route)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segment_idx: Option<u16>,

    /// Did the heading constraint pass? (±90° rule)
    pub heading_constraint_met: bool,

    // === New: Divergence ===
    /// Raw GPS projection - Kalman filtered position (cm)
    /// Positive = GPS ahead of filter, Negative = GPS behind
    pub divergence_cm: i32,

    // === New: GPS quality ===
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hdop: Option<f32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_sats: Option<u8>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix_type: Option<String>,

    // === New: Kalman state ===
    /// Position variance (cm²), represents filter uncertainty
    pub variance_cm2: i32,

    // === New: Corridor info ===
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corridor_start_cm: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub corridor_end_cm: Option<i32>,

    // === New: Next stop (outside corridor) ===
    /// Next stop index and probability (even if not in corridor)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_stop: Option<(u8, Prob8)>,
}

#[derive(Serialize, Deserialize)]
pub struct StopTraceState {
    pub stop_idx: u8,

    /// GPS distance to stop (cm) - based on raw GPS projection (z_gps_cm)
    /// Used for p1 (Feature 1: distance likelihood)
    pub gps_distance_cm: DistCm,

    /// Progress distance to stop (cm) - based on Kalman-filtered position (s_cm)
    /// Used for p3 (Feature 3: progress difference likelihood)
    pub progress_distance_cm: DistCm,

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

/// Individual feature scores for trace output (std/testing only)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureScores {
    pub p1: u8,  // Raw GPS distance likelihood (F1)
    pub p2: u8,  // Speed likelihood (F2)
    pub p3: u8,  // Kalman distance likelihood (F3)
    pub p4: u8,  // Dwell time likelihood (F4)
}

/// v8.4: Voice announcement event
#[derive(Serialize)]
pub struct AnnounceEvent {
    /// GPS timestamp (seconds since epoch)
    pub time: u64,
    /// Stop index being announced
    pub stop_idx: u8,
    /// Route progress at announcement (cm)
    pub s_cm: DistCm,
    /// Velocity at announcement (cm/s)
    pub v_cms: SpeedCms,
}

/// Write an announcement event to the output file
pub fn write_announce_event<W: Write>(
    output: &mut BufWriter<W>,
    event: &AnnounceEvent,
) -> std::io::Result<()> {
    let json = serde_json::to_string(event)?;
    writeln!(output, "{}", json)
}

/// Write a trace record to the output file
pub fn write_trace_record<W: Write>(
    output: &mut BufWriter<W>,
    record: &TraceRecord,
) -> std::io::Result<()> {
    let json = serde_json::to_string(record)?;
    writeln!(output, "{}", json)
}

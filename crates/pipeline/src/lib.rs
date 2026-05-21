//! Bus Arrival Detection Pipeline Library
//!
//! This crate provides a complete pipeline for processing GPS NMEA data
//! and detecting bus arrivals/departures.
//!
//! # Example
//!
//! ```no_run
//! use pipeline::Pipeline;
//!
//! let result = Pipeline::process_nmea_file(
//!     "gps.nmea",
//!     "route_data.bin",
//! )?;
//!
//! println!("Detected {} arrivals", result.arrivals.len());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod gps;
#[cfg(feature = "std")]
pub mod jsonl_reader;
pub mod serde;
pub mod detection_state;
pub mod localization;

// Re-export trace types from detection crate
#[cfg(feature = "std")]
pub use detection::trace::{
    CorridorTrace,
    DetectionTrace,
    GpsTrace,
    KalmanTrace,
    MapMatchingTrace,
    StopTraceState,
    TraceRecord,
};
#[cfg(feature = "std")]
#[derive(Debug)]
pub struct TraceRecordWrapper {
    pub record: TraceRecord,
    pub time_ms: shared::TimestampMs,
    pub lat: f64,
    pub lon: f64,
    pub s_cm: shared::DistCm,
    pub v_cms: shared::SpeedCms,
    pub heading_cdeg: Option<shared::HeadCdeg>,
    pub active_stops: Vec<u8>,
    pub gps_jump: bool,
    pub recovery_idx: Option<u8>,
    pub segment_idx: Option<u16>,
    pub heading_constraint_met: bool,
    pub divergence_cm: i32,
    pub hdop: Option<f32>,
    pub accuracy_cm: Option<shared::DistCm>,
    pub num_sats: Option<u8>,
    pub fix_type: Option<String>,
    pub variance_cm2: i32,
    pub corridor_start_cm: Option<i32>,
    pub corridor_end_cm: Option<i32>,
    pub next_stop: Option<(u8, shared::Prob8)>,
    pub off_route: bool,
    pub status: String,
}

#[cfg(feature = "std")]
impl ::std::ops::Deref for TraceRecordWrapper {
    type Target = TraceRecord;

    fn deref(&self) -> &Self::Target {
        &self.record
    }
}

#[cfg(feature = "std")]
impl ::serde::Serialize for TraceRecordWrapper {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ::serde::Serializer,
    {
        self.record.serialize(serializer)
    }
}

/// Detour re-entry jump threshold in centimeters.
/// When a bus returns from off-route status with a forward jump greater than this,
/// it indicates the bus has snapped back to the route after a detour.
/// Stops behind the snap position should be skipped to prevent phantom arrivals.
pub const DETOUR_JUMP_THRESHOLD_CM: i32 = 10000;

use shared::binfile::RouteData;
use shared::{GpsPoint, KalmanState, DrState};
use thiserror::Error;

#[cfg(feature = "std")]
use std::path::Path;
#[cfg(feature = "std")]
use std::io::BufRead;

// Re-export from sub-crates
pub use detection::state_machine::{StopState, StopEvent};
pub use detection_state::DetectionState;


/// Pipeline result containing arrival and departure events
#[derive(Debug)]
pub struct PipelineResult {
    /// Arrival events detected
    pub arrivals: Vec<ArrivalEvent>,
    /// Departure events detected
    pub departures: Vec<DepartureEvent>,
    /// Trace records
    #[cfg(feature = "std")]
    pub trace_records: Vec<TraceRecordWrapper>,
}

/// Arrival event
pub type ArrivalEvent = shared::ArrivalEvent;

/// Departure event
pub type DepartureEvent = shared::DepartureEvent;






/// Pipeline errors
#[derive(Error, Debug)]
pub enum PipelineError {
    #[error("Failed to read/write file: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Failed to load route data: {0:?}")]
    RouteDataError(#[from] shared::binfile::BusError),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Buffer too small for serialization")]
    BufferTooSmall,
}

/// Pipeline processor
pub struct Pipeline;

/// Localization state (Phase 2: GPS processing)
pub struct LocalizationState {
    /// Kalman filter state
    kalman: KalmanState,
    /// Dead-reckoning state
    dr: DrState,
    /// First fix flag
    is_first_fix: bool,
}

impl LocalizationState {
    pub fn new(_route_data: &RouteData) -> Self {
        Self {
            kalman: KalmanState::new(),
            dr: DrState::new(),
            is_first_fix: true,
        }
    }

    /// Process GPS point and return GpsRecord if valid
    pub fn process_gps(
        &mut self,
        gps: &GpsPoint,
        route_data: &RouteData,
        accuracy_cm: Option<shared::DistCm>,
    ) -> Option<gps::GpsRecord> {
        let result = gps_processor::kalman::process_gps_update(
            &mut self.kalman,
            &mut self.dr,
            gps,
            route_data,
            gps.timestamp,
            self.is_first_fix,
            0, // current_stop_idx - TODO: track in PipelineState if needed
        );

        self.is_first_fix = false;

        match result {
            gps_processor::kalman::ProcessResult::Valid { signals, v_cms, seg_idx, snapped: _ } => {
                let shared::PositionSignals { z_gps_cm, s_cm } = signals;
                let divergence_cm = z_gps_cm - s_cm;
                let hdop = gps.hdop_x10.filter(|&v| v > 0).map(|v| v as f32 / 10.0);
                Some(gps::GpsRecord::new(
                    gps.timestamp,
                    gps.lat,
                    gps.lon,
                    s_cm,
                    v_cms,
                    gps.heading_cdeg,
                    "valid",
                ).with_diagnostics(
                    localization::GpsDiagnostics::new()
                        .with_segment_idx(Some(seg_idx as u16))
                        .with_heading_met(true)
                        .with_divergence_cm(divergence_cm)
                        .with_hdop(hdop)
                        .with_accuracy_cm(accuracy_cm)
                ))
            }
            gps_processor::kalman::ProcessResult::DrOutage { s_cm, v_cms } => {
                Some(gps::GpsRecord::new(
                    gps.timestamp,
                    gps.lat,
                    gps.lon,
                    s_cm,
                    v_cms,
                    gps.heading_cdeg,  // CRITICAL: Preserve heading even in DR mode
                    "dr_outage",
                ).with_diagnostics(
                    localization::GpsDiagnostics::new().with_accuracy_cm(accuracy_cm)
                ))
            }
            gps_processor::kalman::ProcessResult::OffRoute { last_valid_s, last_valid_v, freeze_time: _ } => {
                Some(gps::GpsRecord::new(
                    gps.timestamp,
                    gps.lat,
                    gps.lon,
                    last_valid_s,
                    last_valid_v,
                    None,
                    "off_route",
                ).with_diagnostics(
                    localization::GpsDiagnostics::new().with_accuracy_cm(accuracy_cm)
                ))
            }
            gps_processor::kalman::ProcessResult::SuspectOffRoute { s_cm, v_cms } => {
                Some(gps::GpsRecord::new(
                    gps.timestamp,
                    gps.lat,
                    gps.lon,
                    s_cm,
                    v_cms,
                    None,
                    "suspect_off_route",
                ).with_diagnostics(
                    localization::GpsDiagnostics::new().with_accuracy_cm(accuracy_cm)
                ))
            }
            gps_processor::kalman::ProcessResult::Rejected(_) => None,
            gps_processor::kalman::ProcessResult::Outage => None,
        }
    }
}

impl Pipeline {
    /// Process NMEA file and detect arrivals/departures
    ///
    /// # Arguments
    ///
    /// * `nmea_path` - Path to NMEA log file
    /// * `route_data_path` - Path to route_data.bin
    #[cfg(feature = "std")]
    pub fn process_nmea_file(
        nmea_path: impl AsRef<Path>,
        route_data_path: impl AsRef<Path>,
    ) -> Result<PipelineResult, PipelineError> {
        Self::process_file(nmea_path, route_data_path)
    }

    /// Process GPS file and detect arrivals/departures.
    /// Automatically dispatches by file extension.
    #[cfg(feature = "std")]
    pub fn process_file(
        input_path: impl AsRef<Path>,
        route_data_path: impl AsRef<Path>,
    ) -> Result<PipelineResult, PipelineError> {
        use std::fs::File;
        use std::io::BufReader;

        let route_buffer = std::fs::read(route_data_path.as_ref())?;
        let route_data = RouteData::load(&route_buffer)?;
        let input_path = input_path.as_ref();

        match InputFormat::from_path(input_path) {
            InputFormat::Nmea => {
                let file = File::open(input_path)?;
                let reader = BufReader::new(file);
                Self::process_nmea_reader(reader, &route_data)
            }
            InputFormat::Jsonl => {
                let file = File::open(input_path)?;
                let reader = BufReader::new(file);
                Self::process_jsonl_reader(reader, &route_data)
            }
        }
    }

    /// Process NMEA from a BufRead reader and detect arrivals/departures
    ///
    /// # Arguments
    ///
    /// * `reader` - BufReader over NMEA data
    /// * `route_data` - Loaded route data
    ///
    /// # Returns
    ///
    /// Returns `PipelineResult` containing arrivals, departures, and trace records
    #[cfg(feature = "std")]
    pub fn process_nmea_reader<R: BufRead>(
        reader: R,
        route_data: &RouteData,
    ) -> Result<PipelineResult, PipelineError> {
        let mut result = PipelineResult::new();

        // Initialize localization state
        let mut loc_state = LocalizationState::new(route_data);

        // Initialize detection state
        let mut det_state = DetectionState::new(route_data);

        // Initialize NMEA accumulator
        let mut nmea_acc = gps_processor::FixAccumulator::new();

        // Process NMEA sentences
        for line in reader.lines() {
            let line = line.map_err(PipelineError::IoError)?;

            // Update accumulator with NMEA sentence
            if nmea_acc.update(&line) {
                // Check if we have a complete fix
                if nmea_acc.should_emit() {
                    if let Some((gps, _fix_quality)) = nmea_acc.build() {
                        // Phase 2: Localization (Kalman + Map Matching)
                        if let Some(gps_record) = loc_state.process_gps(&gps, route_data, None) {
                            // Phase 3: Arrival Detection
                            det_state.process_gps_record(&gps_record, route_data, &mut result);

                            // Add trace record (after detection so we have stop states)
                            #[cfg(feature = "std")]
                            result.add_trace_record(&gps_record, &mut det_state, route_data);
                        }
                    }
                }
            }
        }

        Ok(result)
    }

    /// Process JSONL from a BufRead reader and detect arrivals/departures.
    #[cfg(feature = "std")]
    pub fn process_jsonl_reader<R: BufRead>(
        reader: R,
        route_data: &RouteData,
    ) -> Result<PipelineResult, PipelineError> {
        let mut result = PipelineResult::new();

        let mut loc_state = LocalizationState::new(route_data);
        let mut det_state = DetectionState::new(route_data);
        let mut jsonl_reader = jsonl_reader::JsonReader::new();

        for line in reader.lines() {
            let line = line.map_err(PipelineError::IoError)?;

            if let Some(record) = jsonl_reader.parse_line(&line) {
                let gps = record.gps;
                if let Some(gps_record) = loc_state.process_gps(&gps, route_data, record.accuracy_cm) {
                    det_state.process_gps_record(&gps_record, route_data, &mut result);

                    #[cfg(feature = "std")]
                    result.add_trace_record(&gps_record, &mut det_state, route_data);
                }
            }
        }

        Ok(result)
    }

}

#[cfg(feature = "std")]
enum InputFormat {
    Nmea,
    Jsonl,
}

#[cfg(feature = "std")]
impl InputFormat {
    fn from_path(path: &Path) -> Self {
        match path.extension().and_then(|e| e.to_str()) {
            Some("jsonl") => Self::Jsonl,
            _ => Self::Nmea,
        }
    }
}

impl PipelineResult {
    /// Create new PipelineResult
    #[cfg(feature = "std")]
    fn new() -> Self {
        Self {
            arrivals: Vec::new(),
            departures: Vec::new(),
            trace_records: Vec::new(),
        }
    }

    /// Create new PipelineResult (no_std version)
    #[cfg(not(feature = "std"))]
    fn new() -> Self {
        Self {
            arrivals: Vec::new(),
            departures: Vec::new(),
        }
    }

    /// Add a trace record
    #[cfg(feature = "std")]
    fn add_trace_record(&mut self, record: &gps::GpsRecord, det_state: &mut DetectionState, route_data: &RouteData) {
        let (active_stops, stop_states) = det_state.get_trace_info(record, route_data);

        // Compute corridor info from first active stop
        let (corridor_start_cm, corridor_end_cm) = if let Some(&first_idx) = det_state.active_indices().first() {
            let stop = &route_data.stops()[first_idx];
            (Some(stop.corridor_start_cm), Some(stop.corridor_end_cm))
        } else {
            (None, None)
        };

        // Find next stop outside corridor
        let next_stop = if let Some(end) = corridor_end_cm {
            let mut result = None;
            for (idx, stop) in route_data.stops().iter().enumerate() {
                if stop.progress_cm > end {
                    // Get probability from stop_states if available
                    let prob = stop_states.iter()
                        .find(|s| s.stop_idx == idx as u8)
                        .map(|s| s.probability)
                        .unwrap_or(0);
                    // Only include if not at final stop
                    if idx < route_data.stops().len() - 1 {
                        result = Some((idx as u8, prob));
                        break;
                    }
                }
            }
            result
        } else {
            None
        };

        // Determine off-route status from GPS record status
        let gps_jump = false;
        let recovery_idx = None;
        let off_route = record.status == "off_route";
        let status = record.status.to_string();

        let trace_record = TraceRecord {
            gps: GpsTrace {
                time_ms: record.time,
                lat: record.lat,
                lon: record.lon,
                heading_cdeg: record.heading_cdeg,
                hdop: record.hdop,
                accuracy_cm: record.accuracy_cm,
                num_sats: record.num_sats,
                fix_type: record.fix_type.clone(),
            },
            kalman: KalmanTrace {
                s_cm: record.s_cm,
                v_cms: record.v_cms,
                variance_cm2: record.variance_cm2,
                divergence_cm: record.divergence_cm,
            },
            map_matching: MapMatchingTrace {
                segment_idx: record.segment_idx,
                heading_constraint_met: record.heading_constraint_met,
            },
            detection: DetectionTrace {
                status: status.clone(),
                off_route,
                gps_jump,
                recovery_idx,
                off_route_last_s_cm: det_state.off_route_last_s_cm(),
            },
            corridor: CorridorTrace {
                active_stops: active_stops.clone(),
                corridor_start_cm,
                corridor_end_cm,
                next_stop,
            },
            stop_states,
        };

        self.trace_records.push(TraceRecordWrapper {
            time_ms: trace_record.gps.time_ms,
            lat: trace_record.gps.lat,
            lon: trace_record.gps.lon,
            s_cm: trace_record.kalman.s_cm,
            v_cms: trace_record.kalman.v_cms,
            heading_cdeg: trace_record.gps.heading_cdeg,
            active_stops,
            gps_jump,
            recovery_idx,
            segment_idx: trace_record.map_matching.segment_idx,
            heading_constraint_met: trace_record.map_matching.heading_constraint_met,
            divergence_cm: trace_record.kalman.divergence_cm,
            hdop: trace_record.gps.hdop,
            accuracy_cm: trace_record.gps.accuracy_cm,
            num_sats: trace_record.gps.num_sats,
            fix_type: trace_record.gps.fix_type.clone(),
            variance_cm2: trace_record.kalman.variance_cm2,
            corridor_start_cm: trace_record.corridor.corridor_start_cm,
            corridor_end_cm: trace_record.corridor.corridor_end_cm,
            next_stop: trace_record.corridor.next_stop,
            off_route,
            status,
            record: trace_record,
        });
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use std::fs;

    fn load_route_data() -> RouteData<'static> {
        let route_bytes = fs::read("../../test_data/ty225_normal.bin")
            .expect("Failed to load ty225_normal.bin");
        let route_bytes: &'static [u8] = Box::leak(route_bytes.into_boxed_slice());
        RouteData::load(route_bytes).expect("Failed to parse ty225_normal.bin")
    }

    #[test]
    fn add_trace_record_uses_grouped_trace_v2_fields() {
        let route_data = load_route_data();
        let stop = &route_data.stops()[0];
        let mut det_state = DetectionState::new(&route_data);
        let mut result = PipelineResult::new();

        let record = gps::GpsRecord::new(
            1_234_567,
            25.0,
            121.0,
            stop.corridor_start_cm,
            250,
            Some(9000),
            "valid",
        )
        .with_diagnostics(
            localization::GpsDiagnostics::new()
                .with_segment_idx(Some(7))
                .with_heading_met(true)
                .with_divergence_cm(33)
                .with_hdop(Some(1.5))
                .with_accuracy_cm(Some(250))
                .with_num_sats(Some(9))
                .with_fix_type(Some("3d".to_string()))
                .with_variance_cm2(144),
        );

        det_state.process_gps_record(&record, &route_data, &mut result);
        result.add_trace_record(&record, &mut det_state, &route_data);

        let trace = &result.trace_records[0];
        assert_eq!(trace.gps.time_ms, 1_234_567);
        assert_eq!(trace.gps.heading_cdeg, Some(9000));
        assert_eq!(trace.gps.hdop, Some(1.5));
        assert_eq!(trace.kalman.s_cm, stop.corridor_start_cm);
        assert_eq!(trace.kalman.divergence_cm, 33);
        assert_eq!(trace.map_matching.segment_idx, Some(7));
        assert!(trace.map_matching.heading_constraint_met);
        assert_eq!(trace.detection.status, "valid");
        assert!(!trace.detection.off_route);
        assert_eq!(trace.corridor.active_stops, vec![0]);
        assert!(!trace.stop_states.is_empty());
    }

    #[test]
    fn add_trace_record_only_marks_exact_off_route_status_as_off_route() {
        let route_data = load_route_data();
        let mut det_state = DetectionState::new(&route_data);
        let mut result = PipelineResult::new();

        let suspect_record = gps::GpsRecord::new(10, 25.0, 121.0, 1000, 0, None, "suspect_off_route");
        det_state.process_gps_record(&suspect_record, &route_data, &mut result);
        result.add_trace_record(&suspect_record, &mut det_state, &route_data);

        let off_route_record = gps::GpsRecord::new(11, 25.0, 121.0, 2000, 0, None, "off_route");
        det_state.process_gps_record(&off_route_record, &route_data, &mut result);
        result.add_trace_record(&off_route_record, &mut det_state, &route_data);

        let suspect_trace = &result.trace_records[0];
        assert_eq!(suspect_trace.detection.status, "suspect_off_route");
        assert!(!suspect_trace.detection.off_route);

        let off_route_trace = &result.trace_records[1];
        assert_eq!(off_route_trace.detection.status, "off_route");
        assert!(off_route_trace.detection.off_route);
        assert_eq!(off_route_trace.detection.off_route_last_s_cm, Some(2000));
    }
}

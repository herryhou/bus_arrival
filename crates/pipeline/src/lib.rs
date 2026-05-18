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
pub mod serde;
pub mod detection_state;
pub mod localization;

// Re-export trace types from detection crate
#[cfg(feature = "std")]
pub use detection::trace::{TraceRecord, StopTraceState};
#[cfg(feature = "std")]
#[derive(Debug)]
pub struct TraceRecordWrapper(pub TraceRecord);

#[cfg(feature = "std")]
impl ::std::ops::Deref for TraceRecordWrapper {
    type Target = TraceRecord;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(feature = "std")]
impl ::serde::Serialize for TraceRecordWrapper {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ::serde::Serializer,
    {
        self.0.serialize(serializer)
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
    pub fn process_gps(&mut self, gps: &GpsPoint, route_data: &RouteData) -> Option<gps::GpsRecord> {
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
                    localization::GpsDiagnostics::new()
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
                    localization::GpsDiagnostics::new()
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
                    localization::GpsDiagnostics::new()
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
        use std::fs::File;
        use std::io::BufReader;

        // Load route data
        let route_buffer = std::fs::read(route_data_path.as_ref())?;
        let route_data = RouteData::load(&route_buffer)?;

        // Parse NMEA and process
        let nmea_file = File::open(nmea_path.as_ref())?;
        let reader = BufReader::new(nmea_file);

        let result = Self::process_nmea_reader(
            reader,
            &route_data,
        )?;

        Ok(result)
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
                        if let Some(gps_record) = loc_state.process_gps(&gps, route_data) {
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
        let off_route = match record.status {
            "off_route" | "suspect_off_route" => Some(true),
            "valid" | "dr_outage" => Some(false),
            _ => None,
        };

        self.trace_records.push(TraceRecordWrapper(TraceRecord {
            time: record.time,
            lat: record.lat,
            lon: record.lon,
            s_cm: record.s_cm,
            v_cms: record.v_cms,
            heading_cdeg: record.heading_cdeg,
            active_stops,
            stop_states,
            gps_jump: false,  // TODO: implement GPS jump detection
            recovery_idx: None, // TODO: implement recovery
            // New fields
            segment_idx: record.segment_idx,
            heading_constraint_met: record.heading_constraint_met,
            divergence_cm: record.divergence_cm,
            hdop: record.hdop,
            num_sats: record.num_sats,
            fix_type: record.fix_type.clone(),
            variance_cm2: record.variance_cm2,
            corridor_start_cm,
            corridor_end_cm,
            next_stop,
            off_route,
        }));
    }
}

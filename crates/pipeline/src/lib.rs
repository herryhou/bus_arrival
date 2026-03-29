//! Bus Arrival Detection Pipeline Library
//!
//! This crate provides a complete pipeline for processing GPS NMEA data
//! and detecting bus arrivals/departures.
//!
//! # Example
//!
//! ```no_run
//! use pipeline::{Pipeline, PipelineConfig};
//!
//! let config = PipelineConfig::default();
//! let result = Pipeline::process_nmea_file(
//!     "gps.nmea",
//!     "route_data.bin",
//!     "output.jsonl",
//!     &config
//! )?;
//!
//! println!("Detected {} arrivals", result.arrivals.len());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod gps;

use shared::binfile::RouteData;
use shared::{GpsPoint, KalmanState, DrState};
use std::path::Path;
use std::io::{BufRead, Write};
use thiserror::Error;

// Re-export from sub-crates
pub use gps_processor::nmea::NmeaState;
pub use detection::state_machine::{StopState, StopEvent};

/// Configuration for pipeline processing
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// Enable trace output (for debugging)
    pub enable_trace: bool,
    /// Enable announce output
    pub enable_announce: bool,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            enable_trace: false,
            enable_announce: false,
        }
    }
}

/// Pipeline result containing arrival and departure events
#[derive(Debug)]
pub struct PipelineResult {
    /// Arrival events detected
    pub arrivals: Vec<ArrivalEvent>,
    /// Departure events detected
    pub departures: Vec<DepartureEvent>,
    /// Trace records (if enabled)
    pub trace_records: Option<Vec<TraceRecord>>,
    /// Announce events (if enabled)
    pub announce_events: Option<Vec<AnnounceEvent>>,
}

/// Arrival event
pub type ArrivalEvent = shared::ArrivalEvent;

/// Departure event
pub type DepartureEvent = shared::DepartureEvent;

/// Trace record for debugging
#[derive(Debug, Clone, serde::Serialize)]
pub struct TraceRecord {
    pub time: u64,
    pub lat: f64,
    pub lon: f64,
    pub s_cm: i32,
    pub v_cms: i32,
    pub heading_cdeg: Option<i16>,
    pub active_stops: Vec<u8>,
    pub stop_states: Vec<StopTraceState>,
    pub gps_jump: bool,
    pub recovery_idx: Option<u8>,
}

/// Stop state in trace
#[derive(Debug, Clone, serde::Serialize)]
pub struct StopTraceState {
    pub stop_idx: u8,
    pub distance_cm: i32,
    pub fsm_state: String,
    pub dwell_time_s: u16,
    pub probability: u8,
    pub features: FeatureScores,
    pub just_arrived: bool,
}

/// Feature scores for probability model
#[derive(Debug, Clone, serde::Serialize)]
pub struct FeatureScores {
    pub p1: u8,
    pub p2: u8,
    pub p3: u8,
    pub p4: u8,
}

/// Announce event
#[derive(Debug, Clone, serde::Serialize)]
pub struct AnnounceEvent {
    pub time: u64,
    pub stop_idx: u8,
    pub s_cm: i32,
    pub v_cms: i32,
}

/// Pipeline errors
#[derive(Error, Debug)]
pub enum PipelineError {
    #[error("Failed to read/write file: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Failed to load route data: {0:?}")]
    RouteDataError(#[from] shared::binfile::BusError),
}

/// Pipeline processor
pub struct Pipeline;

/// Localization state (Phase 2: GPS processing)
pub struct LocalizationState<'a> {
    /// NMEA parser
    pub nmea: NmeaState,
    /// Kalman filter state
    kalman: KalmanState,
    /// Dead-reckoning state
    dr: DrState,
    /// Route data reference
    route_data: std::marker::PhantomData<&'a ()>,
    /// First fix flag
    is_first_fix: bool,
}

impl<'a> LocalizationState<'a> {
    pub fn new(_route_data: &RouteData) -> Self {
        Self {
            nmea: NmeaState::new(),
            kalman: KalmanState::new(),
            dr: DrState::new(),
            route_data: std::marker::PhantomData,
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
        );

        self.is_first_fix = false;

        match result {
            gps_processor::kalman::ProcessResult::Valid { s_cm, v_cms, seg_idx: _ } => {
                Some(gps::GpsRecord {
                    time: gps.timestamp,
                    lat: gps.lat,
                    lon: gps.lon,
                    s_cm,
                    v_cms,
                    heading_cdeg: Some(gps.heading_cdeg),
                })
            }
            gps_processor::kalman::ProcessResult::DrOutage { s_cm, v_cms } => {
                Some(gps::GpsRecord {
                    time: gps.timestamp,
                    lat: gps.lat,
                    lon: gps.lon,
                    s_cm,
                    v_cms,
                    heading_cdeg: None,
                })
            }
            _ => None,
        }
    }
}

/// Detection state (Phase 3: Arrival detection)
pub struct DetectionState {
    /// Per-stop state machines
    stop_states: Vec<StopState>,
    /// Time counter for announcements
    time_counter: u64,
}

impl DetectionState {
    pub fn new(route_data: &RouteData) -> Self {
        let stop_count = route_data.stops().len();
        let mut stop_states = Vec::with_capacity(stop_count);
        for i in 0..stop_count {
            stop_states.push(StopState::new(i as u8));
        }
        Self {
            stop_states,
            time_counter: 0,
        }
    }

    /// Process a GPS record and update result with arrivals/departures
    pub fn process_gps_record(
        &mut self,
        record: &gps::GpsRecord,
        route_data: &RouteData,
        result: &mut PipelineResult,
    ) {
        self.time_counter = record.time;

        let s_cm = record.s_cm;
        let v_cms = record.v_cms;
        let stops = route_data.stops();

        // Find active stops (corridor filter)
        let mut active_indices = Vec::new();
        for (idx, stop) in stops.iter().enumerate() {
            if s_cm >= stop.corridor_start_cm && s_cm <= stop.corridor_end_cm {
                active_indices.push(idx);
            }
        }

        // Process each active stop
        for idx in active_indices {
            let stop = &stops[idx];
            let stop_state = &mut self.stop_states[idx];

            // Compute probability
            let probability = detection::probability::compute_probability(
                s_cm,
                v_cms,
                stop.progress_cm,
                stop_state.dwell_time_s,
            );

            // Update state machine
            let event = stop_state.update(
                s_cm,
                v_cms,
                stop.progress_cm,
                stop.corridor_start_cm,
                probability,
            );

            // Handle events
            match event {
                StopEvent::Arrived => {
                    result.arrivals.push(ArrivalEvent {
                        time: record.time,
                        stop_idx: idx as u8,
                        s_cm: record.s_cm as i32,
                        v_cms: record.v_cms,
                        probability,
                    });
                }
                StopEvent::Departed => {
                    result.departures.push(DepartureEvent {
                        time: record.time,
                        stop_idx: idx as u8,
                        s_cm: record.s_cm as i32,
                        v_cms: record.v_cms,
                    });
                }
                StopEvent::None => {}
            }
        }

        // Check for announcements (v8.4: corridor entry announcement)
        if result.announce_events.is_some() {
            for (idx, stop_state) in self.stop_states.iter_mut().enumerate() {
                if stop_state.should_announce(s_cm, stops[idx].corridor_start_cm) {
                    result.announce_events.as_mut().unwrap().push(AnnounceEvent {
                        time: record.time,
                        stop_idx: idx as u8,
                        s_cm: record.s_cm as i32,
                        v_cms: record.v_cms,
                    });
                }
            }
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
    /// * `output_path` - Path to write arrival/departure events
    /// * `config` - Pipeline configuration
    pub fn process_nmea_file(
        nmea_path: impl AsRef<Path>,
        route_data_path: impl AsRef<Path>,
        output_path: impl AsRef<Path>,
        config: &PipelineConfig,
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
            config,
        )?;

        // Write output
        Self::write_output(&result, output_path)?;

        Ok(result)
    }

    /// Process NMEA from a BufRead reader and detect arrivals/departures
    ///
    /// # Arguments
    ///
    /// * `reader` - BufReader over NMEA data
    /// * `route_data` - Loaded route data
    /// * `config` - Pipeline configuration
    ///
    /// # Returns
    ///
    /// Returns `PipelineResult` containing arrivals, departures, and optional trace/announce events
    pub fn process_nmea_reader<R: BufRead>(
        reader: R,
        route_data: &RouteData,
        config: &PipelineConfig,
    ) -> Result<PipelineResult, PipelineError> {
        let mut result = PipelineResult::new(config);

        // Initialize localization state
        let mut loc_state = LocalizationState::new(route_data);

        // Initialize detection state
        let mut det_state = DetectionState::new(route_data);

        // Process NMEA sentences
        for line in reader.lines() {
            let line = line.map_err(|e| PipelineError::IoError(e))?;

            if let Some(gps) = loc_state.nmea.parse_sentence(&line) {
                // Phase 2: Localization (Kalman + Map Matching)
                if let Some(gps_record) = loc_state.process_gps(&gps, route_data) {
                    // Add trace record if enabled
                    if config.enable_trace {
                        result.add_trace_record(&gps_record);
                    }

                    // Phase 3: Arrival Detection
                    det_state.process_gps_record(&gps_record, route_data, &mut result);
                }
            }
        }

        Ok(result)
    }

    fn write_output(result: &PipelineResult, output_path: impl AsRef<Path>) -> Result<(), PipelineError> {
        use std::fs::File;
        use std::io::BufWriter;

        let file = File::create(output_path.as_ref())?;
        let mut writer = BufWriter::new(file);

        // Merge arrivals and departures by time for chronological order
        let mut events = Vec::new();
        for arrival in &result.arrivals {
            events.push((arrival.time, serde_json::to_string(arrival).unwrap()));
        }
        for departure in &result.departures {
            events.push((departure.time, serde_json::to_string(departure).unwrap()));
        }
        events.sort_by_key(|(time, _)| *time);

        // Write events in chronological order
        for (_, event_json) in events {
            writeln!(writer, "{}", event_json)?;
        }

        writer.flush()?;
        Ok(())
    }
}

impl PipelineResult {
    /// Create new PipelineResult with optional trace/announce based on config
    fn new(config: &PipelineConfig) -> Self {
        Self {
            arrivals: Vec::new(),
            departures: Vec::new(),
            trace_records: if config.enable_trace { Some(Vec::new()) } else { None },
            announce_events: if config.enable_announce { Some(Vec::new()) } else { None },
        }
    }

    /// Add a trace record (only if trace is enabled)
    fn add_trace_record(&mut self, record: &gps::GpsRecord) {
        if let Some(ref mut trace) = self.trace_records {
            trace.push(TraceRecord {
                time: record.time,
                lat: record.lat,
                lon: record.lon,
                s_cm: record.s_cm as i32,
                v_cms: record.v_cms,
                heading_cdeg: record.heading_cdeg,
                active_stops: Vec::new(),
                stop_states: Vec::new(),
                gps_jump: false,
                recovery_idx: None,
            });
        }
    }
}

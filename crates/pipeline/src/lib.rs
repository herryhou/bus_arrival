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
pub mod serde;

use shared::binfile::RouteData;
use shared::{GpsPoint, KalmanState, DrState};
use thiserror::Error;

#[cfg(feature = "std")]
use std::path::Path;
#[cfg(feature = "std")]
use std::io::{BufRead, Write};

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
    #[cfg(feature = "std")]
    pub trace_records: Option<Vec<TraceRecord>>,
    /// Announce events (if enabled)
    #[cfg(feature = "std")]
    pub announce_events: Option<Vec<AnnounceEvent>>,
}

/// Arrival event
pub type ArrivalEvent = shared::ArrivalEvent;

/// Departure event
pub type DepartureEvent = shared::DepartureEvent;

/// Trace record for debugging
#[cfg(feature = "std")]
#[derive(Debug, Clone, ::serde::Serialize)]
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
#[cfg(feature = "std")]
#[derive(Debug, Clone, ::serde::Serialize)]
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
#[cfg(feature = "std")]
#[derive(Debug, Clone, ::serde::Serialize)]
pub struct FeatureScores {
    pub p1: u8,
    pub p2: u8,
    pub p3: u8,
    pub p4: u8,
}

/// Announce event
#[cfg(feature = "std")]
#[derive(Debug, Clone, ::serde::Serialize)]
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

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Buffer too small for serialization")]
    BufferTooSmall,
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
            gps_processor::kalman::ProcessResult::Valid { signals, v_cms, seg_idx: _ } => {
                let shared::PositionSignals { z_gps_cm: _, s_cm } = signals;
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
    /// Current GPS timestamp counter (for trace output)
    current_timestamp: u64,
    /// Track which stops arrived this frame (for trace output)
    arrived_this_frame: Vec<u8>,
    /// Active stop indices from last update (for trace output)
    active_indices: Vec<usize>,
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
            current_timestamp: 0,
            arrived_this_frame: Vec::new(),
            active_indices: Vec::new(),
        }
    }

    /// Increment timestamp for each GPS record processed
    pub fn tick(&mut self) {
        self.current_timestamp += 1;
    }

    /// Process a GPS record and update result with arrivals/departures
    pub fn process_gps_record(
        &mut self,
        record: &gps::GpsRecord,
        route_data: &RouteData,
        result: &mut PipelineResult,
    ) {
        self.current_timestamp = record.time;

        // Reset per-frame tracking
        self.arrived_this_frame.clear();
        self.active_indices.clear();

        let s_cm = record.s_cm;
        let v_cms = record.v_cms;
        let stops = route_data.stops();

        // Find active stops (corridor filter)
        for (idx, stop) in stops.iter().enumerate() {
            if s_cm >= stop.corridor_start_cm && s_cm <= stop.corridor_end_cm {
                self.active_indices.push(idx);
            }
        }

        // Process each active stop
        for idx in &self.active_indices {
            let stop = &stops[*idx];
            let stop_state = &mut self.stop_states[*idx];

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
                    self.arrived_this_frame.push(*idx as u8);
                    result.arrivals.push(ArrivalEvent {
                        time: record.time,
                        stop_idx: *idx as u8,
                        s_cm: record.s_cm as i32,
                        v_cms: record.v_cms,
                        probability,
                        event_type: shared::ArrivalEventType::Arrival,
                    });
                }
                StopEvent::Departed => {
                    result.departures.push(DepartureEvent {
                        time: record.time,
                        stop_idx: *idx as u8,
                        s_cm: record.s_cm as i32,
                        v_cms: record.v_cms,
                    });
                }
                StopEvent::None => {}
            }
        }

        // Check for announcements (v8.4: corridor entry announcement)
        #[cfg(feature = "std")]
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

    /// Get trace information for the last processed GPS record
    #[cfg(feature = "std")]
    pub fn get_trace_info(&self, record: &gps::GpsRecord, route_data: &RouteData) -> (Vec<u8>, Vec<StopTraceState>) {
        let stops = route_data.stops();

        // Build active_stops list
        let active_stops: Vec<u8> = self.active_indices.iter().map(|i| *i as u8).collect();

        // Build stop_states list for active stops
        let stop_states: Vec<StopTraceState> = self.active_indices.iter().map(|&idx| {
            let stop = &stops[idx];
            let stop_state = &self.stop_states[idx];

            // Re-compute probability for trace output
            let probability = detection::probability::compute_probability(
                record.s_cm,
                record.v_cms,
                stop.progress_cm,
                stop_state.dwell_time_s,
            );

            StopTraceState {
                stop_idx: idx as u8,
                distance_cm: (record.s_cm - stop.progress_cm) as i32,
                fsm_state: format!("{:?}", stop_state.fsm_state),
                dwell_time_s: stop_state.dwell_time_s,
                probability,
                features: FeatureScores {
                    p1: 0, // TODO: compute actual feature scores
                    p2: 0,
                    p3: 0,
                    p4: 0,
                },
                just_arrived: self.arrived_this_frame.contains(&(idx as u8)),
            }
        }).collect();

        (active_stops, stop_states)
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
    #[cfg(feature = "std")]
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
    #[cfg(feature = "std")]
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
                    // Phase 3: Arrival Detection
                    det_state.process_gps_record(&gps_record, route_data, &mut result);

                    // Add trace record if enabled (after detection so we have stop states)
                    #[cfg(feature = "std")]
                    if config.enable_trace {
                        result.add_trace_record(&gps_record, &det_state, route_data);
                    }
                }
            }
        }

        Ok(result)
    }

    #[cfg(feature = "std")]
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
    #[cfg(feature = "std")]
    fn new(config: &PipelineConfig) -> Self {
        Self {
            arrivals: Vec::new(),
            departures: Vec::new(),
            trace_records: if config.enable_trace { Some(Vec::new()) } else { None },
            announce_events: if config.enable_announce { Some(Vec::new()) } else { None },
        }
    }

    /// Create new PipelineResult (no_std version)
    #[cfg(not(feature = "std"))]
    fn new(_config: &PipelineConfig) -> Self {
        Self {
            arrivals: Vec::new(),
            departures: Vec::new(),
        }
    }

    /// Add a trace record (only if trace is enabled)
    #[cfg(feature = "std")]
    fn add_trace_record(&mut self, record: &gps::GpsRecord, det_state: &DetectionState, route_data: &RouteData) {
        if let Some(ref mut trace) = self.trace_records {
            let (active_stops, stop_states) = det_state.get_trace_info(record, route_data);

            trace.push(TraceRecord {
                time: record.time,
                lat: record.lat,
                lon: record.lon,
                s_cm: record.s_cm as i32,
                v_cms: record.v_cms,
                heading_cdeg: record.heading_cdeg,
                active_stops,
                stop_states,
                gps_jump: false,  // TODO: implement GPS jump detection
                recovery_idx: None, // TODO: implement recovery
            });
        }
    }
}

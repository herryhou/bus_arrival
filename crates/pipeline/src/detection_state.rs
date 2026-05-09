//! Arrival detection state machine

use shared::{DistCm, PositionSignals};
use shared::binfile::RouteData;
use crate::{StopTraceState, PipelineResult, ArrivalEvent, DepartureEvent, gps::GpsRecord, DETOUR_JUMP_THRESHOLD_CM};
use pipeline_filter as filter;
use pipeline_probability::ProbabilityEngine;
use detection::state_machine::{StopState, StopEvent};
use detection::probability::GpsStatus;

/// Detection state (Phase 3: Arrival detection)
pub struct DetectionState {
    /// Per-stop state machines
    stop_states: Vec<StopState>,
    /// Current GPS timestamp counter
    current_timestamp: u64,
    /// Track which stops arrived this frame
    arrived_this_frame: Vec<u8>,
    /// Active stop indices from last update
    active_indices: Vec<usize>,
    /// Track whether bus is currently off-route
    off_route: bool,
    /// Track last position during off-route
    off_route_last_s_cm: Option<DistCm>,
    /// Probability computation engine with caching
    prob_engine: ProbabilityEngine,
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
            off_route: false,
            off_route_last_s_cm: None,
            prob_engine: ProbabilityEngine::new(),
        }
    }

    /// Increment timestamp for each GPS record processed
    pub fn tick(&mut self) {
        self.current_timestamp += 1;
    }

    /// Process a GPS record and update result with arrivals/departures
    pub fn process_gps_record(
        &mut self,
        record: &GpsRecord,
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

        // Update off-route state
        match record.status {
            "off_route" => {
                self.off_route = true;
                self.off_route_last_s_cm = Some(record.s_cm);
            }
            "valid" => {
                let just_reentered = self.off_route;
                let large_forward_jump = self.off_route_last_s_cm
                    .map_or(false, |off_route_s| record.s_cm > off_route_s + DETOUR_JUMP_THRESHOLD_CM);

                self.off_route = false;
                self.off_route_last_s_cm = None;

                // Mark intermediate stops to skip on re-entry with large jump
                if just_reentered && large_forward_jump {
                    for (idx, stop) in stops.iter().enumerate() {
                        if stop.progress_cm < record.s_cm {
                            self.stop_states[idx].skip_on_reentry = true;
                        }
                    }
                }
            }
            "dr_outage" => {
                // Keep off_route state
            }
            _ => {}
        }

        // Find active stops (corridor filter)
        let skip_flags: Vec<bool> = self.stop_states.iter()
            .map(|s| s.skip_on_reentry)
            .collect();
        self.active_indices = filter::active_stops(s_cm, &stops, &skip_flags);

        // Process each active stop
        for idx in &self.active_indices {
            let stop = &stops[*idx];
            let stop_state = &mut self.stop_states[*idx];

            let signals = PositionSignals::new(record.s_cm, record.s_cm);
            let gps_status = match record.status {
                "valid" => GpsStatus::Valid,
                "dr_outage" => GpsStatus::DrOutage,
                "off_route" => GpsStatus::OffRoute,
                _ => GpsStatus::Valid,
            };
            let prob_result = self.prob_engine.compute(
                record.time,
                signals,
                v_cms,
                stop,
                stop_state.dwell_time_s,
                gps_status,
            );
            let probability = prob_result.probability;

            let event = stop_state.update(
                s_cm,
                v_cms,
                stop.progress_cm,
                stop.corridor_start_cm,
                probability,
            );

            match event {
                StopEvent::Arrived => {
                    self.arrived_this_frame.push(*idx as u8);
                    result.arrivals.push(ArrivalEvent {
                        time: record.time,
                        stop_idx: *idx as u8,
                        s_cm: record.s_cm,
                        v_cms: record.v_cms,
                        probability,
                        event_type: shared::ArrivalEventType::Arrival,
                    });
                }
                StopEvent::Departed => {
                    result.departures.push(DepartureEvent {
                        time: record.time,
                        stop_idx: *idx as u8,
                        s_cm: record.s_cm,
                        v_cms: record.v_cms,
                    });
                }
                StopEvent::None => {}
            }
        }
    }

    /// Get trace information for the last processed GPS record
    pub fn get_trace_info(&mut self, record: &GpsRecord, route_data: &RouteData) -> (Vec<u8>, Vec<StopTraceState>) {
        let stops = route_data.stops();

        let active_stops: Vec<u8> = self.active_indices.iter().map(|i| *i as u8).collect();

        let z_gps_cm = record.s_cm + record.divergence_cm;

        let stop_states: Vec<StopTraceState> = self.active_indices.iter().map(|&idx| {
            let stop = &stops[idx];
            let stop_state = &self.stop_states[idx];

            let signals = PositionSignals::new(record.s_cm, record.s_cm);
            let gps_status = match record.status {
                "valid" => GpsStatus::Valid,
                "dr_outage" => GpsStatus::DrOutage,
                "off_route" => GpsStatus::OffRoute,
                _ => GpsStatus::Valid,
            };

            // Use cached probability computation (same as process_gps_record)
            let prob_result = self.prob_engine.compute(
                record.time,
                signals,
                record.v_cms,
                stop,
                stop_state.dwell_time_s,
                gps_status,
            );

            StopTraceState {
                stop_idx: idx as u8,
                gps_distance_cm: z_gps_cm - stop.progress_cm,
                progress_distance_cm: record.s_cm - stop.progress_cm,
                fsm_state: format!("{:?}", stop_state.fsm_state),
                dwell_time_s: stop_state.dwell_time_s,
                probability: prob_result.probability,
                features: prob_result.features.clone(),
                just_arrived: self.arrived_this_frame.contains(&(idx as u8)),
                skip_on_reentry: stop_state.skip_on_reentry,
            }
        }).collect();

        (active_stops, stop_states)
    }

    /// Get active stop indices
    pub fn active_indices(&self) -> &[usize] {
        &self.active_indices
    }

    /// Check if currently off-route
    pub fn is_off_route(&self) -> bool {
        self.off_route
    }
}

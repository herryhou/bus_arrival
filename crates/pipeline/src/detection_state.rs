//! Arrival detection state machine

use shared::{DistCm, PositionSignals, Prob8, TimestampMs};
use shared::binfile::RouteData;
use crate::{PipelineResult, ArrivalEvent, DepartureEvent, gps::GpsRecord, StopTraceState};
use detection::state_machine::{StopState, StopEvent};

/// Detection state (Phase 3: Arrival detection)
pub struct DetectionState {
    /// Per-stop state machines
    stop_states: Vec<StopState>,
    /// Current GPS timestamp in milliseconds since epoch
    current_timestamp: TimestampMs,
    /// Track which stops arrived this frame
    arrived_this_frame: Vec<u8>,
    /// Active stop indices from last update
    active_indices: Vec<usize>,
    /// Track whether bus is currently off-route
    off_route: bool,
    /// Track last position during off-route
    off_route_last_s_cm: Option<DistCm>,
    /// Probability snapshot from immediately before the current tick's updates
    previous_probabilities: Vec<Prob8>,
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
            previous_probabilities: vec![0; stop_count],
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
        for (snapshot, stop_state) in self
            .previous_probabilities
            .iter_mut()
            .zip(self.stop_states.iter())
        {
            *snapshot = stop_state.last_probability;
        }

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
                    .is_some_and(|off_route_s| record.s_cm > off_route_s + 10000);

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
        for (idx, stop) in stops.iter().enumerate() {
            if s_cm >= stop.corridor_start_cm && s_cm <= stop.corridor_end_cm
                && !self.stop_states[idx].skip_on_reentry
            {
                self.active_indices.push(idx);
            }
        }

        // Process each active stop
        for idx in &self.active_indices {
            let stop = &stops[*idx];
            let stop_state = &mut self.stop_states[*idx];

            let signals = PositionSignals::new(record.s_cm, record.s_cm);
            let gps_status = match record.status {
                "valid" => detection::probability::GpsStatus::Valid,
                "dr_outage" => detection::probability::GpsStatus::DrOutage,
                "off_route" => detection::probability::GpsStatus::OffRoute,
                _ => detection::probability::GpsStatus::Valid,
            };
            let probability = detection::probability::compute_arrival_probability(
                signals,
                v_cms,
                stop,
                stop_state.dwell_time_s,
                gps_status,
                detection::probability::gaussian_lut(),
                detection::probability::logistic_lut(),
            );

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
    pub fn get_trace_info(&self, record: &GpsRecord, route_data: &RouteData) -> (Vec<u8>, Vec<StopTraceState>) {
        let stops = route_data.stops();

        let active_stops: Vec<u8> = self.active_indices.iter().map(|i| *i as u8).collect();

        let z_gps_cm = record.s_cm + record.divergence_cm;

        let mut trace_indices = self.active_indices.clone();
        for (idx, stop_state) in self.stop_states.iter().enumerate() {
            if (stop_state.announced || stop_state.skip_on_reentry)
                && !trace_indices.contains(&idx)
            {
                trace_indices.push(idx);
            }
        }

        let stop_states: Vec<StopTraceState> = trace_indices.iter().map(|&idx| {
            let stop = &stops[idx];
            let stop_state = &self.stop_states[idx];

            let signals = PositionSignals::new(record.s_cm, record.s_cm);

            let features = detection::probability::compute_feature_scores(
                signals,
                record.v_cms,
                stop,
                stop_state.dwell_time_s,
                detection::probability::gaussian_lut(),
                detection::probability::logistic_lut(),
            );

            let probability = detection::probability::compute_probability(
                record.s_cm,
                record.v_cms,
                stop.progress_cm,
                stop_state.dwell_time_s,
            );

            StopTraceState {
                stop_idx: idx as u8,
                gps_distance_cm: z_gps_cm - stop.progress_cm,
                progress_distance_cm: record.s_cm - stop.progress_cm,
                fsm_state: stop_state.fsm_state,
                dwell_time_s: stop_state.dwell_time_s,
                probability,
                previous_probability: self.previous_probabilities[idx],
                features,
                announced: stop_state.announced,
                skip_on_reentry: stop_state.skip_on_reentry,
                previous_distance_cm: stop_state.previous_distance_cm,
                just_arrived: self.arrived_this_frame.contains(&(idx as u8)),
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

    /// Last route position recorded while off-route, if any.
    pub fn off_route_last_s_cm(&self) -> Option<DistCm> {
        self.off_route_last_s_cm
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn load_route_data() -> RouteData<'static> {
        let route_bytes = fs::read("../../test_data/ty225_normal.bin")
            .expect("Failed to load ty225_normal.bin");
        let route_bytes: &'static [u8] = Box::leak(route_bytes.into_boxed_slice());
        RouteData::load(route_bytes).expect("Failed to parse ty225_normal.bin")
    }

    fn gps_record(status: &'static str, s_cm: DistCm) -> GpsRecord {
        GpsRecord::new(1_234_567, 25.0, 121.0, s_cm, 250, Some(9000), status)
    }

    #[test]
    fn get_trace_info_includes_active_announced_and_skipped_stop_states() {
        let route_data = load_route_data();
        let mut state = DetectionState::new(&route_data);
        let stops = route_data.stops();

        state.active_indices = vec![0];
        state.arrived_this_frame.push(0);

        state.stop_states[0].last_probability = 64;
        state.stop_states[0].announced = false;
        state.stop_states[0].skip_on_reentry = false;
        state.stop_states[0].previous_distance_cm = Some(321);

        state.stop_states[1].last_probability = 128;
        state.stop_states[1].announced = true;
        state.stop_states[1].skip_on_reentry = false;
        state.stop_states[1].previous_distance_cm = Some(-456);

        state.stop_states[2].last_probability = 192;
        state.stop_states[2].announced = false;
        state.stop_states[2].skip_on_reentry = true;
        state.stop_states[2].previous_distance_cm = Some(-789);

        let mut record = gps_record("valid", stops[0].corridor_start_cm + 100);
        record.divergence_cm = 25;

        let (active_stops, stop_states) = state.get_trace_info(&record, &route_data);

        assert_eq!(active_stops, vec![0]);

        let mut emitted_indices: Vec<u8> = stop_states.iter().map(|stop| stop.stop_idx).collect();
        emitted_indices.sort_unstable();
        assert_eq!(emitted_indices, vec![0, 1, 2]);

        let announced = stop_states
            .iter()
            .find(|stop| stop.stop_idx == 1)
            .expect("announced stop should be included");
        assert!(announced.announced);
        assert!(!announced.skip_on_reentry);
        assert_eq!(announced.previous_distance_cm, Some(-456));

        let skipped = stop_states
            .iter()
            .find(|stop| stop.stop_idx == 2)
            .expect("skipped stop should be included");
        assert!(!skipped.announced);
        assert!(skipped.skip_on_reentry);
        assert_eq!(skipped.previous_distance_cm, Some(-789));
    }

    #[test]
    fn get_trace_info_reports_probability_from_before_latest_update() {
        let route_data = load_route_data();
        let mut state = DetectionState::new(&route_data);
        let mut result = PipelineResult::new();
        let stop = &route_data.stops()[0];

        let first = gps_record("valid", stop.corridor_start_cm);
        state.process_gps_record(&first, &route_data, &mut result);
        let expected_previous_probability = state.stop_states[0].last_probability;

        let second = gps_record("valid", stop.progress_cm - 1000);
        state.process_gps_record(&second, &route_data, &mut result);

        let (_, stop_states) = state.get_trace_info(&second, &route_data);
        let trace_state = stop_states
            .iter()
            .find(|trace_state| trace_state.stop_idx == 0)
            .expect("active stop should be present");

        assert_ne!(
            state.stop_states[0].last_probability, expected_previous_probability,
            "test requires the second update to change probability"
        );
        assert_eq!(trace_state.previous_probability, expected_previous_probability);
    }
}

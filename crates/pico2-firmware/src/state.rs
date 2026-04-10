//! GPS processing pipeline state
//!
//! Manages the main state machine for processing GPS updates through
//! the full arrival detection pipeline.

use crate::detection::{compute_arrival_probability_adaptive, find_active_stops};
use gps_processor::kalman::{process_gps_update, ProcessResult};
use shared::{binfile::RouteData, ArrivalEvent, DrState, GpsPoint, KalmanState};

// ===== State Struct =====

/// Global state for the GPS processing pipeline
pub struct State<'a> {
    pub nmea: gps_processor::nmea::NmeaState,
    pub kalman: KalmanState,
    pub dr: DrState,
    pub stop_states: heapless::Vec<detection::state_machine::StopState, 256>,
    pub route_data: &'a RouteData<'a>,
    first_fix: bool,
}

impl<'a> State<'a> {
    pub fn new(route_data: &'a RouteData<'a>) -> Self {
        use detection::state_machine::StopState;
        use gps_processor::nmea::NmeaState;

        let stop_count = route_data.stop_count;
        let mut stop_states = heapless::Vec::new();
        for i in 0..stop_count {
            if let Err(_) = stop_states.push(StopState::new(i as u8)) {
                #[cfg(feature = "firmware")]
                defmt::warn!("Route has {} stops but only 256 supported - stops beyond index 255 will be ignored", stop_count);
                break;
            }
        }

        Self {
            nmea: NmeaState::new(),
            kalman: KalmanState::new(),
            dr: DrState::new(),
            stop_states,
            route_data,
            first_fix: true,
        }
    }

    /// Process a GPS point through the full pipeline
    /// Returns Some(arrival event) if an arrival is detected
    pub fn process_gps(&mut self, gps: &GpsPoint) -> Option<ArrivalEvent> {
        use detection::state_machine::StopEvent;

        // Module ④+⑤: Map matching and projection
        // Module ⑥: Speed constraint filter
        // Module ⑦: Kalman filter
        // Module ⑧: Dead-reckoning
        let result = process_gps_update(
            &mut self.kalman,
            &mut self.dr,
            gps,
            self.route_data,
            gps.timestamp,
            self.first_fix,
        );

        let (s_cm, v_cms) = match result {
            ProcessResult::Valid { s_cm, v_cms, seg_idx: _ } => {
                self.first_fix = false;
                (s_cm, v_cms)
            }
            ProcessResult::Rejected(reason) => {
                #[cfg(feature = "firmware")]
                defmt::warn!("GPS update rejected: {}", reason);
                #[cfg(not(feature = "firmware"))]
                let _ = reason; // Suppress unused warning when firmware feature is disabled
                return None;
            }
            ProcessResult::Outage => {
                #[cfg(feature = "firmware")]
                defmt::warn!("GPS outage exceeded 10 seconds");
                return None;
            }
            ProcessResult::DrOutage { s_cm, v_cms } => {
                #[cfg(feature = "firmware")]
                defmt::debug!("DR mode: s={}cm, v={}cm/s", s_cm, v_cms);
                (s_cm, v_cms)
            }
        };

        // Module ⑨: Stop corridor filtering
        let active_indices = find_active_stops(s_cm, self.route_data);

        // Module ⑩+⑪: Arrival probability and state machine for each active stop
        for stop_idx in active_indices {
            if stop_idx >= self.stop_states.len() {
                continue;
            }

            let stop = match self.route_data.get_stop(stop_idx) {
                Some(s) => s,
                None => continue,
            };
            let stop_state = &mut self.stop_states[stop_idx];

            // Get next sequential stop for adaptive weights
            let next_stop_idx = stop_idx.checked_add(1);
            let next_stop_value = next_stop_idx.and_then(|idx| self.route_data.get_stop(idx));
            let next_stop = next_stop_value.as_ref();

            // Compute arrival probability with adaptive weights
            let probability = compute_arrival_probability_adaptive(
                s_cm,
                v_cms,
                &stop,
                stop_state.dwell_time_s,
                next_stop,
            );

            // Update state machine FIRST (v8.4: FSM transition before announce check)
            let event = stop_state.update(
                s_cm,
                v_cms,
                stop.progress_cm,
                stop.corridor_start_cm,
                probability,
            );

            // THEN check for announcement trigger
            if stop_state.should_announce(s_cm, stop.corridor_start_cm) {
                #[cfg(feature = "firmware")]
                defmt::info!(
                    "Announcement for stop {}: s={}cm, v={}cm/s",
                    stop_idx,
                    s_cm,
                    v_cms
                );
                return Some(ArrivalEvent {
                    time: gps.timestamp,
                    stop_idx: stop_idx as u8,
                    s_cm,
                    v_cms,
                    probability: 0,
                    event_type: shared::ArrivalEventType::Announce,
                });
            }

            match event {
                StopEvent::Arrived => {
                    #[cfg(feature = "firmware")]
                    defmt::info!(
                        "Arrival at stop {}: s={}cm, v={}cm/s, p={}",
                        stop_idx,
                        s_cm,
                        v_cms,
                        probability
                    );
                    return Some(ArrivalEvent {
                        time: gps.timestamp,
                        stop_idx: stop_idx as u8,
                        s_cm,
                        v_cms,
                        probability,
                        event_type: shared::ArrivalEventType::Arrival,
                    });
                }
                StopEvent::Departed => {
                    #[cfg(feature = "firmware")]
                    defmt::info!(
                        "Departure from stop {}: s={}cm, v={}cm/s",
                        stop_idx,
                        s_cm,
                        v_cms
                    );
                }
                StopEvent::None => {}
            }
        }

        None
    }
}

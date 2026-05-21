use crate::types::{StopAnalysis, ValidationResult, StopEvent, StateTransition};
use detection::trace::TraceRecord;
use shared::FsmState;

pub struct Analyzer;

impl Analyzer {
    pub fn analyze(records: Vec<TraceRecord>) -> ValidationResult {
        let mut result = ValidationResult {
            trace_file: String::new(),
            total_records: records.len(),
            time_range: (records[0].gps.time_ms, records.last().unwrap().gps.time_ms),
            stops_analyzed: Default::default(),
            global_issues: Default::default(),
            gps_jump_count: 0,
        };

        for record in &records {
            if record.detection.gps_jump {
                result.gps_jump_count += 1;
            }

            for stop_state in &record.stop_states {
                let stop_idx = stop_state.stop_idx;
                let analysis = result.stops_analyzed
                    .entry(stop_idx)
                    .or_insert_with(|| StopAnalysis::new(stop_idx));

                record_event(analysis, record.gps.time_ms, stop_state.fsm_state,
                             stop_state.progress_distance_cm, record.kalman.s_cm, record.kalman.v_cms,
                             stop_state.just_arrived);
                track_corridor(analysis, record.gps.time_ms, stop_state.progress_distance_cm);
            }
        }

        result
    }

    const CORRIDOR_START_CM: i32 = -8000;
    const CORRIDOR_END_CM: i32 = 4000;
}

fn record_event(analysis: &mut StopAnalysis, time: u64, state: FsmState,
                 distance_cm: i32, s_cm: i32, v_cms: i32, just_arrived: bool) {
    if analysis.first_seen_time.is_none() {
        analysis.first_seen_time = Some(time);
    }

    // Record all state transitions for duplicate detection
    analysis.state_transitions.push(StateTransition {
        time,
        state,
        just_arrived,
    });

    // Record first occurrence of each state
    analysis.events.entry(state).or_insert_with(|| StopEvent {
        time, state, s_cm, v_cms, distance_cm
    });

    if state == FsmState::AtStop {
        if analysis.at_stop_first_time.is_none() {
            analysis.at_stop_first_time = Some(time);
            analysis.at_stop_distance_cm = Some(distance_cm);
            analysis.at_stop_speed_cms = Some(v_cms);
        }
        analysis.at_stop_last_time = Some(time);
    }
}

fn track_corridor(analysis: &mut StopAnalysis, time: u64, distance_cm: i32) {
    if !analysis.in_corridor && distance_cm > Analyzer::CORRIDOR_START_CM {
        analysis.corridor_entry_time = Some(time);
        analysis.in_corridor = true;
    }
    if analysis.in_corridor && distance_cm > Analyzer::CORRIDOR_END_CM {
        analysis.corridor_exit_time = Some(time);
        analysis.in_corridor = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use detection::trace::{
        CorridorTrace, DetectionTrace, FeatureScores, GpsTrace, KalmanTrace, MapMatchingTrace,
        StopTraceState, TraceRecord,
    };

    fn sample_record(time_ms: u64, gps_jump: bool, stop_states: Vec<StopTraceState>) -> TraceRecord {
        TraceRecord {
            gps: GpsTrace {
                time_ms,
                lat: 25.0,
                lon: 121.0,
                heading_cdeg: Some(0),
                hdop: Some(1.2),
                accuracy_cm: Some(120),
                num_sats: Some(12),
                fix_type: Some("3d".to_string()),
            },
            kalman: KalmanTrace {
                s_cm: 10_000,
                v_cms: 50,
                variance_cm2: 100,
                divergence_cm: 10,
            },
            map_matching: MapMatchingTrace {
                segment_idx: Some(5),
                heading_constraint_met: true,
            },
            detection: DetectionTrace {
                status: "normal".to_string(),
                off_route: false,
                gps_jump,
                recovery_idx: None,
                off_route_last_s_cm: None,
            },
            corridor: CorridorTrace {
                active_stops: vec![0],
                corridor_start_cm: Some(2000),
                corridor_end_cm: Some(14000),
                next_stop: Some((1, 50)),
            },
            stop_states,
        }
    }

    #[test]
    fn test_analyze_empty_records() {
        let mut record = sample_record(1, false, vec![]);
        record.corridor.active_stops.clear();
        let records = vec![record];

        let result = Analyzer::analyze(records);
        assert_eq!(result.total_records, 1);
        assert_eq!(result.stops_analyzed.len(), 0);
    }

    #[test]
    fn test_analyze_with_stop_states() {
        let records = vec![sample_record(100, false, vec![StopTraceState {
                stop_idx: 0,
                gps_distance_cm: -100,
                progress_distance_cm: -100,
                fsm_state: FsmState::Approaching,
                dwell_time_s: 0,
                probability: 10,
                previous_probability: 9,
                features: FeatureScores { p1: 5, p2: 3, p3: 2, p4: 0 },
                announced: false,
                skip_on_reentry: false,
                previous_distance_cm: None,
                just_arrived: false,
            }])];

        let result = Analyzer::analyze(records);
        assert_eq!(result.stops_analyzed.len(), 1);
        assert!(result.stops_analyzed[&0].events.contains_key(&FsmState::Approaching));
    }

    #[test]
    fn test_analyze_counts_gps_jumps() {
        let mut first = sample_record(1, true, vec![]);
        first.corridor.active_stops.clear();
        let mut second = sample_record(2, false, vec![]);
        second.corridor.active_stops.clear();
        second.kalman.s_cm = 100;
        second.kalman.v_cms = 100;
        second.kalman.variance_cm2 = 50;
        second.kalman.divergence_cm = 5;
        let records = vec![first, second];

        let result = Analyzer::analyze(records);
        assert_eq!(result.gps_jump_count, 1);
    }
}

use crate::types::{StopAnalysis, ValidationResult, StopEvent, StateTransition};
use arrival_detector::trace::TraceRecord;
use shared::FsmState;

pub struct Analyzer;

impl Analyzer {
    pub fn analyze(records: Vec<TraceRecord>) -> ValidationResult {
        let mut result = ValidationResult {
            trace_file: String::new(),
            total_records: records.len(),
            time_range: (records[0].time, records.last().unwrap().time),
            stops_analyzed: Default::default(),
            global_issues: Default::default(),
            gps_jump_count: 0,
        };

        for record in &records {
            if record.gps_jump {
                result.gps_jump_count += 1;
            }

            for stop_state in &record.stop_states {
                let stop_idx = stop_state.stop_idx;
                let analysis = result.stops_analyzed
                    .entry(stop_idx)
                    .or_insert_with(|| StopAnalysis::new(stop_idx));

                record_event(analysis, record.time, stop_state.fsm_state,
                             stop_state.distance_cm, record.s_cm, record.v_cms,
                             stop_state.just_arrived);
                track_corridor(analysis, record.time, stop_state.distance_cm);
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

    #[test]
    fn test_analyze_empty_records() {
        let records = vec![TraceRecord {
            time: 1,
            lat: 25.0,
            lon: 121.0,
            s_cm: 0,
            v_cms: 100,
            heading_cdeg: Some(0),
            active_stops: vec![],
            stop_states: vec![],
            gps_jump: false,
            recovery_idx: None,
        }];

        let result = Analyzer::analyze(records);
        assert_eq!(result.total_records, 1);
        assert_eq!(result.stops_analyzed.len(), 0);
    }

    #[test]
    fn test_analyze_with_stop_states() {
        use arrival_detector::trace::{StopTraceState, FeatureScores};

        let records = vec![TraceRecord {
            time: 100,
            lat: 25.0,
            lon: 121.0,
            s_cm: 10000,
            v_cms: 50,
            heading_cdeg: Some(0),
            active_stops: vec![0],
            stop_states: vec![StopTraceState {
                stop_idx: 0,
                distance_cm: -100,
                fsm_state: FsmState::Approaching,
                dwell_time_s: 0,
                probability: 10,
                features: FeatureScores { p1: 5, p2: 3, p3: 2, p4: 0 },
                just_arrived: false,
            }],
            gps_jump: false,
            recovery_idx: None,
        }];

        let result = Analyzer::analyze(records);
        assert_eq!(result.stops_analyzed.len(), 1);
        assert!(result.stops_analyzed[&0].events.contains_key(&FsmState::Approaching));
    }

    #[test]
    fn test_analyze_counts_gps_jumps() {
        let records = vec![TraceRecord {
            time: 1,
            lat: 25.0,
            lon: 121.0,
            s_cm: 0,
            v_cms: 100,
            heading_cdeg: Some(0),
            active_stops: vec![],
            stop_states: vec![],
            gps_jump: true,  // GPS jump
            recovery_idx: None,
        }, TraceRecord {
            time: 2,
            lat: 25.0,
            lon: 121.0,
            s_cm: 100,
            v_cms: 100,
            heading_cdeg: Some(0),
            active_stops: vec![],
            stop_states: vec![],
            gps_jump: false,
            recovery_idx: None,
        }];

        let result = Analyzer::analyze(records);
        assert_eq!(result.gps_jump_count, 1);
    }
}

use crate::types::{Issue, Severity, StopAnalysis, ValidationResult};
use shared::FsmState;
use std::collections::HashMap;

pub struct Validator;

impl Validator {
    pub fn validate(result: &mut ValidationResult, ground_truth: Option<&HashMap<u8, u64>>) {
        // Check for global FSM issues
        let has_any_departed = result.stops_analyzed.values()
            .any(|s| s.events.contains_key(&FsmState::Departed));

        if !has_any_departed && result.total_records > 0 {
            result.global_issues.push(Issue {
                severity: Severity::Critical,
                stop_idx: None,
                message: "FSM never transitions to Departed state".to_string(),
            });
        }

        for (&stop_idx, analysis) in &mut result.stops_analyzed {
            validate_stop(analysis, ground_truth.and_then(|gt| gt.get(&stop_idx).copied()));
        }
    }
}

fn validate_stop(analysis: &mut StopAnalysis, gt_dwell: Option<u64>) {
    let required_states = [
        FsmState::Approaching,
        FsmState::Arriving,
        FsmState::AtStop,
        FsmState::Departed,
    ];

    for state in &required_states {
        if !analysis.events.contains_key(state) {
            analysis.issues.push(Issue {
                severity: if *state == FsmState::AtStop {
                    Severity::Critical
                } else {
                    Severity::Warning
                },
                stop_idx: Some(analysis.stop_idx),
                message: format!("Missing FSM state: {:?}", state),
            });
        }
    }

    // Temporal ordering check
    let state_order = [
        FsmState::Idle,
        FsmState::Approaching,
        FsmState::Arriving,
        FsmState::AtStop,
        FsmState::Departed,
        FsmState::TripComplete,
    ];

    let mut last_state_time: Option<u64> = None;
    let mut last_state_idx: Option<usize> = None;

    for (state_idx, state) in state_order.iter().enumerate() {
        if let Some(event) = analysis.events.get(state) {
            let current_time = event.time;

            if let Some((prev_time, prev_idx)) = last_state_time.zip(last_state_idx) {
                if current_time < prev_time {
                    analysis.issues.push(Issue {
                        severity: Severity::Warning,
                        stop_idx: Some(analysis.stop_idx),
                        message: format!(
                            "FSM state out of order: {:?} at t={} occurs before {:?} at t={}",
                            state, current_time, state_order[prev_idx], prev_time
                        ),
                    });
                }
            }

            last_state_time = Some(current_time);
            last_state_idx = Some(state_idx);
        }
    }

    // Position accuracy check
    if let Some(distance_cm) = analysis.at_stop_distance_cm {
        if distance_cm.abs() > 5000 {
            analysis.issues.push(Issue {
                severity: Severity::Warning,
                stop_idx: Some(analysis.stop_idx),
                message: format!("Poor position accuracy: {}m from stop",
                               distance_cm / 100),
            });
        }
    }

    // Dwell time comparison
    if let (Some(actual_dwell), Some(expected_dwell)) = (analysis.dwell_time_s(), gt_dwell) {
        let diff = actual_dwell.abs_diff(expected_dwell);
        if diff > 3 {
            analysis.issues.push(Issue {
                severity: Severity::Warning,
                stop_idx: Some(analysis.stop_idx),
                message: format!("Dwell time mismatch: {}s vs {}s (diff: {}s)",
                               actual_dwell, expected_dwell, diff),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::StopEvent;

    #[test]
    fn test_validate_creates_departed_warning() {
        let mut result = ValidationResult {
            trace_file: "test".to_string(),
            total_records: 1,
            time_range: (0, 10),
            stops_analyzed: Default::default(),
            global_issues: vec![],
            gps_jump_count: 0,
        };

        Validator::validate(&mut result, None);
        assert!(!result.global_issues.is_empty());
        assert_eq!(result.global_issues[0].severity, Severity::Critical);
    }

    #[test]
    fn test_validate_stop_with_complete_fsm() {
        let mut analysis = StopAnalysis::new(0);
        // Add all required states
        for (time, state) in [(10, FsmState::Approaching), (15, FsmState::Arriving),
                                (20, FsmState::AtStop), (30, FsmState::Departed)] {
            analysis.events.entry(state).or_insert_with(|| StopEvent {
                time, state, s_cm: 0, v_cms: 0, distance_cm: 0
            });
        }

        validate_stop(&mut analysis, None);
        assert!(analysis.issues.is_empty());
    }
}

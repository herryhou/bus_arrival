use serde::Serialize;
use shared::FsmState;
use std::collections::BTreeMap;

/// Stop event for analysis tracking
#[derive(Debug, Clone, Serialize)]
pub struct StopEvent {
    pub time: u64,
    pub state: FsmState,
    pub s_cm: i32,
    pub v_cms: i32,
    pub distance_cm: i32,
}

/// Analysis result for a single stop
#[derive(Debug, Serialize)]
pub struct StopAnalysis {
    pub stop_idx: u8,
    pub events: BTreeMap<FsmState, StopEvent>,
    pub first_seen_time: Option<u64>,
    pub at_stop_first_time: Option<u64>,
    pub at_stop_last_time: Option<u64>,
    pub at_stop_distance_cm: Option<i32>,
    pub at_stop_speed_cms: Option<i32>,
    pub corridor_entry_time: Option<u64>,
    pub corridor_exit_time: Option<u64>,
    pub issues: Vec<Issue>,
    #[serde(skip)]
    pub in_corridor: bool,
}

impl StopAnalysis {
    pub fn new(stop_idx: u8) -> Self {
        StopAnalysis {
            stop_idx,
            events: BTreeMap::new(),
            first_seen_time: None,
            at_stop_first_time: None,
            at_stop_last_time: None,
            at_stop_distance_cm: None,
            at_stop_speed_cms: None,
            corridor_entry_time: None,
            corridor_exit_time: None,
            issues: Vec::new(),
            in_corridor: false,
        }
    }

    pub fn dwell_time_s(&self) -> Option<u64> {
        if let (Some(first), Some(last)) = (self.at_stop_first_time, self.at_stop_last_time) {
            Some(last - first)
        } else {
            None
        }
    }

    pub fn is_complete(&self) -> bool {
        [FsmState::Approaching, FsmState::Arriving, FsmState::AtStop, FsmState::Departed]
            .iter()
            .all(|s| self.events.contains_key(s))
    }
}

/// Issue with severity level
#[derive(Debug, Clone, Serialize)]
pub struct Issue {
    pub severity: Severity,
    pub stop_idx: Option<u8>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Critical,
    Warning,
    Info,
}

/// Complete validation result
#[derive(Debug, Serialize)]
pub struct ValidationResult {
    pub trace_file: String,
    pub total_records: usize,
    pub time_range: (u64, u64),
    pub stops_analyzed: BTreeMap<u8, StopAnalysis>,
    pub global_issues: Vec<Issue>,
    pub gps_jump_count: usize,
}

impl ValidationResult {
    pub fn total_stops(&self) -> usize {
        self.stops_analyzed.len()
    }

    pub fn complete_stops(&self) -> usize {
        self.stops_analyzed.values().filter(|s| s.is_complete()).count()
    }

    pub fn stops_with_at_stop(&self) -> usize {
        self.stops_analyzed.values()
            .filter(|s| s.events.contains_key(&FsmState::AtStop))
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stop_analysis_new() {
        let analysis = StopAnalysis::new(5);
        assert_eq!(analysis.stop_idx, 5);
        assert!(!analysis.is_complete());
        assert_eq!(analysis.dwell_time_s(), None);
    }

    #[test]
    fn test_dwell_time_calculation() {
        let mut analysis = StopAnalysis::new(0);
        analysis.at_stop_first_time = Some(100);
        analysis.at_stop_last_time = Some(110);
        assert_eq!(analysis.dwell_time_s(), Some(10));
    }

    #[test]
    fn test_is_complete_returns_false_when_missing_states() {
        let analysis = StopAnalysis::new(0);
        assert!(!analysis.is_complete());
    }
}

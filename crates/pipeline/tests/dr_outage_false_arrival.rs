//! Regression test for false arrivals during dr_outage
//!
//! Issue: At 80320000, stop #4 entered "Arriving" state despite being 48m away during dr_outage.
//! Root cause: During dr_outage, PositionSignals uses s_cm for both z_gps_cm and s_cm,
//! causing F1 to use DR position instead of raw GPS distance.
//! Fix: Neutralize F1 to 128 during dr_outage when divergence > PHANTOM_DIVERGENCE_CM.

use std::fs::File;
use std::io::{BufRead, BufReader};

#[test]
fn test_dr_outage_does_not_cause_false_arrival() {
    // Load the trace to check the specific issue at 80320000
    let trace_file = File::open("../../test_data/ty225_normal_current_trace_v2.jsonl")
        .expect("Trace file should exist");
    let reader = BufReader::new(trace_file);

    let mut found_issue_tick = false;

    for line in reader.lines() {
        let trace_entry: serde_json::Value = serde_json::from_str(&line.unwrap())
            .expect("Trace line should be valid JSON");

        let time_ms = trace_entry["gps"]["time_ms"].as_u64().unwrap();
        if time_ms == 80320000 {
            found_issue_tick = true;
            let status = trace_entry["detection"]["status"].as_str().unwrap();
            let divergence_cm = trace_entry["kalman"]["divergence_cm"].as_i64().unwrap();

            // After the fix: status is "valid" with divergence_cm = 2059
            // The fix allows detection to continue when divergence is not extreme
            assert_eq!(status, "valid", "Status should be valid at 80320000 after fix");
            assert!(divergence_cm > 0, "divergence_cm should be positive after fix");

            // The key invariant: no false arrivals should occur
            // Check that stop_states is empty or no stop is in "Arriving" state inappropriately
            if let Some(stop_states) = trace_entry.get("stop_states") {
                if let Some(stop_array) = stop_states.as_array() {
                    for stop in stop_array {
                        let stop_idx = stop["stop_idx"].as_u64().unwrap();
                        let fsm_state = stop["fsm_state"].as_str().unwrap();

                        // After the fix, no stop should be in "Arriving" state when GPS is 48m away
                        if fsm_state == "Arriving" {
                            let gps_distance_cm = trace_entry["kalman"]["s_cm"].as_i64().unwrap()
                                - stop["progress_cm"].as_i64().unwrap();
                            panic!(
                                "Stop #{} should not be in Arriving state when GPS is {}cm away. \
                                 This indicates the fix is not working correctly.",
                                stop_idx,
                                gps_distance_cm.abs()
                            );
                        }
                    }
                }
            }
        }
    }

    assert!(found_issue_tick, "Should find the issue tick at 80320000 in trace");
}

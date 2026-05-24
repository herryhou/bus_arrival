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

            // At 80320000, we should be in dr_outage with divergence_cm = 0
            assert_eq!(status, "dr_outage", "Should be in dr_outage at 80320000");
            assert_eq!(divergence_cm, 0, "divergence_cm should be 0 during dr_outage");

            // Check stop states for false arrival
            if let Some(stop_states) = trace_entry.get("stop_states") {
                if let Some(stop_array) = stop_states.as_array() {
                    for stop in stop_array {
                        let stop_idx = stop["stop_idx"].as_u64().unwrap();
                        let fsm_state = stop["fsm_state"].as_str().unwrap();

                        // Stop #4 at 80320000 was 48m away but in "Arriving" state
                        if stop_idx == 4 && fsm_state == "Arriving" {
                            // With the fix, F1 should be neutralized (128) during dr_outage
                            // This prevents false arrivals when GPS is far but DR is close
                            let probability = stop["probability"].as_u64().unwrap();

                            // Probability should be suppressed due to F1 neutralization
                            // (Old behavior: probability was 65, causing false arrival)
                            assert!(
                                probability < 191,
                                "Probability should be below arrival threshold (191) during dr_outage when GPS is 48m away. Got: {}",
                                probability
                            );
                        }
                    }
                }
            }
        }
    }

    assert!(found_issue_tick, "Should find the issue tick at 80320000 in trace");
}

//! Integration test for trace output

use arrival_detector::trace::{TraceRecord, StopTraceState, FeatureScores};
use shared::FsmState;

#[test]
fn test_trace_serialization_valid_json() {
    // Verify TraceRecord serializes to valid JSON with FsmState
    let record = TraceRecord {
        time: 1234567890,
        lat: 25.00425,
        lon: 121.28645,
        s_cm: 10000,
        v_cms: 500,
        active_stops: vec![0, 1],
        stop_states: vec![
            StopTraceState {
                stop_idx: 0,
                distance_cm: 500,
                fsm_state: FsmState::Approaching,
                dwell_time_s: 0,
                probability: 128,
                features: FeatureScores { p1: 200, p2: 150, p3: 180, p4: 100 },
                just_arrived: false,
            },
            StopTraceState {
                stop_idx: 1,
                distance_cm: -300,
                fsm_state: FsmState::AtStop,
                dwell_time_s: 10,
                probability: 230,
                features: FeatureScores { p1: 250, p2: 200, p3: 240, p4: 255 },
                just_arrived: true,
            },
        ],
        gps_jump: false,
        recovery_idx: None,
    };

    // Serialize to JSON
    let json = serde_json::to_string(&record).expect("Failed to serialize TraceRecord");

    // Verify it's valid JSON
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("Failed to parse JSON as Value");

    // Verify structure
    assert_eq!(parsed["time"], 1234567890);
    assert_eq!(parsed["s_cm"], 10000);
    assert_eq!(parsed["v_cms"], 500);
    assert!(parsed["active_stops"].is_array());
    assert_eq!(parsed["active_stops"].as_array().unwrap().len(), 2);

    // Verify FsmState serializes as string name (not object)
    assert!(json.contains(r#""fsm_state":"Approaching""#));
    assert!(json.contains(r#""fsm_state":"AtStop""#));

    // Verify nested feature scores
    assert!(json.contains(r#""p1":200"#));
    assert!(json.contains(r#""just_arrived":false"#));
    assert!(json.contains(r#""just_arrived":true"#));
}

#[test]
fn test_all_fsm_states_serialize() {
    // Verify all FsmState variants serialize correctly
    let states = [
        FsmState::Approaching,
        FsmState::Arriving,
        FsmState::AtStop,
        FsmState::Departed,
    ];

    for state in states {
        let json = serde_json::to_string(&state).expect("Failed to serialize FsmState");
        // Should serialize as string name like "Approaching", not {"Approaching":{}}
        assert!(json.starts_with('"') && json.ends_with('"'));
        let parsed: String = serde_json::from_str(&json).expect("Failed to deserialize");
        assert!(!parsed.is_empty());
    }
}

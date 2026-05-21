//! Integration test for trace output

use detection::trace::{
    CorridorTrace,
    DetectionTrace,
    FeatureScores,
    GpsTrace,
    KalmanTrace,
    MapMatchingTrace,
    StopTraceState,
    TraceRecord,
};
use shared::FsmState;

#[test]
fn test_trace_serialization_valid_json() {
    // Verify TraceRecord serializes to grouped v2 JSON with FsmState and new fields
    let record = TraceRecord {
        gps: GpsTrace {
            time_ms: 1_234_567_890,
            lat: 25.00425,
            lon: 121.28645,
            heading_cdeg: Some(-950), // 350.5° converted to -950 cdeg
            hdop: Some(1.2),
            accuracy_cm: Some(150),
            num_sats: Some(12),
            fix_type: Some("3d".to_string()),
        },
        kalman: KalmanTrace {
            s_cm: 10000,
            v_cms: 500,
            variance_cm2: 100,
            divergence_cm: 15,
        },
        map_matching: MapMatchingTrace {
            segment_idx: Some(5),
            heading_constraint_met: true,
        },
        detection: DetectionTrace {
            status: "normal".to_string(),
            off_route: false,
            gps_jump: false,
            recovery_idx: None,
            off_route_last_s_cm: Some(9876),
        },
        corridor: CorridorTrace {
            active_stops: vec![0, 1],
            corridor_start_cm: Some(9500),
            corridor_end_cm: Some(10500),
            next_stop: Some((2, 200)),
        },
        stop_states: vec![
            StopTraceState {
                stop_idx: 0,
                gps_distance_cm: 480,
                progress_distance_cm: 500,
                fsm_state: FsmState::Approaching,
                dwell_time_s: 0,
                probability: 128,
                previous_probability: 64,
                features: FeatureScores { p1: 200, p2: 150, p3: 180, p4: 100 },
                announced: false,
                skip_on_reentry: false,
                previous_distance_cm: Some(550),
                just_arrived: false,
            },
            StopTraceState {
                stop_idx: 1,
                gps_distance_cm: -320,
                progress_distance_cm: -300,
                fsm_state: FsmState::AtStop,
                dwell_time_s: 10,
                probability: 230,
                previous_probability: 220,
                features: FeatureScores { p1: 250, p2: 200, p3: 240, p4: 255 },
                announced: true,
                skip_on_reentry: true,
                previous_distance_cm: Some(-450),
                just_arrived: true,
            },
        ],
    };

    // Serialize to JSON
    let json = serde_json::to_string(&record).expect("Failed to serialize TraceRecord");

    // Verify it's valid JSON
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("Failed to parse JSON as Value");

    // Verify structure
    assert_eq!(parsed["gps"]["time_ms"], 1_234_567_890);
    assert_eq!(parsed["gps"]["lat"], 25.00425);
    assert_eq!(parsed["gps"]["lon"], 121.28645);
    assert_eq!(parsed["kalman"]["s_cm"], 10000);
    assert_eq!(parsed["kalman"]["v_cms"], 500);
    assert!(parsed["corridor"]["active_stops"].is_array());
    assert_eq!(parsed["corridor"]["active_stops"].as_array().unwrap().len(), 2);
    assert!(parsed.get("time_ms").is_none());
    assert!(parsed.get("lat").is_none());
    assert!(parsed.get("lon").is_none());
    assert!(parsed.get("s_cm").is_none());
    assert!(parsed.get("v_cms").is_none());
    assert!(parsed.get("heading_cdeg").is_none());
    assert!(parsed.get("active_stops").is_none());
    assert!(parsed.get("gps_jump").is_none());
    assert!(parsed.get("recovery_idx").is_none());
    assert!(parsed.get("segment_idx").is_none());
    assert!(parsed.get("heading_constraint_met").is_none());
    assert!(parsed.get("divergence_cm").is_none());
    assert!(parsed.get("hdop").is_none());
    assert!(parsed.get("accuracy_cm").is_none());
    assert!(parsed.get("num_sats").is_none());
    assert!(parsed.get("fix_type").is_none());
    assert!(parsed.get("variance_cm2").is_none());
    assert!(parsed.get("corridor_start_cm").is_none());
    assert!(parsed.get("corridor_end_cm").is_none());
    assert!(parsed.get("next_stop").is_none());
    assert!(parsed.get("off_route").is_none());
    assert!(parsed.get("status").is_none());
    assert!(parsed.get("off_route_last_s_cm").is_none());

    // Verify FsmState serializes as string name (not object)
    assert!(json.contains(r#""fsm_state":"Approaching""#));
    assert!(json.contains(r#""fsm_state":"AtStop""#));

    // Verify nested feature scores
    assert!(json.contains(r#""p1":200"#));
    assert!(json.contains(r#""previous_probability":64"#));
    assert!(json.contains(r#""announced":false"#));
    assert!(json.contains(r#""skip_on_reentry":true"#));
    assert!(json.contains(r#""previous_distance_cm":-450"#));
    assert!(json.contains(r#""just_arrived":false"#));
    assert!(json.contains(r#""just_arrived":true"#));

    // Verify grouped fields
    assert_eq!(parsed["gps"]["heading_cdeg"], -950);
    assert_eq!(parsed["gps"]["hdop"], 1.2);
    assert_eq!(parsed["gps"]["accuracy_cm"], 150);
    assert_eq!(parsed["gps"]["num_sats"], 12);
    assert_eq!(parsed["gps"]["fix_type"], "3d");
    assert_eq!(parsed["kalman"]["variance_cm2"], 100);
    assert_eq!(parsed["kalman"]["divergence_cm"], 15);
    assert_eq!(parsed["map_matching"]["segment_idx"], 5);
    assert_eq!(parsed["map_matching"]["heading_constraint_met"], true);
    assert_eq!(parsed["detection"]["status"], "normal");
    assert_eq!(parsed["detection"]["off_route"], false);
    assert_eq!(parsed["detection"]["gps_jump"], false);
    assert!(parsed["detection"]["recovery_idx"].is_null());
    assert_eq!(parsed["detection"]["off_route_last_s_cm"], 9876);
    assert_eq!(parsed["corridor"]["corridor_start_cm"], 9500);
    assert_eq!(parsed["corridor"]["corridor_end_cm"], 10500);
    assert_eq!(parsed["corridor"]["next_stop"][0], 2);
    assert_eq!(parsed["corridor"]["next_stop"][1], 200);
}

#[test]
fn test_all_fsm_states_serialize() {
    // Verify all FsmState variants serialize correctly
    let states = [
        (FsmState::Approaching, "\"Approaching\""),
        (FsmState::Arriving, "\"Arriving\""),
        (FsmState::AtStop, "\"AtStop\""),
        (FsmState::Departed, "\"Departed\""),
    ];

    for (state, expected_json) in states {
        let json = serde_json::to_string(&state).expect("Failed to serialize FsmState");
        assert_eq!(json, expected_json);
    }
}

use pipeline::Pipeline;

#[test]
fn process_real_jsonl_log() {
    let result = Pipeline::process_file(
        "../../test_data/tz_23-gps-log-20260519-063111.jsonl",
        "../../test_data/tz_23_short.bin",
    )
    .expect("pipeline should process the JSONL fixture");

    assert!(
        !result.trace_records.is_empty(),
        "real JSONL log should produce trace records"
    );
    assert!(
        result.trace_records.len() > 1,
        "real JSONL log should produce more than one trace record"
    );
    assert!(
        result.trace_records.iter().any(|r| r.gps.time_ms > 1_000_000),
        "trace records should preserve millisecond timestamps"
    );
    assert!(result.arrivals.len() <= result.trace_records.len());
}

#[test]
fn tz23_loop_crossing_does_not_jump_back_to_segment_30() {
    let result = Pipeline::process_file(
        "../../test_data/tz_23-gps-log-20260519-063111.jsonl",
        "../../test_data/tz_23_short.bin",
    )
    .expect("pipeline should process the JSONL fixture");

    let before_jump = result
        .trace_records
        .iter()
        .find(|r| r.gps.time_ms == 1_779_172_948_240)
        .expect("trace should contain the sample before the loop crossing");
    let at_jump = result
        .trace_records
        .iter()
        .find(|r| r.gps.time_ms == 1_779_172_949_214)
        .expect("trace should contain the loop crossing sample");

    assert_eq!(before_jump.map_matching.segment_idx, Some(130));
    assert_eq!(at_jump.map_matching.segment_idx, Some(130));
    assert!(
        at_jump.kalman.s_cm >= before_jump.kalman.s_cm - 5_000,
        "loop crossing should not jump backward from {} to {}",
        before_jump.kalman.s_cm,
        at_jump.kalman.s_cm
    );
    assert_eq!(at_jump.corridor.active_stops, vec![8]);
    assert!(!at_jump.stop_states.is_empty());
}

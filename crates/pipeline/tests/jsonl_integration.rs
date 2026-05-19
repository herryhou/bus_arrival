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
        result.trace_records.iter().any(|r| r.time_ms > 1_000_000),
        "trace records should preserve millisecond timestamps"
    );
    assert!(result.arrivals.len() <= result.trace_records.len());
}

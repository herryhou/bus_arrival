use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_basic_trace_validation() {
    // Create sample trace file
    let mut trace_file = NamedTempFile::new().unwrap();
    writeln!(trace_file, r#"{{"time":1,"lat":25.0,"lon":121.0,"s_cm":0,"v_cms":100,"heading_cdeg":0,"active_stops":[0],"stop_states":[{{"stop_idx":0,"distance_cm":-7000,"fsm_state":"Approaching","dwell_time_s":0,"probability":10,"features":{{"p1":5,"p2":3,"p3":2,"p4":0}},"just_arrived":false}}],"gps_jump":false,"recovery_idx":null}}"#).unwrap();
    writeln!(trace_file, r#"{{"time":10,"lat":25.001,"lon":121.001,"s_cm":500,"v_cms":50,"heading_cdeg":0,"active_stops":[0],"stop_states":[{{"stop_idx":0,"distance_cm":0,"fsm_state":"AtStop","dwell_time_s":1,"probability":255,"features":{{"p1":10,"p2":10,"p3":10,"p4":10}},"just_arrived":true}}],"gps_jump":false,"recovery_idx":null}}"#).unwrap();

    let output_file = NamedTempFile::new().unwrap();

    // Run validator
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_trace_validator"))
        .arg(trace_file.path())
        .arg("-o")
        .arg(output_file.path())
        .output()
        .unwrap();

    assert!(result.status.success());

    // Verify HTML was generated
    let html = std::fs::read_to_string(output_file.path()).unwrap();
    assert!(html.contains("Trace Validation Report"));
    assert!(html.contains("1"));  // total_records
}

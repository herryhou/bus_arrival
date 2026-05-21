use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_basic_trace_validation() {
    // Create sample trace file with new required fields
    let mut trace_file = NamedTempFile::new().unwrap();
    writeln!(trace_file, r#"{{"gps":{{"time_ms":1,"lat":25.0,"lon":121.0,"heading_cdeg":0,"hdop":1.5,"num_sats":12,"fix_type":"3d"}},"kalman":{{"s_cm":0,"v_cms":100,"variance_cm2":100,"divergence_cm":5}},"map_matching":{{"segment_idx":0,"heading_constraint_met":true}},"detection":{{"status":"normal","off_route":false,"gps_jump":false}},"corridor":{{"active_stops":[0]}},"stop_states":[{{"stop_idx":0,"gps_distance_cm":-7000,"progress_distance_cm":-7000,"fsm_state":"Approaching","dwell_time_s":0,"probability":10,"previous_probability":9,"features":{{"p1":5,"p2":3,"p3":2,"p4":0}},"announced":false,"skip_on_reentry":false,"just_arrived":false}}]}}"#).unwrap();
    writeln!(trace_file, r#"{{"gps":{{"time_ms":10,"lat":25.001,"lon":121.001,"heading_cdeg":0,"hdop":1.2,"num_sats":14,"fix_type":"3d"}},"kalman":{{"s_cm":500,"v_cms":50,"variance_cm2":50,"divergence_cm":0}},"map_matching":{{"segment_idx":1,"heading_constraint_met":true}},"detection":{{"status":"normal","off_route":false,"gps_jump":false}},"corridor":{{"active_stops":[0]}},"stop_states":[{{"stop_idx":0,"gps_distance_cm":0,"progress_distance_cm":0,"fsm_state":"AtStop","dwell_time_s":1,"probability":255,"previous_probability":200,"features":{{"p1":10,"p2":10,"p3":10,"p4":10}},"announced":true,"skip_on_reentry":false,"just_arrived":true}}]}}"#).unwrap();

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

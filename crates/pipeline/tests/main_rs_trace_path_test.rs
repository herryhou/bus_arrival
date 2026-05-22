//! Test that main.rs generates correct trace_v2.jsonl output filenames

use std::path::PathBuf;

/// Helper function from main.rs - duplicated here for testing
fn generate_trace_path(nmea_path: &std::path::Path) -> std::path::PathBuf {
    let mut trace_path = nmea_path.to_path_buf();
    let file_stem = trace_path.file_stem().unwrap_or_default();
    let parent = trace_path.parent();

    let stem_str = file_stem.to_string_lossy();
    let base_name = stem_str.strip_suffix("_nmea").unwrap_or(&stem_str);
    let new_name = format!("{}_trace_v2.jsonl", base_name);

    if let Some(p) = parent {
        trace_path = p.join(new_name);
    } else {
        trace_path = PathBuf::from(new_name);
    }

    trace_path
}

#[test]
fn test_generate_trace_path_uses_v2_suffix() {
    // Test case 1: _nmea suffix
    let input = std::path::Path::new("test_data/ty225_normal_nmea.txt");
    let output = generate_trace_path(input);
    assert_eq!(
        output,
        std::path::Path::new("test_data/ty225_normal_trace_v2.jsonl"),
        "Expected _trace_v2.jsonl suffix for input with _nmea suffix"
    );

    // Test case 2: without _nmea suffix
    let input = std::path::Path::new("test_data/gps_log.txt");
    let output = generate_trace_path(input);
    assert_eq!(
        output,
        std::path::Path::new("test_data/gps_log_trace_v2.jsonl"),
        "Expected _trace_v2.jsonl suffix for input without _nmea suffix"
    );

    // Test case 3: ty225_short_detour
    let input = std::path::Path::new("test_data/ty225_short_detour_nmea.txt");
    let output = generate_trace_path(input);
    assert_eq!(
        output,
        std::path::Path::new("test_data/ty225_short_detour_trace_v2.jsonl"),
        "Expected _trace_v2.jsonl suffix for detour scenario"
    );
}

#[test]
fn test_generate_trace_path_does_not_use_old_format() {
    let input = std::path::Path::new("test_data/ty225_normal_nmea.txt");
    let output = generate_trace_path(input);

    let output_str = output.to_string_lossy();
    assert!(
        !output_str.contains("_trace.jsonl"),
        "Output should NOT contain old _trace.jsonl format. Got: {}",
        output_str
    );
    assert!(
        output_str.contains("_trace_v2.jsonl"),
        "Output must contain _trace_v2.jsonl format. Got: {}",
        output_str
    );
}

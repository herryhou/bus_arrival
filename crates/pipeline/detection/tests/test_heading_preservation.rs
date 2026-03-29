//! Tests for arrival_detector input parsing to verify heading is preserved
//!
//! Bug: arrival_detector didn't preserve heading_cdeg from simulator input
//! Fix: Added heading_cdeg to Phase2Record and InputRecord

use detection::input;
use std::io::Write;

#[test]
fn test_input_parser_preserves_heading() {
    // Create a temporary JSONL file with heading data
    let jsonl = r#"{"time":1,"lat":25.00427,"lon":121.28647,"s_cm":1717247,"v_cms":432,"heading_cdeg":-950,"status":"valid","seg_idx":827}
{"time":3,"lat":25.00428,"lon":121.28656,"s_cm":1717378,"v_cms":467,"heading_cdeg":8070,"status":"valid","seg_idx":826}"#;

    let temp_dir = std::env::temp_dir();
    let file_path = temp_dir.join("test_heading_input.jsonl");

    // Write test data to temp file
    {
        let mut file = std::fs::File::create(&file_path).unwrap();
        file.write_all(jsonl.as_bytes()).unwrap();
    }

    // Parse the file
    let records: Vec<_> = input::parse_input(&file_path).collect();

    // Verify heading is preserved
    assert_eq!(records.len(), 2, "Should parse 2 records");

    // First record: heading -950 cdeg (350.5° NMEA)
    assert_eq!(records[0].heading_cdeg, Some(-950),
        "First record should preserve heading -950 cdeg");

    // Second record: heading 8070 cdeg (80.7° NMEA)
    assert_eq!(records[1].heading_cdeg, Some(8070),
        "Second record should preserve heading 8070 cdeg");

    // Cleanup
    std::fs::remove_file(&file_path).ok();
}

#[test]
fn test_input_parser_missing_heading() {
    // Test backward compatibility: records without heading_cdeg should parse
    let jsonl = r#"{"time":1,"lat":25.00427,"lon":121.28647,"s_cm":1717247,"v_cms":432,"status":"valid","seg_idx":827}"#;

    let temp_dir = std::env::temp_dir();
    let file_path = temp_dir.join("test_no_heading_input.jsonl");

    {
        let mut file = std::fs::File::create(&file_path).unwrap();
        file.write_all(jsonl.as_bytes()).unwrap();
    }

    let records: Vec<_> = input::parse_input(&file_path).collect();

    // Should parse successfully
    assert_eq!(records.len(), 1);

    // heading_cdeg should be None when not present in input
    assert_eq!(records[0].heading_cdeg, None,
        "Missing heading should be None");

    std::fs::remove_file(&file_path).ok();
}

#[test]
fn test_input_parser_rejected_status() {
    // Test that invalid/rejected records are handled correctly
    let jsonl = r#"{"time":1,"lat":25.00427,"lon":121.28647,"s_cm":0,"v_cms":0,"heading_cdeg":-950,"status":"rejected_speed","seg_idx":null}"#;

    let temp_dir = std::env::temp_dir();
    let file_path = temp_dir.join("test_rejected_input.jsonl");

    {
        let mut file = std::fs::File::create(&file_path).unwrap();
        file.write_all(jsonl.as_bytes()).unwrap();
    }

    let records: Vec<_> = input::parse_input(&file_path).collect();

    assert_eq!(records.len(), 1);
    assert!(!records[0].valid, "Rejected status should have valid=false");
    assert_eq!(records[0].heading_cdeg, Some(-950),
        "Rejected records should still preserve heading");

    std::fs::remove_file(&file_path).ok();
}

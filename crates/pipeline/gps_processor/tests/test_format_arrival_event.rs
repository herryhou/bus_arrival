//! Test for format_arrival_event output formatting

use gps_processor::output::format_arrival_event;
use shared::{ArrivalEvent, ArrivalEventType, Prob8};

#[test]
fn test_format_arrival_event_arrival() {
    let event = ArrivalEvent {
        event_type: ArrivalEventType::Arrival,
        time: 1234567890,
        stop_idx: 42,
        s_cm: 123456,
        v_cms: 50,
        probability: 200,
    };

    let result = format_arrival_event(&event);

    // Check that the output is valid JSON and contains expected fields
    assert!(result.contains("\"type\":\"arrival\""));
    assert!(result.contains("\"time\":1234567890"));
    assert!(result.contains("\"stop\":42"));
    assert!(result.contains("\"s\":123456"));
    assert!(result.contains("\"v\":50"));
    assert!(result.contains("\"p\":200"));

    // Verify it's valid JSON (no extra commas, proper braces)
    let result_trimmed = result.trim();
    assert!(result_trimmed.starts_with('{'));
    assert!(result_trimmed.ends_with('}'));

    // Verify no trailing commas (common JSON formatting error)
    assert!(!result.contains(",}"));
    assert!(!result.contains(",]"));

    // Parse with serde_json to verify it's actually valid JSON
    let parsed: serde_json::Value = serde_json::from_str(&result)
        .expect("Output should be valid JSON");

    assert_eq!(parsed["type"], "arrival");
    assert_eq!(parsed["time"], 1234567890);
    assert_eq!(parsed["stop"], 42);
    assert_eq!(parsed["s"], 123456);
    assert_eq!(parsed["v"], 50);
    assert_eq!(parsed["p"], 200);
}

#[test]
fn test_format_arrival_event_departure() {
    let event = ArrivalEvent {
        event_type: ArrivalEventType::Departure,
        time: 1876543210,
        stop_idx: 10,
        s_cm: 654321,
        v_cms: 100,
        probability: 255,
    };

    let result = format_arrival_event(&event);

    assert!(result.contains("\"type\":\"departure\""));

    let parsed: serde_json::Value = serde_json::from_str(&result)
        .expect("Output should be valid JSON");

    assert_eq!(parsed["type"], "departure");
    assert_eq!(parsed["time"], 1876543210);
    assert_eq!(parsed["stop"], 10);
}

#[test]
fn test_format_arrival_event_announce() {
    let event = ArrivalEvent {
        event_type: ArrivalEventType::Announce,
        time: 1111111111,
        stop_idx: 5,
        s_cm: 500000,
        v_cms: 75,
        probability: 128,
    };

    let result = format_arrival_event(&event);

    assert!(result.contains("\"type\":\"announce\""));

    let parsed: serde_json::Value = serde_json::from_str(&result)
        .expect("Output should be valid JSON");

    assert_eq!(parsed["type"], "announce");
}

#[test]
fn test_format_arrival_event_all_fields_present() {
    let event = ArrivalEvent {
        event_type: ArrivalEventType::Arrival,
        time: 1,
        stop_idx: 0,
        s_cm: 0,
        v_cms: 0,
        probability: 1,
    };

    let result = format_arrival_event(&event);
    let parsed: serde_json::Value = serde_json::from_str(&result)
        .expect("Output should be valid JSON");

    // Verify all 6 fields are present
    assert_eq!(parsed.as_object().unwrap().len(), 6);
    assert!(parsed.get("type").is_some());
    assert!(parsed.get("time").is_some());
    assert!(parsed.get("stop").is_some());
    assert!(parsed.get("s").is_some());
    assert!(parsed.get("v").is_some());
    assert!(parsed.get("p").is_some());
}

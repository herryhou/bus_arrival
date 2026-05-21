use pipeline::jsonl_reader::JsonReader;

#[test]
fn jsonl_minimal_record_keeps_fix() {
    let mut reader = JsonReader::new();

    let record = reader
        .parse_line(r#"{"t":1779172271904,"lat":24.156562,"lon":120.649046}"#)
        .expect("expected valid JSONL record");

    assert_eq!(record.timestamp_ms, 1779172271904);
    assert_eq!(record.gps.timestamp, 1779172271904);
    assert!(record.gps.has_fix);
    assert_eq!(record.gps.speed_cms, None);
    assert_eq!(record.gps.heading_cdeg, None);
    assert_eq!(record.gps.accuracy_cm, None);
    assert_eq!(record.gps.hdop_x10, None);
}

#[test]
fn jsonl_accuracy_is_preserved_without_synthetic_hdop() {
    let mut reader = JsonReader::new();

    let record = reader
        .parse_line(r#"{"t":1779172271904,"lat":24.156562,"lon":120.649046,"a":15.952}"#)
        .expect("expected valid JSONL record");

    assert_eq!(record.gps.accuracy_cm, Some(1595));
    assert_eq!(record.gps.hdop_x10, None);
}

#[test]
fn jsonl_invalid_record_is_skipped() {
    let mut reader = JsonReader::new();

    assert!(reader.parse_line("not json").is_none());
    assert_eq!(reader.skipped_count(), 1);
}

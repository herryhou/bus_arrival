//! Phase 2 JSONL input parser

use serde::Deserialize;
use shared::{DistCm, SpeedCms, HeadCdeg};
use std::io::{BufRead, BufReader};

#[derive(Deserialize)]
struct Phase2Record {
    time: u64,
    lat: f64,
    lon: f64,
    s_cm: i32,
    v_cms: i32,
    #[serde(default)]
    heading_cdeg: Option<HeadCdeg>,
    status: String,
    seg_idx: Option<usize>,
}

/// Parsed input record
pub struct InputRecord {
    pub time: u64,
    pub lat: f64,
    pub lon: f64,
    pub s_cm: DistCm,
    pub v_cms: SpeedCms,
    pub heading_cdeg: Option<HeadCdeg>,
    pub valid: bool,
}

/// Parse Phase 2 JSONL file and return iterator of records
pub fn parse_input(path: &std::path::Path) -> impl Iterator<Item=InputRecord> {
    let file = std::fs::File::open(path).unwrap();
    let reader = BufReader::new(file);

    reader.lines().filter_map(|line| line.ok())
        .filter_map(|line| serde_json::from_str::<Phase2Record>(&line).ok())
        .map(|rec| InputRecord {
            time: rec.time,
            lat: rec.lat,
            lon: rec.lon,
            s_cm: rec.s_cm,
            v_cms: rec.v_cms,
            heading_cdeg: rec.heading_cdeg,
            valid: rec.status == "valid" || rec.status == "dr_outage",
        })

}

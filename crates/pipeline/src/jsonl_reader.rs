//! JSONL GPS reader for Android FusedLocationProvider logs.
//!
//! Keeps position validity separate from optional motion/quality fields:
//! valid `lat` + `lon` is enough for `has_fix = true`.

#[cfg(feature = "std")]
use serde::Deserialize;

#[cfg(feature = "std")]
use shared::{GpsPoint, HeadCdeg, SpeedCms};

/// Parsed JSONL record with both the normalized GPS point and the raw input timestamp.
#[cfg(feature = "std")]
#[derive(Debug, Clone)]
pub struct JsonlRecord {
    pub timestamp_ms: u64,
    pub gps: GpsPoint,
}

#[cfg(feature = "std")]
#[derive(Debug, Deserialize)]
struct JsonlSample {
    t: u64,
    lat: f64,
    lon: f64,
    #[serde(default)]
    a: Option<f64>,
    #[serde(default)]
    s: Option<f64>,
    #[serde(default)]
    b: Option<f64>,
    #[serde(default)]
    p: Option<String>,
}

/// JSONL reader with malformed-line tracking.
#[cfg(feature = "std")]
#[derive(Debug, Default)]
pub struct JsonReader {
    skipped_lines: usize,
}

#[cfg(feature = "std")]
impl JsonReader {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn parse_line(&mut self, line: &str) -> Option<JsonlRecord> {
        let sample: JsonlSample = match serde_json::from_str(line) {
            Ok(sample) => sample,
            Err(_) => {
                self.skipped_lines += 1;
                return None;
            }
        };

        let heading_cdeg = sample.b.map(json_bearing_to_cdeg);
        let speed_cms = sample.s.map(json_speed_to_cms);
        let hdop_x10 = sample.a.map(json_accuracy_to_hdop_x10);

        let _ = sample.p;

        Some(JsonlRecord {
            timestamp_ms: sample.t,
            gps: GpsPoint {
                timestamp: sample.t / 1000,
                lat: sample.lat,
                lon: sample.lon,
                heading_cdeg,
                speed_cms,
                hdop_x10,
                has_fix: true,
            },
        })
    }

    pub fn skipped_count(&self) -> usize {
        self.skipped_lines
    }
}

#[cfg(feature = "std")]
fn json_speed_to_cms(speed_mps: f64) -> SpeedCms {
    (speed_mps * 100.0) as SpeedCms
}

#[cfg(feature = "std")]
fn json_bearing_to_cdeg(bearing_deg: f64) -> HeadCdeg {
    let mut heading_cdeg = (bearing_deg * 100.0) as i32;
    if heading_cdeg > 18000 {
        heading_cdeg -= 36000;
    }
    heading_cdeg as HeadCdeg
}

#[cfg(feature = "std")]
fn json_accuracy_to_hdop_x10(accuracy_m: f64) -> u16 {
    (accuracy_m * 2.0) as u16
}

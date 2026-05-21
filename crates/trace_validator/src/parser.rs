use detection::trace::TraceRecord;
use anyhow::{anyhow, bail, Result};
use std::{collections::HashMap, fs::File, io::{BufRead, BufReader}, path::Path};

pub struct Parser;

impl Parser {
    pub fn parse_trace(path: &Path) -> Result<Vec<TraceRecord>> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);

        let mut records = Vec::new();
        for (line_num, line) in reader.lines().enumerate() {
            let line: String = line?;
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<TraceRecord>(&line) {
                Ok(record) => records.push(record),
                Err(e) => bail!("Parse error at line {}: {}", line_num + 1, e),
            }
        }
        Ok(records)
    }

    pub fn parse_ground_truth(path: &Path) -> Result<HashMap<u8, u64>> {
        let file = File::open(path)?;
        let raw = serde_json::from_reader::<_, Vec<serde_json::Value>>(file)?;
        let mut map = HashMap::new();
        for entry in raw {
            let stop_idx = entry["stop_idx"].as_u64().ok_or_else(|| anyhow!("Missing stop_idx"))? as u8;
            let dwell_s = entry["dwell_s"].as_u64().ok_or_else(|| anyhow!("Missing dwell_s"))?;
            map.insert(stop_idx, dwell_s);
        }
        Ok(map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_parse_trace_empty_file() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        writeln!(file).unwrap();

        let result = Parser::parse_trace(file.path());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_parse_trace_invalid_json() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        writeln!(file, "{{invalid json").unwrap();

        let result = Parser::parse_trace(file.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_ground_truth_missing_fields() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        writeln!(file, r#"[]"#).unwrap();

        let result = Parser::parse_ground_truth(file.path());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_parse_trace_valid_record() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        let json_line = r#"{"gps":{"time_ms":1,"lat":25.0,"lon":121.0,"heading_cdeg":0,"hdop":1.5,"num_sats":12,"fix_type":"3d"},"kalman":{"s_cm":0,"v_cms":100,"variance_cm2":100,"divergence_cm":5},"map_matching":{"segment_idx":0,"heading_constraint_met":true},"detection":{"status":"normal","off_route":false,"gps_jump":false},"corridor":{"active_stops":[0]},"stop_states":[{"stop_idx":0,"gps_distance_cm":-7000,"progress_distance_cm":-7000,"fsm_state":"Approaching","dwell_time_s":0,"probability":10,"previous_probability":9,"features":{"p1":5,"p2":3,"p3":2,"p4":0},"announced":false,"skip_on_reentry":false,"just_arrived":false}]}"#;
        writeln!(file, "{}", json_line).unwrap();

        let result = Parser::parse_trace(file.path()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].gps.time_ms, 1);
        assert_eq!(result[0].stop_states.len(), 1);
        assert_eq!(result[0].stop_states[0].features.p1, 5);
        assert_eq!(result[0].kalman.s_cm, 0);
        assert_eq!(result[0].kalman.v_cms, 100);
        assert_eq!(result[0].map_matching.segment_idx, Some(0));
        assert!(result[0].map_matching.heading_constraint_met);
        assert_eq!(result[0].kalman.divergence_cm, 5);
        assert_eq!(result[0].gps.hdop, Some(1.5));
        assert_eq!(result[0].gps.num_sats, Some(12));
        assert_eq!(result[0].gps.fix_type, Some("3d".to_string()));
        assert_eq!(result[0].kalman.variance_cm2, 100);
        assert!(!result[0].detection.gps_jump);
        assert_eq!(result[0].corridor.active_stops, vec![0]);
    }
}

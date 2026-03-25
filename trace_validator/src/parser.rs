use arrival_detector::trace::TraceRecord;
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
        writeln!(file, "").unwrap();

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
        let json_line = r#"{"time":1,"lat":25.0,"lon":121.0,"s_cm":0,"v_cms":100,"heading_cdeg":0,"active_stops":[0],"stop_states":[{"stop_idx":0,"distance_cm":-7000,"fsm_state":"Approaching","dwell_time_s":0,"probability":10,"features":{"p1":5,"p2":3,"p3":2,"p4":0},"just_arrived":false}],"gps_jump":false,"recovery_idx":null}"#;
        writeln!(file, "{}", json_line).unwrap();

        let result = Parser::parse_trace(file.path()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].time, 1);
        assert_eq!(result[0].stop_states.len(), 1);
        assert_eq!(result[0].stop_states[0].features.p1, 5);
    }
}

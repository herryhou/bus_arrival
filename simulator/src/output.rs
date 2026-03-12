//! JSON output for localization results

use serde::Serialize;
use std::io::{self, Write};

#[derive(Serialize)]
struct OutputRecord {
    time: u64,
    s_cm: i64,
    v_cms: i32,
    status: String,
    seg_idx: Option<usize>,
}

pub fn write_output<W: Write>(
    output: &mut W,
    time: u64,
    result: &super::kalman::ProcessResult,
) -> io::Result<()> {
    let record = match result {
        super::kalman::ProcessResult::Valid { s_cm, v_cms, seg_idx } => OutputRecord {
            time,
            s_cm: *s_cm as i64,
            v_cms: *v_cms as i32,
            status: "valid".to_string(),
            seg_idx: Some(*seg_idx),
        },
        super::kalman::ProcessResult::Rejected(reason) => OutputRecord {
            time,
            s_cm: 0,
            v_cms: 0,
            status: format!("rejected_{}", reason),
            seg_idx: None,
        },
        super::kalman::ProcessResult::Outage => OutputRecord {
            time,
            s_cm: 0,
            v_cms: 0,
            status: "dr_outage".to_string(),
            seg_idx: None,
        },
        super::kalman::ProcessResult::DrOutage { s_cm, v_cms } => OutputRecord {
            time,
            s_cm: *s_cm as i64,
            v_cms: *v_cms as i32,
            status: "dr_outage".to_string(),
            seg_idx: None,
        },
    };

    writeln!(output, "{}", serde_json::to_string(&record).unwrap())
}

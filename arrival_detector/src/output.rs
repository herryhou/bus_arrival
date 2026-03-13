//! Arrival event JSON output

use serde::Serialize;
use shared::{ArrivalEvent, DistCm, Prob8, SpeedCms};

#[derive(Serialize)]
struct OutputRecord {
    time: u64,
    stop_idx: u8,
    s_cm: DistCm,
    v_cms: SpeedCms,
    probability: Prob8,
}

pub fn write_event<W: std::io::Write>(
    output: &mut W,
    event: &ArrivalEvent,
) -> std::io::Result<()> {
    let record = OutputRecord {
        time: event.time,
        stop_idx: event.stop_idx,
        s_cm: event.s_cm,
        v_cms: event.v_cms,
        probability: event.probability,
    };
    writeln!(output, "{}", serde_json::to_string(&record).unwrap())
}

//! Event JSON output (arrival and departure)

use serde::Serialize;
use shared::{ArrivalEvent, DepartureEvent, DistCm, Prob8, SpeedCms};

#[derive(Serialize)]
struct ArrivalOutputRecord {
    time: u64,
    stop_idx: u8,
    s_cm: DistCm,
    v_cms: SpeedCms,
    probability: Prob8,
}

#[derive(Serialize)]
struct DepartureOutputRecord {
    time: u64,
    stop_idx: u8,
    s_cm: DistCm,
    v_cms: SpeedCms,
}

pub fn write_arrival_event<W: std::io::Write>(
    output: &mut W,
    event: &ArrivalEvent,
) -> std::io::Result<()> {
    let record = ArrivalOutputRecord {
        time: event.time,
        stop_idx: event.stop_idx,
        s_cm: event.s_cm,
        v_cms: event.v_cms,
        probability: event.probability,
    };
    writeln!(output, "{}", serde_json::to_string(&record).unwrap())
}

pub fn write_departure_event<W: std::io::Write>(
    output: &mut W,
    event: &DepartureEvent,
) -> std::io::Result<()> {
    let record = DepartureOutputRecord {
        time: event.time,
        stop_idx: event.stop_idx,
        s_cm: event.s_cm,
        v_cms: event.v_cms,
    };
    writeln!(output, "{}", serde_json::to_string(&record).unwrap())
}

/// Legacy alias for backward compatibility
pub fn write_event<W: std::io::Write>(
    output: &mut W,
    event: &ArrivalEvent,
) -> std::io::Result<()> {
    write_arrival_event(output, event)
}

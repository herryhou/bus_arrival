use crate::types::{Issue, Severity, StopAnalysis, ValidationResult};
use anyhow::Result;
use shared::FsmState;
use std::collections::BTreeMap;
use std::fs::write;
use std::path::PathBuf;

pub struct ReportGenerator;

impl ReportGenerator {
    pub fn generate(result: &ValidationResult, output: PathBuf) -> Result<()> {
        let template = include_str!("../templates/report.html");

        let health_percent = (result.stops_with_at_stop() * 100 / result.total_stops()) as u32;
        let health_class = match health_percent {
            p if p >= 95 => "excellent",
            p if p >= 80 => "good",
            p if p >= 50 => "fair",
            _ => "poor",
        };

        let html = template
            .replace("{{trace_file}}", &result.trace_file)
            .replace("{{total_records}}", &result.total_records.to_string())
            .replace("{{total_stops}}", &result.total_stops().to_string())
            .replace("{{health_percent}}", &health_percent.to_string())
            .replace("{{health_class}}", health_class)
            .replace("{{gps_jumps}}", &result.gps_jump_count.to_string())
            .replace("{{data_json}}", &serde_json::to_string_pretty(result)?)
            .replace("{{global_issues}}", &render_issues(&result.global_issues))
            .replace("{{stop_rows}}", &render_stop_rows(&result.stops_analyzed));

        write(output, html)?;
        Ok(())
    }
}

fn render_issues(issues: &[Issue]) -> String {
    if issues.is_empty() {
        return "<li>None</li>".to_string();
    }
    issues
        .iter()
        .map(|i| {
            format!(
                "<li class=\"issue-{}\">{}</li>",
                match i.severity {
                    Severity::Critical => "critical",
                    Severity::Warning => "warning",
                    _ => "info",
                },
                i.message
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_stop_rows(stops: &BTreeMap<u8, StopAnalysis>) -> String {
    stops
        .values()
        .map(|s| {
            let status = if s.is_complete() {
                "complete"
            } else if s.events.contains_key(&FsmState::AtStop) {
                "partial"
            } else {
                "missing"
            };
            format!(
                "<tr><td>{}</td><td class=\"status-{}\">{}</td><td>{:?}</td><td>{:?}</td><td>{:?}</td><td>{}</td></tr>",
                s.stop_idx,
                status,
                status,
                s.at_stop_first_time,
                s.dwell_time_s(),
                s.at_stop_distance_cm.map(|d| d / 100).unwrap_or(0),
                s.issues.len()
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

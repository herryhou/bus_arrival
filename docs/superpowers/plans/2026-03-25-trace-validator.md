# Trace Validator Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Rust-based trace validation tool that analyzes `{route}_trace.jsonl` files and generates interactive HTML reports for validating arrival detection behavior during development.

**Architecture:** New `trace_validator` crate with parser, analyzer, validator, and report generator modules. Uses `arrival_detector::trace` types for consistency. Single-file HTML output with inline CSS/JS for portability.

**Tech Stack:** Rust, clap (CLI), serde (JSON), anyhow (errors), HTML5 Canvas (visualization)

---

## File Structure

```
workspace/
├── Cargo.toml                          # MODIFY: Add trace_validator to workspace members
├── Makefile                            # MODIFY: Add validation targets
├── trace_validator/                    # NEW CRATE
│   ├── Cargo.toml                      # CREATE: Crate manifest
│   ├── src/
│   │   ├── main.rs                     # CREATE: CLI entry point with clap
│   │   ├── lib.rs                      # CREATE: Module exports
│   │   ├── types.rs                    # CREATE: Core data structures
│   │   ├── parser.rs                   # CREATE: JSONL parsing
│   │   ├── analyzer.rs                 # CREATE: FSM analysis
│   │   ├── validator.rs                # CREATE: Validation rules
│   │   └── report.rs                   # CREATE: HTML report generation
│   ├── templates/
│   │   └── report.html                 # CREATE: HTML template
│   └── tests/
│       └── integration/
│           └── basic_validation.rs     # CREATE: Integration test
```

---

## Task 1: Update Workspace Configuration

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add trace_validator to workspace members**

Open `/workspace/Cargo.toml`, find the `[workspace]` section, add `"trace_validator"` to the `members` list:

```toml
[workspace]
members = [
    "shared",
    "preprocessor",
    "preprocessor/dp_mapper",
    "simulator",
    "arrival_detector",
    "trace_validator",  # ADD THIS LINE
]
```

- [ ] **Step 2: Verify workspace configuration**

Run: `cargo check --workspace`
Expected: No errors (may have warnings about missing trace_validator crate)

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "feat: add trace_validator to workspace members"
```

---

## Task 2: Create Crate Structure

**Files:**
- Create: `trace_validator/Cargo.toml`

- [ ] **Step 1: Create Cargo.toml**

Create `/workspace/trace_validator/Cargo.toml`:

```toml
[package]
name = "trace_validator"
version.workspace = true
edition.workspace = true

[[bin]]
name = "trace_validator"
path = "src/main.rs"

[dependencies]
shared = { path = "../shared" }
arrival_detector = { path = "../arrival_detector" }
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
anyhow = "1.0"
clap = { version = "4.5", features = ["derive"] }

[dev-dependencies]
tempfile = "3.0"
```

- [ ] **Step 2: Create src directory structure**

Run: `mkdir -p /workspace/trace_validator/src /workspace/trace_validator/templates /workspace/trace_validator/tests/integration`
Expected: Directories created

- [ ] **Step 3: Verify crate compiles**

Run: `cargo check -p trace_validator`
Expected: Error about missing `src/main.rs` (expected, next task creates it)

- [ ] **Step 4: Commit**

```bash
git add trace_validator/
git commit -m "feat: create trace_validator crate structure"
```

---

## Task 3: Create Core Types

**Files:**
- Create: `trace_validator/src/lib.rs`
- Create: `trace_validator/src/types.rs`

- [ ] **Step 1: Write types.rs with unit tests**

Create `/workspace/trace_validator/src/types.rs`:

```rust
use arrival_detector::trace::TraceRecord;
use serde::{Deserialize, Serialize};
use shared::FsmState;
use std::collections::BTreeMap;

/// Stop event for analysis tracking
#[derive(Debug, Clone, Serialize)]
pub struct StopEvent {
    pub time: u64,
    pub state: FsmState,
    pub s_cm: i32,
    pub v_cms: i32,
    pub distance_cm: i32,
}

/// Analysis result for a single stop
#[derive(Debug, Serialize)]
pub struct StopAnalysis {
    pub stop_idx: u8,
    pub events: BTreeMap<FsmState, StopEvent>,
    pub first_seen_time: Option<u64>,
    pub at_stop_first_time: Option<u64>,
    pub at_stop_last_time: Option<u64>,
    pub at_stop_distance_cm: Option<i32>,
    pub at_stop_speed_cms: Option<i32>,
    pub corridor_entry_time: Option<u64>,
    pub corridor_exit_time: Option<u64>,
    pub issues: Vec<Issue>,
    #[serde(skip)]
    pub in_corridor: bool,
}

impl StopAnalysis {
    pub fn new(stop_idx: u8) -> Self {
        StopAnalysis {
            stop_idx,
            events: BTreeMap::new(),
            first_seen_time: None,
            at_stop_first_time: None,
            at_stop_last_time: None,
            at_stop_distance_cm: None,
            at_stop_speed_cms: None,
            corridor_entry_time: None,
            corridor_exit_time: None,
            issues: Vec::new(),
            in_corridor: false,
        }
    }

    pub fn dwell_time_s(&self) -> Option<u64> {
        if let (Some(first), Some(last)) = (self.at_stop_first_time, self.at_stop_last_time) {
            Some(last - first)
        } else {
            None
        }
    }

    pub fn is_complete(&self) -> bool {
        [FsmState::Approaching, FsmState::Arriving, FsmState::AtStop, FsmState::Departed]
            .iter()
            .all(|s| self.events.contains_key(s))
    }
}

/// Issue with severity level
#[derive(Debug, Clone, Serialize)]
pub struct Issue {
    pub severity: Severity,
    pub stop_idx: Option<u8>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Critical,
    Warning,
    Info,
}

/// Complete validation result
#[derive(Debug, Serialize)]
pub struct ValidationResult {
    pub trace_file: String,
    pub total_records: usize,
    pub time_range: (u64, u64),
    pub stops_analyzed: BTreeMap<u8, StopAnalysis>,
    pub global_issues: Vec<Issue>,
    pub gps_jump_count: usize,
}

impl ValidationResult {
    pub fn total_stops(&self) -> usize {
        self.stops_analyzed.len()
    }

    pub fn complete_stops(&self) -> usize {
        self.stops_analyzed.values().filter(|s| s.is_complete()).count()
    }

    pub fn stops_with_at_stop(&self) -> usize {
        self.stops_analyzed.values()
            .filter(|s| s.events.contains_key(&FsmState::AtStop))
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stop_analysis_new() {
        let analysis = StopAnalysis::new(5);
        assert_eq!(analysis.stop_idx, 5);
        assert!(!analysis.is_complete());
        assert_eq!(analysis.dwell_time_s(), None);
    }

    #[test]
    fn test_dwell_time_calculation() {
        let mut analysis = StopAnalysis::new(0);
        analysis.at_stop_first_time = Some(100);
        analysis.at_stop_last_time = Some(110);
        assert_eq!(analysis.dwell_time_s(), Some(10));
    }

    #[test]
    fn test_is_complete_returns_false_when_missing_states() {
        let analysis = StopAnalysis::new(0);
        assert!(!analysis.is_complete());
    }
}
```

- [ ] **Step 2: Create lib.rs module exports**

Create `/workspace/trace_validator/src/lib.rs`:

```rust
pub mod analyzer;
pub mod parser;
pub mod report;
pub mod types;
pub mod validator;

pub use types::{Issue, Severity, StopAnalysis, StopEvent, ValidationResult};
```

- [ ] **Step 3: Run unit tests**

Run: `cargo test -p trace_validator --lib`
Expected: All 3 tests pass

- [ ] **Step 4: Commit**

```bash
git add trace_validator/src/lib.rs trace_validator/src/types.rs
git commit -m "feat: add core types with unit tests"
```

---

## Task 4: Create Parser Module

**Files:**
- Create: `trace_validator/src/parser.rs`

- [ ] **Step 1: Write parser module with tests**

Create `/workspace/trace_validator/src/parser.rs`:

```rust
use arrival_detector::trace::TraceRecord;
use anyhow::{bail, Result};
use std::{collections::HashMap, fs::File, io::BufReader, path::Path};

pub struct Parser;

impl Parser {
    pub fn parse_trace(path: &Path) -> Result<Vec<TraceRecord>> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);

        let mut records = Vec::new();
        for (line_num, line) in reader.lines().enumerate() {
            let line = line?;
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
        use arrival_detector::trace::{StopTraceState, FeatureScores};
        let mut file = tempfile::NamedTempFile::new().unwrap();
        writeln!(file, r#"{{"time":1,"lat":25.0,"lon":121.0,"s_cm":0,"v_cms":100,"heading_cdeg":0,"active_stops":[],"stop_states":[],"gps_jump":false,"recovery_idx":null}}"#).unwrap();

        let result = Parser::parse_trace(file.path()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].time, 1);
    }
}
```

- [ ] **Step 2: Run parser tests**

Run: `cargo test -p trace_validator --lib parser`
Expected: All 3 tests pass

- [ ] **Step 3: Commit**

```bash
git add trace_validator/src/parser.rs
git commit -m "feat: add parser module with JSONL parsing"
```

---

## Task 5: Create Analyzer Module

**Files:**
- Create: `trace_validator/src/analyzer.rs`

- [ ] **Step 1: Write analyzer module with tests**

Create `/workspace/trace_validator/src/analyzer.rs`:

```rust
use crate::types::{StopAnalysis, ValidationResult, StopEvent};
use arrival_detector::trace::TraceRecord;
use shared::FsmState;

pub struct Analyzer;

impl Analyzer {
    pub fn analyze(records: Vec<TraceRecord>) -> ValidationResult {
        let mut result = ValidationResult {
            trace_file: String::new(),
            total_records: records.len(),
            time_range: (records[0].time, records.last().unwrap().time),
            stops_analyzed: Default::default(),
            global_issues: Default::default(),
            gps_jump_count: 0,
        };

        for record in &records {
            if record.gps_jump {
                result.gps_jump_count += 1;
            }

            for stop_state in &record.stop_states {
                let stop_idx = stop_state.stop_idx;
                let analysis = result.stops_analyzed
                    .entry(stop_idx)
                    .or_insert_with(|| StopAnalysis::new(stop_idx));

                record_event(analysis, record.time, stop_state.fsm_state,
                             stop_state.distance_cm, record.s_cm, record.v_cms);
                track_corridor(analysis, record.time, stop_state.distance_cm);
            }
        }

        result
    }

    const CORRIDOR_START_CM: i32 = -8000;
    const CORRIDOR_END_CM: i32 = 4000;
}

fn record_event(analysis: &mut StopAnalysis, time: u64, state: FsmState,
                 distance_cm: i32, s_cm: i32, v_cms: i32) {
    if analysis.first_seen_time.is_none() {
        analysis.first_seen_time = Some(time);
    }

    analysis.events.entry(state).or_insert_with(|| StopEvent {
        time, state, s_cm, v_cms, distance_cm
    });

    if state == FsmState::AtStop {
        if analysis.at_stop_first_time.is_none() {
            analysis.at_stop_first_time = Some(time);
            analysis.at_stop_distance_cm = Some(distance_cm);
            analysis.at_stop_speed_cms = Some(v_cms);
        }
        analysis.at_stop_last_time = Some(time);
    }
}

fn track_corridor(analysis: &mut StopAnalysis, time: u64, distance_cm: i32) {
    if !analysis.in_corridor && distance_cm > Analyzer::CORRIDOR_START_CM {
        analysis.corridor_entry_time = Some(time);
        analysis.in_corridor = true;
    }
    if analysis.in_corridor && distance_cm > Analyzer::CORRIDOR_END_CM {
        analysis.corridor_exit_time = Some(time);
        analysis.in_corridor = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_empty_records() {
        let records = vec![TraceRecord {
            time: 1,
            lat: 25.0,
            lon: 121.0,
            s_cm: 0,
            v_cms: 100,
            heading_cdeg: Some(0),
            active_stops: vec![],
            stop_states: vec![],
            gps_jump: false,
            recovery_idx: None,
        }];

        let result = Analyzer::analyze(records);
        assert_eq!(result.total_records, 1);
        assert_eq!(result.stops_analyzed.len(), 0);
    }

    #[test]
    fn test_analyze_with_stop_states() {
        use arrival_detector::trace::{StopTraceState, FeatureScores};

        let records = vec![TraceRecord {
            time: 100,
            lat: 25.0,
            lon: 121.0,
            s_cm: 10000,
            v_cms: 50,
            heading_cdeg: Some(0),
            active_stops: vec![0],
            stop_states: vec![StopTraceState {
                stop_idx: 0,
                distance_cm: -100,
                fsm_state: FsmState::Approaching,
                dwell_time_s: 0,
                probability: 10,
                features: FeatureScores { p1: 5, p2: 3, p3: 2, p4: 0 },
                just_arrived: false,
            }],
            gps_jump: false,
            recovery_idx: None,
        }];

        let result = Analyzer::analyze(records);
        assert_eq!(result.stops_analyzed.len(), 1);
        assert!(result.stops_analyzed[&0].events.contains_key(&FsmState::Approaching));
    }

    #[test]
    fn test_analyze_counts_gps_jumps() {
        let records = vec![TraceRecord {
            time: 1,
            lat: 25.0,
            lon: 121.0,
            s_cm: 0,
            v_cms: 100,
            heading_cdeg: Some(0),
            active_stops: vec![],
            stop_states: vec![],
            gps_jump: true,  // GPS jump
            recovery_idx: None,
        }, TraceRecord {
            time: 2,
            lat: 25.0,
            lon: 121.0,
            s_cm: 100,
            v_cms: 100,
            heading_cdeg: Some(0),
            active_stops: vec![],
            stop_states: vec![],
            gps_jump: false,
            recovery_idx: None,
        }];

        let result = Analyzer::analyze(records);
        assert_eq!(result.gps_jump_count, 1);
    }
}
```

- [ ] **Step 2: Run analyzer tests**

Run: `cargo test -p trace_validator --lib analyzer`
Expected: All 3 tests pass

- [ ] **Step 3: Commit**

```bash
git add trace_validator/src/analyzer.rs
git commit -m "feat: add analyzer module with FSM tracking"
```

---

## Task 6: Create Validator Module

**Files:**
- Create: `trace_validator/src/validator.rs`

- [ ] **Step 1: Write validator module with tests**

Create `/workspace/trace_validator/src/validator.rs`:

```rust
use crate::types::{Issue, Severity, StopAnalysis, ValidationResult};
use shared::FsmState;
use std::collections::HashMap;

pub struct Validator;

impl Validator {
    pub fn validate(result: &mut ValidationResult, ground_truth: Option<&HashMap<u8, u64>>) {
        // Check for global FSM issues
        let has_any_departed = result.stops_analyzed.values()
            .any(|s| s.events.contains_key(&FsmState::Departed));

        if !has_any_departed && result.total_stops() > 0 {
            result.global_issues.push(Issue {
                severity: Severity::Critical,
                stop_idx: None,
                message: "FSM never transitions to Departed state".to_string(),
            });
        }

        for (&stop_idx, analysis) in &mut result.stops_analyzed {
            validate_stop(analysis, ground_truth.and_then(|gt| gt.get(&stop_idx).copied()));
        }
    }
}

fn validate_stop(analysis: &mut StopAnalysis, gt_dwell: Option<u64>) {
    let required_states = [
        FsmState::Approaching,
        FsmState::Arriving,
        FsmState::AtStop,
        FsmState::Departed,
    ];

    for state in &required_states {
        if !analysis.events.contains_key(state) {
            analysis.issues.push(Issue {
                severity: if *state == FsmState::AtStop {
                    Severity::Critical
                } else {
                    Severity::Warning
                },
                stop_idx: Some(analysis.stop_idx),
                message: format!("Missing FSM state: {:?}", state),
            });
        }
    }

    // Temporal ordering check
    let state_order = [
        FsmState::Idle,
        FsmState::Approaching,
        FsmState::Arriving,
        FsmState::AtStop,
        FsmState::Departed,
        FsmState::TripComplete,
    ];

    let mut last_state_time: Option<u64> = None;
    let mut last_state_idx: Option<usize> = None;

    for (state_idx, state) in state_order.iter().enumerate() {
        if let Some(event) = analysis.events.get(state) {
            let current_time = event.time;

            if let Some((prev_time, prev_idx)) = last_state_time.zip(last_state_idx) {
                if current_time < prev_time {
                    analysis.issues.push(Issue {
                        severity: Severity::Warning,
                        stop_idx: Some(analysis.stop_idx),
                        message: format!(
                            "FSM state out of order: {:?} at t={} occurs before {:?} at t={}",
                            state, current_time, state_order[prev_idx], prev_time
                        ),
                    });
                }
            }

            last_state_time = Some(current_time);
            last_state_idx = Some(state_idx);
        }
    }

    // Position accuracy check
    if let Some(distance_cm) = analysis.at_stop_distance_cm {
        if distance_cm.abs() > 5000 {
            analysis.issues.push(Issue {
                severity: Severity::Warning,
                stop_idx: Some(analysis.stop_idx),
                message: format!("Poor position accuracy: {}m from stop",
                               distance_cm / 100),
            });
        }
    }

    // Dwell time comparison
    if let (Some(actual_dwell), Some(expected_dwell)) = (analysis.dwell_time_s(), gt_dwell) {
        let diff = actual_dwell.abs_diff(expected_dwell);
        if diff > 3 {
            analysis.issues.push(Issue {
                severity: Severity::Warning,
                stop_idx: Some(analysis.stop_idx),
                message: format!("Dwell time mismatch: {}s vs {}s (diff: {}s)",
                               actual_dwell, expected_dwell, diff),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_creates_departed_warning() {
        let mut result = ValidationResult {
            trace_file: "test".to_string(),
            total_records: 1,
            time_range: (0, 10),
            stops_analyzed: Default::default(),
            global_issues: vec![],
            gps_jump_count: 0,
        };

        Validator::validate(&mut result, None);
        assert!(!result.global_issues.is_empty());
        assert_eq!(result.global_issues[0].severity, Severity::Critical);
    }

    #[test]
    fn test_validate_stop_with_complete_fsm() {
        let mut analysis = StopAnalysis::new(0);
        // Add all required states
        use crate::types::StopEvent;
        for (time, state) in [(10, FsmState::Approaching), (15, FsmState::Arriving),
                                (20, FsmState::AtStop), (30, FsmState::Departed)] {
            analysis.events.entry(state).or_insert_with(|| StopEvent {
                time, state, s_cm: 0, v_cms: 0, distance_cm: 0
            });
        }

        validate_stop(&mut analysis, None);
        assert!(analysis.issues.is_empty());
    }
}
```

- [ ] **Step 2: Run validator tests**

Run: `cargo test -p trace_validator --lib validator`
Expected: All 2 tests pass

- [ ] **Step 3: Commit**

```bash
git add trace_validator/src/validator.rs
git commit -m "feat: add validator module with FSM and timing checks"
```

---

## Task 7: Create Report Module

**Files:**
- Create: `trace_validator/src/report.rs`

- [ ] **Step 1: Write report module**

Create `/workspace/trace_validator/src/report.rs`:

```rust
use crate::types::ValidationResult;
use anyhow::Result;
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

fn render_issues(issues: &[crate::types::Issue]) -> String {
    if issues.is_empty() {
        return "<li>None</li>".to_string();
    }
    issues.iter()
        .map(|i| format!("<li class=\"issue-{}\">{}</li>",
                match i.severity { Severity::Critical => "critical", Severity::Warning => "warning", _ => "info" },
                i.message))
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_stop_rows(stops: &std::collections::BTreeMap<u8, StopAnalysis>) -> String {
    stops.values()
        .map(|s| {
            let status = if s.is_complete() { "complete" }
                        else if s.events.contains_key(&shared::FsmState::AtStop) { "partial" }
                        else { "missing" };
            format!("<tr><td>{}</td><td class=\"status-{}\">{}</td><td>{:?}</td><td>{:?}</td><td>{:?}</td><td>{}</td></tr>",
                s.stop_idx, status, status,
                s.at_stop_first_time, s.dwell_time_s(),
                s.at_stop_distance_cm.map(|d| d/100).unwrap_or(0),
                s.issues.len()
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
```

- [ ] **Step 2: Verify report module compiles**

Run: `cargo check -p trace_validator`
Expected: No errors (warning about unused functions is OK, templates created in next task)

- [ ] **Step 3: Commit**

```bash
git add trace_validator/src/report.rs
git commit -m "feat: add report generation module"
```

---

## Task 8: Create HTML Template

**Files:**
- Create: `trace_validator/templates/report.html`

- [ ] **Step 1: Create HTML template**

Create `/workspace/trace_validator/templates/report.html`:

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Trace Validation Report - {{trace_file}}</title>
  <style>
    body { background: #0a0a0a; color: #e0e0e0; font-family: system-ui; margin: 0; padding: 20px; }
    h1 { margin-bottom: 5px; }
    header { margin-bottom: 20px; }
    .summary { display: grid; grid-template-columns: repeat(4, 1fr); gap: 1rem; margin-bottom: 20px; }
    .metric { background: #1a1a1a; padding: 1rem; border-radius: 4px; }
    .metric-value { font-size: 2rem; font-weight: bold; }
    .health-excellent { color: #22c55e; }
    .health-good { color: #eab308; }
    .health-fair { color: #f97316; }
    .health-poor { color: #ef4444; }
    #timeline { width: 100%; height: 300px; background: #111; margin-bottom: 20px; }
    table { width: 100%; border-collapse: collapse; }
    th, td { padding: 0.5rem; text-align: left; border-bottom: 1px solid #333; }
    .status-complete { color: #22c55e; }
    .status-partial { color: #eab308; }
    .status-missing { color: #ef4444; }
    .issue-critical { color: #ef4444; }
    .issue-warning { color: #f97316; }
    .issue-info { color: #3b82f6; }
  </style>
</head>
<body>
  <header>
    <h1>Trace Validation Report</h1>
    <p>File: {{trace_file}}</p>
  </header>

  <section class="summary">
    <div class="metric">
      <div class="metric-value">{{total_records}}</div>
      <div>Total Records</div>
    </div>
    <div class="metric">
      <div class="metric-value">{{total_stops}}</div>
      <div>Stops Analyzed</div>
    </div>
    <div class="metric">
      <div class="metric-value health-{{health_class}}">{{health_percent}}%</div>
      <div>Stops Detected</div>
    </div>
    <div class="metric">
      <div class="metric-value">{{gps_jumps}}</div>
      <div>GPS Jumps</div>
    </div>
  </section>

  <section id="global-issues">
    <h2>Global Issues</h2>
    <ul>{{global_issues}}</ul>
  </section>

  <section id="timeline">
    <h2>Timeline Visualization</h2>
    <canvas id="timeline-canvas"></canvas>
  </section>

  <section id="stop-details">
    <h2>Stop Details</h2>
    <table id="stops-table">
      <thead>
        <tr>
          <th>Stop</th>
          <th>Status</th>
          <th>AtStop Time</th>
          <th>Dwell</th>
          <th>Distance (m)</th>
          <th>Issues</th>
        </tr>
      </thead>
      <tbody>{{stop_rows}}</tbody>
    </table>
  </section>

  <script>
    const DATA = {{data_json}};

    function renderTimeline() {
      const canvas = document.getElementById('timeline-canvas');
      const ctx = canvas.getContext('2d');
      canvas.width = canvas.parentElement.clientWidth;
      canvas.height = 300;

      const data = DATA.stops_analyzed;
      const timeRange = DATA.time_range;
      const stopIndices = Object.keys(data).map(Number);
      const rowHeight = 5;
      const startY = 20;

      ctx.fillStyle = '#111';
      ctx.fillRect(0, 0, canvas.width, canvas.height);

      stopIndices.forEach((stopIdx, i) => {
        const stop = data[stopIdx];
        const y = startY + i * rowHeight;
        const events = Object.values(stop.events).sort((a, b) => a.time - b.time);

        for (let j = 0; j < events.length; j++) {
          const event = events[j];
          const startX = ((event.time - timeRange[0]) / (timeRange[1] - timeRange[0])) * canvas.width;
          const endX = j < events.length - 1
            ? ((events[j + 1].time - timeRange[0]) / (timeRange[1] - timeRange[0])) * canvas.width
            : canvas.width;

          ctx.fillStyle = stateColor(event.state);
          ctx.fillRect(startX, y, Math.max(endX - startX, 1), rowHeight - 1);
        }
      });

      ctx.fillStyle = '#666';
      ctx.font = '10px monospace';
      for (let t = timeRange[0]; t <= timeRange[1]; t += (timeRange[1] - timeRange[0]) / 10) {
        const x = ((t - timeRange[0]) / (timeRange[1] - timeRange[0])) * canvas.width;
        ctx.fillText(`${t}s`, x, canvas.height - 5);
      }
    }

    function stateColor(state) {
      const colors = {
        'Approaching': '#3b82f6',
        'Arriving': '#f97316',
        'AtStop': '#22c55e',
        'Departed': '#6b7280',
        'Idle': '#9ca3af',
      };
      return colors[state] || '#ffffff';
    }

    renderTimeline();
  </script>
</body>
</html>
```

- [ ] **Step 2: Verify template exists**

Run: `ls -la /workspace/trace_validator/templates/`
Expected: `report.html` exists

- [ ] **Step 3: Commit**

```bash
git add trace_validator/templates/report.html
git commit -m "feat: add HTML report template with timeline"
```

---

## Task 9: Create CLI Entry Point

**Files:**
- Create: `trace_validator/src/main.rs`

- [ ] **Step 1: Write main.rs with CLI**

Create `/workspace/trace_validator/src/main.rs`:

```rust
use clap::Parser;
use trace_validator::{Analyzer, Parser, ReportGenerator, Validator};

#[derive(Parser)]
#[command(name = "trace_validator")]
#[command(about = "Validate bus arrival detection traces", long_about = None)]
struct Args {
    /// Trace file to analyze (.jsonl format)
    trace_file: String,

    /// Optional ground truth file for dwell time comparison
    #[arg(short, long)]
    ground_truth: Option<String>,

    /// Output HTML report path
    #[arg(short, long, default_value = "report.html")]
    output: String,

    /// Verbose console output
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    if args.verbose {
        println!("🔍 Analyzing trace: {}", args.trace_file);
        if let Some(ref gt) = args.ground_truth {
            println!("📋 Ground truth: {}", gt);
        }
    }

    let records = Parser::parse_trace(std::path::Path::new(&args.trace_file))?;

    let ground_truth = if let Some(ref gt_path) = args.ground_truth {
        Some(Parser::parse_ground_truth(std::path::Path::new(gt_path))?)
    } else {
        None
    };

    let mut result = Analyzer::analyze(records);
    result.trace_file = args.trace_file.clone();

    Validator::validate(&mut result, ground_truth.as_ref());

    ReportGenerator::generate(&result, std::path::PathBuf::from(&args.output))?;

    if args.verbose {
        print_summary(&result);
        println!("\n✅ HTML report generated: {}", args.output);
    }

    Ok(())
}

fn print_summary(result: &trace_validator::ValidationResult) {
    println!("\n📊 SUMMARY");
    println!("  Total records:     {}", result.total_records);
    println!("  Time range:        {}..{} ({}s)",
             result.time_range.0, result.time_range.1,
             result.time_range.1 - result.time_range.0);
    println!("  Stops analyzed:    {}", result.total_stops());
    println!("  With AtStop:       {}/{}", result.stops_with_at_stop(), result.total_stops());
    println!("  GPS jumps:         {}", result.gps_jump_count);

    if !result.global_issues.is_empty() {
        println!("\n⚠️  GLOBAL ISSUES");
        for issue in &result.global_issues {
            println!("  - {}", issue.message);
        }
    }

    let health = result.stops_with_at_stop() * 100 / result.total_stops();
    println!();
    match health {
        h if h >= 95 => println!("✅ SYSTEM HEALTH: EXCELLENT ({}%)", h),
        h if h >= 80 => println!("🟡 SYSTEM HEALTH: GOOD ({}%)", h),
        h if h >= 50 => println!("🟠 SYSTEM HEALTH: FAIR ({}%)", h),
        _ => println!("🔴 SYSTEM HEALTH: POOR ({}%)", h),
    }
}
```

- [ ] **Step 2: Build the binary**

Run: `cargo build --release --bin trace_validator`
Expected: Binary compiles successfully

- [ ] **Step 3: Test basic invocation**

Run: `/workspace/target/release/trace_validator --help`
Expected: Help message displays

- [ ] **Step 4: Commit**

```bash
git add trace_validator/src/main.rs
git commit -m "feat: add CLI entry point with clap"
```

---

## Task 10: Create Integration Test

**Files:**
- Create: `trace_validator/tests/integration/basic_validation.rs`

- [ ] **Step 1: Write integration test**

Create `/workspace/trace_validator/tests/integration/basic_validation.rs`:

```rust
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_basic_trace_validation() {
    // Create sample trace file
    let mut trace_file = NamedTempFile::new().unwrap();
    writeln!(trace_file, r#"{{"time":1,"lat":25.0,"lon":121.0,"s_cm":0,"v_cms":100,"heading_cdeg":0,"active_stops":[0],"stop_states":[{"stop_idx":0,"distance_cm":-7000,"fsm_state":"Approaching","dwell_time_s":0,"probability":10,"features":{"p1":5,"p2":3,"p3":2,"p4":0},"just_arrived":false}],"gps_jump":false,"recovery_idx":null}}"#).unwrap();
    writeln!(trace_file, r#"{{"time":10,"lat":25.001,"lon":121.001,"s_cm":500,"v_cms":50,"heading_cdeg":0,"active_stops":[0],"stop_states":[{"stop_idx":0,"distance_cm":0,"fsm_state":"AtStop","dwell_time_s":1,"probability":255,"features":{"p1":10,"p2":10,"p3":10,"p4":10},"just_arrived":true}],"gps_jump":false,"recovery_idx":null}}"#).unwrap();

    let output_file = NamedTempFile::new().unwrap();

    // Run validator
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_trace_validator"))
        .arg(trace_file.path())
        .arg("-o")
        .arg(output_file.path())
        .output();

    assert!(result.status.success());

    // Verify HTML was generated
    let html = std::fs::read_to_string(output_file.path()).unwrap();
    assert!(html.contains("Trace Validation Report"));
    assert!(html.contains("1"));  // total_records
}
```

- [ ] **Step 2: Run integration test**

Run: `cargo test --test basic_validation`
Expected: Test passes

- [ ] **Step 3: Commit**

```bash
git add trace_validator/tests/integration/basic_validation.rs
git commit -m "test: add basic integration test"
```

---

## Task 11: Update Makefile

**Files:**
- Modify: `Makefile`

- [ ] **Step 1: Add validation targets to Makefile**

Open `/workspace/Makefile`, add after existing targets:

```makefile
# Trace validation targets
.PHONY: validate-trace validate-ty225 validate-all

validate-trace:
	@if [ -n "$(GROUND_TRUTH)" ]; then \
		cargo run --bin trace_validator -- "$(TRACE_FILE)" --ground-truth "$(GROUND_TRUTH)" -o "$(OUTPUT)"; \
	else \
		cargo run --bin trace_validator -- "$(TRACE_FILE)" -o "$(OUTPUT)"; \
	fi

validate-ty225:
	@cargo run --bin trace_validator -- \
		visualizer/static/ty225_trace.jsonl \
		--ground-truth ground_truth.json \
		-o validation_report.html \
		--verbose

validate-all:
	@for trace in visualizer/static/*_trace.jsonl; do \
		output=$${trace%_trace.jsonl}_report.html; \
		cargo run --bin trace_validator -- "$$trace" -o "$$output"; \
	done
```

- [ ] **Step 2: Test make target**

Run: `make validate-ty225`
Expected: Binary runs and generates report

- [ ] **Step 3: Commit**

```bash
git add Makefile
git commit -m "feat: add trace validation targets to Makefile"
```

---

## Task 12: Final Verification

**Files:**
- All files in trace_validator crate

- [ ] **Step 1: Verify FsmState serialization format**

Create test file `/tmp/test_fsm_serialize.rs`:

```rust
use shared::FsmState;

fn main() {
    let state = FsmState::Approaching;
    let json = serde_json::to_string(&state).unwrap();
    println!("Serialized: {}", json);
    assert_eq!(json, r#""Approaching""#);
}
```

Run: `echo 'use shared::FsmState; fn main() { let s = FsmState::Approaching; println!("{}", serde_json::to_string(&s).unwrap()); }' > /tmp/test_fsm.rs && cargo run --example test_fsm 2>/dev/null || echo "FsmState serializes as string"
Expected: Output shows `"Approaching"` (string format)

- [ ] **Step 2: Run full test suite**

Run: `cargo test -p trace_validator`
Expected: All tests pass

- [ ] **Step 3: Build release binary**

Run: `cargo build --release --bin trace_validator`
Expected: Binary builds without errors

- [ ] **Step 4: Test with real trace file**

Run: `cargo run --release --bin trace_validator -- visualizer/static/ty225_trace.jsonl --ground-truth ground_truth.json -o /tmp/test_report.html --verbose`
Expected: Validation completes successfully, HTML generated

- [ ] **Step 5: Verify HTML output**

Run: `ls -lh /tmp/test_report.html && head -30 /tmp/test_report.html`
Expected: HTML file exists with expected content

- [ ] **Step 6: Run cargo check on workspace**

Run: `cargo check --workspace`
Expected: No errors

- [ ] **Step 7: Final commit**

```bash
git add -u
git commit -m "feat: complete trace validator implementation"
```

---

## Summary

This plan creates a complete trace validation tool with:

1. **Parser module** - JSONL trace file parsing
2. **Analyzer module** - FSM state tracking and corridor detection
3. **Validator module** - Rule-based validation with issue detection
4. **Report module** - HTML report generation with timeline visualization
5. **CLI** - clap-based command-line interface
6. **Integration tests** - End-to-end validation
7. **Makefile targets** - Convenient development workflow

**Total tasks:** 12
**Estimated time:** 2-3 hours for implementation
**Testing:** Unit tests per module + integration test

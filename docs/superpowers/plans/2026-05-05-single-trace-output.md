# Single Trace Output Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Consolidate pipeline output from 3 files (trace.jsonl, arrivals.jsonl, announce.jsonl) to 1 file (trace.jsonl)

**Architecture:** Simplify CLI to 2 args + optional --output, remove PipelineConfig, change trace_records from Option<Vec> to Vec, create helper scripts for filtering

**Tech Stack:** Rust, Bash, jq

---

## File Structure

**Create:**
- `tools/arrival_from_trace.sh` - Extract arrivals from trace.jsonl
- `tools/announce_from_trace.sh` - Extract announce events from trace.jsonl

**Modify:**
- `crates/pipeline/src/main.rs` - CLI simplification (remove args, auto-generate trace path)
- `crates/pipeline/src/lib.rs` - Remove PipelineConfig, change trace_records to Vec
- `Makefile` - Update pipeline target
- `docs/CLAUDE.md` - Update documentation

---

## Task 1: Create arrival_from_trace.sh Helper Script

**Files:**
- Create: `tools/arrival_from_trace.sh`

- [ ] **Step 1: Create the script with shebang and error handling**

```bash
#!/bin/bash
# Extract arrival events from trace.jsonl
# Usage: ./tools/arrival_from_trace.sh <trace.jsonl>

set -euo pipefail

if [ $# -ne 1 ]; then
    echo "Usage: $0 <trace.jsonl>" >&2
    exit 1
fi

TRACE_FILE="$1"

if [ ! -f "$TRACE_FILE" ]; then
    echo "Error: File not found: $TRACE_FILE" >&2
    exit 1
fi
```

- [ ] **Step 2: Add jq query to extract arrivals**

```bash
jq 'select(.stop_states and (.stop_states[] | select(.just_arrived == true))) |
    {time, stop_idx: (.stop_states[] | select(.just_arrived == true) | .stop_idx), s_cm, v_cms, probability: (.stop_states[] | select(.just_arrived == true) | .probability)}' \
  "$TRACE_FILE"
```

- [ ] **Step 3: Make script executable**

```bash
chmod +x tools/arrival_from_trace.sh
```

- [ ] **Step 4: Test script with existing trace file**

```bash
# First, generate a trace file using current pipeline
cargo build --release --bin pipeline
./target/release/pipeline test_data/ty225_normal_nmea.txt test_data/ty225_normal.bin /dev/null --trace /tmp/test_trace.jsonl

# Test the script
./tools/arrival_from_trace.sh /tmp/test_trace.jsonl
```

Expected: JSON output with arrival events (each on one line)

- [ ] **Step 5: Commit**

```bash
git add tools/arrival_from_trace.sh
git commit -m "feat: add arrival_from_trace.sh helper script

Extracts arrival events from trace.jsonl using jq.
Handles edge cases: empty stop_states, multiple arrivals.
"
```

---

## Task 2: Create announce_from_trace.sh Helper Script

**Files:**
- Create: `tools/announce_from_trace.sh`

- [ ] **Step 1: Create the script with shebang and error handling**

```bash
#!/bin/bash
# Extract announce events from trace.jsonl
# Usage: ./tools/announce_from_trace.sh <trace.jsonl>

set -euo pipefail

if [ $# -ne 1 ]; then
    echo "Usage: $0 <trace.jsonl>" >&2
    exit 1
fi

TRACE_FILE="$1"

if [ ! -f "$TRACE_FILE" ]; then
    echo "Error: File not found: $TRACE_FILE" >&2
    exit 1
fi
```

- [ ] **Step 2: Add jq query to extract announce events**

```bash
jq 'select(.active_stops and (.active_stops | length > 0)) |
    {time, stop_idx: .active_stops[0], s_cm, v_cms}' \
  "$TRACE_FILE"
```

- [ ] **Step 3: Make script executable**

```bash
chmod +x tools/announce_from_trace.sh
```

- [ ] **Step 4: Test script with existing trace file**

```bash
./tools/announce_from_trace.sh /tmp/test_trace.jsonl
```

Expected: JSON output with announce events (corridor entries)

- [ ] **Step 5: Commit**

```bash
git add tools/announce_from_trace.sh
git commit -m "feat: add announce_from_trace.sh helper script

Extracts announce events (corridor entries) from trace.jsonl using jq.
Handles edge cases: missing active_stops, empty array.
"
```

---

## Task 3: Remove PipelineConfig and Update PipelineResult

**Files:**
- Modify: `crates/pipeline/src/lib.rs`

- [ ] **Step 1: Read current lib.rs to understand structure**

```bash
# View the current PipelineConfig and PipelineResult definitions
grep -A 10 "struct PipelineConfig" crates/pipeline/src/lib.rs
grep -A 15 "struct PipelineResult" crates/pipeline/src/lib.rs
```

- [ ] **Step 2: Remove PipelineConfig struct**

Find and remove this entire struct (around line 51-57):
```rust
#[derive(Debug, Clone, Default)]
pub struct PipelineConfig {
    /// Enable trace output (for debugging)
    pub enable_trace: bool,
    /// Enable announce output
    pub enable_announce: bool,
}
```

- [ ] **Step 3: Change trace_records from Option<Vec> to Vec**

Find `PipelineResult` struct and change:
```rust
// Before:
#[cfg(feature = "std")]
pub trace_records: Option<Vec<TraceRecord>>,

// After:
#[cfg(feature = "std")]
pub trace_records: Vec<TraceRecord>,
```

Also remove `announce_events` field (around line 69-71):
```rust
// Remove this:
/// Announce events (if enabled)
#[cfg(feature = "std")]
pub announce_events: Option<Vec<AnnounceEvent>>,
```

- [ ] **Step 4: Remove AnnounceEvent struct (no longer needed in public API)**

Find and remove (around line 141-149):
```rust
/// Announce event
#[cfg(feature = "std")]
#[derive(Debug, Clone, ::serde::Serialize)]
pub struct AnnounceEvent {
    pub time: u64,
    pub stop_idx: u8,
    pub s_cm: i32,
    pub v_cms: i32,
}
```

Note: This struct may still be used internally for trace output. Check if it's used elsewhere.

- [ ] **Step 5: Update PipelineResult::new() method**

Find `impl PipelineResult` and update the `new()` method (around line 630-647):

```rust
// Before:
#[cfg(feature = "std")]
fn new(config: &PipelineConfig) -> Self {
    Self {
        arrivals: Vec::new(),
        departures: Vec::new(),
        trace_records: if config.enable_trace { Some(Vec::new()) } else { None },
        announce_events: if config.enable_announce { Some(Vec::new()) } else { None },
    }
}

// After:
#[cfg(feature = "std")]
fn new(_config: &PipelineConfig) -> Self {
    Self {
        arrivals: Vec::new(),
        departures: Vec::new(),
        trace_records: Vec::new(),
    }
}
```

Wait - we're removing PipelineConfig, so the signature should change. Let's fix this in the next step.

- [ ] **Step 6: Run cargo check to see compilation errors**

```bash
cargo check -p pipeline
```

Expected: Errors about PipelineConfig being used in process_nmea_file and other places. We'll fix these in the next tasks.

- [ ] **Step 7: Commit**

```bash
git add crates/pipeline/src/lib.rs
git commit -m "refactor(pipeline): remove PipelineConfig, change trace_records to Vec

- Remove PipelineConfig struct (no longer needed)
- Change trace_records from Option<Vec> to Vec (trace always enabled)
- Remove announce_events from PipelineResult
- Temporary: compilation errors will be fixed in next commits
"
```

---

## Task 4: Update process_nmea_file() Signature

**Files:**
- Modify: `crates/pipeline/src/lib.rs`

- [ ] **Step 1: Find process_nmea_file() signature**

```bash
grep -n "pub fn process_nmea_file" crates/pipeline/src/lib.rs
```

- [ ] **Step 2: Remove config parameter from process_nmea_file()**

Change (around line 524):
```rust
// Before:
pub fn process_nmea_file(
    nmea_path: impl AsRef<Path>,
    route_data_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
    config: &PipelineConfig,
) -> Result<PipelineResult, PipelineError>

// After:
pub fn process_nmea_file(
    nmea_path: impl AsRef<Path>,
    route_data_path: impl AsRef<Path>,
) -> Result<PipelineResult, PipelineError>
```

- [ ] **Step 3: Update process_nmea_file() implementation**

Remove the output_path parameter and write_output() call (around line 547-550):
```rust
// Before:
let result = Self::process_nmea_reader(
    reader,
    &route_data,
    config,
)?;

// Write output
Self::write_output(&result, output_path)?;

Ok(result)

// After:
let result = Self::process_nmea_reader(
    reader,
    &route_data,
)?;

Ok(result)
```

- [ ] **Step 4: Update process_nmea_reader() signature**

Change (around line 565):
```rust
// Before:
pub fn process_nmea_reader<R: BufRead>(
    reader: R,
    route_data: &RouteData,
    config: &PipelineConfig,
) -> Result<PipelineResult, PipelineError>

// After:
pub fn process_nmea_reader<R: BufRead>(
    reader: R,
    route_data: &RouteData,
) -> Result<PipelineResult, PipelineError>
```

- [ ] **Step 5: Update process_nmea_reader() implementation**

Change the result initialization (around line 570):
```rust
// Before:
let mut result = PipelineResult::new(config);

// After:
let mut result = PipelineResult::new();
```

- [ ] **Step 6: Update trace collection to always add records**

Remove the config.enable_trace check (around line 590):
```rust
// Before:
if config.enable_trace {
    result.add_trace_record(&gps_record, &det_state, route_data);
}

// After:
result.add_trace_record(&gps_record, &det_state, route_data);
```

- [ ] **Step 7: Remove write_output() function**

Find and remove the entire `write_output()` function (around line 600-625).

- [ ] **Step 8: Fix PipelineResult::new() signature**

```rust
// Before:
#[cfg(feature = "std")]
fn new(config: &PipelineConfig) -> Self {

// After:
#[cfg(feature = "std")]
fn new() -> Self {
```

And update the body:
```rust
// Before:
trace_records: if config.enable_trace { Some(Vec::new()) } else { None },

// After:
trace_records: Vec::new(),
```

- [ ] **Step 9: Update add_trace_record() to work with Vec not Option**

Change (around line 651):
```rust
// Before:
fn add_trace_record(&mut self, record: &gps::GpsRecord, det_state: &DetectionState, route_data: &RouteData) {
    if let Some(ref mut trace) = self.trace_records {
        // ... rest of code
    }
}

// After:
fn add_trace_record(&mut self, record: &gps::GpsRecord, det_state: &DetectionState, route_data: &RouteData) {
    let (active_stops, stop_states) = det_state.get_trace_info(record, route_data);
    // ... rest of code, indented one level less
    self.trace_records.push(TraceRecord {
        // ... fields
    });
}
```

The full function body needs to be un-nested. Let me provide the complete updated function:

```rust
fn add_trace_record(&mut self, record: &gps::GpsRecord, det_state: &DetectionState, route_data: &RouteData) {
    let (active_stops, stop_states) = det_state.get_trace_info(record, route_data);

    // Compute corridor info from first active stop
    let (corridor_start_cm, corridor_end_cm) = if let Some(&first_idx) = det_state.active_indices.first() {
        let stop = &route_data.stops()[first_idx];
        (Some(stop.corridor_start_cm), Some(stop.corridor_end_cm))
    } else {
        (None, None)
    };

    // Find next stop outside corridor
    let next_stop = if let Some(end) = corridor_end_cm {
        let mut result = None;
        for (idx, stop) in route_data.stops().iter().enumerate() {
            if stop.progress_cm > end {
                let prob = stop_states.iter()
                    .find(|s| s.stop_idx == idx as u8)
                    .map(|s| s.probability)
                    .unwrap_or(0);
                if idx < route_data.stops().len() - 1 {
                    result = Some((idx as u8, prob));
                    break;
                }
            }
        }
        result
    } else {
        None
    };

    self.trace_records.push(TraceRecord {
        time: record.time,
        lat: record.lat,
        lon: record.lon,
        s_cm: record.s_cm,
        v_cms: record.v_cms,
        heading_cdeg: record.heading_cdeg,
        active_stops,
        stop_states,
        gps_jump: false,
        recovery_idx: None,
        status: record.status.to_string(),
        off_route: det_state.off_route,
        segment_idx: record.segment_idx,
        heading_constraint_met: record.heading_constraint_met,
        divergence_cm: record.divergence_cm,
        hdop: record.hdop,
        num_sats: record.num_sats,
        fix_type: record.fix_type.clone(),
        variance_cm2: record.variance_cm2,
        corridor_start_cm,
        corridor_end_cm,
        next_stop,
    });
}
```

- [ ] **Step 10: Remove DetectionState::process_gps_record() announce event collection**

Find and remove the announce event collection code (around line 443-457):
```rust
// Remove this entire block:
#[cfg(feature = "std")]
if let Some(ref mut announce_events) = result.announce_events {
    if !self.off_route {
        for (idx, stop_state) in self.stop_states.iter_mut().enumerate() {
            if stop_state.should_announce(s_cm, stops[idx].corridor_start_cm) {
                announce_events.push(AnnounceEvent {
                    time: record.time,
                    stop_idx: idx as u8,
                    s_cm: record.s_cm,
                    v_cms: record.v_cms,
                });
            }
        }
    }
}
```

- [ ] **Step 11: Run cargo check**

```bash
cargo check -p pipeline
```

Expected: Should pass (lib.rs changes complete)

- [ ] **Step 12: Run tests**

```bash
cargo test -p pipeline
```

Expected: Tests should pass (regression_tests uses result.arrivals directly)

- [ ] **Step 13: Commit**

```bash
git add crates/pipeline/src/lib.rs
git commit -m "refactor(pipeline): remove config params, always collect trace

- Remove config parameter from process_nmea_file() and process_nmea_reader()
- Remove write_output() function (no longer writes merged arrivals/departures)
- Update add_trace_record() to work with Vec instead of Option
- Remove announce event collection from DetectionState
- Trace is now always enabled and written directly from main.rs
"
```

---

## Task 5: Simplify CLI in main.rs

**Files:**
- Modify: `crates/pipeline/src/main.rs`

- [ ] **Step 1: Read current main.rs structure**

```bash
head -80 crates/pipeline/src/main.rs
```

- [ ] **Step 2: Remove output positional argument from Args struct**

Change (around line 67-73):
```rust
// Before:
struct Args {
    nmea: PathBuf,
    route_data: PathBuf,
    output: PathBuf,
    trace: Option<PathBuf>,
    announce: Option<PathBuf>,
}

// After:
struct Args {
    nmea: PathBuf,
    route_data: PathBuf,
    output: Option<PathBuf>,  // Now optional --output flag
}
```

- [ ] **Step 3: Update parse_args() to remove --trace and --announce flags**

Change the function (around line 76-134):
```rust
fn parse_args() -> Result<Args, Box<dyn std::error::Error>> {
    let mut nmea = None;
    let mut route_data = None;
    let mut output = None;

    let mut args_iter = std::env::args().skip(1);

    while let Some(arg) = args_iter.next() {
        match arg.as_str() {
            "--output" => {
                if let Some(output_path) = args_iter.next() {
                    output = Some(PathBuf::from(output_path));
                } else {
                    return Err("--output requires an argument".into());
                }
            }
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            arg if arg.starts_with('-') => {
                return Err(format!("Unknown option: {}", arg).into());
            }
            _ => {
                // Positional arguments: nmea route_data
                if nmea.is_none() {
                    nmea = Some(PathBuf::from(arg));
                } else if route_data.is_none() {
                    route_data = Some(PathBuf::from(arg));
                } else {
                    return Err("Too many arguments. Usage: pipeline <nmea> <route_data> [--output <trace.jsonl>]".into());
                }
            }
        }
    }

    let nmea = nmea.ok_or("Missing NMEA input file")?;
    let route_data = route_data.ok_or("Missing route_data.bin file")?;

    Ok(Args {
        nmea,
        route_data,
        output,
    })
}
```

- [ ] **Step 4: Add helper function to generate trace output path**

Add this function after print_help() (around line 155):
```rust
/// Generate trace output path from NMEA input path
/// Example: test_data/ty225_normal_nmea.txt -> test_data/ty225_normal_trace.jsonl
fn generate_trace_path(nmea_path: &PathBuf) -> PathBuf {
    let mut trace_path = nmea_path.clone();

    // Replace extension with _trace.jsonl
    let file_stem = trace_path.file_stem().unwrap_or_default();
    let parent = trace_path.parent();

    let new_name = format!("{}_trace.jsonl", file_stem.to_string_lossy());

    if let Some(p) = parent {
        trace_path = p.join(new_name);
    } else {
        trace_path = PathBuf::from(new_name);
    }

    trace_path
}
```

- [ ] **Step 5: Update main() function to use new CLI**

Change (around line 10-64):
```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use pipeline::Pipeline;
    use std::io::Write;

    let args = parse_args()?;

    // Determine trace output path
    let trace_path = args.output.unwrap_or_else(|| {
        let auto_path = generate_trace_path(&args.nmea);
        eprintln!("Auto-generating trace output: {}", auto_path.display());
        auto_path
    });

    // Run pipeline
    let result = Pipeline::process_nmea_file(
        &args.nmea,
        &args.route_data,
    )?;

    // Write trace file
    use std::io::BufWriter;
    let file = std::fs::File::create(&trace_path)?;
    let mut writer = BufWriter::new(file);
    for trace_record in &result.trace_records {
        writeln!(writer, "{}", serde_json::to_string(trace_record)?)?;
    }
    writer.flush()?;
    eprintln!("Trace written to: {}", trace_path.display());

    // Print summary
    eprintln!("=== Pipeline Complete ===");
    eprintln!("Processed {} GPS updates", result.trace_records.len());
    eprintln!("Detected {} arrivals", result.arrivals.len());
    eprintln!("Detected {} departures", result.departures.len());

    Ok(())
}
```

- [ ] **Step 6: Update print_help() function**

Change (around line 137-154):
```rust
fn print_help() {
    println!("Bus Arrival Detection Pipeline");
    println!();
    println!("Usage: pipeline [OPTIONS] <nmea> <route_data>");
    println!();
    println!("Arguments:");
    println!("  <nmea>       NMEA log file (GPS data)");
    println!("  <route_data> Route data binary file");
    println!();
    println!("Options:");
    println!("  --output <file>    Trace output file (default: auto-generated from input name)");
    println!("  -h, --help         Show this help message");
    println!();
    println!("Examples:");
    println!("  pipeline gps.nmea route_data.bin");
    println!("  pipeline gps.nmea route_data.bin --output custom_trace.jsonl");
    println!();
    println!("Helper scripts:");
    println!("  ./tools/arrival_from_trace.sh trace.jsonl > arrivals.jsonl");
    println!("  ./tools/announce_from_trace.sh trace.jsonl > announce.jsonl");
}
```

- [ ] **Step 7: Run cargo check**

```bash
cargo check -p pipeline --bin pipeline
```

Expected: Should pass

- [ ] **Step 8: Build and test with new CLI**

```bash
cargo build --release --bin pipeline

# Test with auto-generated output path
./target/release/pipeline test_data/ty225_normal_nmea.txt test_data/ty225_normal.bin

# Verify trace file was created
ls -la test_data/ty225_normal_trace.jsonl

# Test with explicit output path
./target/release/pipeline test_data/ty225_normal_nmea.txt test_data/ty225_normal.bin --output /tmp/test_trace.jsonl

# Test helper scripts
./tools/arrival_from_trace.sh test_data/ty225_normal_trace.jsonl | head -5
./tools/announce_from_trace.sh test_data/ty225_normal_trace.jsonl | head -5
```

Expected: Trace file created, helper scripts extract events correctly

- [ ] **Step 9: Run tests**

```bash
cargo test -p pipeline
```

Expected: All tests pass

- [ ] **Step 10: Commit**

```bash
git add crates/pipeline/src/main.rs
git commit -m "refactor(pipeline): simplify CLI to 2 args

- Remove output positional argument
- Remove --trace and --announce flags
- Add --output flag (optional, auto-generates if not specified)
- Add generate_trace_path() helper function
- Update help text with new usage and helper script examples
- Write trace file directly from main()
"
```

---

## Task 6: Update Makefile

**Files:**
- Modify: `Makefile`

- [ ] **Step 1: Read current pipeline targets in Makefile**

```bash
grep -A 10 "^pipeline:" Makefile
grep -A 10 "^pipeline-no-gen:" Makefile
```

- [ ] **Step 2: Remove ANNOUNCE_OUT variable**

Find and remove (around line 56):
```make
# Remove this line:
ANNOUNCE_OUT := $(DATA_DIR)/$(ROUTE_NAME)_$(SCENARIO)_announce.jsonl
```

- [ ] **Step 3: Update pipeline target**

Change (around line 177-184):
```make
# Before:
pipeline: gen_nmea preprocess
	@echo "=== Running unified pipeline ==="
	@echo "Binary: $(PIPELINE)"
	@echo "Source: pipeline/"
	$(PIPELINE) $(NMEA_OUT) $(ROUTE_DATA_BIN) $(DETECTOR_OUT) --trace $(TRACE_OUT) --announce $(ANNOUNCE_OUT)
	@echo "Generated: $(DETECTOR_OUT)"
	@echo "Generated: $(TRACE_OUT)"
	@echo "Generated: $(ANNOUNCE_OUT)"

# After:
pipeline: gen_nmea preprocess
	@echo "=== Running unified pipeline ==="
	@echo "Binary: $(PIPELINE)"
	@echo "Source: pipeline/"
	$(PIPELINE) $(NMEA_OUT) $(ROUTE_DATA_BIN)
	@echo "Generated: $(TRACE_OUT)"
```

- [ ] **Step 4: Update pipeline-no-gen target**

Change (around line 187-200):
```make
# Before:
pipeline-no-gen: preprocess
	@echo "=== Running unified pipeline (using existing NMEA) ==="
	@echo "Binary: $(PIPELINE)"
	@echo "Source: pipeline/"
	@echo "Using existing NMEA: $(NMEA_OUT)"
	@if [ ! -f "$(NMEA_OUT)" ]; then \
		echo "Error: NMEA file not found: $(NMEA_OUT)"; \
		echo "Run 'make gen_nmea' first to generate it."; \
		exit 1; \
	fi
	$(PIPELINE) $(NMEA_OUT) $(ROUTE_DATA_BIN) $(DETECTOR_OUT) --trace $(TRACE_OUT) --announce $(ANNOUNCE_OUT)
	@echo "Generated: $(DETECTOR_OUT)"
	@echo "Generated: $(TRACE_OUT)"
	@echo "Generated: $(ANNOUNCE_OUT)"

# After:
pipeline-no-gen: preprocess
	@echo "=== Running unified pipeline (using existing NMEA) ==="
	@echo "Binary: $(PIPELINE)"
	@echo "Source: pipeline/"
	@echo "Using existing NMEA: $(NMEA_OUT)"
	@if [ ! -f "$(NMEA_OUT)" ]; then \
		echo "Error: NMEA file not found: $(NMEA_OUT)"; \
		echo "Run 'make gen_nmea' first to generate it."; \
		exit 1; \
	fi
	$(PIPELINE) $(NMEA_OUT) $(ROUTE_DATA_BIN)
	@echo "Generated: $(TRACE_OUT)"
```

- [ ] **Step 5: Update run target summary**

Change (around line 67-76):
```make
# Before:
run: build gen_nmea preprocess pipeline
	@echo ""
	@echo "=== Pipeline Complete ==="
	@echo "Route: $(ROUTE_NAME)"
	@echo "Scenario: $(SCENARIO)"
	@echo "NMEA output: $(NMEA_OUT)"
	@echo "Route data: $(ROUTE_DATA_BIN)"
	@echo "Output: $(DETECTOR_OUT)"
	@echo "Trace output: $(TRACE_OUT)"
	@echo "Announce output: $(ANNOUNCE_OUT)"

# After:
run: build gen_nmea preprocess pipeline
	@echo ""
	@echo "=== Pipeline Complete ==="
	@echo "Route: $(ROUTE_NAME)"
	@echo "Scenario: $(SCENARIO)"
	@echo "NMEA output: $(NMEA_OUT)"
	@echo "Route data: $(ROUTE_DATA_BIN)"
	@echo "Trace output: $(TRACE_OUT)"
	@echo ""
	@echo "Extract arrivals: ./tools/arrival_from_trace.sh $(TRACE_OUT) > arrivals.jsonl"
	@echo "Extract announce: ./tools/announce_from_trace.sh $(TRACE_OUT) > announce.jsonl"
```

- [ ] **Step 6: Update run-legacy target (remove ANNOUNCE_OUT reference)**

Change (around line 92-102):
```make
# Before:
run-legacy: build gen_nmea preprocess simulate detect
	@echo ""
	@echo "=== Legacy Pipeline Complete ==="
	@echo "Route: $(ROUTE_NAME)"
	@echo "Scenario: $(SCENARIO)"
	@echo "NMEA output: $(NMEA_OUT)"
	@echo "Route data: $(ROUTE_DATA_BIN)"
	@echo "Simulator output: $(SIMULATOR_OUT)"
	@echo "Arrival detector output: $(DETECTOR_OUT)"
	@echo "Trace output: $(TRACE_OUT)"
	@echo "Announce output: $(ANNOUNCE_OUT)"

# After:
run-legacy: build gen_nmea preprocess simulate detect
	@echo ""
	@echo "=== Legacy Pipeline Complete ==="
	@echo "Route: $(ROUTE_NAME)"
	@echo "Scenario: $(SCENARIO)"
	@echo "NMEA output: $(NMEA_OUT)"
	@echo "Route data: $(ROUTE_DATA_BIN)"
	@echo "Simulator output: $(SIMULATOR_OUT)"
	@echo "Arrival detector output: $(DETECTOR_OUT)"
	@echo "Trace output: $(TRACE_OUT)"
```

- [ ] **Step 7: Update clean target**

Change (around line 203-210):
```make
# Before:
clean:
	@echo "=== Cleaning generated files ==="
	rm -f $(DATA_DIR)/nmea_*.txt
	rm -f $(DATA_DIR)/sim_*.jsonl
	rm -f $(DATA_DIR)/arrivals_*.jsonl
	rm -f $(DATA_DIR)/trace_*.jsonl
	rm -f $(ROUTE_DATA_BIN)
	@echo "Clean complete"

# After:
clean:
	@echo "=== Cleaning generated files ==="
	rm -f $(DATA_DIR)/*_nmea.txt
	rm -f $(DATA_DIR)/*_trace.jsonl
	rm -f $(DATA_DIR)/*.bin
	@echo "Clean complete"
```

- [ ] **Step 8: Test Makefile targets**

```bash
# Test pipeline target
make pipeline ROUTE_NAME=ty225 SCENARIO=normal

# Verify trace file exists
ls -la test_data/ty225_normal_trace.jsonl

# Test helper scripts work
./tools/arrival_from_trace.sh test_data/ty225_normal_trace.jsonl | wc -l
./tools/announce_from_trace.sh test_data/ty225_normal_trace.jsonl | wc -l

# Test full run
make run ROUTE_NAME=ty225 SCENARIO=normal
```

Expected: All targets work correctly

- [ ] **Step 9: Commit**

```bash
git add Makefile
git commit -m "build: update Makefile for single trace output

- Remove ANNOUNCE_OUT variable
- Update pipeline and pipeline-no-gen targets to use 2-arg CLI
- Update run target summary with helper script examples
- Simplify clean target
"
```

---

## Task 7: Update Documentation

**Files:**
- Modify: `docs/CLAUDE.md`

- [ ] **Step 1: Read current CLAUDE.md build commands section**

```bash
grep -A 20 "## Build Commands" docs/CLAUDE.md
```

- [ ] **Step 2: Update build commands section**

Find the "## Build Commands" section and update the pipeline usage examples:

```markdown
## Build Commands

```bash
# Build all binaries (host + firmware)
cargo build --release
make build

# Run full pipeline with test data (generates trace.jsonl)
make run ROUTE_NAME=ty225 SCENARIO=normal

# Generate route data from GeoJSON
cargo run -p preprocessor -- test_data/ty225_route.json test_data/ty225_stops.json test_data/ty225.bin

# Run pipeline (NMEA + route_data → trace.jsonl)
cargo run -p pipeline -- test_data/ty225_normal_nmea.txt test_data/ty225.bin

# Extract arrivals from trace
./tools/arrival_from_trace.sh test_data/ty225_normal_trace.jsonl > arrivals.jsonl

# Extract announce events from trace
./tools/announce_from_trace.sh test_data/ty225_normal_trace.jsonl > announce.jsonl

# Build Pico 2 W firmware (no_std, RP2350)
cargo build --release --target thumbv8m.main-none-eabi -p pico2-firmware
make build-firmware
```
```

- [ ] **Step 3: Add jq filtering examples section**

Add after "## Build Commands" section:

```markdown
## Filtering trace.jsonl

The pipeline now outputs a single `trace.jsonl` file containing all state machine information. Use jq or helper scripts to extract what you need.

### Using helper scripts

```bash
# Extract arrivals
./tools/arrival_from_trace.sh trace.jsonl > arrivals.jsonl

# Extract announce events (corridor entries)
./tools/announce_from_trace.sh trace.jsonl > announce.jsonl
```

### Using jq directly

```bash
# Extract arrivals
jq 'select(.stop_states[].just_arrived == true) |
    {time, stop_idx: .stop_states[0].stop_idx, s_cm, v_cms, probability}' \
  trace.jsonl > arrivals.jsonl

# Extract announce events (corridor entry)
jq 'select(.active_stops | length > 0) |
    {time, stop_idx: .active_stops[0], s_cm, v_cms}' \
  trace.jsonl > announce.jsonl

# Find all Approaching/Arriving states
jq '.stop_states[]? | select(.fsm_state == "Approaching" or .fsm_state == "Arriving")' \
  trace.jsonl

# Filter by time range
jq 'select(.time >= 1234567890 and .time <= 1234567900)' trace.jsonl

# Show off-route episodes
jq 'select(.off_route == true)' trace.jsonl
```
```

- [ ] **Step 4: Update Output section (if it exists)**

Find and update any section describing the output files. Remove references to arrivals.jsonl and announce.jsonl as separate outputs.

- [ ] **Step 5: Commit**

```bash
git add docs/CLAUDE.md
git commit -m "docs: update CLAUDE.md for single trace output

- Update build commands to use new 2-arg CLI
- Add jq filtering examples section
- Remove references to separate arrivals/announce output files
- Document helper script usage
"
```

---

## Task 8: Full Integration Test

**Files:**
- Test: All changes together

- [ ] **Step 1: Build all binaries**

```bash
cargo build --release
```

Expected: No errors

- [ ] **Step 2: Run full pipeline with normal scenario**

```bash
make run ROUTE_NAME=ty225 SCENARIO=normal
```

Expected: Trace file generated, summary shows correct counts

- [ ] **Step 3: Run full pipeline with detour scenario**

```bash
make run-detour
```

Expected: Trace file generated, detour handled correctly

- [ ] **Step 4: Test helper scripts**

```bash
./tools/arrival_from_trace.sh test_data/ty225_normal_trace.jsonl | head -5
./tools/announce_from_trace.sh test_data/ty225_normal_trace.jsonl | head -5
```

Expected: Valid JSON output

- [ ] **Step 5: Run all tests**

```bash
cargo test
```

Expected: All tests pass (especially regression_tests)

- [ ] **Step 6: Verify trace file format**

```bash
# Check it's valid JSONL
while read -r line; do
    echo "$line" | jq . > /dev/null || exit 1
done < test_data/ty225_normal_trace.jsonl
echo "All lines valid JSON"
```

Expected: All lines are valid JSON

- [ ] **Step 7: Compare with old output (if available)**

If you have a backup of old trace.jsonl, compare that the format is compatible:

```bash
# Just verify we have the same fields
head -1 test_data/ty225_normal_trace.jsonl | jq 'keys'
```

Expected: All expected fields present

- [ ] **Step 8: Final commit if any fixes needed**

If any issues found and fixed during testing, commit the fixes.

---

## Task 9: Clean Up Old Output Files (Optional)

**Files:**
- Clean: Old test output files

- [ ] **Step 1: Remove old arrivals/announce files from test_data**

```bash
# These are no longer generated by the pipeline
rm -f test_data/*_arrivals.json
rm -f test_data/*_announce.jsonl
```

- [ ] **Step 2: Update any other scripts or tools that reference old outputs**

Check for any references in the codebase:
```bash
grep -r "arrivals.jsonl" tools/ --exclude-dir=".git"
grep -r "announce.jsonl" tools/ --exclude-dir=".git"
```

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "chore: remove obsolete output files and references

Clean up old arrivals/announce files that are no longer generated.
"
```

---

## Self-Review Checklist

After completing all tasks, verify:

- [ ] All spec requirements are implemented
- [ ] No placeholder code remains
- [ ] All tests pass
- [ ] Helper scripts work correctly
- [ ] Documentation is updated
- [ ] Makefile targets work
- [ ] No compilation warnings

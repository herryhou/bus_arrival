# JSONL GPS Input Feature Design

**Date:** 2026-05-19
**Status:** Design
**Author:** Claude + User

## Overview

Add support for reading GPS data from JSONL files (Android FusedLocationProvider format) as an alternative to NMEA input. The pipeline will auto-detect input format based on file extension.

## Motivation

The Android app captures GPS logs in JSONL format (from FusedLocationProvider). Currently, the pipeline only accepts NMEA input. Adding JSONL support enables:
- Direct processing of Android-captured GPS logs
- Validation against real-world GPS data
- Easier testing with pre-recorded mobile GPS

## JSONL Format

```json
{"t":1779172271904,"lat":24.156562,"lon":120.649046,"a":15.952,"s":0.481,"b":90.0,"p":"fused"}
```

| Field | Type | Description |
|-------|------|-------------|
| `t` | int | Timestamp in milliseconds |
| `lat` | float | Latitude in degrees |
| `lon` | float | Longitude in degrees |
| `a` | float | Accuracy in meters (68% confidence) |
| `s` | float | Speed in m/s |
| `b` | float | Bearing/heading in degrees |
| `p` | string | Provider ("fused", "gps") - ignored |

## Architecture

### New Module: `jsonl_reader.rs`

```rust
/// JSONL → GpsPoint converter
pub struct JsonReader {
    skipped_lines: usize,
}

impl JsonReader {
    pub fn new() -> Self;
    pub fn parse_line(&mut self, line: &str) -> Option<GpsPoint>;
    pub fn skipped_count(&self) -> usize;
}
```

### Format Detection

```rust
enum InputFormat { Nmea, Jsonl }

impl InputFormat {
    fn from_path(path: &Path) -> Self {
        match path.extension().and_then(|e| e.to_str()) {
            Some("jsonl") => InputFormat::Jsonl,
            _ => InputFormat::Nmea,
        }
    }
}
```

### Unified Entry Point

```rust
impl Pipeline {
    /// Process GPS input file (auto-detects NMEA vs JSONL)
    pub fn process_file(
        input_path: impl AsRef<Path>,
        route_data_path: impl AsRef<Path>,
    ) -> Result<PipelineResult, PipelineError>;
}
```

## Data Conversion

| JSONL field | GpsPoint field | Conversion |
|-------------|----------------|------------|
| `t` (ms) | `timestamp` (sec) | `t / 1000` |
| `lat` | `lat` | direct (f64) |
| `lon` | `lon` | direct (f64) |
| `s` (m/s) | `speed_cms` | `s * 100` |
| `b` (deg) | `heading_cdeg` | `b * 100` |
| `a` (m) | `hdop_x10` | `a * 2` |
| - | `has_fix` | `s != None && b != None && a != None` |

### Accuracy to HDOP Conversion

Formula: `hdop_x10 = accuracy_m * 2`

Rationale: Android's `accuracy` is 68% confidence radius. Conservative mapping assumes ~4m base GPS error, so HDOP ≈ accuracy / 4m, scaled to hdop_x10 format.

### Fix Quality Logic

`has_fix = (s != None && b != None && a != None)`

Partial GPS data (missing speed, bearing, or accuracy) indicates incomplete fix, similar to NMEA `FixQuality::PositionOnly` or `MotionOnly`.

## Error Handling

- **Invalid JSON**: Skip line, increment counter
- **Missing required fields** (`t`, `lat`, `lon`): Skip line, increment counter
- **Invalid numeric values**: Skip line, increment counter
- **Missing optional fields** (`s`, `b`, `a`): Use `None` for that field

**Summary output:**
```
Processed 1000 GPS updates, skipped 3 malformed lines
```

## Testing

### Unit Tests (`jsonl_reader.rs`)

- `test_parse_complete()` - Valid complete record
- `test_parse_minimal()` - Missing optional fields
- `test_has_fix_with_all_fields()` - has_fix = true
- `test_has_fix_missing_speed()` - has_fix = false
- `test_has_fix_missing_accuracy()` - has_fix = false
- `test_invalid_json_skipped()` - Error handling
- `test_missing_lat_skipped()` - Error handling
- `test_skipped_count()` - Counter increment
- `test_timestamp_ms_to_sec()` - Conversion
- `test_speed_m_to_cm()` - Conversion
- `test_bearing_deg_to_cdeg()` - Conversion
- `test_accuracy_to_hdop()` - Conversion

### Integration Test

Process `test_data/tz_23-gps-log-20260519-063111.jsonl` and verify trace output.

## Makefile Integration

No new targets needed. Existing `validate-trace` auto-detects format from file extension.

```makefile
validate-trace:
	@if [ -n "$(INPUT_FILE)" && -n "$(ROUTE_DATA)" ]; then \
		cargo run --release --bin pipeline -- \
			"$(INPUT_FILE)" "$(ROUTE_DATA)" -o "$(OUTPUT)"; \
	fi
```

**Usage:**
```bash
# NMEA (existing)
make validate-trace INPUT_FILE=test_data/ty225_normal_nmea.txt ...

# JSONL (new)
make validate-trace INPUT_FILE=test_data/tz_23-gps-log-20260519-063111.jsonl ...
```

## Output Naming

Auto-generated trace output:
- `foo.jsonl` → `foo_trace.jsonl`
- `bar_nmea.txt` → `bar_trace.jsonl`

## Implementation Steps

1. Add `jsonl_reader.rs` module with `JsonReader` and unit tests
2. Add `InputFormat` enum to `lib.rs`
3. Add `Pipeline::process_file()` unified entry point
4. Update `main.rs` to use `process_file()`
5. Integration test with real GPS log
6. Update Makefile `validate-trace` to use `INPUT_FILE` variable

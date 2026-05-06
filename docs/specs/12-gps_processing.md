# GPS Processing (Timestamp-Driven)

## Overview

The system uses a **timestamp-driven architecture** for GPS processing. NMEA sentences (RMC, GGA, GSA) are accumulated across UART reads, and exactly one GPS fix is emitted per timestamp change.

**Key Principle:** GPS timestamp is the only authority. No artificial timers.

## Architecture

### FixAccumulator

The `FixAccumulator` (source: `crates/pipeline/gps_processor/src/accumulator.rs`) implements timestamp-driven GPS processing:

```rust
pub struct FixAccumulator {
    timestamp: Option<u64>,      // GPS timestamp (seconds since epoch)
    lat: Option<f64>,            // Latitude (degrees)
    lon: Option<f64>,            // Longitude (degrees)
    speed: Option<SpeedCms>,     // Speed (cm/s)
    heading: Option<HeadCdeg>,   // Heading (0.01°)
    hdop: Option<u16>,           // HDOP * 10
    has_fix: bool,               // Valid fix flag
    last_emitted_timestamp: Option<u64>, // Last emitted timestamp
}
```

### Emission Semantics

Emits the **previous** snapshot when a **new** timestamp arrives:

```text
RMC(t=01) + GGA(t=01) → accumulate
RMC(t=02) arrives → should_emit() true → emit (t=01) → reset
```

**Critical:** Does NOT emit on first timestamp. Only emits after seeing second timestamp (requires `last_emitted_timestamp.is_some()`).

## FixQuality Classification

Data quality is explicit via the `FixQuality` enum (source: `crates/shared/src/lib.rs`):

```rust
pub enum FixQuality {
    Full,          // RMC + GGA/GSA - has position, motion, and quality metrics
    PositionOnly,  // GGA only - has position and quality, no motion data
    MotionOnly,    // RMC only - has position and motion, no quality metrics
}
```

### Quality Determination

| Has Motion | Has Quality | FixQuality |
|------------|-------------|------------|
| Yes | Yes | Full |
| No | Yes | PositionOnly |
| Yes | No | MotionOnly |
| No | No | PositionOnly (default) |

## Key Properties

### 1. Timestamp is Authoritative

- No artificial timer (no `Timer::after(1s)`)
- GPS timestamp from NMEA drives the loop
- Exactly ONE `process_gps()` call per GPS second (Δt = 1s)

### 2. Handles Split Bursts

UART fragmentation is transparent:

```text
UART read 1: "RMC(t=01)"
[UART delay]
UART read 2: "GGA(t=01)"
UART read 3: "RMC(t=02)" → triggers emission of (t=01)
```

The accumulator preserves partial data across reads. Only timestamp change triggers emission.

### 3. Out-of-Order Handling

NMEA sentences can arrive in any order within the same timestamp:

```text
GGA(t=01) → accumulates position + quality
RMC(t=01) → accumulates motion data
Result: Full quality fix (both datasets merged)
```

### 4. Missing Data Handling

All fields except `lat`, `lon`, `timestamp` are `Option<T>`:

- Missing speed → `speed_cms: None`
- Missing heading → `heading_cdeg: None`
- Missing HDOP → `hdop_x10: None`

Downstream code must handle `None` values appropriately (see constraints spec).

## Event-Driven Main Loop

**Firmware:** `crates/pico2-firmware/src/main.rs`

```rust
loop {
    match uart::read_nmea_sentence_async(&mut uart, &mut line_buf).await {
        Ok(Some(sentence)) => {
            fix_accumulator.update(&sentence);

            // Timestamp change is the ONLY trigger
            if fix_accumulator.should_emit() {
                if let Some((gps, _quality)) = fix_accumulator.build() {
                    // Exactly ONE process_gps() call per GPS second
                    if let Some(arrival) = state.process_gps(&gps) {
                        // Emit arrival event
                    }
                }
                fix_accumulator.reset(); // Reset AFTER emission
            }
        }
        Ok(None) => continue, // UART idle
        Err(e) => { /* handle error */ }
    }

    // NO Timer::after(1s) - GPS timestamp drives the loop
}
```

## NMEA Sentence Support

### Supported Sentence Types

| Sentence | Data Contributed | Required |
|----------|------------------|----------|
| **RMC** (GPRMC, GNRMC) | lat, lon, speed, heading, timestamp | Yes (for motion) |
| **GGA** (GPGGA, GNGGA) | lat, lon, hdop, timestamp, fix quality | Yes (for position) |
| **GSA** (GPGSA, GNGSA) | hdop | Optional |

### Parse Order

Sentences are parsed in order of arrival. Later sentences overwrite earlier fields for the same timestamp:

```text
RMC(t=01) → sets speed=100, heading=90
GSA(t=01) → updates hdop=15 (overwrites any previous hdop)
```

## Critical Behaviors

### First Fix Edge Case

The accumulator does NOT emit the first GPS fix. Only emits after seeing the second timestamp:

```text
RMC(t=01) → accumulates, should_emit() = false
RMC(t=02) → should_emit() = true, emits (t=01)
```

**Rationale:** Ensures we have complete data for each timestamp before emitting.

### Timestamp Jump Handling

If GPS seconds are lost (e.g., t=01, t=03 - t=02 missing):

```text
RMC(t=01) + GGA(t=01) → accumulate
RMC(t=03) arrives → should_emit() true → emit (t=01) → reset
RMC(t=03) → now accumulating t=03
```

The accumulator handles jumps gracefully. Each timestamp is emitted exactly once.

### Invalid Checksum Rejection

Sentences with invalid NMEA checksums are silently rejected:

```rust
if !verify_checksum(sentence) {
    return false; // Sentence ignored
}
```

## Integration with Downstream

### Kalman Filter

`process_gps()` in `crates/pico2-firmware/src/state.rs` receives `GpsPoint` with `Option<T>` fields:

```rust
let speed_cms = gps.speed_cms.unwrap_or(0);           // Missing → assume stopped
let heading_cdeg = gps.heading_cdeg.unwrap_or(i16::MIN); // Missing → invalid
```

### Probability Model

Detection uses HDOP for adaptive Kalman gains (see `00-constraints.md`):

```rust
let hdop_x10 = gps.hdop_x10.unwrap_or(9990); // Missing → worst quality
```

## Testing

### Unit Tests

Source: `crates/pipeline/gps_processor/src/accumulator.rs`

- `test_new_accumulator_is_empty` - Initial state
- `test_rmc_only_creates_motion_only_fix` - RMC-only quality
- `test_gga_only_creates_position_only_fix` - GGA-only quality
- `test_rmc_then_gga_creates_full_fix` - Full quality merge
- `test_emit_on_second_timestamp` - Emission timing
- `test_out_of_order_sentences` - Order independence
- `test_reset_clears_state` - Reset behavior
- `test_invalid_checksum_rejected` - Checksum validation

### Integration Tests

Source: `crates/pipeline/tests/split_burst_test.rs`

- `test_split_burst_emits_once` - UART fragmentation handling
- `test_timestamp_jump` - Lost GPS second handling

Source: `crates/pipeline/tests/nmea_burst_regression.rs`

- `test_real_nmea_burst_one_tick_per_second` - Regression test with real data

## Constraints

### MUST

- Use timestamp-driven emission (no timers)
- Handle split bursts transparently
- Classify fix quality explicitly
- Reject invalid checksums
- Emit exactly once per timestamp

### MUST NOT

- Use artificial timers to drive GPS processing
- Assume sentence arrival order
- Emit on first timestamp
- Process incomplete fixes

## Related Specifications

- **`00-constraints.md`** - Semantic types, integer arithmetic
- **`02-kalman_filter.md`** - GPS update processing
- **`05-arrival_probability.md`** - HDOP-dependent gains
- **`bus_arrival_tech_report_v8.md`** - Algorithm background

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2026-05-06 | Initial timestamp-driven GPS processing spec |

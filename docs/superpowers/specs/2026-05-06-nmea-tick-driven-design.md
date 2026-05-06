# NMEA Tick-Driven Architecture Design

**Date:** 2026-05-06
**Author:** Claude Opus 4.6
**Status:** Design

## Problem Statement

The current GPS processing system implicitly assumes "each parsed NMEA sentence = one tick", but the specification defines Δt = 1 second. This mismatch causes:

1. **Double processing:** GPS modules send RMC → GSA → GGA bursts at 1Hz. Both GSA and GGA return complete `GpsPoint`, causing `process_gps()` to run twice per cycle.
2. **Incorrect dwell_time:** Increments twice per second instead of once, corrupting the F4 probability feature.
3. **Wasted compute:** Full pipeline (Kalman filter, map matching, FSM) runs unnecessarily.
4. **Duplicate events:** Can emit duplicate ARRIVAL/ANNOUNCE events for the same tick.

## Root Cause

The system lacks an explicit time model. NMEA sentences are treated as discrete events rather than parts of a single temporal snapshot.

## Solution: Tick-Driven Architecture

Split GPS processing into two stages:

1. **Accumulation Stage:** Parse all NMEA sentences in the burst, merge complementary data into a single fix
2. **Tick Stage:** Run `process_gps()` exactly once per second with the complete fix

```
RMC + GGA + GSA → FixAccumulator → One GpsPoint → process_gps()
```

## FixAccumulator Design

### Structure

```rust
struct FixAccumulator {
    timestamp: Option<u64>,
    lat: Option<f64>,
    lon: Option<f64>,
    speed: Option<SpeedCms>,
    heading: Option<HeadCdeg>,
    hdop: Option<u16>,
    has_fix: bool,
    last_emitted_timestamp: Option<u64>,
}
```

### Key Methods

- `new()` - Initialize empty accumulator
- `update(&mut self, sentence: &str) -> bool` - Accept any NMEA sentence in any order
- `is_complete(&self) -> bool` - Check if minimum required fields are present
- `build(&self) -> Option<GpsPoint>` - Produce complete GpsPoint
- `reset(&mut self)` - Clear state for new second
- `is_new_second(&self, timestamp: u64) -> bool` - Detect timestamp change

### Data Sources

| Sentence | Provides |
|----------|----------|
| RMC | timestamp, lat/lon, speed, heading |
| GGA | lat/lon, HDOP, fix quality |
| GSA | HDOP |

### Completeness Criteria

A fix is complete when it has:
- `has_fix == true`
- Valid latitude and longitude
- Valid timestamp

### Robustness Guarantees

- **Order-independent:** Accepts sentences in any sequence
- **Partial data OK:** RMC-only or GGA-only produces valid fix
- **Missing sentences tolerated:** Works even if GSA is skipped
- **HDOP optional:** Uses default (999) if not provided

## Main Loop Integration

```rust
let mut fix_accumulator = FixAccumulator::new();
let mut last_emitted_timestamp: Option<u64> = None;

loop {
    // Stage 1: Accumulate all sentences in burst
    loop {
        match read_nmea_sentence_async(&mut uart, &mut line_buf).await {
            Ok(Some(sentence)) => {
                fix_accumulator.update(sentence);
                line_buf.reset();
            }
            Ok(None) => break, // FIFO empty, burst complete
            Err(e) => { /* handle error */ break; }
        }
    }

    // Stage 2: Process once per second (tick-driven)
    if let Some(timestamp) = fix_accumulator.timestamp {
        if last_emitted_timestamp != Some(timestamp) {
            if let Some(gps) = fix_accumulator.build() {
                // ONE process_gps() call per second
                if let Some(arrival) = state.process_gps(&gps) {
                    // emit arrival
                }
            }
            last_emitted_timestamp = Some(timestamp);
        }
    }

    fix_accumulator.reset();
    Timer::after(Duration::from_secs(1)).await;
}
```

## Edge Case Handling

| Scenario | Behavior |
|----------|----------|
| GPS outage (no sentences) | timestamp is None, no processing, counter resets |
| Only GGA arrives | Builds valid fix (position + HDOP), heading/speed default |
| Only RMC arrives | Builds valid fix (position + speed + heading), HDOP defaults |
| Checksum failures | Sentence rejected, accumulator unchanged |
| Same timestamp (burst) | `is_new_second()` false, skips until next second |
| Timestamp rollover (midnight) | `Option<u64>` comparison handles None → Some transition |
| All sentences missing | `is_complete()` false, no processing this cycle |

### Default Values for Missing Fields

- `speed`: 0 cm/s
- `heading`: i16::MIN (sentinel, handled by downstream logic)
- `hdop`: 999 (worst quality)

## Testing Strategy

### Unit Tests
- `FixAccumulator` with various sentence combinations (RMC-only, GGA-only, RMC+GGA, RMC+GSA+GGA)
- Out-of-order sentence sequences
- Missing sentences
- Checksum failures
- Timestamp rollover

### Integration Tests
- Real NMEA bursts from test data (`tpF805_normal_nmea.txt`)
- Verify `process_gps()` called exactly once per second
- Verify `dwell_time_s` increments by 1 per second
- Verify no duplicate ARRIVAL/ANNOUNCE events

### Regression Test
- Compare trace output before/after fix
- Validate against ground truth

## Files to Modify

1. **`crates/pipeline/gps_processor/src/nmea.rs`**
   - Add `FixAccumulator` struct and implementation
   - Add accumulation mode methods

2. **`crates/pico2-firmware/src/main.rs`**
   - Replace direct `parse_sentence()` → `process_gps()` flow
   - Add accumulator and timestamp tracking
   - Implement two-stage (accumulate → tick) loop

3. **Tests**
   - Add accumulator unit tests in `nmea.rs`
   - Add integration test using existing NMEA test data

## Impact

- **Correct Δt = 1s:** Kalman + DR + FSM obey spec
- **Accurate dwell_time:** Increments once per second
- **No duplicate events:** FSM runs once per tick
- **Reduced compute:** Pipeline runs once per second instead of 2-3x
- **Better data quality:** Combines RMC (motion) + GGA (accuracy) for superior Kalman input

## Scope

- ~200 lines new code
- Minimal refactoring
- Isolated change, low risk

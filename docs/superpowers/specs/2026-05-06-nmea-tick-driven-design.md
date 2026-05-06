# NMEA Timestamp-Driven Architecture Design

**Date:** 2026-05-06
**Author:** Claude Opus 4.6
**Status:** Design (Revised v3 - Production-ready, addressed all risks)

## Problem Statement

The current GPS processing system implicitly assumes "each parsed NMEA sentence = one tick", but the specification defines Δt = 1 second. This mismatch causes:

1. **Double processing:** GPS modules send RMC → GSA → GGA bursts at 1Hz. Both GSA and GGA return complete `GpsPoint`, causing `process_gps()` to run twice per cycle.
2. **Incorrect dwell_time:** Increments twice per second instead of once, corrupting the F4 probability feature.
3. **Wasted compute:** Full pipeline (Kalman filter, map matching, FSM) runs unnecessarily.
4. **Duplicate events:** Can emit duplicate ARRIVAL/ANNOUNCE events for the same tick.

## Root Cause

The system lacks an explicit time model. NMEA sentences are treated as discrete events rather than parts of a single temporal snapshot.

## Solution: Timestamp-Driven Architecture

**Core principle:** GPS timestamp is the only authority. NMEA sentences are fragments of a single temporal snapshot, not discrete events.

- Accumulate NMEA sentences across read boundaries
- Emit exactly once per GPS timestamp change
- No artificial timer - GPS time drives the loop

```
on_sentence(sentence) → acc.update()
                       → acc.should_emit()? → state.tick()
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
    last_emitted_timestamp: Option<u64>,  // Internal emission tracking
}

enum FixQuality {
    Full,        // RMC + GGA (or GSA): position + motion + quality
    PositionOnly,// GGA only: position + quality, no motion
    MotionOnly,  // RMC only: position + motion, no quality
}
```

### Key Methods

- `new()` - Initialize empty accumulator
- `update(&mut self, sentence: &str) -> bool` - Accept any NMEA sentence in any order
- `should_emit(&mut self) -> bool` - Check if timestamp changed (emits **previous** snapshot), updates last_emitted
- `build(&self) -> Option<(GpsPoint, FixQuality)>` - Produce fix with quality indicator
- `reset(&mut self)` - Clear state after emission (only when emitting)

### Emission Semantics

**Critical:** `should_emit()` emits the **previous** snapshot when a **new** timestamp arrives.

```
RMC(t=01) + GGA(t=01) → accumulate
RMC(t=02) arrives → should_emit() true → emit (t=01) snapshot → reset
```

**First fix edge case:** Do NOT emit on first timestamp. Only emit after seeing second timestamp (requires `last_emitted_timestamp.is_some()`).

### Completeness Criteria

A fix is complete when it has:
- `has_fix == true`
- Valid latitude and longitude
- Valid timestamp

Quality tier determined by which fields are present.

### Data Sources

| Sentence | Provides |
|----------|----------|
| RMC | timestamp, lat/lon, speed, heading |
| GGA | lat/lon, HDOP, fix quality |
| GSA | HDOP |

### Robustness Guarantees

- **Order-independent:** Accepts sentences in any sequence
- **Partial data OK:** RMC-only or GGA-only produces valid fix (with appropriate quality tag)
- **Missing sentences tolerated:** Works even if GSA is skipped
- **Split bursts handled:** Accumulates across multiple reads if UART fragments burst
- **No dangerous defaults:** Missing fields remain None, letting estimation layer decide fallback
- **GPS silence behavior:** No new timestamp → no emission → system freezes (DR handles outages internally via `ProcessResult::DrOutage`)
- **Timestamp source:** Primarily RMC; GGA provides timestamp as fallback if RMC missing

## Main Loop Integration

**Critical:** No artificial timer. GPS timestamp change drives emission.

```rust
let mut fix_accumulator = FixAccumulator::new();

loop {
    match read_nmea_sentence_async(&mut uart, &mut line_buf).await {
        Ok(Some(sentence)) => {
            fix_accumulator.update(sentence);
            line_buf.reset();

            // Timestamp change is the ONLY trigger
            if fix_accumulator.should_emit() {
                if let Some((gps, quality)) = fix_accumulator.build() {
                    // Exactly ONE process_gps() call per GPS second
                    if let Some(arrival) = state.process_gps(&gps) {
                        emit_arrival_event(&mut uart, &arrival).await;
                    }
                }
                // Only reset after emission, preserving partial data
                fix_accumulator.reset();
            }
        }
        Ok(None) => {
            // UART idle - wait for next sentence
            // DO NOT process or reset - accumulator preserves state
            continue;
        }
        Err(e) => {
            // Handle error - accumulator preserves state
            handle_uart_error(e);
        }
    }

    // Persist state (rate-limited, unrelated to GPS timing)
    if should_persist(&state) {
        persist_state(&mut flash, &state).await;
    }
}
```

**Key changes:**
- Removed `Timer::after(1s)` - GPS time is authoritative
- Removed two-stage "drain then process" pattern
- Emission happens immediately on timestamp change
- Reset only when emitting, preserving partial burst data
- No reliance on "FIFO empty = burst complete"

## Edge Case Handling

| Scenario | Behavior |
|----------|----------|
| First fix (startup) | `should_emit()` false (no prior timestamp), accumulates until second timestamp |
| GPS outage (no sentences) | `should_emit()` false, no processing, state preserved |
| Only GGA arrives | Builds `PositionOnly` fix, speed/heading are None |
| Only RMC arrives | Builds `MotionOnly` fix, HDOP is None |
| Checksum failures | Sentence rejected, accumulator unchanged |
| Same timestamp (burst) | `should_emit()` false, continues accumulating |
| Timestamp rollover (midnight) | `Option<u64>` comparison handles None → Some transition |
| Split burst across reads | Accumulator preserves partial data until complete |
| Timestamp jump (lost second) | Emits once for new timestamp, Kalman/DR handle Δt |
| All sentences missing | `build()` returns None, no processing |
| RMC missing, GGA present | Uses GGA timestamp, emits on next timestamp change |
| Long GPS silence | No emission, system freezes (DR handles internally) |

### Data Quality Handling

Missing fields remain `None` - estimation layer decides fallback:
- `speed: None` - Kalman uses last valid velocity or DR
- `heading: None` - Map matching uses relaxed heading constraint
- `hdop: None` - Detection uses worst-case probability weights

## Testing Strategy

### Unit Tests
- `FixAccumulator` with various sentence combinations (RMC-only, GGA-only, RMC+GGA, RMC+GSA+GGA)
- Out-of-order sentence sequences
- Missing sentences
- Checksum failures
- Timestamp rollover
- `should_emit()` behavior with same/different timestamps
- `build()` returns appropriate `FixQuality` tag

### Critical Integration Tests

**Split burst test:**
```
read1: RMC at 12:00:01
read2: (delay, UART idle)
read3: GGA at 12:00:01
read4: GSA at 12:00:01
read5: RMC at 12:00:02  → triggers emission of 12:00:01 fix
```
Verify: exactly ONE tick for 12:00:01, not partial emissions

**Timestamp jump test:**
```
12:00:01 → emit
12:00:03 → emit (12:00:02 was lost)
```
Verify: exactly one tick per received timestamp, Kalman Δt handles jump

**Burst ordering test:**
```
GGA → RMC → GSA (out of order)
```
Verify: accumulator handles correctly, produces Full quality fix

**RMC missing test:**
```
t=01: RMC + GGA → emit (t=01) on t=02
t=02: GGA only → emit (t=02) on t=03
t=03: RMC arrives → emit (t=03) on t=04
```
Verify: emits once per timestamp, no stall when RMC missing

### Integration Tests
- Real NMEA bursts from test data (`tpF805_normal_nmea.txt`)
- Verify `process_gps()` called exactly once per second
- Verify `dwell_time_s` increments by 1 per second
- Verify no duplicate ARRIVAL/ANNOUNCE events

### Regression Test
- Compare trace output before/after fix
- Validate against ground truth

## Files to Modify

1. **`crates/shared/src/types.rs`** (or where `GpsPoint` is defined)
   - Change `speed`, `heading`, `hdop` to `Option<T>` types
   - Add `FixQuality` enum

2. **`crates/pipeline/gps_processor/src/nmea.rs`**
   - Add `FixAccumulator` struct and implementation
   - Add `FixQuality` enum
   - Modify existing code to handle optional fields
   - **Performance:** Fast path for `FixQuality::Full` (all fields Some) to minimize branching

3. **`crates/pico2-firmware/src/main.rs`**
   - Remove `Timer::after(1s)`
   - Replace direct `parse_sentence()` → `process_gps()` flow
   - Add `FixAccumulator` with event-driven emission
   - Remove two-stage "drain FIFO" loop

4. **`crates/pipeline/gps_processor/src/kalman.rs`** and **`detection/`**
   - Update to handle `Option<T>` fields in `GpsPoint`
   - Provide appropriate fallbacks for missing data
   - **Performance:** Fast path for complete fixes, fallback only when needed

5. **Tests**
   - Add accumulator unit tests in `nmea.rs`
   - Add split burst integration test
   - Add timestamp jump integration test
   - Add RMC missing test
   - Update existing tests for `Option<T>` fields

## Impact

- **Correct Δt = 1s:** GPS timestamp is authoritative, no timer drift
- **Accurate dwell_time:** Increments once per GPS second
- **No duplicate events:** FSM runs exactly once per timestamp
- **Reduced compute:** Pipeline runs once per second, not 2-3x
- **Better data quality:** Combines RMC + GGA for superior Kalman input
- **Explicit quality tiers:** Downstream code knows data completeness
- **Handles split bursts:** Accumulates across UART read boundaries
- **No dangerous defaults:** Missing fields are explicit, not synthetic

## Scope

- ~300 lines new code
- Moderate refactoring (GpsPoint field changes affect Kalman, detection)
- Well-bounded change with clear contract (FixQuality enum)
- Requires updating estimation fallback logic for Option fields

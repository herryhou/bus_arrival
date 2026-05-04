# Common Assertion Patterns

Common assertion patterns for regression tests in the bus arrival detection system.

## Pattern 1: Progress Distance Validation

Assert that progress distance jumps to the expected range after route events.

```rust
// After detour re-entry, progress should jump to stop 6 area (~1775m)
let progress_after = state.last_valid_s_cm();
assert!(progress_after > 170_000,
        "Progress should be at stop 6 area (~1775m), got {}cm",
        progress_after);
assert!(progress_after < 185_000,
        "Progress should not overshoot stop 6 significantly");
```

**Use when:** Testing off-route recovery, detour re-entry, GPS jump recovery

## Pattern 2: Stop Index Detection

Assert the correct stop is detected after specific events.

```rust
// After re-entry at stop 6, should detect stop 6, not stop 2
let detected_stop = state.last_known_stop_index();
assert_eq!(detected_stop, Some(6),
           "Should detect stop 6 after detour re-entry");
```

**Use when:** Testing arrival detection, stop corridor logic, re-entry behavior

## Pattern 3: State Machine Mode

Assert the system is in the expected mode.

```rust
use pico2_firmware::state::SystemMode;

// Should recover to Normal mode after valid GPS on route
let mode = state.system_mode();
assert_eq!(mode, SystemMode::Normal,
           "Should be in Normal mode, not stuck in OffRoute");
```

**Use when:** Testing mode transitions, off-route detection, recovery logic

## Pattern 4: No Regression

Assert the bug symptom does NOT occur.

```rust
// Bug: Progress would get stuck at ~1072m (stop 2 area)
let progress = state.last_valid_s_cm();
let error_from_wrong = (progress - 107_000).abs();
assert!(error_from_wrong > 10_000,
        "Progress {}cm is too close to wrong stop 2 position (107000cm)",
        progress);
```

**Use when:** The bug causes a specific wrong value that should be avoided

## Pattern 5: Monotonic Progress

Assert progress always increases (never goes backward).

```rust
// Collect progress values throughout test
let mut prev_progress = 0;
for trace_entry in trace_entries {
    let current = trace_entry.s_cm;
    assert!(current >= prev_progress,
            "Progress went backward: {} -> {}",
            prev_progress, current);
    prev_progress = current;
}
```

**Use when:** Testing GPS processing, Kalman filtering, DR updates

## Pattern 6: Dwell Time Detection

Assert stop dwells are detected correctly.

```rust
// Count how many ticks we're at the same stop
let dwell_ticks = trace_entries.iter()
    .filter(|e| e.stop_idx == Some(6) && e.speed_cms < 50)
    .count();
assert!(dwell_ticks >= 8,
        "Should have at least 8 ticks of dwell at stop 6, got {}",
        dwell_ticks);
```

**Use when:** Testing arrival/departure detection, stop dwell logic

## Pattern 7: Confidence Threshold

Assert estimation confidence is above/below threshold.

```rust
// During valid GPS, confidence should be high
let confidence = state.estimation_confidence();
assert!(confidence > 0.7,
        "Confidence should be high during valid GPS, got {}",
        confidence);
```

**Use when:** Testing confidence signals, estimation quality

## Pattern 8: Event Count

Assert expected number of events occurred.

```rust
// Should have exactly 3 arrival events
let arrivals = parse_arrivals(output_file);
assert_eq!(arrivals.len(), 3,
           "Expected 3 arrivals, got {}: {:?}",
           arrivals.len(), arrivals);
```

**Use when:** Testing end-to-end pipeline, event generation

## Pattern 9: Tolerance-Based Comparison

Assert value is within acceptable tolerance of expected.

```rust
const PROGRESS_TOLERANCE_CM: i32 = 500; // 5m tolerance
const EXPECTED_STOP_6_PROGRESS_CM: i32 = 177_500;

let progress = state.last_valid_s_cm();
let error = (progress - EXPECTED_STOP_6_PROGRESS_CM).abs();
assert!(error <= PROGRESS_TOLERANCE_CM,
        "Progress {}cm is {}cm from expected {}cm (tolerance: {}cm)",
        progress, error, EXPECTED_STOP_6_PROGRESS_CM, PROGRESS_TOLERANCE_CM);
```

**Use when:** GPS position has inherent noise, exact match not possible

## Pattern 10: File Loading

As a baseline, assert test files can be loaded.

```rust
assert!(fs::metadata(nmea_file).is_ok(),
        "NMEA file should exist: {}", nmea_file);
assert!(fs::metadata(route_bin).is_ok(),
        "Route bin file should exist: {}", route_bin);
```

**Use when:** Creating new test, before implementing full assertions

## Choosing the Right Pattern

| Bug Symptom | Use Pattern |
|-------------|-------------|
| Progress stuck/wrong | Pattern 1, Pattern 4, Pattern 9 |
| Wrong stop detected | Pattern 2 |
| Mode transition issues | Pattern 3 |
| Progress goes backward | Pattern 5 |
| Dwell not detected | Pattern 6 |
| Confidence issues | Pattern 7 |
| Event count mismatch | Pattern 8 |

## Tolerance Guidelines

| Measurement | Recommended Tolerance | Rationale |
|-------------|----------------------|-----------|
| Progress distance | ±500 cm (±5m) | GPS noise ±15m, projection errors |
| Stop detection | Exact | Stop index is discrete |
| Mode | Exact | Mode is discrete |
| Dwell time | ±2 ticks | 1Hz GPS, 8s nominal dwell |
| Confidence | ±0.1 | Continuous 0-1 scale |

# Close Stop Fix Design

## Overview

Fix the problem where buses fail to detect arrivals at closely-spaced stops (<100m apart). The issue occurs when corridor overlap protection causes the second stop's corridor to start AFTER the stop location itself.

## Problem Analysis

### Current Behavior (tpF805 Route, Stop #2 → #3)

```
Stop #2: progress_cm = 127,689
Stop #3: progress_cm = 135,621
Distance: 7,932 cm (79.32m)

Current corridors (with 20m overlap protection):
- Stop #2: corridor_end ≈ 131,468 cm
- Stop #3: corridor_start ≈ 134,179 cm

Critical Issue: Stop #3's corridor starts 1,442 cm PAST the stop location!
```

**Result**: Bus enters Stop #3's corridor after already passing the stop, causing:
- Low dwell_time_s (just entered corridor)
- Low p4 probability feature
- Total probability = 185 < threshold 191
- Stop #3 not detected

## Solution: Three-Tier Architecture

```
Input GPS → Tier 1 (Main Loop) → Tier 2 (Preprocess) → Tier 3 (Probability) → Output
```

## Tier 1: Enhanced Main Loop

**File**: `arrival_detector/src/main.rs`

**Purpose**: Pass next stop information to probability calculation.

**Changes**:
```rust
// Determine next stop among active stops
let next_stop_opt = active_indices.iter()
    .position(|&idx| idx == stop_idx)
    .and_then(|pos| {
        if pos + 1 < active_indices.len() {
            Some(&stops[active_indices[pos + 1]])
        } else {
            None
        }
    });

let prob = probability::arrival_probability_adaptive(
    record.s_cm,
    record.v_cms,
    stop,
    state.dwell_time_s,
    &gaussian_lut,
    &logistic_lut,
    next_stop_opt,  // NEW parameter
);
```

## Tier 2: Preprocess Corridor Redesign

**File**: `preprocessor/src/stops.rs`

**Purpose**: Adjust corridor ratios for closely-spaced stops.

**New Function**:
```rust
pub fn preprocess_close_stop_corridors(stops: &mut [Stop]) {
    const CLOSE_STOP_THRESHOLD_CM: i32 = 12_000; // 120m
    const PRE_RATIO: u32 = 55;
    const POST_RATIO: u32 = 35;

    for i in 0..stops.len().saturating_sub(1) {
        let distance = stops[i + 1].progress_cm - stops[i].progress_cm;

        if distance < CLOSE_STOP_THRESHOLD_CM {
            stops[i].corridor_end_cm =
                stops[i].progress_cm + (distance * POST_RATIO as i32) / 100;
            stops[i + 1].corridor_start_cm =
                stops[i + 1].progress_cm - (distance * PRE_RATIO as i32) / 100;
        }
    }
}
```

**Corridor Ratio for Close Stops**:
- 55% before stop (pre-corridor)
- 10% gap between corridors
- 35% after stop (post-corridor)

**For Stop #2/#3 (79.32m apart)**:
```
Stop #2 corridor: [123,326 ~ 130,465]
Gap: 793 cm (8m)
Stop #3 corridor: [131,258 ~ 138,397]
```

## Tier 3: Adaptive Probability Weights

**File**: `arrival_detector/src/probability.rs`

**Purpose**: Remove dwell time penalty for close stops.

**New Function**:
```rust
pub fn arrival_probability_adaptive(
    s_cm: DistCm,
    v_cms: SpeedCms,
    stop: &shared::Stop,
    dwell_time_s: u16,
    gaussian_lut: &[u8; 256],
    logistic_lut: &[u8; 128],
    next_stop: Option<&shared::Stop>,
) -> Prob8 {
    // Feature calculations (unchanged)
    let d_cm = (s_cm - stop.progress_cm).abs();
    let idx1 = ((d_cm as i64 * 64) / 2750).min(255) as usize;
    let p1 = gaussian_lut[idx1] as u32;

    let idx2 = (v_cms / 10).max(0).min(127) as usize;
    let p2 = logistic_lut[idx2] as u32;

    let idx3 = ((d_cm as i64 * 64) / 2000).min(255) as usize;
    let p3 = gaussian_lut[idx3] as u32;

    let p4 = ((dwell_time_s as u32) * 255 / 10).min(255) as u32;

    // Adaptive weights
    let (w1, w2, w3, w4) = if let Some(next) = next_stop {
        let dist_to_next = (next.progress_cm - stop.progress_cm).abs();
        if dist_to_next < 12_000 {
            (19, 5, 8, 0)  // Close stop: remove p4
        } else {
            (13, 6, 10, 3)  // Normal stop
        }
    } else {
        (13, 6, 10, 3)  // Last stop
    };

    ((w1 * p1 + w2 * p2 + w3 * p3 + w4 * p4) / 32) as u8
}
```

**Weight Comparison**:
| Condition | w1 | w2 | w3 | w4 | Sum |
|-----------|----|----|----|----|-----|
| Close stop (<120m) | 19 | 5 | 8 | 0 | 32 |
| Normal stop | 13 | 6 | 10 | 3 | 32 |

## Data Flow

### Preprocess Phase
```
Stop positions → project_stops_validated() → preprocess_close_stop_corridors() → Binary route_data.bin
```

### Runtime Phase
```
GPS record → find_active_stops() → For each: determine next_stop → arrival_probability_adaptive() → state.update() → Arrival events
```

## Error Handling

### Edge Cases

1. **Distance too small (<2000cm)**: Skip adjustment, minimum corridor sizes apply
2. **First/last stop**: Loop handles via `saturating_sub(1)` and `Option<&Stop>`
3. **Next stop before current** (route reversal): Distance check handles naturally
4. **Exactly 120m threshold**: No adjustment (uses `<` not `<=`)

### Validation

```rust
assert!(stops[i].corridor_start_cm < stops[i].progress_cm);
assert!(stops[i].corridor_end_cm > stops[i].progress_cm);
assert!(stops[i].corridor_end_cm < stops[i+1].corridor_start_cm); // gap maintained
```

## Testing

### Unit Tests

1. **Corridor adjustment**: Verify 55%/10%/35% ratio for 79m stops
2. **120m threshold**: Verify no adjustment at exactly 120m
3. **Adaptive probability**: Verify weights change based on next_stop distance
4. **Last stop**: Verify graceful handling when next_stop is None

### Integration Tests

1. **tpF805 route**: Verify Stop #3 is now detected
2. **50m stops**: Verify extreme case works
3. **200m stops**: Verify normal behavior unchanged

### Expected Results

```
Before: Stop #3 probability = 185, state = Arriving
After:  Stop #3 probability >= 191, state = AtStop
```

## Implementation Order

1. **Tier 2** (Preprocess) - Foundation for other tiers
2. **Tier 3** (Probability) - Depends on preprocess output
3. **Tier 1** (Main Loop) - Depends on probability signature

## Files Modified

1. `preprocessor/src/stops.rs` - Add `preprocess_close_stop_corridors()`
2. `preprocessor/src/main.rs` - Call new preprocess function
3. `arrival_detector/src/probability.rs` - Add `arrival_probability_adaptive()`
4. `arrival_detector/src/main.rs` - Pass next_stop to probability function

## Backward Compatibility

- Existing route_data.bin files will be regenerated by new preprocessor
- Standard stops (>120m apart) unchanged behavior
- No changes to FSM state transition logic
- No changes to corridor filter logic

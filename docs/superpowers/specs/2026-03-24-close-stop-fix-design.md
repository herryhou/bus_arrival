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

Critical Issue: Stop #3's corridor starts only 1,442 cm BEFORE the stop location!
(Normal pre-corridor is 8,000cm, but compressed to just 1,442cm due to overlap protection)
```

**Result**: Bus enters Stop #3's corridor very late (only 14m before stop), causing:
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

**Purpose**: Pass sequential next stop information to probability calculation.

**Rationale**: The probability model needs to know if the NEXT stop in the route sequence is close (<120m), regardless of whether it's currently active. This allows adaptive weights to be applied proactively.

**Changes**:
```rust
// Before the active_stops loop, calculate next_stop for each stop
let next_stops: Vec<Option<&Stop>> = stops.iter()
    .enumerate()
    .map(|(i, _)| {
        if i + 1 < stops.len() {
            Some(&stops[i + 1])
        } else {
            None
        }
    })
    .collect();

// Inside the active_stops loop:
for &stop_idx in &active_indices {
    let stop = &stops[stop_idx];
    let state = &mut stop_states[stop_idx];
    let next_stop = next_stops[stop_idx];  // Sequential next, not next active

    let prob = probability::arrival_probability_adaptive(
        record.s_cm,
        record.v_cms,
        stop,
        state.dwell_time_s,
        &gaussian_lut,
        &logistic_lut,
        next_stop,  // NEW parameter
    );
    // ... rest of processing
}
```

**Key Point**: We pass the sequential next stop from the route, NOT the next active stop. This is critical because when stops overlap, the next active stop might be the same as the current stop.

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

**Rationale for 55%/10%/35% Ratio**:
1. **Pre-corridor (55%)**: Prioritizes early detection over post-corridor, giving the system more time to build dwell_time_s probability
2. **Post-corridor (35%)**: Reduced from 40% to accommodate the tight spacing, but still sufficient for departure detection
3. **Gap (10%)**: Maintains separation between corridors to prevent ambiguous state when both stops could be active

**Relationship with Existing Overlap Protection**:
The existing preprocessor applies a 20m (δ_sep = 2000cm) gap enforcement AFTER corridor boundaries are set. For close stops (<120m), this existing logic would produce invalid corridors (starting after the stop). The `preprocess_close_stop_corridors()` function is called BEFORE the existing overlap protection, ensuring corridors are valid before the 20m gap check is applied. This is a complementary fix, not a replacement.

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
    v_cms: SpeedCms,  // Type alias for i32, matches record.v_cms
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
            (14, 7, 11, 0)  // Close stop: remove p4, maintain proportions
        } else {
            (13, 6, 10, 3)  // Normal stop
        }
    } else {
        (13, 6, 10, 3)  // Last stop
    };

    ((w1 * p1 + w2 * p2 + w3 * p3 + w4 * p4) / 32) as u8
}
```

**Weight Redistribution Rationale**:
When removing p4 (weight=3) for close stops, we scale remaining weights proportionally:
- Original: w1=13, w2=6, w3=10, w4=3 (sum=32)
- Without p4: 13+6+10=29, scale factor = 32/29 ≈ 1.103
- Scaled: w1≈14, w2≈7, w3≈11, w4=0 (sum=32)

This maintains the relative importance of each feature while ensuring sum=32 to prevent u8 overflow.

**Weight Comparison**:
| Condition | w1 | w2 | w3 | w4 | Sum |
|-----------|----|----|----|----|-----|
| Close stop (<120m) | 14 | 7 | 11 | 0 | 32 |
| Normal stop | 13 | 6 | 10 | 3 | 32 |

**Probability Calculation Examples**:

*Example 1: Close stop (Stop #3 at time=481 with new corridor)*
```
Input: s_cm=135,584, stop.progress_cm=135,621, v_cms=641, dwell_time_s≈6s

Context: With new corridor_start=131,258, bus entered corridor at time≈469.
         By time=481, dwell has accumulated for ~6 seconds.

Features:
- p1 (distance): d_cm=|135584-135621|=37, idx1=0, p1≈255 (at stop)
- p2 (speed): v_cms=641, idx2=64, p2≈105 (approaching)
- p3 (progress): d_cm=37, idx3=1, p3≈255 (very near)
- p4 (dwell): dwell≈6s, p4=(6×255)/10=153

With adaptive weights (14,7,11,0):
prob = (14×255 + 7×105 + 11×255 + 0×153) / 32
     = (3570 + 735 + 2805 + 0) / 32
     = 7110 / 32 = 222 ✓ (well above threshold 191)

Note: With p4 weight removed, high p1/p3 compensate for moderate speed.
```

*Example 2: Normal stop (>120m apart)*
```
Standard weights (13,6,10,3) apply unchanged.
```

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

1. **Distance too small (<2000cm)**: Skip adjustment, minimum corridor sizes apply. This prevents degenerate corridors when stops are extremely close (e.g., same physical location).

2. **First/last stop**: Loop handles via `saturating_sub(1)` and `Option<&Stop>`. First stop has no previous stop to compare with; last stop has no next stop (next_stop=None in probability calculation).

3. **Next stop before current** (route reversal): Distance check handles naturally via `.abs()`. The system will detect the distance but won't apply close-stop logic since route progress is expected to be monotonic.

4. **Exactly 120m threshold**: No adjustment (uses `<` not `<=`). This provides clear boundary behavior and matches the overlap protection threshold.

5. **Overlapping corridors after preprocessing**: The existing 20m overlap protection in `project_stops_validated()` still applies AFTER this function, ensuring a minimum gap even with adjusted corridors.

6. **Sequential stops with large gaps (>120m)**: No modification applied; standard corridor sizes (80m pre, 40m post) are used.

7. **Three or more consecutive close stops**: Each adjacent pair is processed independently. For stops A, B, C where A→B and B→C are both <120m:

   - Processing A→B pair adjusts: B.corridor_start_cm (pre-corridor from B's perspective)
   - Processing B→C pair adjusts: B.corridor_end_cm (post-corridor from B's perspective)

   These two adjustments affect **different boundaries** of B's corridor and do not conflict. Example:
   ```
   A.progress = 100,000cm
   B.progress = 108,000cm (A→B: d1=8,000cm)
   C.progress = 115,000cm (B→C: d2=7,000cm)

   After A→B processing: B.corridor_start = 108,000 - 0.55×8000 = 103,600cm
   After B→C processing: B.corridor_end   = 108,000 + 0.35×7000 = 110,450cm

   B's final corridor: [103,600 ~ 110,450] - valid and symmetric around B
   ```

   The validation assert ensures corridor_start < progress < corridor_end for all cases.

8. **GPS drift at corridor boundaries**: The 10% gap between close-stop corridors provides buffer against GPS noise triggering incorrect state transitions.

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

**Recommended order: Tier 2 → Tier 3 → Tier 1**

1. **Tier 2** (Preprocess) - Implement `preprocess_close_stop_corridors()` first. This is static preprocessing with no runtime dependencies. Can be verified independently by checking corridor boundaries in the generated route_data.bin.

2. **Tier 3** (Probability) - Implement `arrival_probability_adaptive()` function. The new function signature takes `next_stop: Option<&Stop>` parameter. This can be implemented and unit tested independently.

3. **Tier 1** (Main Loop) - Update main.rs to calculate `next_stops` array and pass to probability function. Depends on Tier 3's function signature being complete.

**Rationale for this order**:
- Tier 2 is pure data transformation with clear verification (corridor boundaries)
- Tier 3 adds the adaptive logic but doesn't change existing behavior until called
- Tier 1 wires everything together and should be done last
- Each step can be tested independently before moving to the next

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

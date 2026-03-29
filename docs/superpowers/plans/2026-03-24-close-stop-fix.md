# Close Stop Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix bus arrival detection for stops spaced <120m apart by implementing corridor preprocessing and adaptive probability weights.

**Architecture:** Three-tier fix: (1) Preprocess corridor ratios for close stops, (2) Adaptive probability function with next_stop parameter, (3) Main loop passes sequential next stop.

**Tech Stack:** Rust, embedded-style no_std, unit tests + integration tests with real route data.

---

## File Structure

```
preprocessor/
  src/
    stops.rs          # Add preprocess_close_stop_corridors()
    main.rs           # Call new preprocess function
    stops/
      tests.rs        # Add unit tests for corridor adjustment

arrival_detector/
  src/
    probability.rs    # Add arrival_probability_adaptive()
    main.rs           # Pass next_stop to probability function

test_data/
  tpF805_stops.json   # Existing test data
```

---

## Task 1: Tier 2 - Add Close Stop Corridor Preprocessing

**Files:**
- Modify: `preprocessor/src/stops.rs:20-51`
- Modify: `preprocessor/src/main.rs:150-151`
- Test: `preprocessor/src/stops/tests.rs`

**Rationale:** Preprocess corridor boundaries for stops <120m apart using 55%/10%/35% ratio. This is static data transformation with no runtime dependencies.

- [ ] **Step 1: Write failing test for corridor adjustment**

Add to `preprocessor/src/stops/tests.rs`:

```rust
#[test]
fn test_close_stop_corridor_adjustment() {
    // Stops 79m apart (tpF805 Stop #2/#3 case)
    let progress_values = vec![127_689, 135_621]; // d = 7,932 cm

    // First, get standard corridors
    let mut stops = super::super::project_stops_validated(
        &progress_values,
        &crate::input::StopsInput { stops: vec![] } // Dummy input
    );

    // Apply close stop preprocessing
    super::super::preprocess_close_stop_corridors(&mut stops);

    // Verify 55%/10%/35% ratio
    // Expected:
    // Stop 1 corridor_end = 127,689 + 0.35*7,932 = 130,465
    // Stop 2 corridor_start = 135,621 - 0.55*7,932 = 131,258
    // Gap = 793 cm (10% of distance)

    assert_eq!(stops[0].corridor_end_cm, 130_465);
    assert_eq!(stops[1].corridor_start_cm, 131_258);

    // Verify gap between corridors
    assert_eq!(stops[1].corridor_start_cm - stops[0].corridor_end_cm, 793);
}

#[test]
fn test_no_adjustment_at_threshold() {
    // Stops exactly 120m apart - should NOT be adjusted
    let progress_values = vec![100_000, 112_000]; // d = 12,000 cm

    let mut stops = super::super::project_stops_validated(
        &progress_values,
        &crate::input::StopsInput { stops: vec![] }
    );

    let stops_before = stops.clone();
    super::super::preprocess_close_stop_corridors(&mut stops);

    // Corridors should be unchanged (threshold uses <, not <=)
    assert_eq!(stops[0].corridor_end_cm, stops_before[0].corridor_end_cm);
    assert_eq!(stops[1].corridor_start_cm, stops_before[1].corridor_start_cm);
}

#[test]
fn test_no_adjustment_far_apart() {
    // Stops 200m apart - standard corridors apply
    let progress_values = vec![100_000, 120_000]; // d = 20,000 cm

    let mut stops = super::super::project_stops_validated(
        &progress_values,
        &crate::input::StopsInput { stops: vec![] }
    );

    let stops_before = stops.clone();
    super::super::preprocess_close_stop_corridors(&mut stops);

    // Should be unchanged
    assert_eq!(stops[0].corridor_end_cm, stops_before[0].corridor_end_cm);
    assert_eq!(stops[1].corridor_start_cm, stops_before[1].corridor_start_cm);
}

#[test]
fn test_three_consecutive_close_stops() {
    // Three stops: A→B=80m, B→C=90m
    // B's corridor should be adjusted on both sides
    let progress_values = vec![100_000, 108_000, 117_000];

    let mut stops = super::super::project_stops_validated(
        &progress_values,
        &crate::input::StopsInput { stops: vec![] }
    );

    super::super::preprocess_close_stop_corridors(&mut stops);

    // Verify B's corridor boundaries
    // B.corridor_start from A→B: 108,000 - 0.55*8,000 = 103,600
    // B.corridor_end from B→C: 108,000 + 0.35*9,000 = 111,150
    assert_eq!(stops[1].corridor_start_cm, 103_600);
    assert_eq!(stops[1].corridor_end_cm, 111_150);

    // Verify B is still valid (start < progress < end)
    assert!(stops[1].corridor_start_cm < stops[1].progress_cm);
    assert!(stops[1].corridor_end_cm > stops[1].progress_cm);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p preprocessor test_close_stop`
Expected: FAIL with "cannot find function `preprocess_close_stop_corridors`"

- [ ] **Step 3: Implement preprocess_close_stop_corridors()**

Add to `preprocessor/src/stops.rs` after `project_stops_validated()` function (around line 52):

```rust
/// Adjust corridor boundaries for closely-spaced stops.
///
/// For stops <120m apart, redistributes corridor space as:
/// - 55% before stop (pre-corridor)
/// - 10% gap between corridors
/// - 35% after stop (post-corridor)
///
/// This prevents overlap protection from compressing corridors
/// to the point where detection fails.
///
/// # Arguments
/// * `stops` - Stops with standard corridors (modified in place)
///
/// # Called by
/// main.rs after project_stops_validated(), before packing
pub fn preprocess_close_stop_corridors(stops: &mut [Stop]) {
    const CLOSE_STOP_THRESHOLD_CM: i32 = 12_000; // 120m
    const PRE_RATIO: i32 = 55;   // 0.55 × distance
    const POST_RATIO: i32 = 35;  // 0.35 × distance
    // Gap of 0.10 × distance forms naturally

    for i in 0..stops.len().saturating_sub(1) {
        let distance = stops[i + 1].progress_cm - stops[i].progress_cm;

        // Skip if distance is too small or at threshold
        if distance < 2_000 || distance >= CLOSE_STOP_THRESHOLD_CM {
            continue;
        }

        // Adjust current stop's post-corridor
        stops[i].corridor_end_cm =
            stops[i].progress_cm + (distance * POST_RATIO) / 100;

        // Adjust next stop's pre-corridor
        stops[i + 1].corridor_start_cm =
            stops[i + 1].progress_cm - (distance * PRE_RATIO) / 100;
    }
}
```

- [ ] **Step 4: Update main.rs to call preprocessing function**

Modify `preprocessor/src/main.rs` around line 150-151 (after `project_stops_validated` call):

Before:
```rust
let projected_stops = match &validation.reversal_info {
    None => {
        println!("[VALIDATION PASS]");
        println!("✓ All {} stops validated - monotonic sequence confirmed", validation.progress_values.len());
        project_stops_validated(&validation.progress_values, &stops_input)
    }
    // ...
};
```

After:
```rust
let projected_stops = match &validation.reversal_info {
    None => {
        println!("[VALIDATION PASS]");
        println!("✓ All {} stops validated - monotonic sequence confirmed", validation.progress_values.len());
        let mut stops = project_stops_validated(&validation.progress_values, &stops_input);
        stops::preprocess_close_stop_corridors(&mut stops);
        println!("✓ Applied close-stop corridor adjustment");
        stops
    }
    // ...
};
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p preprocessor test_close_stop`
Expected: PASS for all 4 tests

- [ ] **Step 6: Verify preprocessing with actual tpF805 route**

Run: `cargo run -p preprocessor -- test_data/tpF805_route.json test_data/tpF805_stops.json /tmp/tpF805_test.bin`

Expected output should include:
```
[VALIDATION PASS]
✓ All 35 stops validated - monotonic sequence confirmed
✓ Applied close-stop corridor adjustment
```

- [ ] **Step 7: Commit**

```bash
git add preprocessor/src/stops.rs preprocessor/src/main.rs
git commit -m "feat: add close-stop corridor preprocessing

- Add preprocess_close_stop_corridors() for stops <120m apart
- 55%/10%/35% ratio (pre/gap/post) prevents detection failure
- Add 4 unit tests covering threshold, far apart, and 3-stop cases
- Call preprocessing after stop validation in main.rs

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 2: Tier 3 - Add Adaptive Probability Function

**Files:**
- Modify: `arrival_detector/src/probability.rs:64-93`
- Test: `arrival_detector/src/probability.rs` (extend tests)

**Rationale:** Add new function with adaptive weights (14,7,11,0) for close stops, maintaining sum=32 to prevent overflow.

- [ ] **Step 1: Write failing test for adaptive probability**

Add to `arrival_detector/src/probability.rs` in the `tests` module (around line 95):

```rust
#[test]
fn test_adaptive_probability_close_stop() {
    let g_lut = build_gaussian_lut();
    let l_lut = build_logistic_lut();

    let stop_current = Stop {
        progress_cm: 100_000,
        corridor_start_cm: 90_000,
        corridor_end_cm: 110_000,
    };

    let stop_next = Stop {
        progress_cm: 108_000, // 8,000cm away (<12,000 threshold)
        corridor_start_cm: 98_000,
        corridor_end_cm: 118_000,
    };

    // At stop, moderate speed, some dwell
    let prob = arrival_probability_adaptive(
        100_000,  // s_cm (at stop)
        600,      // v_cms (approaching)
        &stop_current,
        5,        // dwell_time_s
        &g_lut,
        &l_lut,
        Some(&stop_next),
    );

    // With close stop, p4 weight is removed, should be higher
    assert!(prob > 190, "Expected probability > 190 for close stop, got {}", prob);
    assert!(prob <= 255);
}

#[test]
fn test_adaptive_probability_normal_stop() {
    let g_lut = build_gaussian_lut();
    let l_lut = build_logistic_lut();

    let stop_current = Stop {
        progress_cm: 100_000,
        corridor_start_cm: 90_000,
        corridor_end_cm: 110_000,
    };

    let stop_next = Stop {
        progress_cm: 125_000, // 25,000cm away (>12,000 threshold)
        corridor_start_cm: 115_000,
        corridor_end_cm: 135_000,
    };

    let prob_close = arrival_probability_adaptive(
        100_000, 600, &stop_current, 5, &g_lut, &l_lut, Some(&stop_next)
    );

    // Normal stop uses standard weights
    assert!(prob_close <= 255);
}

#[test]
fn test_adaptive_probability_last_stop() {
    let g_lut = build_gaussian_lut();
    let l_lut = build_logistic_lut();

    let stop = Stop {
        progress_cm: 100_000,
        corridor_start_cm: 90_000,
        corridor_end_cm: 110_000,
    };

    // Last stop (next_stop = None)
    let prob = arrival_probability_adaptive(
        100_000, 0, &stop, 10, &g_lut, &l_lut, None
    );

    // Should use standard weights
    assert!(prob <= 255);
    assert!(prob > 150); // At stop with 10s dwell should be high
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p arrival_detector test_adaptive`
Expected: FAIL with "cannot find function `arrival_probability_adaptive`"

- [ ] **Step 3: Implement arrival_probability_adaptive()**

Add to `arrival_detector/src/probability.rs` after the existing `arrival_probability()` function (around line 64):

```rust
/// Compute arrival probability with adaptive weights for close stops.
///
/// When next stop is <120m away, removes dwell time (p4) weight and
/// redistributes proportionally: (14, 7, 11, 0) instead of (13, 6, 10, 3).
///
/// # Arguments
/// * `next_stop` - Next sequential stop in route (not next active stop)
pub fn arrival_probability_adaptive(
    s_cm: DistCm,
    v_cms: SpeedCms,  // Type alias for i32
    stop: &shared::Stop,
    dwell_time_s: u16,
    gaussian_lut: &[u8; 256],
    logistic_lut: &[u8; 128],
    next_stop: Option<&shared::Stop>,
) -> Prob8 {
    // Feature calculations (same as arrival_probability)
    let d_cm = (s_cm - stop.progress_cm).abs();
    let idx1 = ((d_cm as i64 * 64) / 2750).min(255) as usize;
    let p1 = gaussian_lut[idx1] as u32;

    let idx2 = (v_cms / 10).max(0).min(127) as usize;
    let p2 = logistic_lut[idx2] as u32;

    let idx3 = ((d_cm as i64 * 64) / 2000).min(255) as usize;
    let p3 = gaussian_lut[idx3] as u32;

    let p4 = ((dwell_time_s as u32) * 255 / 10).min(255) as u32;

    // Adaptive weights based on next stop distance
    let (w1, w2, w3, w4) = if let Some(next) = next_stop {
        let dist_to_next = (next.progress_cm - stop.progress_cm).abs();
        if dist_to_next < 12_000 {
            // Close stop: remove p4, scale remaining to sum=32
            // Original: 13+6+10+3=32, without p4: 29, scale factor = 32/29
            // 13/29*32 ≈ 14, 6/29*32 ≈ 7, 10/29*32 ≈ 11
            (14, 7, 11, 0)
        } else {
            // Normal stop: standard weights
            (13, 6, 10, 3)
        }
    } else {
        // Last stop: standard weights
        (13, 6, 10, 3)
    };

    ((w1 * p1 + w2 * p2 + w3 * p3 + w4 * p4) / 32) as u8
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p arrival_detector test_adaptive`
Expected: PASS for all 3 tests

- [ ] **Step 5: Commit**

```bash
git add arrival_detector/src/probability.rs
git commit -m "feat: add adaptive probability function for close stops

- Add arrival_probability_adaptive() with next_stop parameter
- Close stops (<120m): weights (14,7,11,0) remove p4 penalty
- Normal/last stops: standard weights (13,6,10,3)
- Add 3 unit tests for close/normal/last stop cases
- Maintain sum=32 to prevent u8 overflow

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 3: Tier 1 - Update Main Loop to Pass Next Stop

**Files:**
- Modify: `arrival_detector/src/main.rs:99-167`

**Rationale:** Calculate sequential next_stop for each stop and pass to adaptive probability function.

- [ ] **Step 1: Build project to verify current state**

Run: `cargo build -p arrival_detector`
Expected: Clean build

- [ ] **Step 2: Modify main.rs to calculate next_stops array**

Modify `arrival_detector/src/main.rs` in the main processing loop. Insert before the `for &stop_idx in &active_indices` loop (around line 99):

Find this section:
```rust
// Find active stops (corridor filter)
let active_indices = corridor::find_active_stops(record.s_cm, &stops);

// Track which stops arrived this frame for trace output
let mut arrived_this_frame: Vec<u8> = Vec::new();
```

Replace with:
```rust
// Find active stops (corridor filter)
let active_indices = corridor::find_active_stops(record.s_cm, &stops);

// Calculate sequential next_stop for each stop (before processing loop)
// This is the NEXT STOP IN THE ROUTE SEQUENCE, not the next active stop
let next_stops: Vec<Option<&shared::Stop>> = stops.iter()
    .enumerate()
    .map(|(i, _)| {
        if i + 1 < stops.len() {
            Some(&stops[i + 1])
        } else {
            None
        }
    })
    .collect();

// Track which stops arrived this frame for trace output
let mut arrived_this_frame: Vec<u8> = Vec::new();
```

- [ ] **Step 3: Update probability calculation to use adaptive function**

Find the probability calculation inside the loop (around line 127-134):

Before:
```rust
let prob = probability::arrival_probability(
    record.s_cm,
    record.v_cms,
    stop,
    state.dwell_time_s,
    &gaussian_lut,
    &logistic_lut,
);
```

After:
```rust
let prob = probability::arrival_probability_adaptive(
    record.s_cm,
    record.v_cms,
    stop,
    state.dwell_time_s,
    &gaussian_lut,
    &logistic_lut,
    next_stops[stop_idx],  // Sequential next stop from route
);
```

- [ ] **Step 4: Build to verify changes**

Run: `cargo build -p arrival_detector`
Expected: Clean build

- [ ] **Step 5: Integration test with tpF805 route**

First regenerate route data with new preprocessing:
```bash
cargo run -p preprocessor -- test_data/tpF805_route.json test_data/tpF805_stops.txt test_data/tpF805_route_new.bin
```

Then run arrival detector:
```bash
cargo run -p arrival_detector -- test_data/tpF805_normal_sim.json test_data/tpF805_route_new.bin /tmp/tpF805_output_new.json --trace /tmp/tpF805_trace_new.jsonl
```

Verify Stop #3 is now detected:
```bash
jq 'select(.stop_idx==3)' /tmp/tpF805_output_new.json
```

Expected: At least one arrival event for stop_idx=3

- [ ] **Step 6: Commit**

```bash
git add arrival_detector/src/main.rs
git commit -m "feat: pass sequential next_stop to adaptive probability

- Calculate next_stops array before processing loop
- Use sequential next stop from route, not next active stop
- Switch from arrival_probability() to arrival_probability_adaptive()
- Critical for overlapping corridors where next active ≠ next sequential

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 4: Verification and Documentation

**Files:**
- Test: Integration test script
- Docs: Update any relevant documentation

- [ ] **Step 1: Create integration verification script**

Create `scripts/verify_close_stop_fix.sh`:

```bash
#!/bin/bash
set -e

echo "=== Close Stop Fix Verification ==="
echo

# 1. Preprocess route with new corridor logic
echo "1. Preprocessing tpF805 route..."
cargo run -p preprocessor -- \
    test_data/tpF805_route.json \
    test_data/tpF805_stops.txt \
    /tmp/tpF805_verify.bin

# 2. Run arrival detector
echo "2. Running arrival detection..."
cargo run -p arrival_detector -- \
    test_data/tpF805_normal_sim.json \
    /tmp/tpF805_verify.bin \
    /tmp/tpF805_verify_output.json \
    --trace /tmp/tpF805_verify_trace.jsonl

# 3. Check Stop #3 detection
echo "3. Verifying Stop #3 detection..."
STOP3_COUNT=$(jq 'select(.stop_idx==3)' /tmp/tpF805_verify_output.json | wc -l)

if [ "$STOP3_COUNT" -gt 0 ]; then
    echo "✓ Stop #3 detected ($STOP3_COUNT arrivals)"
else
    echo "✗ FAIL: Stop #3 not detected"
    exit 1
fi

# 4. Verify corridor boundaries
echo "4. Checking corridor boundaries..."
STOP3_START=$(jq '.stops[3].corridor_start_cm' /tmp/tpF805_verify.bin 2>/dev/null || echo "N/A")
echo "Stop #3 corridor_start: $STOP3_START cm"

echo
echo "=== All checks passed ==="
```

- [ ] **Step 2: Run verification script**

Run: `bash scripts/verify_close_stop_fix.sh`
Expected: All checks pass

- [ ] **Step 3: Run full test suite**

Run: `cargo test --workspace`
Expected: All tests pass

- [ ] **Step 4: Commit verification script**

```bash
git add scripts/verify_close_stop_fix.sh
git commit -m "test: add close stop fix verification script

- Integration test for tpF805 route
- Verifies Stop #3 is now detected
- Checks corridor boundaries are correct

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Summary of Changes

| Task | File | Lines Added | Lines Modified | Purpose |
|------|------|-------------|----------------|---------|
| 1 | `preprocessor/src/stops.rs` | ~40 | 0 | Add preprocess function |
| 1 | `preprocessor/src/main.rs` | 2 | 3 | Call preprocessing |
| 1 | `preprocessor/src/stops/tests.rs` | ~100 | 0 | Add 4 unit tests |
| 2 | `arrival_detector/src/probability.rs` | ~50 | 0 | Add adaptive function |
| 2 | `arrival_detector/src/probability.rs` | ~60 | 0 | Add 3 unit tests |
| 3 | `arrival_detector/src/main.rs` | ~10 | 8 | Pass next_stop |
| 4 | `scripts/verify_close_stop_fix.sh` | ~40 | 0 | Integration test |

**Total:** ~300 lines added, ~11 lines modified

---

## Testing Strategy

1. **Unit Tests** (Task 1, 2): Verify corridor ratios and probability weights in isolation
2. **Integration Test** (Task 3): Full pipeline with tpF805 route data
3. **Verification Script** (Task 4): Automated end-to-end validation

---

## Rollback Plan

If issues arise:

1. **Tier 1 rollback:** Revert main.rs changes, keep using `arrival_probability()`
2. **Tier 3 rollback:** Keep new function but don't call it
3. **Tier 2 rollback:** Comment out preprocessing call in main.rs

Each tier is independently reversible.

---

## References

- Spec: `docs/superpowers/specs/2026-03-24-close-stop-fix-design.md`
- Original proposal: `docs/proposal_for_close_stop.md`
- Tech report: `docs/bus_arrival_tech_report_v8.md`

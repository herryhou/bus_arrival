---
name: regression-test-save
description: This skill should be used when the user asks to "save regression test", "preserve failing test case", "promote detour bug to regression", or mentions needing to save/capture a specific bug found during testing for the bus arrival detection system. Provides workflow for preserving edge cases found during random testing as permanent regression tests.
version: 1.0.0
---

# Regression Test Preservation

Save and preserve failing test cases found during random detour testing as permanent regression tests for the bus arrival detection system.

## Purpose

Random testing (e.g., `make run-detour`) produces varying behaviors that may expose bugs. Once a bug is found, the exact NMEA input must be preserved to prevent losing the edge case when test data is regenerated. This skill provides a systematic workflow for saving failing cases as permanent regression tests.

## When to Use

Use this skill when:
- A bug is discovered during detour scenario testing
- Need to preserve exact NMEA input that triggered wrong behavior
- Want to create a regression test before fixing a bug
- Converting a one-time finding into a permanent test case

## Workflow Overview

The two-track testing strategy separates **fuzzing** (finding bugs) from **regression** (preventing regressions):

| Track | Purpose | Regenerable? |
|-------|---------|--------------|
| **Fuzzing** (`ty225_short_detour_nmea.txt`) | Find NEW bugs via random variation | ✅ Yes |
| **Regression** (`test_data/regression/`) | Preserve known bug cases | ❌ Never |

When `gen_nmea` produces new random detour data:
- May find NEW bugs → promote to regression
- May lose OLD cases → already preserved in regression/

## Step-by-Step Workflow

### Step 1: Discover the Bug

Run detour scenario repeatedly to observe behavior variations:

```bash
make run-detour
```

Analyze the output:
- Check `trace.jsonl` for unexpected state transitions
- Look for wrong stop indices or stuck progress values
- Identify crashes, hangs, or assertion failures

### Step 2: Save the Regression Case

Use the helper script to preserve the failing NMEA:

```bash
./tools/save_regression.sh <case-name> "<bug-description>"
```

Example:
```bash
./tools/save_regression.sh stop_skip_backtrack \
  "Progress stuck at stop 2 after detour re-entry to stop 6"
```

**What the script does:**
- Copies `ty225_short_detour_nmea.txt` → `test_data/regression/ty225_short_detour_<case>.txt`
- Copies `trace.jsonl` → `test_data/regression/ty225_short_detour_<case>_trace.jsonl`
- Prompts for bug details (root cause, fix location)
- Updates `test_data/regression/README.md` with documentation
- Creates test template in `crates/pipeline/tests/regression_tests.rs`

### Step 3: Document the Bug

The script will prompt for:

1. **Bug description** - What went wrong (one line)
2. **Root cause** - Why it happened
3. **Fix location** - Which file/function to fix

This information is stored in `test_data/regression/README.md` for future reference.

### Step 4: Implement Test Assertions

Edit `crates/pipeline/tests/regression_tests.rs` to add assertions:

```rust
#[test]
fn test_<case_name>() {
    // Bug: <description>
    // Root cause: <why>
    // Fix: <where>
    //
    // This test ensures the bug does not regress.

    let nmea_file = "../../../test_data/regression/ty225_short_detour_<case>.txt";
    let route_bin = "../../../test_data/ty225_short_detour.bin";

    // Load route data
    let route_bytes = fs::read(route_bin)
        .expect("Failed to load route data");
    let route_data = shared::binfile::RouteData::load(&route_bytes)
        .expect("Failed to parse route data");

    // Load and process NMEA...
    // Add assertions specific to the bug

    // Example assertion:
    assert!(progress_after_reentry > 170_000,
            "Progress should jump to stop 6 area, not stuck at stop 2");
}
```

### Step 5: Fix the Bug

Fix the bug in the source code. The test will initially fail, then pass after the fix.

### Step 6: Verify the Fix

Run the regression test to confirm the bug is fixed:

```bash
cargo test -p pipeline --test regression_tests -- test_<case_name>
```

Or run all regression tests:
```bash
make regression-test
```

## Directory Structure

```
test_data/
├── regression/                    # Preserved NMEA files (never regenerated)
│   ├── README.md                  # Documents each bug case
│   ├── ty225_short_detour_<case1>.txt
│   ├── ty225_short_detour_<case1>_trace.jsonl
│   └── ty225_short_detour_<case2>.txt
└── expected/                      # Expected outputs (optional)
    ├── <case>_arrivals.jsonl
    └── <case>_trace.jsonl

tools/
└── save_regression.sh             # Helper script

crates/pipeline/tests/
└── regression_tests.rs            # Regression test suite
```

## Makefile Targets

Convenient targets for regression testing:

```bash
# Run all regression tests
make regression-test

# Save a regression case (alternative to running script directly)
make regression-save CASE_NAME=<name> DESC="<description>"
```

## Expected Outputs (Optional)

For tests that validate exact output behavior:

```bash
# Run pipeline with regression NMEA
cargo run --release -p pipeline -- \
    test_data/regression/ty225_short_detour_<case>.txt \
    test_data/ty225_short_detour.bin \
    test_data/regression/temp_output.jsonl \
    --trace test_data/regression/temp_trace.jsonl

# Verify output is correct, then save as expected
mv test_data/regression/temp_output.jsonl \
   test_data/expected/<case>_arrivals.jsonl
mv test_data/regression/temp_trace.jsonl \
   test_data/expected/<case>_trace.jsonl
```

**Note:** Many regression tests only validate internal state (progress, stop_idx) and don't need expected outputs.

## Best Practices

1. **Descriptive naming:** Use `<route>_<scenario>_<bug-description>.txt`
   - Example: `ty225_short_detour_stop_skip_backtrack.txt`

2. **Document thoroughly:** Include root cause and fix location in README

3. **One bug per test:** Each regression test should verify one specific bug

4. **Independent tests:** Tests should not depend on each other

5. **Commit with fix:** Commit the regression test and bug fix together

## Common Patterns

### Pattern 1: Progress Stuck After Re-entry

```rust
// Bug: Progress remained at ~1072m (stop 2) instead of jumping to ~1775m (stop 6)
let progress_after_reentry = state.last_valid_s_cm();
assert!(progress_after_reentry > 170_000,
        "Progress should advance to stop 6 area after detour re-entry");
```

### Pattern 2: Wrong Stop Detection

```rust
// Bug: Detected stop 2 instead of stop 6 after detour
let detected_stop = state.last_known_stop_index();
assert_eq!(detected_stop, 6,
           "Should detect stop 6 after re-entry, not stop 2");
```

### Pattern 3: State Machine Transitions

```rust
// Bug: System stuck in OffRoute mode instead of recovering
let mode = state.system_mode();
assert_eq!(mode, SystemMode::Normal,
           "Should recover to Normal mode after valid GPS on route");
```

## Additional Resources

### Reference Files

- **`references/README-templates.md`** - README documentation templates
- **`references/test-patterns.md`** - Common assertion patterns

### Examples

- **`examples/stop-skip-test.rs`** - Complete regression test example

### Scripts

- **`scripts/save_regression.sh`** - Helper script for saving cases

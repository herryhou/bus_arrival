# Regression Tests

This directory contains **saved NMEA files** that exposed bugs. These are **permanent** — they preserve exact edge cases found during testing.

## Purpose

When `make run-detour` produces random behaviors and you find a bug:
1. **Save the NMEA** here with a descriptive name
2. **Document the bug** in this README
3. **Write a test** that validates the fix
4. **Never regenerate** — these are permanent fixtures

## File Naming

Use descriptive names: `<route>_<scenario>_<bug-description>.txt`

Example: `ty225_short_detour_stop_skip_backtrack.txt`

## Current Regression Cases

### stop6_missed_at_reentry

**File:** `ty225_short_detour_stop6_missed_at_reentry.txt`
**Bug:** Stop 6 should be detected with 8s dwell at detour re-entry but is currently missed
**Root Cause:** Bus moves too fast through stop 6's corridor after detour re-entry snap, passing through without triggering dwell detection
**Fix:** detection/src/arrival_detector.rs - arrival detection logic needs to handle re-entry dwell
**Test:** `test_stop6_missed_at_reentry()`
**Added:** 2026-05-04

**Expected Behavior (from ground truth):**
- At tick 80143, off-route ends and position snaps to ~175107 cm (near stop 6)
- Stop 6 should be detected with 8 seconds of dwell at re-acquisition
- Stop 6 appears in arrivals with `dwell_s: 8`

**Current (Bug) Behavior:**
- Position snaps correctly to ~175107 cm
- Stop 6 is announced (corridor entry) but NOT detected as an arrival
- Bus passes through stop 6's corridor too fast (v_cms=1210 at re-entry)
- No dwell is detected, FSM never reaches "AtStop" state

---

## Workflow

```bash
# 1. Run detour scenario until you find a bug
make run-detour

# 2. When you see wrong behavior, save the NMEA
cp test_data/ty225_short_detour_nmea.txt test_data/regression/ty225_short_detour_<description>.txt
# Save the trace too for documentation
cp test_data/ty225_short_detour_trace.jsonl test_data/regression/ty225_short_detour_<description>_trace.jsonl

# 3. Document the bug in this README (under "Current Regression Cases")

# 4. Write a test in crates/pipeline/tests/regression_tests.rs
#    (See that file for the test template)

# 5. Fix the bug in code

# 6. Verify test passes: cargo test -p pipeline --test regression_tests
```

## Two-Track Strategy

| Track | Purpose | Regenerable? |
|-------|---------|--------------|
| **Regression** (this dir) | Preserved bug cases | ❌ Never |
| **Fuzzing** (detour_nmea.txt) | Find NEW bugs | ✅ Yes |

When gen_nmea produces new detour data:
- It might find NEW bugs → add to regression
- It might lose OLD bugs → already preserved in regression/

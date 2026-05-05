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

### stop5_skip_on_reentry

**File:** `ty225_short_detour_stop6_missed_at_reentry.txt`
**Bug:** Stop 5 should be SKIPPED on re-entry from off-route
**Root Cause:** The snap progress on re-entry is near stop 5, but the snap point is located between stop 5 and stop 6, so stop 5 should be skipped
**Fix:** detection/src/arrival_detector.rs - arrival detection logic needs to skip stops when snap point is past them on re-entry
**Test:** `test_skip_stop5_on_offroute_reentry()`
**Added:** 2026-05-04

**Expected Behavior:**
- At tick 80143, off-route ends and position snaps to ~175107 cm (between stop 5 and stop 6)
- Stop 5 is SKIPPED (snap point is past it)
- Stop 6 is detected (Approaching/Arriving states, though may not reach AtStop)
- Detected stops: [0, 1, 6, 7, 8, 9]

**Trace Data:**
- `ty225_short_detour_stop6_missed_at_reentry_trace.jsonl` — Full state machine trace for reference

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

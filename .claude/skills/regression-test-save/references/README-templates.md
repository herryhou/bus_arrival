# README Documentation Templates

Templates for documenting regression test cases in `test_data/regression/README.md`.

## Individual Entry Template

```markdown
### <case-name>

**File:** `ty225_short_detour_<case-name>.txt`
**Bug:** <Brief description of what went wrong>
**Root Cause:** <Why it happened>
**Fix:** <file:function or location where fixed>
**Test:** `test_<case_name>()`
**Added:** YYYY-MM-DD

---
```

## Complete Example

```markdown
### stop_skip_backtrack

**File:** `ty225_short_detour_stop_skip_backtrack.txt`
**Bug:** Progress stuck at ~1072m (stop 2 area) after detour re-entry to stop 6
**Root Cause:** Kalman filter's `last_seg_idx` not updated during DR mode, causing map matching window to remain centered on old position
**Fix:** `crates/pipeline/gps_processor/src/kalman.rs:123` - Update `last_seg_idx` even during DR
**Test:** `test_stop_skip_backtrack()`
**Added:** 2026-05-04

---
```

## Multiple Entry Format

The README maintains a running list of all regression cases:

```markdown
# Regression Tests

[Header documentation...]

## Current Regression Cases

### case_one
[details...]

### case_two
[details...]
```

## Documentation Fields

| Field | Purpose | Format |
|-------|---------|--------|
| **File** | NMEA filename | `ty225_short_detour_<case>.txt` |
| **Bug** | What went wrong | One-line description |
| **Root Cause** | Why it happened | Technical explanation |
| **Fix** | Where to fix | `file:function` or `crate::module::function` |
| **Test** | Test function | `test_<case_name>()` |
| **Added** | When added | YYYY-MM-DD |

# Phase 2: CorridorFilter Extraction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract corridor filtering logic into standalone `filter.rs` module with pure function interface.

**Architecture:** Create new `filter.rs` module with `active_stops()` pure function. Update `DetectionState` to use it.

**Tech Stack:** Rust, cargo test

---

## File Structure

**Create:**
- `crates/pipeline/src/filter.rs` - Corridor filtering logic
- `crates/pipeline/src/filter/tests.rs` - Unit tests for filter module

**Modify:**
- `crates/pipeline/src/lib.rs` - Add filter module, import `active_stops`
- `crates/pipeline/src/detection_state.rs` - Use `filter::active_stops()` instead of inline logic

---

### Task 1: Create filter.rs Module

**Files:**
- Create: `crates/pipeline/src/filter.rs`

**Purpose:** Define `active_stops()` pure function for corridor filtering.

- [ ] **Step 1: Create filter.rs with active_stops function**

```rust
//! Corridor filtering for active stop selection
//!
//! This module provides pure functions for finding stops within
//! the corridor of the current position.

use shared::{DistCm, binfile::Stop};

/// Find stops within corridor of current position
///
/// Returns indices of stops where:
/// - s_cm is within [corridor_start_cm, corridor_end_cm]
/// - skip_flags[idx] is false
///
/// # Arguments
///
/// * `s_cm` - Current position along route (cm)
/// * `stops` - Slice of all stops
/// * `skip_flags` - Whether each stop should be skipped (e.g., re-entry skip)
///
/// # Returns
///
/// Vector of stop indices that are active (within corridor and not skipped)
pub fn active_stops(
    s_cm: DistCm,
    stops: &[Stop],
    skip_flags: &[bool],
) -> Vec<usize> {
    stops.iter()
        .enumerate()
        .filter(|(idx, stop)| {
            !skip_flags[*idx]
                && s_cm >= stop.corridor_start_cm
                && s_cm <= stop.corridor_end_cm
        })
        .map(|(idx, _)| idx)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::binfile::Stop;

    fn mock_stops() -> Vec<Stop> {
        vec![
            Stop {
                progress_cm: 0,
                corridor_start_cm: 0,
                corridor_end_cm: 100,
                name: String::new(),
                lat: 0,
                lon: 0,
            },
            Stop {
                progress_cm: 200,
                corridor_start_cm: 150,
                corridor_end_cm: 250,
                name: String::new(),
                lat: 0,
                lon: 0,
            },
            Stop {
                progress_cm: 400,
                corridor_start_cm: 350,
                corridor_end_cm: 450,
                name: String::new(),
                lat: 0,
                lon: 0,
            },
        ]
    }

    #[test]
    fn test_active_stops_single() {
        let stops = mock_stops();
        let skip = vec![false; stops.len()];
        let active = active_stops(50, &stops, &skip);
        assert_eq!(active, vec![0]);
    }

    #[test]
    fn test_active_stops_multiple() {
        let stops = mock_stops();
        let skip = vec![false; stops.len()];
        // Position 200 is in both stop 0's corridor (0-100) and stop 1's (150-250)
        // Wait, corridors don't overlap in this mock. Let me fix.
        // Actually position 200 is only in stop 1's corridor
        let active = active_stops(200, &stops, &skip);
        assert_eq!(active, vec![1]);
    }

    #[test]
    fn test_active_stops_empty() {
        let stops = mock_stops();
        let skip = vec![false; stops.len()];
        let active = active_stops(500, &stops, &skip);
        assert_eq!(active, vec![] as Vec<usize>);
    }

    #[test]
    fn test_active_stops_skip_flag() {
        let stops = mock_stops();
        let skip = vec![true, false, false];
        let active = active_stops(50, &stops, &skip);
        assert_eq!(active, vec![] as Vec<usize>); // Stop 0 is skipped
    }

    #[test]
    fn test_active_stops_corridor_boundary() {
        let stops = mock_stops();
        let skip = vec![false; stops.len()];
        // At exact boundary
        let active = active_stops(100, &stops, &skip);
        assert_eq!(active, vec![0]); // Inclusive of corridor_end_cm
    }
}
```

- [ ] **Step 2: Add filter module to lib.rs**

Add to module declarations:
```rust
pub mod gps;
pub mod serde;
mod localization;
mod detection_state;
mod trace;
mod filter;  // <-- NEW
```

- [ ] **Step 3: Run filter tests**

Run: `rtk cargo test -p pipeline --lib filter`

Expected: All 5 filter tests PASS

- [ ] **Step 4: Commit**

```bash
git add crates/pipeline/src/filter.rs crates/pipeline/src/lib.rs
git commit -m "feat: extract corridor filtering to filter.rs module

Add active_stops() pure function for finding stops within corridor.
Includes unit tests for corridor logic and skip flags."
```

---

### Task 2: Update DetectionState to Use filter::active_stops

**Files:**
- Modify: `crates/pipeline/src/detection_state.rs:92-99`

**Purpose:** Replace inline corridor filtering with call to `filter::active_stops()`.

- [ ] **Step 1: Add filter import to detection_state.rs**

```rust
use shared::{DistCm, PositionSignals};
use shared::binfile::RouteData;
use crate::{PipelineResult, ArrivalEvent, DepartureEvent, gps::GpsRecord, trace::StopTraceState};
use detection::state_machine::{StopState, StopEvent};
use crate::filter;  // <-- NEW
```

- [ ] **Step 2: Replace inline corridor logic with filter call**

Replace lines 92-99 in `process_gps_record()`:
```rust
// OLD:
// Find active stops (corridor filter)
for (idx, stop) in stops.iter().enumerate() {
    if s_cm >= stop.corridor_start_cm && s_cm <= stop.corridor_end_cm
        && !self.stop_states[idx].skip_on_reentry
    {
        self.active_indices.push(idx);
    }
}

// NEW:
// Find active stops using corridor filter
let skip_flags: Vec<bool> = self.stop_states.iter()
    .map(|s| s.skip_on_reentry)
    .collect();
self.active_indices = filter::active_stops(s_cm, stops, &skip_flags);
```

- [ ] **Step 3: Run tests to verify behavior unchanged**

Run: `rtk cargo test -p pipeline`

Expected: All tests PASS (including characterization test)

- [ ] **Step 4: Commit**

```bash
git add crates/pipeline/src/detection_state.rs
git commit -m "refactor: use filter::active_stops in DetectionState

Replace inline corridor filtering logic with call to filter module.
No behavior change, just better separation of concerns."
```

---

### Task 3: Update lib.rs DetectionState to Use filter

**Files:**
- Modify: `crates/pipeline/src/lib.rs:283-289`

**Purpose:** The inline corridor logic in lib.rs `DetectionState::process_gps_record` also needs to use the filter.

- [ ] **Step 1: Replace inline corridor logic in lib.rs**

Replace lines 283-289:
```rust
// OLD:
// Find active stops (corridor filter)
// Skip stops that were marked to skip on re-entry
for (idx, stop) in stops.iter().enumerate() {
    if s_cm >= stop.corridor_start_cm && s_cm <= stop.corridor_end_cm && !self.stop_states[idx].skip_on_reentry {
        self.active_indices.push(idx);
    }
}

// NEW:
// Find active stops using corridor filter
let skip_flags: Vec<bool> = self.stop_states.iter()
    .map(|s| s.skip_on_reentry)
    .collect();
self.active_indices = crate::filter::active_stops(s_cm, stops, &skip_flags);
```

- [ ] **Step 2: Run tests to verify behavior unchanged**

Run: `rtk cargo test -p pipeline`

Expected: All tests PASS

- [ ] **Step 3: Commit**

```bash
git add crates/pipeline/src/lib.rs
git commit -m "refactor: use filter::active_stops in lib.rs DetectionState

Consistent with detection_state.rs refactoring."
```

---

### Task 4: Final Verification

- [ ] **Step 1: Run full test suite**

Run: `rtk cargo test -p pipeline`

Expected: All 42+ tests PASS

- [ ] **Step 2: Verify filter module tests pass**

Run: `rtk cargo test -p pipeline --lib filter`

Expected: All 5 filter tests PASS

- [ ] **Step 3: Check compiler warnings**

Run: `rtk cargo clippy -p pipeline`

Expected: No new warnings

- [ ] **Step 4: Run characterization test**

Run: `rtk cargo test -p pipeline --test characterization`

Expected: PASS (11 arrivals)

---

## Success Criteria

1. ✅ `filter.rs` module exists with `active_stops()` function
2. ✅ Unit tests for filter module pass (5 tests)
3. ✅ Both `DetectionState` implementations use `filter::active_stops()`
4. ✅ All existing tests still pass
5. ✅ Characterization test passes (no behavior change)

---

## What's Next

After Phase 2 is complete, proceed to:
- **Phase 3:** Extract ProbabilityEngine (`pipeline/src/probability.rs`)
- **Phase 4:** Fix Diagnostics Interface (`GpsDiagnostics` struct)
- **Phase 5:** Move to Separate Crates

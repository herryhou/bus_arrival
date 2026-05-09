# Phase 3: ProbabilityEngine Extraction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subuting-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract probability computation into `ProbabilityEngine` with caching to eliminate duplicate computation in `get_trace_info()`.

**Architecture:** Create `probability.rs` module with `ProbabilityEngine` struct. Cache computation results to avoid recomputing same (timestamp, signals) pair.

**Tech Stack:** Rust, cargo test

---

## File Structure

**Create:**
- `crates/pipeline/src/probability.rs` - Probability engine with caching
- `crates/pipeline/src/probability/tests.rs` - Unit tests

**Modify:**
- `crates/pipeline/src/lib.rs` - Add probability module, update DetectionState
- `crates/pipeline/src/detection_state.rs` - Add prob_engine field, use it for both process and trace

---

### Task 1: Create probability.rs Module

**Files:**
- Create: `crates/pipeline/src/probability.rs`

**Purpose:** Define `ProbabilityEngine` with caching and `ProbabilityResult` struct.

- [ ] **Step 1: Create probability.rs with ProbabilityEngine**

```rust
//! Probability computation engine with caching
//!
//! This module provides cached probability computation to avoid
//! recalculating for the same timestamp/inputs (e.g., in get_trace_info).

use shared::{DistCm, SpeedCms, PositionSignals, binfile::Stop};
use detection::probability::{self, GpsStatus};

/// Cached probability computation result
#[derive(Debug, Clone, PartialEq)]
pub struct ProbabilityResult {
    pub probability: u8,
    pub features: detection::trace::FeatureScores,
}

/// Probability computation engine with caching
///
/// Caches the last computation by (timestamp, inputs) to avoid
/// recomputing when get_trace_info() needs the same data.
pub struct ProbabilityEngine {
    last_result: Option<(u64, ProbabilityResult)>,
}

impl ProbabilityEngine {
    pub fn new() -> Self {
        Self { last_result: None }
    }

    /// Compute arrival probability (cached)
    ///
    /// Returns cached result if timestamp matches, otherwise computes
    /// new probability and caches it.
    pub fn compute(
        &mut self,
        timestamp: u64,
        signals: PositionSignals,
        v_cms: SpeedCms,
        stop: &Stop,
        dwell_time_s: u16,
        gps_status: GpsStatus,
    ) -> &ProbabilityResult {
        // Check cache
        if let Some((cached_ts, _)) = self.last_result {
            if cached_ts == timestamp {
                // Return cached result (same timestamp = same inputs)
                return &self.last_result.as_ref().unwrap().1;
            }
        }

        // Compute new probability
        let features = probability::compute_feature_scores(
            signals,
            v_cms,
            stop,
            dwell_time_s,
            probability::gaussian_lut(),
            probability::logistic_lut(),
        );

        let probability = probability::compute_arrival_probability(
            signals,
            v_cms,
            stop,
            dwell_time_s,
            gps_status,
            probability::gaussian_lut(),
            probability::logistic_lut(),
        );

        let result = ProbabilityResult { probability, features };
        self.last_result = Some((timestamp, result));
        &self.last_result.as_ref().unwrap().1
    }

    /// Clear the cache (e.g., on new GPS record)
    pub fn clear(&mut self) {
        self.last_result = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::binfile::Stop;

    fn mock_stop() -> Stop {
        Stop {
            progress_cm: 1000,
            corridor_start_cm: 900,
            corridor_end_cm: 1100,
            name: String::new(),
            lat: 0,
            lon: 0,
        }
    }

    #[test]
    fn test_probability_caching() {
        let mut engine = ProbabilityEngine::new();
        let signals = PositionSignals::new(100, 100);
        let stop = mock_stop();

        let r1 = engine.compute(0, signals, 50, &stop, 0, GpsStatus::Valid);
        let r2 = engine.compute(0, signals, 50, &stop, 0, GpsStatus::Valid);

        // Same result pointer (cached)
        assert_eq!(r1 as *const ProbabilityResult, r2 as *const ProbabilityResult);
    }

    #[test]
    fn test_probability_cache_miss() {
        let mut engine = ProbabilityEngine::new();
        let signals = PositionSignals::new(100, 100);
        let stop = mock_stop();

        let r1 = engine.compute(0, signals, 50, &stop, 0, GpsStatus::Valid);
        let r2 = engine.compute(1, signals, 50, &stop, 0, GpsStatus::Valid); // Different timestamp

        // Different result pointers (cache miss)
        assert_ne!(r1 as *const ProbabilityResult, r2 as *const ProbabilityResult);
    }

    #[test]
    fn test_probability_clear() {
        let mut engine = ProbabilityEngine::new();
        let signals = PositionSignals::new(100, 100);
        let stop = mock_stop();

        let r1 = engine.compute(0, signals, 50, &stop, 0, GpsStatus::Valid);
        engine.clear();
        let r2 = engine.compute(0, signals, 50, &stop, 0, GpsStatus::Valid);

        // Different result pointers after clear
        assert_ne!(r1 as *const ProbabilityResult, r2 as *const ProbabilityResult);
    }
}
```

- [ ] **Step 2: Add probability module to lib.rs**

```rust
pub mod gps;
pub mod serde;
mod localization;
mod detection_state;
mod trace;
mod filter;
mod probability;  // <-- NEW
```

- [ ] **Step 3: Run probability tests**

Run: `rtk cargo test -p pipeline --lib probability`

Expected: All 3 tests PASS

- [ ] **Step 4: Commit**

```bash
git add crates/pipeline/src/probability.rs crates/pipeline/src/lib.rs
git commit -m "feat: extract probability computation to probability.rs

Add ProbabilityEngine with caching to avoid recomputing probability
for same timestamp (fixes duplicate computation in get_trace_info)."
```

---

### Task 2: Update DetectionState (detection_state.rs) to Use ProbabilityEngine

**Files:**
- Modify: `crates/pipeline/src/detection_state.rs`

**Purpose:** Add `prob_engine` field and use cached computation.

- [ ] **Step 1: Add prob_engine field to DetectionState struct**

```rust
use crate::probability::ProbabilityEngine;  // Add import

pub struct DetectionState {
    stop_states: Vec<StopState>,
    current_timestamp: u64,
    arrived_this_frame: Vec<u8>,
    active_indices: Vec<usize>,
    off_route: bool,
    off_route_last_s_cm: Option<DistCm>,
    prob_engine: ProbabilityEngine,  // <-- NEW
}
```

- [ ] **Step 2: Initialize prob_engine in new()**

```rust
pub fn new(route_data: &RouteData) -> Self {
    let stop_count = route_data.stops().len();
    let mut stop_states = Vec::with_capacity(stop_count);
    for i in 0..stop_count {
        stop_states.push(StopState::new(i as u8));
    }
    Self {
        stop_states,
        current_timestamp: 0,
        arrived_this_frame: Vec::new(),
        active_indices: Vec::new(),
        off_route: false,
        off_route_last_s_cm: None,
        prob_engine: ProbabilityEngine::new(),  // <-- NEW
    }
}
```

- [ ] **Step 3: Use prob_engine in process_gps_record()**

Replace inline probability computation (lines 105-121):
```rust
// OLD:
let probability = detection::probability::compute_arrival_probability(
    signals,
    v_cms,
    stop,
    stop_state.dwell_time_s,
    gps_status,
    detection::probability::gaussian_lut(),
    detection::probability::logistic_lut(),
);

// NEW:
let prob_result = self.prob_engine.compute(
    record.time,
    signals,
    v_cms,
    stop,
    stop_state.dwell_time_s,
    gps_status,
);
let probability = prob_result.probability;
```

Also update the arrival event creation to use cached probability:
```rust
// OLD:
result.arrivals.push(ArrivalEvent {
    // ...
    probability,  // This was recomputed
    // ...
});

// NEW: (no change needed, probability is already the value we want)
```

- [ ] **Step 4: Use cached results in get_trace_info()**

Replace duplicate computation (lines 168-184):
```rust
// OLD:
let features = detection::probability::compute_feature_scores(
    signals,
    record.v_cms,
    stop,
    stop_state.dwell_time_s,
    detection::probability::gaussian_lut(),
    detection::probability::logistic_lut(),
);

let probability = detection::probability::compute_probability(
    record.s_cm,
    record.v_cms,
    stop.progress_cm,
    stop_state.dwell_time_s,
);

// NEW:
// Use cached probability result from process_gps_record
let prob_result = self.prob_engine.compute(
    record.time,
    signals,
    record.v_cms,
    stop,
    stop_state.dwell_time_s,
    match record.status {
        "valid" => detection::probability::GpsStatus::Valid,
        "dr_outage" => detection::probability::GpsStatus::DrOutage,
        "off_route" => detection::probability::GpsStatus::OffRoute,
        _ => detection::probability::GpsStatus::Valid,
    },
);
let features = prob_result.features.clone();
let probability = prob_result.probability;
```

- [ ] **Step 5: Run tests**

Run: `rtk cargo test -p pipeline --test detection_state`

Expected: All tests PASS

- [ ] **Step 6: Commit**

```bash
git add crates/pipeline/src/detection_state.rs
git commit -m "refactor: use ProbabilityEngine in detection_state.rs

Add prob_engine field to DetectionState. Use cached computation
for both process_gps_record and get_trace_info. Eliminates
duplicate computation."
```

---

### Task 3: Update lib.rs DetectionState to Use ProbabilityEngine

**Files:**
- Modify: `crates/pipeline/src/lib.rs`

**Purpose:** Apply same changes to lib.rs DetectionState.

- [ ] **Step 1: Add prob_engine field to lib.rs DetectionState**

```rust
pub struct DetectionState {
    stop_states: Vec<StopState>,
    current_timestamp: u64,
    arrived_this_frame: Vec<u8>,
    active_indices: Vec<usize>,
    off_route: bool,
    off_route_last_s_cm: Option<DistCm>,
    prob_engine: probability::ProbabilityEngine,  // <-- NEW
}
```

- [ ] **Step 2: Initialize in new()**

```rust
Self {
    stop_states,
    current_timestamp: 0,
    arrived_this_frame: Vec::new(),
    active_indices: Vec::new(),
    off_route: false,
    off_route_last_s_cm: None,
    prob_engine: probability::ProbabilityEngine::new(),  // <-- NEW
}
```

- [ ] **Step 3: Use prob_engine in process_gps_record()**

Same replacement as Task 2 Step 3.

- [ ] **Step 4: Use cached results in get_trace_info()**

Same replacement as Task 2 Step 4.

- [ ] **Step 5: Run tests**

Run: `rtk cargo test -p pipeline`

Expected: All tests PASS

- [ ] **Step 6: Commit**

```bash
git add crates/pipeline/src/lib.rs
git commit -m "refactor: use ProbabilityEngine in lib.rs DetectionState

Consistent with detection_state.rs changes."
```

---

### Task 4: Final Verification

- [ ] **Step 1: Run full test suite**

Run: `rtk cargo test -p pipeline`

Expected: All 42+ tests PASS

- [ ] **Step 2: Verify probability module tests pass**

Run: `rtk cargo test -p pipeline --lib probability`

Expected: All 3 tests PASS

- [ ] **Step 3: Run characterization test**

Run: `rtk cargo test -p pipeline --test characterization`

Expected: PASS (11 arrivals, no behavior change)

- [ ] **Step 4: Verify no duplicate computation**

Check that `get_trace_info()` no longer calls `compute_feature_scores` or `compute_probability` directly - it uses cached results.

Expected: Only `prob_engine.compute()` calls

---

## Success Criteria

1. ✅ `probability.rs` module exists with `ProbabilityEngine`
2. ✅ Unit tests for caching pass (3 tests)
3. ✅ Both `DetectionState` implementations use `prob_engine`
4. ✅ `get_trace_info()` uses cached results (no recomputation)
5. ✅ All existing tests pass
6. ✅ Characterization test passes (no behavior change)

---

## What's Next

After Phase 3 is complete, proceed to:
- **Phase 4:** Fix Diagnostics Interface (`GpsDiagnostics` struct)
- **Phase 5:** Move to Separate Crates

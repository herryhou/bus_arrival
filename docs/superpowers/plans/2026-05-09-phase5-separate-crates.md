# Phase 5: Separate Crates Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract `filter` and `probability` modules into separate crates (`pipeline/filter` and `pipeline/probability`) for firmware readiness.

**Architecture:** Create two new crates under `crates/pipeline/`. Move code from modules to crates. Update imports in `pipeline` crate.

**Tech Stack:** Rust, cargo, workspace configuration

---

## File Structure

**Create:**
- `crates/pipeline/filter/Cargo.toml` - Filter crate manifest
- `crates/pipeline/filter/src/lib.rs` - Filter crate (moved from pipeline/src/filter.rs)
- `crates/pipeline/probability/Cargo.toml` - Probability crate manifest
- `crates/pipeline/probability/src/lib.rs` - Probability crate (moved from pipeline/src/probability.rs)

**Modify:**
- `crates/pipeline/Cargo.toml` - Add filter and probability as dependencies
- `crates/pipeline/src/lib.rs` - Update imports to use external crates
- `crates/pipeline/src/detection_state.rs` - Update imports
- `Cargo.toml` - Add new workspace members (optional, if not using path wildcard)

**Delete:**
- `crates/pipeline/src/filter.rs` - Moved to separate crate
- `crates/pipeline/src/probability.rs` - Moved to separate crate

---

### Task 1: Create pipeline/filter Crate

**Files:**
- Create: `crates/pipeline/filter/Cargo.toml`
- Create: `crates/pipeline/filter/src/lib.rs`

**Purpose:** Extract filter module into standalone crate.

- [ ] **Step 1: Create filter/Cargo.toml**

```toml
[package]
name = "pipeline-filter"
version = "0.1.0"
edition = "2024"

[dependencies]
shared = { path = "../../shared" }

[dev-dependencies]
shared = { path = "../../shared", features = ["std"] }
```

- [ ] **Step 2: Read current filter.rs**

Run: `rtk read crates/pipeline/src/filter.rs`

Copy the entire content.

- [ ] **Step 3: Create filter/src/lib.rs**

Paste the filter.rs content. Ensure it compiles as a crate root:

```rust
//! Corridor filtering for active stop selection
//!
//! This crate provides pure functions for finding stops within
//! the corridor of the current position.
//!
//! # Features
//!
//! - `std`: Enable std library (default: no_std for firmware)
//!
//! # Example
//!
//! ```rust
//! use pipeline_filter::active_stops;
//! use shared::{DistCm, binfile::Stop};
//!
//! let stops = vec![/* ... */];
//! let skip = vec![false; stops.len()];
//! let active = active_stops(5000, &stops, &skip);
//! ```

#![no_std]

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
    // ... (keep existing tests)
}
```

- [ ] **Step 4: Build filter crate**

Run: `rtk cargo build -p pipeline-filter`

Expected: Builds successfully

- [ ] **Step 5: Test filter crate**

Run: `rtk cargo test -p pipeline-filter`

Expected: All 5 tests PASS

- [ ] **Step 6: Commit**

```bash
git add crates/pipeline/filter/
git commit -m "feat: create pipeline-filter crate

Extract filter module into standalone crate for firmware extraction.
Pure functions, no_std compatible."
```

---

### Task 2: Create pipeline/probability Crate

**Files:**
- Create: `crates/pipeline/probability/Cargo.toml`
- Create: `crates/pipeline/probability/src/lib.rs`

**Purpose:** Extract probability module into standalone crate.

- [ ] **Step 1: Create probability/Cargo.toml**

```toml
[package]
name = "pipeline-probability"
version = "0.1.0"
edition = "2024"

[dependencies]
shared = { path = "../../shared" }
detection = { path = "../../detection" }

[dev-dependencies]
shared = { path = "../../shared", features = ["std"] }
detection = { path = "../../detection", features = ["std"] }
```

- [ ] **Step 2: Read current probability.rs**

Run: `rtk read crates/pipeline/src/probability.rs`

Copy the entire content.

- [ ] **Step 3: Create probability/src/lib.rs**

Paste the probability.rs content. Ensure it compiles as a crate root:

```rust
//! Probability computation engine with caching
//!
//! This crate provides cached probability computation to avoid
//! recalculating for the same timestamp/inputs.
//!
//! # Features
//!
//! - `std`: Enable std library (default: no_std for firmware)
//!
//! # Example
//!
//! ```rust
//! use pipeline_probability::{ProbabilityEngine, GpsDiagnostics};
//! use shared::PositionSignals;
//!
//! let mut engine = ProbabilityEngine::new();
//! let result = engine.compute(timestamp, signals, v_cms, &stop, dwell, gps_status);
//! ```

#![no_std]

use shared::{DistCm, SpeedCms, PositionSignals, binfile::Stop};
use detection::probability::{self, GpsStatus};

// ... (rest of probability code)

#[cfg(test)]
mod tests {
    // ... (keep existing tests)
}
```

- [ ] **Step 4: Build probability crate**

Run: `rtk cargo build -p pipeline-probability`

Expected: Builds successfully

- [ ] **Step 5: Test probability crate**

Run: `rtk cargo test -p pipeline-probability`

Expected: All 3 tests PASS

- [ ] **Step 6: Commit**

```bash
git add crates/pipeline/probability/
git commit -m "feat: create pipeline-probability crate

Extract probability module into standalone crate for firmware extraction.
Cached computation engine, no_std compatible."
```

---

### Task 3: Update Pipeline Crate to Use External Crates

**Files:**
- Modify: `crates/pipeline/Cargo.toml`
- Modify: `crates/pipeline/src/lib.rs`
- Modify: `crates/pipeline/src/detection_state.rs`

**Purpose:** Remove local modules, use external crate dependencies.

- [ ] **Step 1: Update pipeline/Cargo.toml**

Add dependencies:
```toml
[dependencies]
shared = { path = "../shared" }
detection = { path = "../detection" }
gps_processor = { path = "../gps_processor" }
pipeline-filter = { path = "filter" }
pipeline-probability = { path = "probability" }
thiserror = "0.1"

[dev-dependencies]
shared = { path = "../shared", features = ["std"] }
detection = { path = "../detection", features = ["std"] }
gps_processor = { path = "../gps_processor", features = ["std"] }
```

- [ ] **Step 2: Update lib.rs module declarations**

```rust
// OLD:
mod filter;
mod probability;

// NEW: (remove these lines, they're now external crates)
```

- [ ] **Step 3: Update lib.rs imports**

```rust
// OLD:
use crate::filter;
use crate::probability::ProbabilityEngine;

// NEW:
use pipeline_filter;
use pipeline_probability::ProbabilityEngine;
```

- [ ] **Step 4: Update detection_state.rs imports**

```rust
// OLD:
use crate::filter;
use crate::probability::ProbabilityEngine;

// NEW:
use pipeline_filter;
use pipeline_probability::ProbabilityEngine;
```

Also update function calls:
```rust
// OLD:
self.active_indices = filter::active_stops(s_cm, stops, &skip_flags);

// NEW:
self.active_indices = pipeline_filter::active_stops(s_cm, stops, &skip_flags);
```

- [ ] **Step 5: Delete old module files**

```bash
rm crates/pipeline/src/filter.rs
rm crates/pipeline/src/probability.rs
```

- [ ] **Step 6: Build pipeline crate**

Run: `rtk cargo build -p pipeline`

Expected: Builds successfully with external crates

- [ ] **Step 7: Test pipeline crate**

Run: `rtk cargo test -p pipeline`

Expected: All 42+ tests PASS

- [ ] **Step 8: Commit**

```bash
git add crates/pipeline/Cargo.toml crates/pipeline/src/lib.rs crates/pipeline/src/detection_state.rs
git commit -m "refactor: use external pipeline-filter and pipeline-probability crates

Remove local modules, use standalone crates for firmware extraction."
```

---

### Task 4: Update Workspace (if needed)

**Files:**
- Modify: `Cargo.toml` (workspace root)

**Purpose:** Add new members to workspace if not using wildcard.

- [ ] **Step 1: Check current workspace configuration**

Run: `rtk read Cargo.toml | head -30`

Check if workspace uses `members = ["crates/*"]` or explicit list.

- [ ] **Step 2: Add new members if explicit list**

```toml
[workspace]
members = [
    "crates/shared",
    "crates/preprocessor",
    "crates/gps_processor",
    "crates/detection",
    "crates/pipeline",
    "crates/pipeline/filter",      # NEW
    "crates/pipeline/probability", # NEW
    "crates/trace_validator",
    "crates/pico2-firmware",
]
```

- [ ] **Step 3: Verify workspace build**

Run: `rtk cargo build --workspace`

Expected: All crates build successfully

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml
git commit -m "chore: add pipeline-filter and pipeline-probability to workspace"
```

---

### Task 5: Final Verification

- [ ] **Step 1: Run full test suite**

Run: `rtk cargo test --workspace`

Expected: All tests PASS

- [ ] **Step 2: Run pipeline tests specifically**

Run: `rtk cargo test -p pipeline`

Expected: All 42+ tests PASS

- [ ] **Step 3: Run characterization test**

Run: `rtk cargo test -p pipeline --test characterization`

Expected: PASS (11 arrivals)

- [ ] **Step 4: Verify no_std compatibility**

Run: `rtk cargo build -p pipeline-filter --no-default-features`
Run: `rtk cargo build -p pipeline-probability --no-default-features`

Expected: Both build without std

- [ ] **Step 5: Check for broken imports**

Run: `rtk cargo check --workspace`

Expected: No errors

---

## Success Criteria

1. ✅ `pipeline-filter` crate exists and passes tests
2. ✅ `pipeline-probability` crate exists and passes tests
3. ✅ Pipeline crate uses external crates (not local modules)
4. ✅ All workspace tests pass
5. ✅ no_std compatibility verified
6. ✅ Characterization test passes (no behavior change)

---

## Phase 5 Complete - All Phases Done! 🎉

**Summary of Refactoring:**

1. ✅ **Phase 1:** Quick wins (constant, PhantomData removal, characterization test)
2. ✅ **Phase 2:** CorridorFilter extraction
3. ✅ **Phase 3:** ProbabilityEngine extraction (fixed duplicate computation)
4. ✅ **Phase 4:** Diagnostics interface fix (GpsDiagnostics struct)
5. ✅ **Phase 5:** Separate crates for firmware extraction

**Code is now ready for firmware extraction!**

The `pipeline-filter` and `pipeline-probability` crates are:
- Pure functions with clean interfaces
- no_std compatible
- Unit tested
- Ready for RP2350 firmware integration

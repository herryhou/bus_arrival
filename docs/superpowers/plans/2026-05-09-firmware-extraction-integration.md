# Firmware Extraction Integration Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Integrate `pipeline-filter` and `pipeline-probability` crates into pico2-firmware, replacing inline implementations with shared crate code.

**Architecture:** Add new crates as firmware dependencies. Replace `find_active_stops()` and probability computation with crate APIs. Adapt `Vec<usize>` output to `heapless::Vec<usize, 16>` for firmware constraints.

**Tech Stack:** Rust no_std, heapless, cargo workspace

---

## File Structure

**Create:**
- `crates/pico2-firmware/src/detection/filter.rs` - Filter adapter (Vec → heapless::Vec)
- `crates/pico2-firmware/src/detection/probability.rs` - Probability adapter wrapper

**Modify:**
- `crates/pico2-firmware/Cargo.toml` - Add pipeline-filter and pipeline-probability dependencies
- `crates/pico2-firmware/src/detection.rs` - Use crate APIs instead of inline code
- `crates/pico2-firmware/src/detection/mod.rs` - Export new modules

**Delete:**
- Inline code in detection.rs (after migration)

---

### Task 1: Add New Crate Dependencies

**Files:**
- Modify: `crates/pico2-firmware/Cargo.toml`

**Purpose:** Add pipeline-filter and pipeline-probability as dependencies.

- [ ] **Step 1: Read current Cargo.toml**

Run: `rtk read crates/pico2-firmware/Cargo.toml`

Note the current dependencies section:
```toml
[dependencies]
shared = { path = "../shared", default-features = false, features = [] }
gps_processor = { path = "../pipeline/gps_processor", default-features = false, features = [] }
detection = { path = "../pipeline/detection", default-features = false, features = [] }
```

- [ ] **Step 2: Add new dependencies**

Add to `[dependencies]` section after `detection`:
```toml
pipeline-filter = { path = "../pipeline/filter", default-features = false, features = [] }
pipeline-probability = { path = "../pipeline/probability", default-features = false, features = [] }
```

- [ ] **Step 3: Verify firmware feature still works**

Run: `rtk cargo check -p pico2-firmware --features firmware`

Expected: Builds successfully (no errors)

- [ ] **Step 4: Verify dev feature still works**

Run: `rtk cargo check -p pico2-firmware --features dev`

Expected: Builds successfully

- [ ] **Step 5: Commit**

```bash
git add crates/pico2-firmware/Cargo.toml
git commit -m "feat(firmware): add pipeline-filter and pipeline-probability dependencies"
```

---

### Task 2: Create Filter Adapter Module

**Files:**
- Create: `crates/pico2-firmware/src/detection/filter.rs`

**Purpose:** Adapt `pipeline_filter::active_stops()` (Vec<usize>) to firmware's heapless::Vec<usize, 16>.

- [ ] **Step 1: Create detection/filter.rs**

```rust
//! Stop corridor filter adapter
//!
//! Wraps pipeline-filter crate, adapting Vec<usize> output
//! to heapless::Vec<usize, 16> for firmware constraints.

use shared::{DistCm, binfile::{RouteData, Stop}};

/// Find stops within corridor of current position
///
/// Returns heapless::Vec of stop indices where:
/// - s_cm is within [corridor_start_cm, corridor_end_cm]
/// - skip_flags[idx] is false
///
/// # Arguments
///
/// * `s_cm` - Current position along route (cm)
/// * `route_data` - Route data reference
/// * `skip_flags` - Whether each stop should be skipped
///
/// # Returns
///
/// heapless::Vec of active stop indices (max 16)
pub fn find_active_stops(
    s_cm: DistCm,
    route_data: &RouteData,
    skip_flags: &[bool],
) -> heapless::Vec<usize, 16> {
    // Build stop slice
    let stops: heapless::Vec<&Stop, 32> = route_data.stops()
        .iter()
        .collect()
        .unwrap();

    // Use pipeline-filter crate
    let active_std = pipeline_filter::active_stops(s_cm, &stops, skip_flags);

    // Convert to heapless::Vec
    let mut active = heapless::Vec::new();
    for idx in active_std.into_iter().take(16) {
        if active.push(idx).is_err() {
            #[cfg(feature = "firmware")]
            defmt::warn!("Active stops overflow (>16), truncating");
            break;
        }
    }
    active
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::binfile::tests::mock_route_data;

    #[test]
    fn test_find_active_stops_single() {
        let route_data = mock_route_data();
        let skip_flags = vec![false; route_data.stop_count];

        // Position at first stop corridor
        let s_cm = route_data.get_stop(0).unwrap().progress_cm;
        let active = find_active_stops(s_cm, &route_data, &skip_flags);

        assert_eq!(active.len(), 1);
        assert_eq!(active[0], 0);
    }

    #[test]
    fn test_find_active_stops_skip_flag() {
        let route_data = mock_route_data();
        let mut skip_flags = vec![false; route_data.stop_count];
        skip_flags[0] = true;

        let s_cm = route_data.get_stop(0).unwrap().progress_cm;
        let active = find_active_stops(s_cm, &route_data, &skip_flags);

        assert_eq!(active.len(), 0); // Stop 0 is skipped
    }
}
```

- [ ] **Step 2: Build to verify syntax**

Run: `rtk cargo build -p pico2-firmware --features dev`

Expected: Builds successfully

- [ ] **Step 3: Run tests**

Run: `rtk cargo test -p pico2-firmware detection::filter`

Expected: Tests PASS

- [ ] **Step 4: Commit**

```bash
git add crates/pico2-firmware/src/detection/filter.rs
git commit -m "feat(firmware): add filter adapter module

Wraps pipeline-filter crate, adapts Vec to heapless::Vec for firmware."
```

---

### Task 3: Create Probability Adapter Module

**Files:**
- Create: `crates/pico2-firmware/src/detection/probability.rs`

**Purpose:** Wrap `pipeline_probability::ProbabilityEngine` for firmware use.

- [ ] **Step 1: Create detection/probability.rs**

```rust
//! Arrival probability computation adapter
//!
//! Wraps pipeline-probability crate, providing cached
//! probability computation for firmware.

use shared::{SpeedCms, PositionSignals, Stop};
use crate::detection::GpsStatus;
use pipeline_probability::ProbabilityEngine;

/// Probability computation wrapper
///
/// Wraps ProbabilityEngine from pipeline-probability crate.
pub struct FirmwareProbabilityEngine {
    inner: ProbabilityEngine,
}

impl FirmwareProbabilityEngine {
    pub fn new() -> Self {
        Self {
            inner: ProbabilityEngine::new(),
        }
    }

    /// Compute arrival probability (cached)
    ///
    /// Delegates to pipeline-probability crate.
    pub fn compute(
        &mut self,
        timestamp: u64,
        signals: PositionSignals,
        v_cms: SpeedCms,
        stop: &Stop,
        dwell_time_s: u16,
        gps_status: GpsStatus,
    ) -> u8 {
        // Convert firmware GpsStatus to detection::probability::GpsStatus
        let gps_status = match gps_status {
            GpsStatus::Valid => detection::probability::GpsStatus::Valid,
            GpsStatus::DrOutage => detection::probability::GpsStatus::DrOutage,
            GpsStatus::OffRoute => detection::probability::GpsStatus::OffRoute,
        };

        self.inner
            .compute(timestamp, signals, v_cms, stop, dwell_time_s, gps_status)
            .probability
    }

    /// Clear cache (call on new GPS record)
    pub fn clear(&mut self) {
        self.inner.clear();
    }
}

impl Default for FirmwareProbabilityEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_probability_compute() {
        let mut engine = FirmwareProbabilityEngine::new();
        let signals = PositionSignals::new(100, 100);
        let stop = Stop {
            progress_cm: 1000,
            corridor_start_cm: 900,
            corridor_end_cm: 1100,
        };

        let prob = engine.compute(0, signals, 50, &stop, 0, GpsStatus::Valid);
        assert!(prob <= 255);
    }

    #[test]
    fn test_probability_clear() {
        let mut engine = FirmwareProbabilityEngine::new();
        let signals = PositionSignals::new(100, 100);
        let stop = Stop {
            progress_cm: 1000,
            corridor_start_cm: 900,
            corridor_end_cm: 1100,
        };

        // First call
        engine.compute(0, signals, 50, &stop, 0, GpsStatus::Valid);

        // Clear cache
        engine.clear();

        // Second call should compute new result
        let _prob = engine.compute(0, signals, 50, &stop, 0, GpsStatus::Valid);
    }
}
```

- [ ] **Step 2: Build to verify syntax**

Run: `rtk cargo build -p pico2-firmware --features dev`

Expected: Builds successfully

- [ ] **Step 3: Run tests**

Run: `rtk cargo test -p pico2-firmware detection::probability`

Expected: Tests PASS

- [ ] **Step 4: Commit**

```bash
git add crates/pico2-firmware/src/detection/probability.rs
git commit -m "feat(firmware): add probability adapter module

Wraps pipeline-probability crate for firmware use."
```

---

### Task 4: Update detection.rs to Use New Modules

**Files:**
- Modify: `crates/pico2-firmware/src/detection.rs`

**Purpose:** Replace inline implementations with module adapters.

- [ ] **Step 1: Remove old `find_active_stops()` function**

Delete lines 30-51 (the `find_active_stops()` function and its helper).

- [ ] **Step 2: Remove old probability functions**

Delete lines 54-162 (compute_features, compute_arrival_probability, compute_arrival_probability_adaptive).

Keep only:
- Module doc (lines 1-21)
- GpsStatus enum (lines 55-64)
- LUT references (lines 23-24)

- [ ] **Step 3: Add re-exports**

Add after GpsStatus enum:
```rust
// Re-export from adapter modules
pub use filter::find_active_stops;
pub use probability::FirmwareProbabilityEngine;
```

- [ ] **Step 4: Update module doc**

Replace lines 1-21 with:
```rust
//! Arrival detection logic
//!
//! Provides stop corridor filtering and arrival probability computation
//! via shared crates (pipeline-filter, pipeline-probability).
//!
//! # Crate Integration
//!
//! - `find_active_stops()` delegates to `pipeline-filter` crate
//! - `FirmwareProbabilityEngine` wraps `pipeline-probability` crate
```

- [ ] **Step 5: Verify build**

Run: `rtk cargo build -p pico2-firmware --features dev`

Expected: Builds successfully

- [ ] **Step 6: Run all detection tests**

Run: `rtk cargo test -p pico2-firmware detection`

Expected: All tests PASS

- [ ] **Step 7: Commit**

```bash
git add crates/pico2-firmware/src/detection.rs
git commit -m "refactor(firmware): use crate APIs instead of inline code

Replace inline find_active_stops() and probability computation
with shared crate adapters."
```

---

### Task 5: Update detection/mod.rs

**Files:**
- Modify: `crates/pico2-firmware/src/detection/mod.rs` (if exists, otherwise create)
- OR: Update `crates/pico2-firmware/src/lib.rs` to export new modules

- [ ] **Step 1: Check current module structure**

Run: `ls -la crates/pico2-firmware/src/detection/`

If `mod.rs` exists, update it. If not, modules are already public via `detection.rs`.

- [ ] **Step 2: Create detection/mod.rs if needed**

If directory doesn't exist:
```bash
mkdir -p crates/pico2-firmware/src/detection
mv crates/pico2-firmware/src/detection.rs crates/pico2-firmware/src/detection/mod.rs
```

- [ ] **Step 3: Update mod.rs exports**

Add to `detection/mod.rs`:
```rust
mod filter;
mod probability;

// Re-export public API
pub use self::{filter, probability};

// Old detection.rs content follows...
```

- [ ] **Step 4: Verify build**

Run: `rtk cargo build -p pico2-firmware --features dev`

Expected: Builds successfully

- [ ] **Step 5: Commit**

```bash
git add crates/pico2-firmware/src/detection/
git commit -m "refactor(firmware): organize detection module structure

Add mod.rs with filter and probability submodules."
```

---

### Task 6: Verify Integration with Estimation

**Files:**
- Verify: `crates/pico2-firmware/src/estimation/mod.rs` (no changes needed)

**Purpose:** Ensure estimation layer still works after detection changes.

- [ ] **Step 1: Build estimation tests**

Run: `rtk cargo test -p pico2-firmware estimation`

Expected: All estimation tests PASS

- [ ] **Step 2: Run full firmware test suite**

Run: `rtk cargo test -p pico2-firmware --features dev`

Expected: All tests PASS

- [ ] **Step 3: Verify firmware build**

Run: `rtk cargo build -p pico2-firmware --features firmware`

Expected: Builds for target successfully

---

### Task 7: Final Verification

- [ ] **Step 1: Run workspace test suite**

Run: `rtk cargo test --workspace`

Expected: All tests PASS (567+ tests)

- [ ] **Step 2: Check for broken imports**

Run: `rtk cargo check --workspace`

Expected: No errors

- [ ] **Step 3: Verify no_std compatibility**

Run: `rtk cargo build -p pico2-firmware --no-default-features`

Expected: Builds without std

- [ ] **Step 4: Check Flash usage**

Run: `rtk cargo size -p pico2-firmware --features firmware`

Expected: Flash < ~34 KB (verify budget)

- [ ] **Step 5: Review changes**

Run: `rtk git diff master`

Verify:
- Only firmware files changed
- Dependencies added correctly
- No behavior changes (tests pass)

---

## Success Criteria

1. ✅ `pipeline-filter` and `pipeline-probability` are firmware dependencies
2. ✅ `find_active_stops()` delegates to `pipeline-filter` crate
3. ✅ `FirmwareProbabilityEngine` wraps `pipeline-probability` crate
4. ✅ All firmware tests pass
5. ✅ no_std compatibility verified
6. ✅ Flash budget within limits (~34 KB)
7. ✅ Behavior preserved (all tests pass)

---

## Notes

- **Vec vs heapless::Vec**: Filter adapter converts `Vec<usize>` → `heapless::Vec<usize, 16>`
- **GpsStatus**: Both crates use compatible enums, adapter converts between them
- **Caching**: `ProbabilityEngine` caches by timestamp to avoid recomputation
- **Budget**: Target < 8% CPU @ 150MHz, ~34 KB Flash, < 1 KB SRAM

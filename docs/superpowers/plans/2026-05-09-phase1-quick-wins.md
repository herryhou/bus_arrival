# Phase 1: Quick Wins Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix simple code hygiene issues (magic number, PhantomData anti-pattern) and add characterization test to prevent regressions.

**Architecture:** Direct code fixes in existing files + new characterization test. No structural changes.

**Tech Stack:** Rust, cargo test

---

## File Structure

**Create:**
- `crates/pipeline/tests/characterization.rs` - Characterization test

**Modify:**
- `crates/pipeline/src/detection_state.rs` - Replace magic number with constant
- `crates/pipeline/src/localization.rs` - Remove PhantomData lifetime
- `crates/pipeline/src/lib.rs` - Update LocalizationState signature (remove lifetime)

---

### Task 1: Add Characterization Test

**Files:**
- Create: `crates/pipeline/tests/characterization.rs`

**Purpose:** Capture current pipeline behavior before any changes. This test ensures ty225_normal scenario produces exactly 11 arrivals.

- [ ] **Step 1: Create characterization test file**

```rust
//! Characterization test for pipeline behavior
//!
//! This test captures the current expected behavior of the pipeline.
//! Any regression in arrival/departure detection should fail this test.

use pipeline::Pipeline;

#[test]
fn test_ty225_normal_characterization() {
    let result = Pipeline::process_nmea_file(
        "../../test_data/ty225_normal_nmea.txt",
        "../../test_data/ty225_normal.bin",
    ).expect("Pipeline processing should succeed");

    // Characterize: arrival count for ty225_normal scenario
    assert_eq!(result.arrivals.len(), 11,
        "ty225_normal should detect exactly 11 arrivals");

    // Characterize: departure count
    assert_eq!(result.departures.len(), 11,
        "ty225_normal should detect exactly 11 departures");

    // Characterize: first arrival is at stop 0
    let first = &result.arrivals[0];
    assert_eq!(first.stop_idx, 0,
        "First arrival should be at stop index 0");
    assert!(first.s_cm > 0,
        "First arrival should have positive position");

    // Characterize: last arrival is at final stop
    let last = &result.arrivals.last().expect("Should have arrivals");
    assert!(last.stop_idx > 0,
        "Last arrival should be at a later stop");
}
```

- [ ] **Step 2: Run test to verify it passes**

Run: `rtk cargo test -p pipeline --test characterization`

Expected: PASS (current behavior is correct)

- [ ] **Step 3: Commit**

```bash
git add crates/pipeline/tests/characterization.rs
git commit -m "test: add characterization test for ty225_normal scenario"
```

---

### Task 2: Replace Magic Number with Constant

**Files:**
- Modify: `crates/pipeline/src/detection_state.rs:71-72`
- Modify: `crates/pipeline/src/lib.rs:252-253`

**Purpose:** Replace `10000` magic number with named constant `DETOUR_JUMP_THRESHOLD_CM`.

- [ ] **Step 1: Add constant to detection_state.rs**

Add at top of file after imports:

```rust
/// Threshold distance (cm) for detecting detour re-entry jumps
/// Jumps larger than this indicate the bus has completed a detour
/// and snapped back to the route.
pub const DETOUR_JUMP_THRESHOLD_CM: i32 = 10000;
```

- [ ] **Step 2: Update detection_state.rs to use constant**

Replace line 72:
```rust
// OLD:
let large_forward_jump = self.off_route_last_s_cm
    .map_or(false, |off_route_s| record.s_cm > off_route_s + 10000);

// NEW:
let large_forward_jump = self.off_route_last_s_cm
    .map_or(false, |off_route_s| record.s_cm > off_route_s + DETOUR_JUMP_THRESHOLD_CM);
```

- [ ] **Step 3: Update lib.rs to use constant**

Add import at top of file:
```rust
use crate::detection_state::DETOUR_JUMP_THRESHOLD_CM;
```

Replace line 253:
```rust
// OLD:
let large_forward_jump = self.off_route_last_s_cm
    .map_or(false, |off_route_s| record.s_cm > off_route_s + 10000);

// NEW:
let large_forward_jump = self.off_route_last_s_cm
    .map_or(false, |off_route_s| record.s_cm > off_route_s + DETOUR_JUMP_THRESHOLD_CM);
```

- [ ] **Step 4: Run tests to verify no behavior change**

Run: `rtk cargo test -p pipeline`

Expected: All tests PASS (characterization test still passes)

- [ ] **Step 5: Commit**

```bash
git add crates/pipeline/src/detection_state.rs crates/pipeline/src/lib.rs
git commit -m "refactor: replace magic number 10000 with DETOUR_JUMP_THRESHOLD_CM constant"
```

---

### Task 3: Remove PhantomData Lifetime from LocalizationState

**Files:**
- Modify: `crates/pipeline/src/localization.rs:8,13,18-26`
- Modify: `crates/pipeline/src/lib.rs:82-92,421,453`

**Purpose:** Remove unnecessary lifetime parameter. `PhantomData<&'a ()>` is used but `LocalizationState` doesn't actually hold a reference to route_data.

- [ ] **Step 1: Update localization.rs struct definition**

```rust
// OLD:
pub struct LocalizationState<'a> {
    kalman: KalmanState,
    dr: DrState,
    route_data: std::marker::PhantomData<&'a ()>,
    is_first_fix: bool,
}

// NEW:
pub struct LocalizationState {
    kalman: KalmanState,
    dr: DrState,
    is_first_fix: bool,
}
```

- [ ] **Step 2: Update localization.rs impl block**

```rust
// OLD:
impl<'a> LocalizationState<'a> {
    pub fn new(_route_data: &RouteData) -> Self {
        Self {
            kalman: KalmanState::new(),
            dr: DrState::new(),
            route_data: std::marker::PhantomData,
            is_first_fix: true,
        }
    }
    // ... rest of impl
}

// NEW:
impl LocalizationState {
    pub fn new(_route_data: &RouteData) -> Self {
        Self {
            kalman: KalmanState::new(),
            dr: DrState::new(),
            is_first_fix: true,
        }
    }
    // ... rest of impl
}
```

- [ ] **Step 3: Update lib.rs to remove lifetime**

```rust
// OLD:
pub struct LocalizationState<'a> { ... }
impl<'a> LocalizationState<'a> { ... }
let mut loc_state = LocalizationState::new(route_data);

// NEW:
pub struct LocalizationState { ... }
impl LocalizationState { ... }
let mut loc_state = LocalizationState::new(route_data);
```

Line 82: Remove `<'a>` from struct definition
Line 93: Remove `<'a>` from impl block
Line 453: No change needed (already calls without lifetime annotation)

- [ ] **Step 4: Run tests to verify no behavior change**

Run: `rtk cargo test -p pipeline`

Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/pipeline/src/localization.rs crates/pipeline/src/lib.rs
git commit -m "refactor: remove unnecessary lifetime from LocalizationState

PhantomData<&'a ()> was used but struct doesn't hold any reference.
This removes the anti-pattern and simplifies the API."
```

---

### Task 4: Final Verification

- [ ] **Step 1: Run full test suite**

Run: `rtk cargo test -p pipeline`

Expected: All 42+ tests PASS

- [ ] **Step 2: Verify characterization test still passes**

Run: `rtk cargo test -p pipeline --test characterization -- test_ty225_normal_characterization`

Expected: PASS with exactly 11 arrivals

- [ ] **Step 3: Check for any compiler warnings**

Run: `rtk cargo clippy -p pipeline`

Expected: No new warnings introduced

- [ ] **Step 4: Final commit if any cleanups needed**

(Only if additional changes were made)

---

## Success Criteria

1. ✅ Characterization test passes (11 arrivals for ty225_normal)
2. ✅ Magic number `10000` replaced with `DETOUR_JUMP_THRESHOLD_CM`
3. ✅ `LocalizationState` no longer has lifetime parameter
4. ✅ All existing tests still pass
5. ✅ No new compiler warnings

---

## What's Next

After Phase 1 is complete, proceed to:
- **Phase 2:** Extract CorridorFilter (`pipeline/src/filter.rs`)
- **Phase 3:** Extract ProbabilityEngine (`pipeline/src/probability.rs`)
- **Phase 4:** Fix Diagnostics Interface (`GpsDiagnostics` struct)
- **Phase 5:** Move to Separate Crates

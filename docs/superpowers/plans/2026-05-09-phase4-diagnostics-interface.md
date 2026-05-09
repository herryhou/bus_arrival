# Phase 4: Diagnostics Interface Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace 7-parameter `with_diagnostics()` tuple with clean `GpsDiagnostics` struct.

**Architecture:** Create `GpsDiagnostics` struct in `localization.rs`. Update `GpsRecord::with_diagnostics()` to accept struct instead of 7 parameters.

**Tech Stack:** Rust, cargo test

---

## File Structure

**Modify:**
- `crates/pipeline/src/localization.rs` - Add `GpsDiagnostics` struct
- `crates/pipeline/src/gps.rs` - Update `with_diagnostics()` signature
- `crates/pipeline/src/lib.rs` - Update all `with_diagnostics()` call sites

---

### Task 1: Create GpsDiagnostics Struct

**Files:**
- Modify: `crates/pipeline/src/localization.rs`

**Purpose:** Define `GpsDiagnostics` struct to replace 7-parameter tuple.

- [ ] **Step 1: Add GpsDiagnostics struct to localization.rs**

Add after imports (around line 7):

```rust
/// GPS diagnostics information
///
/// Encapsulates diagnostic data from GPS processing.
/// Replaces the 7-parameter tuple approach.
#[derive(Debug, Clone, Default)]
pub struct GpsDiagnostics {
    pub segment_idx: Option<u16>,
    pub heading_met: bool,
    pub divergence_cm: i32,
    pub hdop: Option<f32>,
    pub num_sats: Option<u8>,
    pub fix_type: Option<String>,
    pub variance_cm2: i32,
}

impl GpsDiagnostics {
    /// Create new diagnostics with minimal fields
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder method for segment_idx
    pub fn with_segment_idx(mut self, idx: Option<u16>) -> Self {
        self.segment_idx = idx;
        self
    }

    /// Builder method for heading_met
    pub fn with_heading_met(mut self, met: bool) -> Self {
        self.heading_met = met;
        self
    }

    /// Builder method for divergence_cm
    pub fn with_divergence_cm(mut self, div: i32) -> Self {
        self.divergence_cm = div;
        self
    }

    /// Builder method for hdop
    pub fn with_hdop(mut self, hdop: Option<f32>) -> Self {
        self.hdop = hdop;
        self
    }

    /// Builder method for num_sats
    pub fn with_num_sats(mut self, sats: Option<u8>) -> Self {
        self.num_sats = sats;
        self
    }

    /// Builder method for fix_type
    pub fn with_fix_type(mut self, fix: Option<String>) -> Self {
        self.fix_type = fix;
        self
    }

    /// Builder method for variance_cm2
    pub fn with_variance_cm2(mut self, var: i32) -> Self {
        self.variance_cm2 = var;
        self
    }
}
```

- [ ] **Step 2: Run compiler to check**

Run: `rtk cargo check -p pipeline`

Expected: Compiles (new struct doesn't break anything yet)

- [ ] **Step 3: Commit**

```bash
git add crates/pipeline/src/localization.rs
git commit -m "feat: add GpsDiagnostics struct to localization.rs

Provides clean interface for GPS diagnostics data.
Will replace 7-parameter with_diagnostics() calls."
```

---

### Task 2: Update GpsRecord::with_diagnostics() Signature

**Files:**
- Modify: `crates/pipeline/src/gps.rs`

**Purpose:** Change `with_diagnostics()` to accept `GpsDiagnostics` struct.

- [ ] **Step 1: Read current gps.rs to understand signature**

Run: `rtk read crates/pipeline/src/gps.rs`

Look for `with_diagnostics` method signature.

- [ ] **Step 2: Update with_diagnostics() signature**

```rust
// OLD:
pub fn with_diagnostics(
    mut self,
    segment_idx: Option<u16>,
    heading_constraint_met: bool,
    divergence_cm: i32,
    hdop: Option<f32>,
    num_sats: Option<u8>,
    fix_type: Option<String>,
    variance_cm2: i32,
) -> Self {
    self.segment_idx = segment_idx;
    self.heading_constraint_met = heading_constraint_met;
    self.divergence_cm = divergence_cm;
    self.hdop = hdop;
    self.num_sats = num_sats;
    self.fix_type = fix_type;
    self.variance_cm2 = variance_cm2;
    self
}

// NEW:
pub fn with_diagnostics(mut self, diag: crate::localization::GpsDiagnostics) -> Self {
    self.segment_idx = diag.segment_idx;
    self.heading_constraint_met = diag.heading_met;
    self.divergence_cm = diag.divergence_cm;
    self.hdop = diag.hdop;
    self.num_sats = diag.num_sats;
    self.fix_type = diag.fix_type;
    self.variance_cm2 = diag.variance_cm2;
    self
}
```

- [ ] **Step 3: Update all call sites in localization.rs**

```rust
// OLD:
Some(gps::GpsRecord::new(
    gps.timestamp,
    gps.lat,
    gps.lon,
    s_cm,
    v_cms,
    gps.heading_cdeg,
    "valid",
).with_diagnostics(
    Some(seg_idx as u16),
    true,
    divergence_cm,
    hdop,
    None,
    None,
    0,
))

// NEW:
use crate::localization::GpsDiagnostics;

Some(gps::GpsRecord::new(
    gps.timestamp,
    gps.lat,
    gps.lon,
    s_cm,
    v_cms,
    gps.heading_cdeg,
    "valid",
).with_diagnostics(
    GpsDiagnostics::new()
        .with_segment_idx(Some(seg_idx as u16))
        .with_heading_met(true)
        .with_divergence_cm(divergence_cm)
        .with_hdop(hdop)
))
```

Apply similar pattern to DrOutage and OffRoute cases:
```rust
// DrOutage:
GpsDiagnostics::new()
    .with_heading_met(false)

// OffRoute:
GpsDiagnostics::new()
    .with_heading_met(false)
```

- [ ] **Step 4: Update all call sites in lib.rs**

Same pattern as localization.rs. Find all `with_diagnostics(` calls and replace with `GpsDiagnostics` builder.

- [ ] **Step 5: Run tests**

Run: `rtk cargo test -p pipeline`

Expected: All tests PASS

- [ ] **Step 6: Commit**

```bash
git add crates/pipeline/src/gps.rs crates/pipeline/src/localization.rs crates/pipeline/src/lib.rs
git commit -m "refactor: replace 7-param with_diagnostics with GpsDiagnostics struct

Clean interface using struct instead of positional parameters.
All call sites updated to use builder pattern."
```

---

### Task 3: Add Unit Tests for GpsDiagnostics

**Files:**
- Modify: `crates/pipeline/src/localization.rs`

**Purpose:** Test `GpsDiagnostics` builder methods.

- [ ] **Step 1: Add tests to localization.rs**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostics_builder() {
        let diag = GpsDiagnostics::new()
            .with_segment_idx(Some(5))
            .with_heading_met(true)
            .with_divergence_cm(100)
            .with_hdop(Some(1.5))
            .with_num_sats(Some(12))
            .with_fix_type(Some("3D".to_string()))
            .with_variance_cm2(50);

        assert_eq!(diag.segment_idx, Some(5));
        assert_eq!(diag.heading_met, true);
        assert_eq!(diag.divergence_cm, 100);
        assert_eq!(diag.hdop, Some(1.5));
        assert_eq!(diag.num_sats, Some(12));
        assert_eq!(diag.fix_type, Some("3D".to_string()));
        assert_eq!(diag.variance_cm2, 50);
    }

    #[test]
    fn test_diagnostics_default() {
        let diag = GpsDiagnostics::new();

        assert_eq!(diag.segment_idx, None);
        assert_eq!(diag.heading_met, false);
        assert_eq!(diag.divergence_cm, 0);
        assert_eq!(diag.hdop, None);
        assert_eq!(diag.num_sats, None);
        assert_eq!(diag.fix_type, None);
        assert_eq!(diag.variance_cm2, 0);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `rtk cargo test -p pipeline --lib localization`

Expected: Both new tests PASS

- [ ] **Step 3: Commit**

```bash
git add crates/pipeline/src/localization.rs
git commit -m "test: add unit tests for GpsDiagnostics builder"
```

---

### Task 4: Final Verification

- [ ] **Step 1: Run full test suite**

Run: `rtk cargo test -p pipeline`

Expected: All 42+ tests PASS

- [ ] **Step 2: Verify no 7-parameter calls remain**

Run: `rtk grep -n "with_diagnostics(" crates/pipeline/src/`

Expected: All calls now use `GpsDiagnostics` builder pattern

- [ ] **Step 3: Run characterization test**

Run: `rtk cargo test -p pipeline --test characterization`

Expected: PASS (11 arrivals, no behavior change)

- [ ] **Step 4: Check compiler warnings**

Run: `rtk cargo clippy -p pipeline`

Expected: No new warnings

---

## Success Criteria

1. ✅ `GpsDiagnostics` struct exists in `localization.rs`
2. ✅ `with_diagnostics()` accepts single `GpsDiagnostics` parameter
3. ✅ All call sites use builder pattern
4. ✅ Unit tests for `GpsDiagnostics` pass
5. ✅ All existing tests pass
6. ✅ No 7-parameter tuples remain

---

## What's Next

After Phase 4 is complete, proceed to:
- **Phase 5:** Move to Separate Crates (`pipeline/filter/` and `pipeline/probability/`)

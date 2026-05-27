# Acquiring State Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add "acquiring" state to handle cold boot scenario (app launch, first GPS fix) properly, preventing meaningless "suspect" state with `off_route_last_s_cm = 0`.

**Architecture:** Introduce explicit `is_cold_boot` flag in `KalmanState` to mark cold start. Add `Acquiring` variant to `ProcessResult`. During acquiring, detection disabled, position frozen at 0. After 2 consecutive good matches with heading constraint, snap to route and transition to Valid.

**Tech Stack:** Rust (no_std embedded + std pipeline), shared types crate

---

## Task 1: Add `is_cold_boot` field to `KalmanState`

**Files:**
- Modify: `crates/shared/src/lib.rs:243-257`
- Test: `crates/shared/tests/` (existing tests compile)

- [ ] **Step 1: Add `is_cold_boot` field to `KalmanState` struct**

Modify the `KalmanState` struct in `crates/shared/src/lib.rs`:

```rust
/// 1D Kalman filter state for route progress estimation.
#[repr(C)]
#[derive(Debug, Clone)]
pub struct KalmanState {
    pub s_cm: DistCm,
    pub v_cms: SpeedCms,
    pub last_seg_idx: usize,
    /// Consecutive ticks with poor GPS match (off-route suspect counter)
    pub off_route_suspect_ticks: u8,
    /// Consecutive ticks with good GPS match (off-route clear counter)
    pub off_route_clear_ticks: u8,
    /// Frozen position when off-route is first suspected (for immediate position freezing)
    pub frozen_s_cm: Option<DistCm>,
    /// Timestamp when position was frozen, in milliseconds since epoch.
    pub off_route_freeze_time: Option<TimestampMs>,
    /// Off-route freeze context for spatial anchoring during recovery
    pub freeze_ctx: Option<FreezeContext>,
    /// Cold boot flag: true during initial acquisition before first route snap
    pub is_cold_boot: bool,
}
```

- [ ] **Step 2: Update `KalmanState::new()` to initialize `is_cold_boot = false`**

```rust
impl KalmanState {
    pub fn new() -> Self {
        KalmanState {
            s_cm: 0,
            v_cms: 0,
            last_seg_idx: 0,
            off_route_suspect_ticks: 0,
            off_route_clear_ticks: 0,
            frozen_s_cm: None,
            off_route_freeze_time: None,
            freeze_ctx: None,
            is_cold_boot: false,
        }
    }
```

- [ ] **Step 3: Update `KalmanState::init()` to set `is_cold_boot = true`**

```rust
    /// Cold start initialization from first valid GPS fix.
    /// Should be paired with 3-second warm-up period (see tech report Section 19.5).
    pub fn init(z_cm: DistCm, v_gps_cms: SpeedCms, seg_idx: usize) -> Self {
        KalmanState {
            s_cm: z_cm,
            v_cms: v_gps_cms,
            last_seg_idx: seg_idx,
            off_route_suspect_ticks: 0,
            off_route_clear_ticks: 0,
            frozen_s_cm: None,
            off_route_freeze_time: None,
            freeze_ctx: None,
            is_cold_boot: true,  // NEW: mark as cold boot
        }
    }
```

- [ ] **Step 4: Run tests to verify changes compile**

Run: `cargo test -p shared`

Expected: PASS (all existing tests still pass)

- [ ] **Step 5: Commit**

```bash
git add crates/shared/src/lib.rs
git commit -m "feat(shared): add is_cold_boot flag to KalmanState

Per spec 2026-05-27-acquiring-state-design.md:
- Add is_cold_boot: bool field to KalmanState
- Initialize to false in new()
- Set to true in init() (cold start path)"
```

---

## Task 2: Add `is_cold_start()` helper to hysteresis module

**Files:**
- Modify: `crates/pipeline/gps_processor/src/kalman/hysteresis.rs:1-40`
- Test: `crates/pipeline/gps_processor/src/kalman/hysteresis.rs:107-319` (existing tests)

- [ ] **Step 1: Add `is_cold_start()` helper function after `OffRouteStatus` enum**

Add this function after line 38 in `hysteresis.rs`:

```rust
/// Check if we're in cold start mode (acquiring initial route lock)
///
/// Returns `state.is_cold_boot`. This is the explicit marker for cold boot
/// vs. off-route recovery. When true, snap logic allows re-entry anywhere
/// on route (no min_s constraint). When false, snap forward only.
pub fn is_cold_start(state: &KalmanState) -> bool {
    state.is_cold_boot
}
```

- [ ] **Step 2: Update module exports to include the new function**

Add to the re-export section at the top of the file (after line 20):

```rust
pub use hysteresis::{
    OFF_ROUTE_D2_THRESHOLD, OFF_ROUTE_CLEAR_TICKS, OFF_ROUTE_CONFIRM_TICKS,
    OffRouteStatus, update_off_route_hysteresis, reset_off_route_state,
    is_cold_start,  // NEW export
};
```

- [ ] **Step 3: Run tests to verify changes compile**

Run: `cargo test -p gps_processor --lib hysteresis`

Expected: PASS (all existing hysteresis tests still pass)

- [ ] **Step 4: Commit**

```bash
git add crates/pipeline/gps_processor/src/kalman/hysteresis.rs
git add crates/pipeline/gps_processor/src/kalman/mod.rs
git commit -m "feat(hysteresis): add is_cold_start() helper

Returns state.is_cold_boot. Explicit cold boot marker for
snap logic to distinguish cold start from off-route recovery."
```

---

## Task 3: Add `Acquiring` variant to `ProcessResult` enum

**Files:**
- Modify: `crates/pipeline/gps_processor/src/kalman/mod.rs:43-68`
- Test: (no direct tests, integration tests cover usage)

- [ ] **Step 1: Add `Acquiring` variant to `ProcessResult` enum**

```rust
/// ProcessResult from GPS update
pub enum ProcessResult {
    Valid {
        signals: PositionSignals,
        v_cms: SpeedCms,
        seg_idx: usize,
        snapped: bool,
    },
    Rejected(&'static str),
    Outage,
    DrOutage {
        s_cm: DistCm,
        v_cms: SpeedCms,
    },
    /// GPS is off-route — position frozen, awaiting re-acquisition
    OffRoute {
        last_valid_s: DistCm,
        last_valid_v: SpeedCms,
        freeze_time: TimestampMs,
    },
    /// GPS is suspect off-route — position frozen, awaiting confirmation
    SuspectOffRoute {
        s_cm: DistCm,
        v_cms: SpeedCms,
    },
    /// GPS is acquiring initial route lock (cold boot)
    Acquiring {
        seg_idx: usize,
        match_d2: i64,
        heading_constraint_met: bool,
    },
}
```

- [ ] **Step 2: Run tests to verify enum change compiles**

Run: `cargo test -p gps_processor --lib`

Expected: PASS (enum change is backward compatible)

- [ ] **Step 3: Commit**

```bash
git add crates/pipeline/gps_processor/src/kalman/mod.rs
git commit -m "feat(kalman): add Acquiring variant to ProcessResult

Per spec 2026-05-27-acquiring-state-design.md:
- Add Acquiring variant for cold boot state
- Includes seg_idx, match_d2, heading_constraint_met
- Distinguishes cold start from off-route scenarios"
```

---

## Task 4: Update `process_gps_update` to return `Acquiring` on cold boot

**Files:**
- Modify: `crates/pipeline/gps_processor/src/kalman/mod.rs:71-261`
- Test: `crates/pipeline/tests/` (integration tests)

- [ ] **Step 1: Add cold boot check at start of `process_gps_update`**

After the GPS outage check (after line 83), add cold boot handling:

```rust
pub fn process_gps_update(
    state: &mut KalmanState,
    dr: &mut DrState,
    gps: &GpsPoint,
    route_data: &RouteData,
    _current_time: TimestampMs,
    is_first_fix: bool,
    current_stop_idx: u8,
) -> ProcessResult {
    // 1. Check for GPS outage
    if !gps.has_fix {
        return handle_outage(state, dr, gps.timestamp);
    }

    // 1.5. Handle cold boot acquiring state
    if state.is_cold_boot {
        return handle_cold_boot_acquire(state, dr, gps, route_data, current_stop_idx);
    }
```

- [ ] **Step 2: Implement `handle_cold_boot_acquire()` function**

Add this function before `process_gps_update`:

```rust
/// Handle cold boot acquisition: await 2 good matches with heading constraint
fn handle_cold_boot_acquire(
    state: &mut KalmanState,
    dr: &mut DrState,
    gps: &GpsPoint,
    route_data: &RouteData,
    current_stop_idx: u8,
) -> ProcessResult {
    // Convert GPS to absolute coordinates
    let (gps_x, gps_y) = crate::map_match::latlon_to_cm_absolute_with_lat_avg(
        gps.lat,
        gps.lon,
        route_data.lat_avg_deg,
    );

    // Map match with relaxed heading (cold start)
    let (seg_idx, match_d2) = crate::map_match::find_best_segment_restricted(
        gps_x,
        gps_y,
        gps.heading_cdeg.unwrap_or(i16::MIN),
        gps.speed_cms.unwrap_or(0),
        route_data,
        state.last_seg_idx,
        true, // relaxed heading during cold boot
    );

    // Check heading constraint
    let heading_constraint_met = crate::map_match::check_heading_constraint(
        gps.heading_cdeg.unwrap_or(i16::MIN),
        seg_idx,
        route_data,
    );

    // Increment clear counter on good match
    if match_d2 <= OFF_ROUTE_D2_THRESHOLD && heading_constraint_met {
        state.off_route_clear_ticks = state.off_route_clear_ticks.saturating_add(1);
    } else {
        state.off_route_clear_ticks = 0;
    }

    // After 2 consecutive good matches, snap to route and enter Valid
    if state.off_route_clear_ticks >= 2 {
        // Project to route for snap position
        let z_reentry = crate::map_match::project_to_route(gps_x, gps_y, seg_idx, route_data);

        // Snap: s_cm = z_reentry anywhere on route (no min_s constraint during cold boot)
        state.s_cm = z_reentry;
        state.is_cold_boot = false;  // Clear cold boot flag
        state.frozen_s_cm = None;
        state.off_route_suspect_ticks = 0;
        state.off_route_clear_ticks = 0;

        // Initialize velocity from GPS
        let v_gps = gps.speed_cms.unwrap_or(0).clamp(0, V_MAX_CMS);
        state.v_cms = state.v_cms + 3 * (v_gps - state.v_cms) / 10;
        state.last_seg_idx = seg_idx;
        dr.last_gps_time = Some(gps.timestamp);
        dr.last_valid_s = state.s_cm;
        dr.filtered_v = state.v_cms;

        let signals = PositionSignals {
            z_gps_cm: z_reentry,
            s_cm: state.s_cm,
        };
        return ProcessResult::Valid {
            signals,
            v_cms: state.v_cms,
            seg_idx,
            snapped: true,
        };
    }

    // Still acquiring: return Acquiring result
    dr.last_gps_time = Some(gps.timestamp);
    ProcessResult::Acquiring {
        seg_idx,
        match_d2,
        heading_constraint_met,
    }
}
```

- [ ] **Step 3: Update `handle_outage` to clear `is_cold_boot` on GPS outage**

Modify the `handle_outage` function to clear the cold boot flag:

```rust
fn handle_outage(state: &mut KalmanState, dr: &mut DrState, timestamp: TimestampMs) -> ProcessResult {
    let dt = match dr.last_gps_time {
        Some(t) => timestamp.saturating_sub(t) / 1000,
        None => return ProcessResult::Rejected("no previous fix"),
    };

    // Clear cold boot flag on GPS outage (shouldn't happen, but safety)
    state.is_cold_boot = false;

    if dt > 10 {
        // Set recovery flag even for long outages to allow relaxed heading filter
        // on first GPS fix after recovery. This improves map matching when GPS
        // heading is unreliable after extended signal loss.
        dr.in_recovery = true;
        return ProcessResult::Outage;
    }

    // ... rest of function unchanged
```

- [ ] **Step 4: Run tests to verify logic compiles**

Run: `cargo test -p gps_processor --lib`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/pipeline/gps_processor/src/kalman/mod.rs
git commit -m "feat(kalman): add cold boot acquiring flow

Per spec 2026-05-27-acquiring-state-design.md:
- Add handle_cold_boot_acquire() function
- Return Acquiring result during cold boot
- After 2 good matches + heading, snap and enter Valid
- Clear is_cold_boot flag on successful snap"
```

---

## Task 5: Add "acquiring" status serialization to output

**Files:**
- Modify: `crates/pipeline/gps_processor/src/output.rs:51-172`
- Test: (manual trace verification)

- [ ] **Step 1: Add `Acquiring` case to `s_cm_for_active` match**

Modify the match statement at line 61:

```rust
    let s_cm_for_active = match result {
        super::kalman::ProcessResult::Valid { signals, .. } => Some(signals.s_cm),
        super::kalman::ProcessResult::DrOutage { s_cm, .. } => Some(*s_cm),
        super::kalman::ProcessResult::OffRoute { last_valid_s, .. } => Some(*last_valid_s),
        super::kalman::ProcessResult::SuspectOffRoute { s_cm, .. } => Some(*s_cm),
        super::kalman::ProcessResult::Acquiring { .. } => Some(0), // Cold boot: s_cm = 0
        _ => None,
    };
```

- [ ] **Step 2: Add `Acquiring` case to `OutputRecord` match**

Add before the closing brace of the `match result` statement (after line 171):

```rust
        super::kalman::ProcessResult::Acquiring {
            seg_idx,
            match_d2,
            heading_constraint_met,
        } => OutputRecord {
            time,
            lat,
            lon,
            s_cm: 0,
            v_cms: 0,
            heading_cdeg: Some(heading_cdeg),
            status: "acquiring".to_string(),
            seg_idx: Some(*seg_idx),
            active_stops,
            stop_states,
            gps_jump: false,
            recovery_idx: None,
        },
```

- [ ] **Step 3: Run tests to verify output changes compile**

Run: `cargo test -p gps_processor --lib output`

Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/pipeline/gps_processor/src/output.rs
git commit -m "feat(output): add acquiring status serialization

Per spec 2026-05-27-acquiring-state-design.md:
- Add \"acquiring\" status for Acquiring result
- s_cm = 0 during acquiring
- Include seg_idx in trace output"
```

---

## Task 6: Verify with integration test

**Files:**
- Test: `crates/pipeline/tests/scenarios/normal.rs` (or create new test)
- Test data: `test_data/` (existing traces)

- [ ] **Step 1: Run existing integration tests to verify no regression**

Run: `cargo test -p pipeline --test integration_test`

Expected: PASS (existing tests still pass)

- [ ] **Step 2: Generate a test trace with cold boot scenario**

Run: `make run ROUTE_NAME=ty225 SCENARIO=normal > test_trace.jsonl`

Expected: Trace should show "acquiring" status on first few GPS updates, then "valid" after snap

- [ ] **Step 3: Verify trace output format**

Check that the trace contains:
```json
{
  "status": "acquiring",
  "s_cm": 0,
  "v_cms": 0,
  "seg_idx": 29
}
```

After snap:
```json
{
  "status": "valid",
  "s_cm": 58687,
  "v_cms": 47
}
```

- [ ] **Step 4: Commit verification results**

```bash
git add docs/superpowers/plans/2026-05-27-acquiring-state-implementation.md
git commit -m "test(acquiring): verify cold boot flow with integration test

Confirmed:
- \"acquiring\" status shows on cold boot
- s_cm = 0 during acquiring
- After 2 good matches + heading, snaps to valid position
- Transitions to \"valid\" with actual s_cm"
```

---

## Self-Review Checklist

- [ ] **Spec coverage:**
  - `is_cold_boot` field added to `KalmanState` ✓ (Task 1)
  - `is_cold_start()` helper added ✓ (Task 2)
  - `Acquiring` variant added to `ProcessResult` ✓ (Task 3)
  - Cold boot flow implemented in `process_gps_update` ✓ (Task 4)
  - "acquiring" status serialized in output ✓ (Task 5)

- [ ] **No placeholders:**
  - All code blocks contain actual implementation ✓
  - All commands are exact ✓
  - All file paths are exact ✓

- [ ] **Type consistency:**
  - `is_cold_boot: bool` consistent across all uses ✓
  - `ProcessResult::Acquiring` fields match usage ✓
  - Function signatures match declarations ✓

---

## Execution Notes

**Key implementation detail:** The `is_cold_boot` flag is set to `true` in `KalmanState::init()` (called on first GPS fix) and cleared after successful snap. This explicitly marks cold boot vs. off-route recovery, allowing snap logic to use different constraints (anywhere on route for cold boot, forward-only for recovery).

**Test strategy:** Run existing integration tests first to ensure no regression. The cold boot flow is a new path that doesn't affect existing warm-start behavior.

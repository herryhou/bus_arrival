# Off-Route Detection and Handling

**Date:** 2026-04-14
**Status:** Design Approved
**Related Issues:** E7 - Recovery Triggers Cannot Fire Even If Wired Up

---

## Overview

**Purpose:** Detect and handle sustained off-route conditions where GPS signal is present but consistently doesn't fit route geometry.

**Scope:**
- Adds off-route detection to the GPS processing pipeline
- Freezes position during off-route episodes
- Re-acquires with recovery when GPS returns to route
- Does NOT detect along-route drift (documented limitation)

**Problem Statement:**

The current system handles three GPS situations:

| Situation | Status | Detection |
|-----------|--------|-----------|
| **GPS outage** | ✅ Handled | `has_fix = false`, DR kicks in |
| **GPS noise spike** | ✅ Handled | Speed constraint rejects outliers |
| **Sustained off-route** | ❌ **Not handled** | GPS present but consistently doesn't fit route geometry |

Sustained off-route has two real-world causes:
- Deep urban canyon with multipath causing GPS drift away from road
- Bus physically deviated (detour, depot, wrong route loaded)

The system cannot distinguish these causes — and shouldn't need to. The correct response is the same: **freeze position and wait**.

**Key Design Decisions:**
- Configurable threshold (compile-time constant)
- Silent handling — no operator alerts
- Hysteresis: 5 ticks to confirm, 2 ticks to clear
- Freeze position — don't advance DR during off-route

---

## Architecture

### System State Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                    State Machine States                     │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌──────────┐    match_d2 > 25M for 5 ticks    ┌─────────┐ │
│  │  Normal  │──────────────────────────────────→│ Suspended│ │
│  │Operation│                                   │ (Off-   │ │
│  │          │←──────────────────────────────────│  Route) │ │
│  └──────────┘    match_d2 < 25M for 2 ticks     └────┬────┘ │
│       ▲                                              │       │
│       │              match_d2 < 25M for 2 ticks    │       │
│       │                                              ▼       │
│       │                                    ┌─────────────────┐│
│       │                                    │  Re-acquisition  ││
│       │                                    │  (run recovery)  ││
│       │                                    └─────────────────┘│
│       └──────────────────────────────────────────────────────┘
│                                                             │
│ Linear chain: Normal → Suspended → Re-acquisition → Normal │
└─────────────────────────────────────────────────────────────┘
```

### New State Fields

**In `KalmanState` (shared/src/lib.rs):**
```rust
pub struct KalmanState {
    // ... existing fields ...

    /// Consecutive ticks with match_d2 > threshold
    off_route_suspect_ticks: u8,

    /// Consecutive ticks with match_d2 < threshold
    off_route_clear_ticks: u8,
}
```

### New ProcessResult Variant

**In `ProcessResult` (shared/src/lib.rs):**
```rust
pub enum ProcessResult {
    Valid {
        signals: PositionSignals,
        v_cms: SpeedCms,
        seg_idx: usize,
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
    },
}
```

**Note:** `match_d2` is NOT added to `ProcessResult::Valid` — it's only used internally in `kalman.rs` for the off-route threshold check.

---

## Component Changes

### Affected Modules

1. **`map_match.rs`**
   - Change `find_best_segment_restricted()` return type: `(usize, i64)`
   - Extract pure distance² without heading penalty

2. **`shared/src/lib.rs`**
   - Add `off_route_suspect_ticks`, `off_route_clear_ticks` to `KalmanState`
   - Add `ProcessResult::OffRoute` variant
   - Note: These types are defined in shared, not kalman.rs

3. **`kalman.rs`**
   - Update `find_best_segment_restricted()` call to capture `(seg_idx, match_d2)`
   - Add off-route detection logic after map matching
   - Return `OffRoute` when threshold exceeded for 5 ticks
   - Reset both counters in `handle_outage()`
   - Disable detection during warmup (check in_warmup flag)

4. **`state.rs`**
   - Add `needs_recovery_on_reacquisition` and `off_route_freeze_time` fields to `State`
   - Handle `ProcessResult::OffRoute` variant
   - Implement re-acquisition recovery logic with elapsed time calculation
   - Freeze position, suspend detection during off-route

5. **`recovery.rs`**
   - No changes (existing `find_stop_index()` will be used)

### Configurable Constants

```rust
/// Off-route distance threshold (cm²) — 50m² = 25,000,000 cm²
const OFF_ROUTE_D2_THRESHOLD: i64 = 25_000_000;

/// Ticks to confirm off-route (avoid false positives from multipath)
const OFF_ROUTE_CONFIRM_TICKS: u8 = 5;

/// Ticks to clear off-route (fast re-acquisition)
const OFF_ROUTE_CLEAR_TICKS: u8 = 2;
```

---

## Data Flow

### Normal Operation Flow

```
GPS → NMEA parse → process_gps_update()
                    ↓
            find_best_segment_restricted()
            returns (seg_idx, match_d2)
                    ↓
            Check: match_d2 > OFF_ROUTE_D2_THRESHOLD?
                    ↓
         No → Continue → Kalman update → Return Valid
         Yes → Increment suspect_ticks
                Check: suspect_ticks >= 5?
                    ↓
                 Yes → Return OffRoute {last_valid_s, last_valid_v}
```

### OffRoute Flow

```
state.rs receives ProcessResult::OffRoute
    ↓
Set needs_recovery_on_reacquisition = true
    ↓
Freeze position (do NOT advance DR)
    ↓
Suspend arrival detection
    ↓
Return None (no event)
```

### Re-acquisition Flow

```
GPS returns to route (match_d2 < threshold for 2 ticks)
    ↓
kalman.rs returns ProcessResult::Valid
    ↓
state.rs checks needs_recovery_on_reacquisition
    ↓
Calculate elapsed_seconds = gps.timestamp - off_route_freeze_time
    ↓
Call recovery::find_stop_index(s_cm, v_cms, elapsed_seconds, stops, last_idx)
    ↓
Apply recovered stop index (or continue with existing if recovery returns None)
    ↓
Clear off_route_freeze_time, resume normal operation
```

---

## Error Handling & Edge Cases

### Error Handling

1. **Recovery fails to find stop index**
   - Continue with existing stop states
   - Better to have stale state than corrupt with bad guess
   - System will eventually correct via normal operation

2. **Off-route persists for extended time**
   - Position remains frozen at last_valid_s
   - No arrival detection (correct behavior)
   - Resumes when GPS returns to route

3. **Repeated off-route cycles**
   - Each cycle triggers recovery on re-acquisition
   - State is re-synchronized each time
   - No accumulated state corruption

### Edge Cases

1. **Route start/end**
   - Off-route detection works normally
   - Recovery may return None if no stop is within 200m (spatial miss)
   - Continue with existing stop states; normal operation self-corrects

2. **Sparse stop regions**
   - Distance to nearest stop may be > 200m
   - But off-route uses distance to route segment, not stops
   - Detection works independently of stop density

3. **Near depot / route origin**
   - Same logic applies
   - Recovery handles re-synchronization

4. **Simultaneous GPS outage + off-route**
   - Outage takes precedence (checked first in process_gps_update)
   - **Off-route counters MUST be explicitly reset in handle_outage()**
   - This prevents immediate re-trigger on GPS recovery when positions are good

5. **First GPS fix ever / Warmup period**
   - Off-route detection MUST be disabled during warmup
   - Cold-start with noisy first fix could trigger false off-route
   - Detection only enabled after WARMUP_TICKS_REQUIRED (3 ticks)

### Documented Limitations

- Cannot detect along-route drift (GPS stays on road but advances faster than bus)
- Single GPS sensor cannot distinguish urban canyon vs physical deviation
- These are fundamental limitations, not implementation bugs

---

## Testing Strategy

### Unit Tests

1. **Map matching score extraction** (`map_match.rs`)
   - Verify `(seg_idx, d²)` returns pure distance without heading penalty
   - Test that d² is geometric distance only

2. **Off-route detection logic** (`kalman.rs`)
   - `test_off_route_confirm_after_5_ticks()`: Confirm at exactly 5 ticks
   - `test_off_route_no_trigger_before_5_ticks()`: No trigger at 4 ticks
   - `test_off_route_hysteresis()`: 5 to confirm, 2 to clear
   - `test_off_route_counter_resets_on_outage()`: Outage clears state

3. **State machine handling** (`state.rs`)
   - `test_off_route_freezes_position()`: Verify DR doesn't advance
   - `test_off_route_suspends_detection()`: No arrival events during off-route
   - `test_re_acquisition_runs_recovery()`: Recovery called on return

### Integration Tests

1. **Full off-route cycle** (`test_recovery_integration.rs`)
   - Generate synthetic NMEA with urban canyon drift
   - Verify: Normal → OffRoute → Re-acquisition → Normal

2. **Synthetic scenarios** (new test module)
   - Urban canyon: GPS drifts 50-100m away for 10+ seconds
   - Quick multipath: Brief spike that doesn't trigger (tests hysteresis)
   - Re-acquisition: Return to route with position jump

### Test Data Generation

- Create NMEA sequences simulating off-route conditions
- Use known route (ty225) for realistic geometry
- Parametric tests for different thresholds and durations

### Validation

- All existing tests must continue passing
- No regression in GPS jump or restart recovery
- Off-route detection should not interfere with normal operation

---

## Implementation Tasks

- [ ] Add `off_route_suspect_ticks`, `off_route_clear_ticks` to `KalmanState` (shared/src/lib.rs)
- [ ] Add `ProcessResult::OffRoute` variant to `ProcessResult` enum (shared/src/lib.rs)
- [ ] Add `needs_recovery_on_reacquisition`, `off_route_freeze_time` to `State` (state.rs)
- [ ] Change `find_best_segment_restricted` return type to `(usize, i64)` (map_match.rs)
- [ ] Update `find_best_segment_restricted()` call in `process_gps_update` to capture `match_d2` (kalman.rs)
- [ ] Add off-route detection logic in `process_gps_update` with hysteresis (kalman.rs)
- [ ] Add warmup guard: disable off-route detection during warmup (kalman.rs)
- [ ] Reset off-route counters in `handle_outage()` (kalman.rs)
- [ ] Handle `ProcessResult::OffRoute` in `state.rs` process_gps
- [ ] Implement re-acquisition recovery logic with elapsed time calculation (state.rs)
- [ ] Unit tests for off-route detection hysteresis (kalman.rs tests)
- [ ] Unit tests for counter reset on outage (kalman.rs tests)
- [ ] Unit tests for warmup guard preventing false triggers (kalman.rs tests)
- [ ] Integration tests for full off-route → re-acquisition cycle (state.rs tests)
- [ ] Document limitation: cannot detect along-route drift

---

## References

- Code Review: [docs/claude_review.md](../claude_review.md) - E7, D3 (heading penalty issue)
- Tech Report v8.9: [docs/bus_arrival_tech_report_v8.md](../bus_arrival_tech_report_v8.md) (Section 15)
- Related: H2 (flash persistence), existing recovery module
- GitHub Issue: https://github.com/herryhou/bus_arrival/issues/1

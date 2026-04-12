# I5: Warmup Counter Stuck on Repeated Rejections

**Date:** 2026-04-12
**Status:** Design Approved
**Related:** Code review finding in `docs/claude_review.md`

---

## Overview

**Goal:** Fix warmup counter to advance even when GPS is rejected, preventing permanent stuck state when initial GPS quality is poor.

**Scope:**
- Replace single `warmup_counter` with two-track system
- `warmup_valid_ticks`: Counts only valid GPS with Kalman updates (convergence)
- `warmup_total_ticks`: Counts all GPS ticks including first fix (timeout safety valve)
- Detection enables when either threshold is met

**Files affected:**
- `crates/pico2-firmware/src/state.rs` - Update warmup logic and state struct

---

## Problem Statement

Per code review I5: When GPS samples are repeatedly rejected (e.g., large initial position error causing speed constraint failures), `warmup_counter` stays at 0 forever and the system never enters normal detection.

**Current behavior:**
```rust
ProcessResult::Valid { .. } => {
    // warmup_counter increments here
    if warmup_counter < 3 {
        warmup_counter += 1;
        return None;
    }
}
ProcessResult::Rejected(_) => {
    return None;  // warmup_counter never increments!
}
```

**Root cause analysis:**

Warmup exists to allow the Kalman filter to converge to stable position and velocity estimates. The Kalman has two operations:

- **Predict step** (`ŝ += v̂`) — happens every tick regardless of GPS quality
- **Measurement update** — only happens when GPS is accepted (runs `update_adaptive`)

Warmup is meaningful only if measurement updates are happening. Three consecutive `Rejected` ticks leave the Kalman in dead-reckoning mode — `v̂` is extrapolated from the first fix, `ŝ` drifts, and covariance inflates. The filter has NOT converged.

The stuck scenario typically occurs when:
1. First fix initializes `s_cm` at a noisy position
2. Subsequent correct GPS samples are rejected by speed constraint (look like jumps from bad starting point)
3. Warmup counter stays at 0 while Kalman drifts from wrong starting point

---

## Solution Design

### Two-Counter System

Separate two independent concerns that a single counter conflates:

```rust
pub struct State<'a> {
    // ... existing fields
    /// Number of valid GPS ticks with Kalman updates (convergence counter)
    warmup_valid_ticks: u8,
    /// Total ticks since first fix (timeout safety valve)
    warmup_total_ticks: u8,
    // ... other fields
}
```

**`warmup_valid_ticks`**: Counts only valid GPS where Kalman `update_adaptive` runs. Represents filter convergence progress.

**`warmup_total_ticks`**: Counts all GPS ticks including first fix. Represents elapsed time as a safety valve.

### Update Logic

**Constants:**
```rust
const WARMUP_TICKS_REQUIRED: u8 = 3;   // Kalman convergence requirement
const WARMUP_TIMEOUT_TICKS: u8 = 10;   // Maximum wait time (matches DR outage tolerance)
```

**Per-result-type behavior:**

| ProcessResult | warmup_valid_ticks | warmup_total_ticks | Detection |
|---------------|-------------------|-------------------|-----------|
| Valid | +1 | +1 | Enable if valid≥3 OR total≥10 |
| Rejected | (unchanged) | +1 | Blocked |
| DrOutage | (unchanged) | +1 | Proceeds to detection |
| Outage | →0 | →0 | Blocked, reset |

**First fix special case:**
The first-fix path bypasses speed constraint, monotonicity check, and `update_adaptive`. It's a raw state initialization, not a Kalman measurement update. Therefore:
- `warmup_total_ticks += 1` (time has elapsed)
- `warmup_valid_ticks` unchanged (no convergence contribution)

This matches spec wording "3 GPS ticks **after** first fix" — the first fix is the reference point, not one of the three counted ticks.

### Detection Enable Condition

```rust
// After first_fix and warmup_just_reset checks:
self.warmup_total_ticks = self.warmup_total_ticks.saturating_add(1);

if self.warmup_valid_ticks < WARMUP_TICKS_REQUIRED {
    self.warmup_valid_ticks += 1;

    // Only block detection if total time hasn't expired
    if self.warmup_total_ticks < WARMUP_TIMEOUT_TICKS {
        return None;
    }
}
// Proceed to detection
```

Detection is enabled when **either** condition is met:
- `warmup_valid_ticks >= 3`: Kalman has converged (normal case)
- `warmup_total_ticks >= 10`: Timeout safety valve (noisy startup case)

### Why 10 Seconds for Timeout

The DR module tolerates a 10-second outage before entering `GPS_LOST`. A warmup timeout longer than 10 seconds would mean the `Outage` branch fires and resets counters before the timeout can release detection — an impossible state.

Keeping both at 10 seconds shares the same invariant: "10 seconds is the maximum uncertainty we tolerate."

---

## State Transitions

```
[first_fix]
    ↓
warmup_total_ticks = 1, warmup_valid_ticks = 0
    ↓
[Valid] → total++, valid++
    ↓
[Valid] → total++, valid++
    ↓
[Valid] → total++, valid++ → valid=3 → DETECTION ENABLED

OR

[first_fix]
    ↓
warmup_total_ticks = 1, warmup_valid_ticks = 0
    ↓
[Rejected] → total++ (valid unchanged)
    ↓
[Rejected] → total++ (valid unchanged)
    ↓
... (repeated rejections) ...
    ↓
[Rejected] → total++ → total=10 → DETECTION ENABLED
```

---

## Implementation Notes

### Code Changes

**In `State` struct initialization:**
```rust
Self {
    // ... existing fields
    warmup_valid_ticks: 0,
    warmup_total_ticks: 0,
    warmup_just_reset: false,
    // ...
}
```

**In `process_gps` method, `ProcessResult::Valid` branch:**
```rust
ProcessResult::Valid { signals, v_cms, seg_idx: _ } => {
    // ... existing recovery and first_fix handling ...

    if self.first_fix {
        self.first_fix = false;
        self.warmup_total_ticks = 1;  // First fix counts toward timeout only
        return None;
    }

    if self.warmup_just_reset {
        self.warmup_just_reset = false;
        return None;
    }

    // Increment total time counter
    self.warmup_total_ticks = self.warmup_total_ticks.saturating_add(1);

    // Check convergence requirement
    if self.warmup_valid_ticks < WARMUP_TICKS_REQUIRED {
        self.warmup_valid_ticks += 1;

        // Block detection unless timeout expired
        if self.warmup_total_ticks < WARMUP_TIMEOUT_TICKS {
            return None;
        }
    }

    // Proceed to detection
    // ... existing recovery tracking and return ...
}
```

**In `ProcessResult::Rejected` branch:**
```rust
ProcessResult::Rejected(reason) => {
    #[cfg(feature = "firmware")]
    defmt::warn!("GPS update rejected: {}", reason);

    // Increment timeout counter even on rejection
    if !self.first_fix {
        self.warmup_total_ticks = self.warmup_total_ticks.saturating_add(1);
    }

    return None;  // Still block detection
}
```

**In `ProcessResult::Outage` branch:**
```rust
ProcessResult::Outage => {
    #[cfg(feature = "firmware")]
    defmt::warn!("GPS outage exceeded 10 seconds");

    // Reset both counters on true signal loss
    if !self.first_fix {
        self.warmup_valid_ticks = 0;
        self.warmup_total_ticks = 0;
        self.warmup_just_reset = true;
        #[cfg(feature = "firmware")]
        defmt::debug!("GPS outage reset warmup counters");
    }
    return None;
}
```

**`DrOutage` branch remains unchanged** — it proceeds to detection directly without touching warmup counters.

---

## Testing

### Unit Tests

**Test 1: Normal warmup (3 valid GPS)**
```
first_fix → total=1, valid=0
Valid → total=2, valid=1 → blocked
Valid → total=3, valid=2 → blocked
Valid → total=4, valid=3 → ENABLED
```

**Test 2: Noisy startup (timeout path)**
```
first_fix → total=1, valid=0
Rejected → total=2, valid=0
Rejected → total=3, valid=0
... (7 more rejections) ...
Rejected → total=10, valid=0 → ENABLED (timeout)
```

**Test 3: Mixed valid and rejected**
```
first_fix → total=1, valid=0
Valid → total=2, valid=1
Rejected → total=3, valid=1
Valid → total=4, valid=2
Rejected → total=5, valid=2
Rejected → total=6, valid=2
Rejected → total=7, valid=2
Rejected → total=8, valid=2
Rejected → total=9, valid=2
Valid → total=10, valid=3 → ENABLED (convergence)
```

**Test 4: Outage resets counters**
```
(total=5, valid=2) → Outage → (total=0, valid=0)
```

**Test 5: First fix doesn't count toward valid**
```
first_fix → total=1, valid=0 (valid NOT incremented)
```

---

## Verification

### Success Criteria

- [ ] Two-counter system implemented
- [ ] First fix increments `warmup_total_ticks` only
- [ ] Valid GPS increments both counters
- [ ] Rejected GPS increments `warmup_total_ticks` only
- [ ] Outage resets both counters
- [ ] Detection enables when `valid >= 3` OR `total >= 10`
- [ ] All unit tests pass
- [ ] No regression in existing warmup behavior

### Expected Behavior Changes

**Before fix:**
- Repeated GPS rejections → warmup stuck at 0 → permanent detection block

**After fix:**
- Repeated GPS rejections → timeout counter advances → detection enables after 10 seconds
- Normal warmup (3 valid GPS) unchanged
- True outage behavior unchanged (resets counters)

---

## Summary

| Change | Impact |
|--------|--------|
| Two-counter system | Separates convergence from timeout |
| warmup_valid_ticks | Counts only valid GPS (Kalman convergence) |
| warmup_total_ticks | Counts all ticks (timeout safety valve) |
| Rejections increment total | Prevents permanent stuck state |
| 10-second timeout | Matches DR outage tolerance |

**Files to modify:** 1 file (`state.rs`)
**New constants:** 2 (`WARMUP_TICKS_REQUIRED`, `WARMUP_TIMEOUT_TICKS`)
**Tests:** 5+ unit tests for various scenarios

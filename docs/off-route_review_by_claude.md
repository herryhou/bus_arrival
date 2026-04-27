# Consolidated Review: Off-Route / Re-entry & Adjacent Correctness

**Target:** `pico2_firmware_source.txt` + `bus_arrival_tech_report_v8.md` (v8.9)
**Focus:** Module ⑬ off-route detection, re-entry snap, downstream coupling

---

## 🔴 Critical (runtime bugs, data corruption)

---

### C1 — `off_route_freeze_time` cleared before recovery call → `elapsed_seconds = 1`

**File:** `state.rs`, Valid branch (~line 969)
**Mechanism:** On the re-entry tick, `ProcessResult::Valid` is returned. The Valid branch executes two sequential recovery paths:

1. **H1 jump check** (`should_trigger_recovery`) fires because `snapped_s_cm` vs `last_valid_s_cm` is a large jump. Calls `find_stop_index` with correct `dt_since_last_fix`. Calls `reset_stop_states_after_recovery` → result A.

2. **`needs_recovery_on_reacquisition` check** — flag is true (set by the preceding `OffRoute` result). Attempts `elapsed_seconds = gps.timestamp.saturating_sub(self.kalman.off_route_freeze_time.unwrap_or(...))`. But `off_route_freeze_time` was already cleared inside `update_off_route_hysteresis` at the Normal transition (line 3510). Falls back to `1`. Calls `find_stop_index` with `dt = 1` — velocity exclusion window collapses to 1667 cm (one second of travel). Any stop more than ~16 m ahead of re-entry is hard-excluded. Calls `reset_stop_states_after_recovery` again → result B overwrites result A.

**Consequence:** The correct recovery from path 1 is silently destroyed. The second call returns `None` more often (too-tight velocity window), leaving `last_known_stop_index` stale; or returns a wrong nearby stop.

**Fix:**
```rust
// Option A: gate the needs_recovery path if H1 already ran
if self.needs_recovery_on_reacquisition && !h1_recovery_ran {
    ...
}

// Option B: preserve off_route_freeze_time until after the needs_recovery block
// (don't clear it inside update_off_route_hysteresis; clear it here instead)
```

---

### C2 — `reset_off_route_state` does not clear `frozen_s_cm` → spurious re-entry snap

**File:** `kalman.rs`, `reset_off_route_state` (~line 3523)

```rust
pub fn reset_off_route_state(state: &mut KalmanState) {
    state.off_route_suspect_ticks = 0;
    state.off_route_clear_ticks = 0;
    state.off_route_freeze_time = None;
    // ← frozen_s_cm is NOT cleared
}
```

`reset_off_route_state` is called from `handle_outage` (GPS DR timeout path). Two failure scenarios:

**Scenario A — GPS outage during Suspect (ticks 1–4):**
Counters are zeroed, `frozen_s_cm` remains `Some`. On GPS recovery, `is_actually_suspect = off_route_suspect_ticks > 0 || frozen_s_cm.is_some()` is true via the second condition. After 2 good ticks, hysteresis clears `frozen_s_cm`, sets `had_frozen_position = true, frozen_s_cm = None`, triggers the full re-entry snap. The bus was never confirmed off-route; the snap is spurious.

**Scenario B — GPS outage > 10 s during confirmed OffRoute:**
`handle_outage` returns `ProcessResult::Outage` before reaching `reset_off_route_state`, so neither counters nor `frozen_s_cm` are cleared. `frozen_s_cm` persists indefinitely through the outage. Snap fires on GPS recovery using a freeze-point that may be minutes stale. Behavior is arguably correct but completely undocumented.

**Fix:**
```rust
pub fn reset_off_route_state(state: &mut KalmanState) {
    state.off_route_suspect_ticks = 0;
    state.off_route_clear_ticks = 0;
    state.off_route_freeze_time = None;
    state.frozen_s_cm = None;  // ← add this
}
```

For Scenario B, use a separate `confirmed_off_route: bool` flag if the re-entry snap on long outage recovery is desired behavior.

---

### C3 — Recovery has no pre-freeze spatial anchor

**File:** `recovery.rs`, `find_stop_index`

Recovery receives `last_index` (the pre-freeze stop index) only as a backward-exclusion filter: `i >= last_index.saturating_sub(1)`. It does not use the frozen `s_cm` or the direction of travel before freeze. The velocity constraint `dist_to_stop > V_MAX_CMS * dt` is the only forward bound — and as shown in C1, `dt` frequently collapses to 1.

**Consequence:** On routes with loops, parallel segments, or closely-spaced stops, recovery can select a stop cluster that is physically inconsistent with the pre-freeze trajectory. The bus rejoining the route at stop 7 (having frozen at stop 2) could recover to stop 3 if it happens to be geometrically close to the re-entry GPS point.

**Fix:** Store a `FreezeContext` at freeze time and pass it to recovery:

```rust
pub struct FreezeContext {
    pub frozen_s_cm: DistCm,
    pub frozen_stop_idx: u8,
}

pub fn find_stop_index(
    s_cm: DistCm,
    v_filtered: SpeedCms,
    dt_since_last_fix: u64,
    stops: &[Stop],
    freeze_ctx: &FreezeContext,
) -> Option<usize> {
    // Prefer candidates at s_cm >= freeze_ctx.frozen_s_cm - TOLERANCE
    // Apply larger penalty for stops behind frozen_stop_idx
    ...
}
```

---

## 🟠 Medium (incorrect behavior under specific conditions)

---

### M1 — `ProcessResult::DrOutage` reused for Suspect → can unblock detection with frozen position

**File:** `kalman.rs` ~line 3628, `state.rs` DrOutage branch

During Suspect (ticks 1–4), the function returns `ProcessResult::DrOutage { s_cm: frozen, v_cms }`. This is indistinguishable from genuine GPS signal loss to all callers. Consequences:

- `warmup_total_ticks` increments each Suspect tick (timeout counter)
- If Suspect lasts ≥ `WARMUP_TIMEOUT_TICKS` ticks, the timeout expires and detection unblocks with a frozen `s_cm`, enabling false arrivals
- Trace output reports `"dr_outage"` for what is a route-departure event, making post-hoc debugging impossible

**Fix:** Introduce `ProcessResult::SuspectOffRoute { s_cm, v_cms }` and handle it identically to `OffRoute` in `state.rs` (return `None`, do not increment warmup timeout).

---

### M2 — `OffRoute` and `DrOutage` hit same partial neutralization in probability

**File:** `probability.rs` (~line 198 comment: "Neutralize to 128 during dr_outage or off_route")

Both statuses neutralize only `p3` to 128. For `DrOutage` this is correct — the model is still valid, only the GPS observation is degraded. For `OffRoute` the vehicle is not on the route at all; the entire probability computation is meaningless. `p1` (raw GPS distance), `p2` (speed), and `p4` (dwell) are still partially computed and can push the fused probability above threshold 191.

This is masked in the normal code path because `OffRoute` causes `return None` before detection runs. But the Suspect path (M1 above) allows detection to run with frozen `s_cm` and `DrOutage` status — hitting this partial neutralization instead of full suppression.

**Fix:**
```rust
if gps_status == GpsStatus::OffRoute {
    return 0; // entire probability invalid
}
```

---

### M3 — Re-entry snaps `v_cms` from raw GPS speed, bypassing EMA

**File:** `kalman.rs` ~line 3657

```rust
state.v_cms = gps.speed_cms;
```

The first GPS measurement after re-entry is typically the worst-quality fix of the sequence (HDOP spike, heading uncertainty from multipath). Setting Kalman velocity directly to raw GPS speed bypasses the EMA smoothing (`3/10` blend) that all other ticks use. A bus stopped at a traffic light near re-entry snaps `v_cms = 0`, causing the next predict step to not advance `s_cm`, holding position for one extra tick. A bus re-entering at speed snaps `v_cms` to an inflated value, overshooting the next predict.

**Fix:**
```rust
// Blend instead of hard assignment
state.v_cms = state.v_cms + (3 * (gps.speed_cms.max(0).min(V_MAX_CMS) - state.v_cms)) / 10;
```

---

### M4 — `_v_filtered` unused in `find_stop_index`; velocity exclusion uses worst-case V_MAX

**File:** `recovery.rs`

```rust
pub fn find_stop_index(
    s_cm: DistCm,
    _v_filtered: SpeedCms,   // "Reserved for future use"
    dt_since_last_fix: u64,
    ...
```

The velocity exclusion bound is `V_MAX_CMS * dt` (worst-case 60 km/h). A bus that was stationary or moving slowly before the detour passes the velocity constraint for stops far ahead of where it could physically be. The actual filtered speed estimate is available but ignored.

**Fix:** Replace worst-case bound with actual estimate:
```rust
let effective_v = if _v_filtered > 0 { _v_filtered as u64 } else { V_MAX_CMS as u64 };
let max_reachable = effective_v * dt_since_last_fix;
```

---

### M5 — Persistence model ignores proximity to off-route events

**File:** `state.rs`, `should_persist` / `mark_persisted`

State is persisted `(s_cm, stop_idx)` when stop index changes, rate-limited to once per 60 s. There is no awareness of off-route status. If the bus is in Suspect state (about to be confirmed off-route) and the stop index just changed, the incorrect position is written to Flash. On restart, the system anchors to the wrong segment.

**Fix:** Gate persistence:
```rust
pub fn should_persist(&self, current_stop: u8) -> bool {
    if self.kalman.frozen_s_cm.is_some() { return false; } // off-route or suspect
    if self.kalman.off_route_suspect_ticks > 0 { return false; }
    ...
}
```

---

## 🟡 Low (documentation errors, latent risks)

---

### L1 — Spec Appendix A and v8.9 changelog list `θ_off-route = 500 m` (should be 50 m)

**File:** `bus_arrival_tech_report_v8.md`, Appendix A and v8.9 parameter table

Code: `OFF_ROUTE_D2_THRESHOLD = 25_000_000 cm² = 5000² cm²` → **50 m**.

Both the appendix parameter table and the v8.9 changelog parameter table say `50,000 cm (500 m)`. Section 16.2 and Section 16.4 body text are correct (5,000 cm / 50 m). The appendix entries need correction:

```
// Wrong:
θ_off-route  |  50,000 cm（500 m）

// Correct:
θ_off-route  |  5,000 cm（50 m）
```

---

### L2 — Trace timestamps out of order in Section 16.7

**File:** `bus_arrival_tech_report_v8.md`, Section 16.7

```jsonl
{"time":80228, ...}    // "Re-entry (Suspect→Normal)"
{"time":80223, ...}    // "Re-entry snap confirmed"
```

80228 precedes 80223 in the listing. GPS timestamps are monotonically increasing. This is a copy-paste ordering error in the documentation.

---

### L3 — `reset_off_route_state` missing `frozen_s_cm` — not documented in contract

**File:** `kalman.rs` docstring for `reset_off_route_state`

The function comment says "Reset off-route counters (called on GPS outage)". It does not state that `frozen_s_cm` is intentionally preserved. This is either a bug (C2 above) or an intentional invariant that must be explicitly documented. Either way the current comment is misleading.

---

## 🟢 Confirmed solid — do not change

These were reviewed against the code and are correctly implemented:

| Component | Reason |
|-----------|--------|
| Filter-then-rank map matching | Fixes unit inconsistency cleanly; deterministic; correct dual-tracker invariants |
| Integer unit system (cm, cdeg, cm/s) | Correct decision for RP2350; type aliases enforce semantics at compile time |
| `heading_eligible` hard gate | Clean separation of filter vs rank; stopping behavior (w=0 → gate open) is correct |
| Speed constraint filter (Module ⑥) | 37 m / 1 s bound is physically sound; reject-then-DR behavior on jump is correct |
| One-time announcement rule | `can_reactivate()` always false is the right call; `announced` flag survives recovery correctly |
| `dwell_time_s` reset on corridor exit | Correctly resets to 0; only increments inside `update()` which is gated by `return None` during OffRoute |
| Adaptive weights for close stops (< 120 m) | p4 removal + 32-normalized rebalance is correct; `next_stop` passed by route order (not active index) is the right choice |
| DP Mapper stop projection | Viterbi-like DAG with snap-forward fallback is globally optimal and handles loops correctly |

---

## Summary Table

| ID | Severity | Component | Issue |
|----|----------|-----------|-------|
| C1 | 🔴 Critical | `state.rs` Valid branch | `off_route_freeze_time` cleared before recovery → `dt=1` → second recovery overwrites first with wrong result |
| C2 | 🔴 Critical | `reset_off_route_state` | `frozen_s_cm` not cleared on GPS outage → spurious re-entry snap after Suspect+Outage |
| C3 | 🔴 Critical | `find_stop_index` | No pre-freeze spatial anchor → wrong cluster on loops, parallels, long detours |
| M1 | 🟠 Medium | `kalman.rs` Suspect path | `DrOutage` reused for Suspect → warmup timeout can unblock detection with frozen position |
| M2 | 🟠 Medium | `probability.rs` | `OffRoute` partially neutralized (p3 only) instead of fully suppressed; exploitable via M1 |
| M3 | 🟠 Medium | Re-entry snap | `v_cms` hard-set from raw GPS speed; bypasses EMA; overshoots or stalls next predict tick |
| M4 | 🟠 Medium | `find_stop_index` | `_v_filtered` unused; velocity exclusion uses worst-case V_MAX, not actual speed |
| M5 | 🟠 Medium | `should_persist` | Persistence ignores off-route/Suspect status; can write wrong anchor to Flash |
| L1 | 🟡 Low | Spec Appendix A + v8.9 changelog | `θ_off-route` listed as 500 m (should be 50 m); code is correct |
| L2 | 🟡 Low | Spec Section 16.7 | Trace timestamps 80228 before 80223 (out of order) |
| L3 | 🟡 Low | `reset_off_route_state` docstring | Missing contract: does not state `frozen_s_cm` is intentionally preserved |
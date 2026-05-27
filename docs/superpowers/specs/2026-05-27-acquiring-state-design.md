# Acquiring State Design

**Date:** 2026-05-27
**Status:** Approved
**Stakeholders:** @herry

## Problem Statement

Current behavior on cold boot (app launch, first GPS fix):
- Android: Returns `s_cm = 0`, enters "suspect" state with meaningless `off_route_last_s_cm = 0`
- Rust: Returns `s_cm = 58687` immediately, enters "valid" state
- Neither handles deadhead scenario properly (bus near route but not in service)

**Root cause:** No "acquiring" state to represent "we have GPS but haven't confirmed bus is on active route."

## Solution

Add "acquiring" state that:
- Behaves like off-route (detection disabled, position frozen)
- Uses `is_cold_boot: bool` flag as explicit marker
- Snaps to route after 2 consecutive good matches with heading constraint
- Transitions to Valid after successful snap

## Architecture

### State Table

| State | `is_cold_boot` | `frozen_s` | `s_cm` | Detection | Re-entry behavior |
|-------|---------------|-----------|--------|-----------|-------------------|
| **Acquiring** | `true` | `None` | 0 | Disabled | Snap anywhere on route |
| **Suspect/OffRoute** | `false` | `Some(actual_pos)` | frozen | Disabled | Snap forward only |
| **Valid** | `false` | `None` | tracking | Enabled | N/A |

### Helper Function

```rust
fn is_cold_start(state: &KalmanState) -> bool {
    state.is_cold_boot
}
```

### Key Insight

`is_cold_boot: bool` is the explicit cold boot marker. When true, snap logic allows re-entry anywhere on route (no `min_s_cm` constraint). No ambiguity with position 0 on route.

## Components & Data Flow

### Rust Changes

| Component | Change |
|-----------|--------|
| `shared/src/lib.rs` | Add `is_cold_boot: bool` to `KalmanState` |
| `mod.rs:process_gps_update` | Set `is_cold_boot = true` on first fix, add `Acquiring` result |
| `hysteresis.rs` | Add `is_cold_start()` helper (returns `state.is_cold_boot`) |
| `output.rs` | Add `"acquiring"` status serialization |
| `ProcessResult` | Add `Acquiring` variant |

### Android Changes

| Component | Change |
|-----------|--------|
| `StateModels.kt` | Add `isColdBoot: Boolean` to `KalmanState` |
| `service/DetectionPipeline.kt` | Set `isColdBoot = true` on first fix, fix init order |
| `service/TraceTick.kt` | Add `"acquiring"` status enum value |
| `detection/hysteresis/Hysteresis.kt` | Add `isColdStart()` helper |

### Cold Boot Flow

```
GPS fix → Map match → Check: is_cold_start()?
                      ↓ Yes
                  is_cold_boot = true
                  Skip detection
                  Await 2 good matches + heading
                      ↓
                  Snap: s_cm = z_reentry
                  is_cold_boot = false
                  Enter Valid
```

## State Transitions

```
┌─────────────┐
│  Cold Boot  │
│is_cold_boot │
│   = true    │
└──────┬──────┘
       │
2× good match + heading?
       │ Yes
┌──────▼──────┐
│    Snap     │
│ s_cm=z_entry│
└──────┬──────┘
       │
is_cold_boot = false
       │
┌──────▼──────┐
│    Valid    │
└─────────────┘
```

### Transition Conditions

| From | To | Condition |
|------|-----|-----------|
| Cold Boot | Valid | 2 consecutive good matches + heading constraint met |
| Valid | Suspect | `match_d2 > threshold` (freeze at actual `s_cm`) |
| Suspect | Valid | 2 consecutive good matches, forward snap succeeds |

**Key distinction:**
- Cold Boot → Valid: `is_cold_boot = true` → snap allowed anywhere on route
- Suspect → Valid: `is_cold_boot = false` → snap forward only from frozen position

## Trace Output Format

### During Acquiring

```json
{
  "gps": {...},
  "kalman": {
    "s_cm": 0,
    "v_cms": 0
  },
  "map_matching": {
    "segment_idx": 29,
    "heading_constraint_met": false
  },
  "detection": {
    "status": "acquiring",
    "off_route": false
  }
}
```

**Note:** `off_route_last_s_cm` is omitted during acquiring (no frozen position yet).

### After Snap (Valid)

```json
{
  "kalman": {
    "s_cm": 58687,
    "v_cms": 47
  },
  "detection": {
    "status": "valid",
    "off_route": false
  }
}
```

## Implementation Changes

### Shared Types (`shared/src/lib.rs`)
- Add `is_cold_boot: bool` field to `KalmanState`

### Rust Changes

| Component | Change |
|-----------|--------|
| `shared/src/lib.rs` | Add `is_cold_boot: bool` to `KalmanState` |
| `mod.rs:process_gps_update` | Set `is_cold_boot = true` on first fix, add `Acquiring` result |
| `hysteresis.rs` | Add `is_cold_start()` helper (returns `state.is_cold_boot`) |
| `output.rs` | Add `"acquiring"` status serialization |
| `ProcessResult` | Add `Acquiring` variant |

### Android Changes

| Component | Change |
|-----------|--------|
| `StateModels.kt` | Add `isColdBoot: Boolean` to `KalmanState` |
| `service/DetectionPipeline.kt` | Set `isColdBoot = true` on first fix, fix init order |
| `service/TraceTick.kt` | Add `"acquiring"` status enum value |
| `detection/hysteresis/Hysteresis.kt` | Add `isColdStart()` helper |

### No Changes To
- Snap logic (reused as-is with `is_cold_boot` check for constraint behavior)
- Detection probability (disabled during acquiring)

## Success Criteria

1. Cold boot trace shows `"status": "acquiring"` with `s_cm: 0`
2. After snap, trace shows `"status": "valid"` with actual `s_cm`
3. No false arrivals during deadhead scenarios
4. Re-uses existing snap logic (minimal code changes)

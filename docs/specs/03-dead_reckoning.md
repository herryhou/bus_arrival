# Dead Reckoning Specification

## Overview

Dead-reckoning compensates for GPS outages by extrapolating position from last known velocity. When GPS signal is lost (no fix), the system continues to estimate position advancement using a filtered velocity estimate that decays over time.

## Invariants (MUST)

- [x] DR decay: `(9/10)^dt × 10000` integer approximation
- [x] DR timeout: 10 seconds maximum
- [x] On GPS recovery: re-sync with Kalman filter using soft-resync
- [x] `in_recovery` flag cleared after first valid GPS

## Decay Factors (precomputed)

| dt | Factor | Value | Notes |
|----|--------|-------|-------|
| 0 | 10000 | 1.0 | No decay |
| 1 | 9000 | 0.9 | 10% decay per second |
| 2 | 8100 | 0.81 | 19% cumulative decay |
| 3 | 7290 | 0.729 | 27.1% cumulative decay |
| 4 | 6561 | 0.6561 | 34.39% cumulative decay |
| 5 | 5905 | 0.5905 | 40.95% cumulative decay |
| 6 | 5314 | 0.5314 | 46.86% cumulative decay |
| 7 | 4783 | 0.4783 | 52.17% cumulative decay |
| 8 | 4305 | 0.4305 | 56.95% cumulative decay |
| 9 | 3874 | 0.3874 | 61.26% cumulative decay |
| 10 | 3487 | 0.3487 | 65.13% cumulative decay |

**Formula:** `v_filtered(t) = v_filtered(t-1) * (9/10)^dt`

The decay is normalized by time delta (`dt`), not applied as a constant factor. This ensures consistent decay behavior regardless of update interval.

## Dead Reckoning Algorithm

### 1. Position Update

During GPS outage (no fix):

```
s(t) = s(t-1) + v_filtered * dt
```

Where:
- `s(t)` = current position estimate (cm)
- `v_filtered` = filtered velocity (cm/s)
- `dt` = time since last valid GPS (seconds)

### 2. Velocity Decay

After position update, apply exponential decay to filtered velocity:

```
v_filtered(t) = v_filtered(t-1) * decay_factor[dt]
```

The decay factor is looked up from the precomputed table based on `dt`.

### 3. Velocity Filtering (EMA)

When GPS is valid, update filtered velocity using exponential moving average:

```
v_filtered = v_filtered + 3*(v_gps - v_filtered)/10
```

This uses α = 0.3 (30% smoothing) to reduce noise while remaining responsive to actual speed changes.

### 4. GPS Recovery (Soft Resync)

On first valid GPS after outage (`in_recovery == true`):

```
s_resync = s_DR + 2*(z_gps - s_DR)/10
v_resync = v_DR + 2*(v_gps - v_DR)/10
```

Uses conservative 2/10 gain for both position and velocity to handle potentially noisy first post-outage GPS data. The `in_recovery` flag is cleared after applying soft-resync.

## State Management

### DrState Structure

```rust
pub struct DrState {
    pub last_gps_time: Option<u64>,    // Timestamp of last valid GPS
    pub last_valid_s: DistCm,           // Last valid position (cm)
    pub filtered_v: SpeedCms,           // Filtered velocity (cm/s)
    pub in_recovery: bool,              // Recovery mode flag
}
```

### State Transitions

1. **Normal GPS** → Update filtered velocity with EMA, clear `in_recovery`
2. **GPS Outage** → Set `in_recovery = true`, extrapolate position with decay
3. **GPS Recovery** → Apply soft-resync (2/10 gain), clear `in_recovery`
4. **Outage > 10s** → Return `Outage` error, stop dead-reckoning

## Related Files

- `crates/shared/src/lib.rs` — `DrState` definition
- `crates/pipeline/gps_processor/src/kalman.rs` — DR handling in `process_gps_update()`, `handle_outage()`, decay factor LUT

## Implementation Notes

### Integer Arithmetic

All calculations use integer arithmetic for RP2350 compatibility (no hardware FPU). Decay factors are precomputed as integers scaled by 10000 to maintain precision while avoiding floating-point operations.

### Decay Normalization

The decay is **normalized by time delta**, not applied as a constant per-tick factor. This means:
- 1-second gap: `v *= 0.9`
- 2-second gap: `v *= 0.81` (not `v *= 0.9 * 0.9`)
- 5-second gap: `v *= 0.5905` (single lookup, not 5 iterations)

This prevents excessive decay during long gaps between GPS updates.

### Recovery Behavior

The `in_recovery` flag is set **even for long outages** (> 10s) to allow relaxed heading filter on first GPS fix. This improves map matching when GPS heading is unreliable after extended signal loss, even though dead-reckoning stops after 10 seconds.

## Testing

Dead-reckoning behavior is validated in `crates/pipeline/gps_processor/src/kalman.rs` tests:

- `test_dr_decay_normalization` — Verifies decay factor LUT values and normalization
- `test_ema_velocity_filter_*` — Tests EMA velocity filtering with integer arithmetic
- Integration tests — Full pipeline validation with GPS outage scenarios

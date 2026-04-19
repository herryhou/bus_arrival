# 1D Kalman Filter Specification

## Overview
1D Kalman filter for route progress estimation. Fixed-point arithmetic with adaptive Kalman gain based on HDOP.

**State variables:**
- `s_cm`: Filtered route position (cm)
- `v_cms`: Filtered velocity (cm/s)
- `last_seg_idx`: Last matched segment

## Inputs

| Parameter | Type | Range | Description |
|-----------|------|-------|-------------|
| `z_cm` | DistCm | ±214 km | Raw GPS projection (cm) |
| `v_gps_cms` | SpeedCms | 0..1667 | GPS speed (cm/s) |
| `hdop_x10` | u16 | 0..255 | HDOP × 10 |

## Outputs

| Parameter | Type | Range | Description |
|-----------|------|-------|-------------|
| `s_cm` | DistCm | ±214 km | Filtered position (cm) |
| `v_cms` | SpeedCms | 0..1667 | Filtered velocity (cm/s) |

## Invariants (MUST)

- [ ] `v_cms ≥ 0` ALWAYS (clamped after update)
- [ ] Kalman gain: numerator × 256 (fixed-point denominator)
- [ ] Standard Ks = 51/256 (position), Kv = 77/256 (velocity)
- [ ] Adaptive Ks based on HDOP: 77 (good), 51 (fair), 26 (poor), 13 (bad)
- [ ] Cold start: `s_cm = z_cm`, `v_cms = v_gps_cms`

## Formulas

**Prediction step:**
```rust
s_pred = s_cm + v_cms
v_pred = v_cms
```

**Update step (standard):**
```rust
s_cm = s_pred + (51 * (z_cm - s_pred)) / 256
v_cms = (v_pred + (77 * (v_gps_cms - v_pred)) / 256).max(0)
```

**Update step (HDOP-adaptive):**
```rust
ks = match hdop_x10 {
    0..=20 => 77,
    21..=30 => 51,
    31..=50 => 26,
    _ => 13,
}
s_cm = s_pred + (ks * (z_cm - s_pred)) / 256
v_cms = (v_pred + (77 * (v_gps_cms - v_pred)) / 256).max(0)
```

## State Structure

```rust
#[repr(C)]
pub struct KalmanState {
    pub s_cm: DistCm,              // Filtered position
    pub v_cms: SpeedCms,           // Filtered velocity
    pub last_seg_idx: usize,       // Last matched segment
    pub off_route_suspect_ticks: u8,    // Off-route detection counter
    pub off_route_clear_ticks: u8,      // Off-route clear counter
    pub frozen_s_cm: Option<DistCm>,    // Frozen position during off-route
}
```

## Cold Start Initialization

```rust
pub fn init(z_cm: DistCm, v_gps_cms: SpeedCms, seg_idx: usize) -> Self {
    KalmanState {
        s_cm: z_cm,
        v_cms: v_gps_cms,
        last_seg_idx: seg_idx,
        off_route_suspect_ticks: 0,
        off_route_clear_ticks: 0,
        frozen_s_cm: None,
    }
}
```

## HDOP-Adaptive Kalman Gain

The position Kalman gain (Ks) adapts based on GPS quality (HDOP):

| HDOP Range | Quality | Ks Value | Description |
|------------|---------|----------|-------------|
| 0.0 - 2.0 | Excellent | 77/256 ≈ 0.30 | Trust GPS highly |
| 2.1 - 3.0 | Good | 51/256 ≈ 0.20 | Moderate trust |
| 3.1 - 5.0 | Fair | 26/256 ≈ 0.10 | Low trust |
| > 5.0 | Poor | 13/256 ≈ 0.05 | Minimal trust |

**Rationale:** When GPS is poor (high HDOP), the filter relies more on prediction (velocity) and less on raw position measurements to reduce noise.

## Version Notes

- v8.5: Added `.max(0)` constraint to prevent negative velocity
- v8.4: HDOP-adaptive Kalman gain

## Related Files

- `crates/shared/src/lib.rs` — KalmanState implementation
- `crates/pipeline/gps_processor/src/kalman.rs` — GPS processing with Kalman

# Cross-Cutting Constraints (MUST READ FIRST)

## Overview

This document defines constraints that apply to ALL modules in the bus arrival detection system. Read this before any other spec.

## Semantic Type System

All physical quantities use semantic integer types. MUST NOT use float/f64 for runtime calculations.

| Type | Definition | Range | Purpose |
|------|------------|-------|---------|
| `DistCm` | `i32` | ±214 km | Distance in centimeters |
| `SpeedCms` | `i32` | 0..214 km/h | Speed in cm/s |
| `HeadCdeg` | `i16` | -180°..+180° | Heading in 0.01° |
| `GeoCdeg` | `i16` | -180°..+180° | Lat/lon in 0.01° |
| `Prob8` | `u8` | 0..255 | Probability × 255 |
| `Dist2` | `i64` | ±4.6×10¹⁸ cm² | Squared distance |

### Type Definitions

Source: `crates/shared/src/lib.rs`

```rust
/// Distance in centimeters.
/// Range: ±21,474,836 cm ≈ ±214 km — sufficient for bus routes.
pub type DistCm = i32;

/// Speed in centimeters per second.
/// Range: 0..21,474,836 cm/s ≈ 0..214 km/h — covers bus speeds.
pub type SpeedCms = i32;

/// Heading in hundredths of a degree.
/// Range: -18000..18000 = -180°..+180°
pub type HeadCdeg = i16;

/// Geographic coordinate in hundredths of a degree.
/// Range: -18000..18000 = -180°..+180°
/// Used for latitude and longitude (NOT heading/direction).
pub type GeoCdeg = i16;

/// Probability scaled 0..255 (u8 = probability × 255).
/// Precision: 1/256 ≈ 0.004 — sufficient for arrival decisions.
pub type Prob8 = u8;

/// Squared distance (cm²) for intermediate calculations.
/// Prevents overflow in dot products: (2×10⁶)² ≈ 4×10¹² < i64::MAX.
pub type Dist2 = i64;
```

## Resource Budgets (RP2350 Platform)

| Resource | Budget | Notes |
|----------|--------|-------|
| CPU | < 8% @ 150MHz | For 1Hz GPS updates |
| SRAM (runtime) | < 1 KB | Excluding route data |
| Flash (route data) | ~10-12 KB | v8.8 optimized grid |

## Integer-Only Arithmetic

**MUST:** All runtime calculations use integer arithmetic only.
**MUST NOT:** Use floating-point operations in firmware (no hardware FPU).

### Fixed-Point Patterns

- **Kalman gain:** fractions stored as numerator/256
  - Position gain: 51/256 to 77/256 (HDOP-adaptive)
  - Velocity gain: 77/256 (fixed)
  - Soft resync: 2/10 (recovery mode)

- **LUT-based:** Gaussian, logistic precomputed as u8[256]
  - `GAUSSIAN_LUT_SIZE`: 256 entries
  - `SPEED_LUT_MAX_IDX`: 127 entries (0-127 cm/s → 0-255 probability)

- **Dot products:** Use `Dist2` (i64) to prevent overflow
  - Example: `(2×10⁶)² ≈ 4×10¹² < i64::MAX`

- **DR decay factors:** (9/10)^dt × 10000 for integer arithmetic
  ```rust
  const DR_DECAY_NUMERATOR: [u32; 11] = [
      10000, // dt=0: 1.0
      9000,  // dt=1: 0.9
      8100,  // dt=2: 0.81
      // ... up to dt=10
  ];
  ```

## Physical Constraints

Source: `crates/pipeline/gps_processor/src/kalman.rs`

| Parameter | Value | Source |
|-----------|-------|--------|
| `V_MAX_CMS` | 1667 cm/s (60 km/h) | Urban bus speed |
| `SIGMA_GPS_CM` | 2000 cm (20 m) | GPS noise margin |
| `GPS_JUMP_THRESHOLD` | 5000 cm (50 m) | Recovery trigger |
| `OFF_ROUTE_D2_THRESHOLD` | 25000000 cm² | Off-route detection (50 m) |

### Speed Constraint Filter

GPS updates are rejected if they exceed physical limits:

```rust
pub fn check_speed_constraint(z_new: DistCm, z_prev: DistCm, dt: i32) -> bool {
    let dist_abs = (z_new - z_prev).unsigned_abs() as i32;
    let max_dist = V_MAX_CMS * dt.max(1) + SIGMA_GPS_CM;
    dist_abs <= max_dist
}
```

### Monotonicity Constraint

Per spec Section 8.3: reject if `z(t) - ŝ(t-1) < -5000 cm` (-50 m).

```rust
fn check_monotonic(z_new: DistCm, z_prev: DistCm) -> bool {
    z_new >= z_prev - 5000
}
```

## Probability Model Constants

Source: `crates/shared/src/probability_constants.rs`

| Parameter | Value | Purpose |
|-----------|-------|---------|
| `SIGMA_D_CM` | 2750 cm | Distance likelihood sigma (F1) |
| `SIGMA_P_CM` | 2000 cm | Progress difference sigma (F3) |
| `V_STOP_CMS` | 200 cm/s (7.2 km/h) | Stop speed threshold |
| `SPEED_LUT_MAX_IDX` | 127 | Speed LUT resolution |
| `GAUSSIAN_LUT_SIZE` | 256 | Gaussian LUT resolution |

### Position Signals

Per spec Section 13.2: F1 uses raw GPS projection, F3 uses Kalman-filtered position.

```rust
pub struct PositionSignals {
    /// Raw GPS projection onto route (for F1, sigma_d = 2750 cm)
    pub z_gps_cm: DistCm,
    /// Kalman-filtered route position (for F3, sigma_p = 2000 cm)
    pub s_cm: DistCm,
}
```

The two signals are independent by design:
- `z_gps_cm` reflects current sensor observation (noisy, ±30m)
- `s_cm` reflects system-integrated estimate (smoothed, ±10-20m)

## Off-Route Detection

Source: `crates/pipeline/gps_processor/src/kalman.rs`

Off-route detection uses hysteresis to avoid false positives from transient multipath:

| Parameter | Value | Purpose |
|-----------|-------|---------|
| `OFF_ROUTE_D2_THRESHOLD` | 25000000 cm² | Distance threshold (50 m) |
| `OFF_ROUTE_CONFIRM_TICKS` | 5 | Ticks to confirm off-route |
| `OFF_ROUTE_CLEAR_TICKS` | 2 | Ticks to clear off-route |
| `JUMP_RECOVERY_THRESHOLD` | 5000 cm | GPS jump recovery trigger |

### Behavior

- **Position freezing:** Immediate when off-route first suspected
- **Recovery:** Re-synchronizes stop indices when GPS returns to route
- **Arrival suppression:** During off-route episodes
- **Limitation:** Cannot detect "along-route drift" (requires external ground truth)

## Kalman Filter Gains

HDOP-adaptive Kalman gains (per spec Section 11.3):

| HDOP Range | Position Gain (Ks) | Velocity Gain (Kv) |
|------------|-------------------|-------------------|
| 0.0 - 2.0 | 77/256 | 77/256 |
| 2.1 - 3.0 | 51/256 | 77/256 |
| 3.1 - 5.0 | 26/256 | 77/256 |
| > 5.0 | 13/256 | 77/256 |

### Soft Resync (Recovery Mode)

After GPS outage or off-route recovery, use conservative 2/10 gain for both position and velocity:

```rust
// Soft resync for position: ŝ_resync = ŝ_DR + (2/10)*(z_gps - ŝ_DR)
state.s_cm = state.s_cm + 2 * (z_raw - state.s_cm) / 10;

// Soft resync for velocity: v_resync = v_DR + (2/10)*(v_gps - v_DR)
state.v_cms = state.v_cms + 2 * (gps.speed_cms - state.v_cms) / 10;
```

## Dead-Reckoning (GPS Outage)

Source: `crates/pipeline/gps_processor/src/kalman.rs`

Maximum outage: 10 seconds (per spec Section 11.2)

### EMA Velocity Filter

Per spec Section 11.1: `v_filtered(t) = v_filtered(t-1) + 3*(v_gps - v_filtered(t-1))/10`

```rust
pub fn update_dr_ema(v_filtered_prev: SpeedCms, v_gps: SpeedCms) -> SpeedCms {
    v_filtered_prev + 3 * (v_gps - v_filtered_prev) / 10
}
```

Uses α = 3/10 = 0.3 for smoothing.

## Binary Format

Route data uses v5.1 sparse grid format with XIP (Execute-in-Place) support for zero-copy Flash access.

See: `docs/spatial_grid_binary_format.md` + `crates/shared/src/binfile.rs`

### Key Structures

```rust
pub struct RouteNode {
    pub x_cm: DistCm,        // X coordinate (cm)
    pub y_cm: DistCm,        // Y coordinate (cm)
    pub cum_dist_cm: DistCm, // Cumulative distance (cm)
    pub seg_len_mm: i32,     // Segment length (mm)
    pub dx_cm: i16,          // Segment vector X (cm)
    pub dy_cm: i16,          // Segment vector Y (cm)
    pub heading_cdeg: HeadCdeg, // Heading (0.01°)
    pub _pad: i16,           // Alignment padding
}
// Size: 24 bytes (no padding)

pub struct Stop {
    pub progress_cm: DistCm,       // Position along route (cm)
    pub corridor_start_cm: DistCm, // -80 m from stop
    pub corridor_end_cm: DistCm,   // +40 m from stop
}
// Size: 12 bytes
```

## Related Files

- **Type definitions:** `crates/shared/src/lib.rs`
- **Probability constants:** `crates/shared/src/probability_constants.rs`
- **Binary format:** `docs/spatial_grid_binary_format.md`
- **Kalman filter:** `crates/pipeline/gps_processor/src/kalman.rs`
- **Tech report:** `docs/bus_arrival_tech_report_v8.md`

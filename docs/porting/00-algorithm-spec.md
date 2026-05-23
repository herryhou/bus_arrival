# Bus Arrival Detection - Algorithm Specification

**Target:** Senior developers porting to iOS/ESP32
**Scope:** Platform-agnostic algorithm description
**Reference:** `bus_arrival_tech_report_v8.md` sections 1-16

## Overview

3-phase pipeline: NMEA GPS input → Map Matching + Kalman Filter → Bayesian Arrival Detection → `trace_v2.jsonl`

Each phase has well-defined inputs/outputs. Implement in any language/platform.

## Phase 1: Map Matching

### Purpose
Find the best route segment for each GPS position.

### Inputs
- GPS position: (x_cm, y_cm) in local projected coordinates
- GPS heading: HeadCdeg (0.01° units)
- GPS speed: SpeedCms (cm/s)
- Route data: Spatial grid index + precomputed segments

### Algorithm
1. **Grid Search** - Spatial grid lookup (O(1) → O(k) where k≈5-15)
   - Grid size: 100m × 100m cells
   - Search 3×3 neighborhood around GPS position
   - Returns candidate segments

2. **Heading Filter** - Filter-then-Rank architecture
   - Heading gate: ±90° (9000 cdeg) at full speed
   - Speed-adaptive: fully open at standstill, ±90° at ≥3 km/h
   - Formula: `threshold = 36000 - (27000 × w / 256)` where w = heading_weight(speed)

3. **Distance Scoring** - Pure distance² (no sqrt)
   - Point-to-segment projection (clamped to segment bounds)
   - Score = distance² in cm²
   - Best eligible segment wins

### Outputs
- `segment_idx`: Index of best matching segment
- `z_cm`: Projected position along route (1D coordinate)
- `heading_constraint_met`: Boolean flag

### Key Constants
- `MAX_HEADING_DIFF_CDEG = 9000` (90°)
- `GRID_SIZE_CM = 10000` (100m)
- `V_RAMP_CMS = 83` (3 km/h - heading ramp threshold)

## Phase 2: State Estimation

### Purpose
Smooth GPS observations and handle GPS outages via 1D Kalman Filter + Dead-Reckoning.

### Inputs
- `z_cm`: Raw GPS projection from Phase 1
- `v_gps_cms`: GPS speed in cm/s
- `hdop_x10`: HDOP × 10 (optional, for adaptive gain)

### 2.1 Speed Constraint Filter
Reject GPS jumps > 37m:

```
D_max = V_max × Δt + σ_gps = 1667 × 1 + 2000 = 3667 cm

If |z_new - ŝ_prev| > D_max:
    Reject GPS sample (skip Kalman update, only predict)
```

### 2.2 1D Kalman Filter

**State:** X = [s, v]ᵀ (route progress cm, speed cm/s)

**Predict step:**
```
s̃(t+1) = ŝ(t) + v̂(t) × 1s
ṽ(t+1) = v̂(t)
```

**Update step:**
```
K_s = 51/256 (default) or 77/26/13 (HDOP-adaptive)
K_v = 77/256

ŝ = s̃ + K_s × (z_gps - s̃)
v̂ = ṽ + K_v × (v_gps - ṽ)
v̂ = max(v̂, 0)  # Clamp negative
```

**HDOP-adaptive gain:**
| HDOP range | K_s (numerator/256) |
|------------|---------------------|
| ≤ 2.0      | 77 (≈0.30)          |
| 2.1-3.0    | 51 (≈0.20)          |
| 3.1-5.0    | 26 (≈0.10)          |
| > 5.0      | 13 (≈0.05)          |

### 2.3 Dead-Reckoning (GPS Outage)

When GPS invalid (HDOP > 10 or no fix):
```
ŝ_DR(t) = ŝ(t_last) + v̂_filtered × (t - t_last)

Max duration: 10 seconds
```

### 2.4 Divergence Detection (Off-Route)

5-tick hysteresis confirmation:
```
If z_gps² > 25000000 cm² (50m) from route:
    divergence_counter += 1
Else:
    divergence_counter = 0

If divergence_counter >= 5:
    Enter OFF_ROUTE mode
    Freeze s_cm (stop progress estimation)
```

### Outputs
- `s_cm`: Filtered route progress position
- `v_cms`: Filtered speed
- `variance_cm2`: Position uncertainty (optional)
- `divergence_cm`: Off-route distance

## Phase 3: Arrival Detection

### Purpose
Detect when bus arrives at each stop using spatial filter + probability model + state machine.

### Inputs
- `s_cm`, `v_cms`: From Phase 2
- `z_gps_cm`: Raw GPS projection
- Stop data: position, corridor boundaries

### 3.1 Stop Corridor Filter

Each stop has a corridor (not a point):
```
corridor_start = s_stop - 8000 cm  (80m before)
corridor_end   = s_stop + 4000 cm  (40m after)
```

**Only activate detection when:**
```
corridor_start ≤ s_hat ≤ corridor_end
```

**Corridor overlap protection:**
- Adjacent corridors must have ≥20m gap
- If stops < 120m apart: redistribute corridor (55% pre / 10% gap / 35% post)

### 3.2 Arrival Probability Model

4-feature weighted fusion:

**Feature F1 - Raw GPS distance:**
```
p1 = gaussian_lut(|z_gps - s_stop|, σ=2750 cm)
```

**Feature F2 - Speed likelihood:**
```
p2 = logistic_lut(v_cms, v_stop=200 cm/s)
```

**Feature F3 - Kalman progress:**
```
p3 = gaussian_lut(|s_hat - s_stop|, σ=2000 cm)
```

**Feature F4 - Dwell time:**
```
p4 = min((dwell_time_s × 255) / 10, 255)
```

**Fusion:**
```
P = (13×p1 + 6×p2 + 10×p3 + 3×p4) / 32

If next_stop < 120m away:
    P = (14×p1 + 7×p2 + 11×p3) / 32  # No p4 weight
```

**Threshold:**
```
If P > 191 (0.75 × 255):
    Trigger arrival
```

### 3.3 State Machine

Per-stop FSM states:

| State | Condition to enter |
|-------|-------------------|
| Idle | Default (outside corridor) |
| Approaching | s_hat ≥ corridor_start |
| Arriving | d_to_stop < 5000 cm |
| AtStop | d_to_stop < 5000 cm AND P > 191 |
| Departed | d_to_stop > 4000 cm AND s_hat > s_stop |

**Key rules:**
- Forward-only transitions (no return to previous states)
- Per-stop tracking (each stop has independent FSM)
- One-time announcement per stop per trip

### Outputs
- `ArrivalEvent`: stop_idx, timestamp, dwell_s
- FSM state per stop
- Announcement trigger at corridor entry

## Platform Porting Notes

### Integer Math (No-FPU Platforms)
- All distances in cm (i32), speeds in cm/s (i32)
- Avoid sqrt: use distance² for comparisons
- Fixed-point: fractions as numerator/256
- LUTs for Gaussian, logistic functions

### Floating-Point (iOS, etc.)
- May use f64 for intermediate calculations
- Final output must match integer semantics (±1 cm tolerance)
- Kalman filter can use full covariance matrix (optional)

### Memory Constraints
- Route data: ~10-12 KB for typical 12km route
- Runtime state: < 1 KB SRAM
- LUTs: 256 bytes Gaussian + 128 bytes logistic

### Thread Safety
- Pipeline is stateful: single-threaded or mutex-protected
- Each GPS tick (1 Hz) updates all state sequentially

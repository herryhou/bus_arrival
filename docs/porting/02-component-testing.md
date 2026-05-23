# Component Testing Guide

## Overview

Each pipeline phase can be tested independently with defined inputs, expected outputs, and pass criteria.

## Map Matching Tests

### Test 1: Segment Selection Accuracy

**Purpose:** Verify correct segment selection for various GPS positions.

**Test Vector:**
```
Input:
  gps_pos = (x: 27564320, y: 27678910)  # cm
  gps_heading = 2750  # cdeg (27.50°)
  gps_speed = 555     # cm/s (~20 km/h)
  route: ty225

Expected Output:
  segment_idx = 42
  dist2 < 250000 cm² (50m threshold)
  heading_constraint_met = true
```

**Pass Criteria:**
- Selected segment within 50m of true position
- Heading diff < 90° at full speed
- Grid search returns ≤15 candidates

### Test 2: Heading Constraint Filtering

**Purpose:** Verify heading gate rejects wrong-direction segments.

**Test Vector:**
```
Input:
  gps_heading = 0     # North
  gps_speed = 833     # cm/s (30 km/h)
  candidate_segments: [heading=0, heading=9000, heading=18000]

Expected Output:
  eligible: [segment with heading=0]
  rejected: [segments with heading=9000, 18000]
```

**Pass Criteria:**
- Segments with heading diff > 90° rejected at full speed
- All segments accepted when speed = 0 (gate open)

### Test 3: Projection Correctness

**Purpose:** Verify GPS-to-route projection math.

**Test Vector:**
```
Input:
  segment: P0=(0,0), P1=(10000, 0)  # 100m east
  gps_pos = (5000, 3000)  # 50m east, 30m north

Expected Output:
  t = 0.5 (midpoint of segment)
  z_cm = cum_dist[0] + 5000
  distance² = 9000000 cm² (30m)
```

**Pass Criteria:**
- Projection clamped to segment bounds
- Distance² computed correctly (no sqrt)
- Handle negative t and t > len2 correctly

## Kalman Filter Tests

### Test 1: Convergence

**Purpose:** Verify filter converges to stable estimate.

**Test Vector:**
```
Initial:
  s_cm = 0, v_cms = 0

Input sequence (10 ticks):
  z_cm = [10000, 20000, 30000, ...]  # 10m per tick
  v_gps = 1000  # constant 10 m/s

Expected Output:
  After 10 ticks:
    s_hat ≈ 100000 cm (±500 cm)
    v_hat ≈ 1000 cm/s (±50 cm/s)
```

**Pass Criteria:**
- Converges within 5 ticks
- Steady-state error < 5% of true value
- No oscillation or divergence

### Test 2: Divergence Threshold

**Purpose:** Verify GPS jump rejection.

**Test Vector:**
```
State:
  s_prev = 100000 cm
  v_prev = 500 cm/s

Input:
  z_new = 150000 cm  # 50m jump!

Expected Output:
  Reject GPS sample
  s_hat = 100500 cm (prediction only)
  v_hat = 500 cm/s (unchanged)
```

**Pass Criteria:**
- Jumps > 37m rejected
- Kalman state unchanged after rejection
- Prediction step still executes

### Test 3: Dead-Reckoning Accuracy

**Purpose:** Verify position estimation during GPS outage.

**Test Vector:**
```
Last valid:
  s = 100000 cm
  v = 500 cm/s

GPS outage: 5 seconds

Expected Output:
  s_DR = 100000 + 500×5 = 102500 cm
  Error < 1000 cm (1m per second of outage)
```

**Pass Criteria:**
- Linear projection during outage
- Error grows < 20cm per second
- Re-acquisition resyncs correctly

## Detection Tests

### Test 1: Probability Calibration

**Purpose:** Verify probability model outputs correct range.

**Test Vector:**
```
Input:
  d_to_stop = 1000 cm  # 10m away
  v = 200 cm/s         # ~walking speed
  dwell_time = 5s

Expected Output:
  P ≈ 200-230 (high confidence)

Calculation:
  p1 = gaussian_lut(1000, 2750) ≈ 200
  p2 = logistic_lut(200, 200) ≈ 128
  p3 = gaussian_lut(1000, 2000) ≈ 220
  p4 = min(5×255/10, 255) = 127

  P = (13×200 + 6×128 + 10×220 + 3×127) / 32
    ≈ 191 (threshold)
```

**Pass Criteria:**
- P > 191 when at stop (d < 5m, v < 200, dwell > 5s)
- P < 100 when far from stop
- Monotonic increase as bus approaches

### Test 2: FSM Transitions

**Purpose:** Verify state machine correctness.

**Test Vector:**
```
Stop: s_stop = 100000 cm
Corridor: [92000, 104000]

Input sequence:
  t=0:  s=90000 → State: Idle
  t=1:  s=92000 → State: Approaching
  t=5:  s=98000 → State: Arriving (d < 5000)
  t=8:  s=99500, P=220 → State: AtStop
  t=15: s=105000 → State: Departed
```

**Pass Criteria:**
- All transitions occur at correct positions
- No reverse transitions (AtStop → Arriving)
- Departed state persists

### Test 3: Corridor Filtering

**Purpose:** Verify only stops in corridor are evaluated.

**Test Vector:**
```
Bus position: s = 50000 cm

Stops:
  Stop 0: s = 10000, corridor = [2000, 14000]
  Stop 1: s = 48000, corridor = [40000, 52000]  ✓ Active
  Stop 2: s = 60000, corridor = [52000, 64000]
  Stop 3: s = 90000, corridor = [82000, 94000]

Expected Output:
  active_indices = [1]
  Only Stop 1 evaluated for arrival
```

**Pass Criteria:**
- Only 1-3 stops active at any time
- Corridor overlap protection enforced
- No missed activations

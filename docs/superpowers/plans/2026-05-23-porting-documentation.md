# Porting Documentation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create 5 modular documentation files in `docs/porting/` for porting bus arrival detection to iOS/ESP32 platforms.

**Architecture:** Extract algorithm specs from v8 tech report, document data formats from code/specs, create testing guides from existing tests.

**Tech Stack:** Markdown, existing tech report (`bus_arrival_tech_report_v8.md`), module specs in `docs/specs/`, test data in `test_data/`.

---

## File Structure

```
docs/porting/
├── 00-algorithm-spec.md         (Platform-agnostic 3-phase pipeline)
├── 01-data-formats.md           (NMEA, binary route, trace_v2.jsonl)
├── 02-component-testing.md      (Per-component test criteria)
├── 03-golden-tests.md           (End-to-end validation)
└── 04-porting-checklist.md      (iOS/ESP32 considerations)
```

---

### Task 1: Create directory and algorithm spec

**Files:**
- Create: `docs/porting/00-algorithm-spec.md`
- Source: `docs/bus_arrival_tech_report_v8.md` (sections 1-16)
- Source: `docs/specs/*.md` (for algorithm details)

- [ ] **Step 1: Create directory**

```bash
mkdir -p docs/porting
```

- [ ] **Step 2: Write 00-algorithm-spec.md header**

```markdown
# Bus Arrival Detection - Algorithm Specification

**Target:** Senior developers porting to iOS/ESP32
**Scope:** Platform-agnostic algorithm description
**Reference:** `bus_arrival_tech_report_v8.md` sections 1-16

## Overview

3-phase pipeline: NMEA GPS input → Map Matching + Kalman Filter → Bayesian Arrival Detection → `trace_v2.jsonl`

Each phase has well-defined inputs/outputs. Implement in any language/platform.
```

- [ ] **Step 3: Write Phase 1 - Map Matching section**

```markdown
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
```

- [ ] **Step 4: Write Phase 2 - State Estimation section**

```markdown
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
```

- [ ] **Step 5: Write Phase 3 - Arrival Detection section**

```markdown
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
```

- [ ] **Step 6: Add platform notes section**

```markdown
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
```

- [ ] **Step 7: Commit**

```bash
git add docs/porting/00-algorithm-spec.md
git commit -m "docs(porting): add algorithm specification"
```

---

### Task 2: Create data formats documentation

**Files:**
- Create: `docs/porting/01-data-formats.md`
- Source: `docs/specs/10-spatial_index.md`, `docs/specs/12-gps_processing.md`
- Source: `crates/shared/src/lib.rs` (type definitions)
- Source: `test_data/*.jsonl` (trace format examples)

- [ ] **Step 1: Write header and NMEA section**

```markdown
# Data Formats Specification

## NMEA Input Format

### Required Sentences

**$GPRMC / $GNRMC** - Recommended Minimum data

```
$GPRMC,hhmmss.ss,A,ddmm.mmmm,N,dddmm.mmmm,E,sss.s,ddd.d,ddmmyy,,,D*hh<CR><LF>
```

| Field | Position | Type | Description |
|-------|----------|------|-------------|
| Time | 1 | string | UTC timestamp hhmmss.ss |
| Status | 2 | char | 'A' = valid, 'V' = warning |
| Lat | 3 | ddmm.mmmm | Latitude ddmm.mmmm |
| N/S | 4 | char | 'N' or 'S' |
| Lon | 5 | dddmm.mmmm | Longitude dddmm.mmmm |
| E/W | 6 | char | 'E' or 'W' |
| Speed_knots | 7 | float | Speed in knots |
| Heading | 8 | float | True heading in degrees |
| Date | 9 | ddmmyy | Date ddmmyy |

**Conversion:**
- Speed: cm/s = knots × 51.44
- Heading: cdeg = degrees × 100

**$GPGGA / $GNGGA** - Fix Data

```
$GPGGA,hhmmss.ss,llll.ll,a,yyyyy.yy,a,x,xx,x.x,x.x,M,x.x,M,x.x,xxxx*hh<CR><LF>
```

| Field | Position | Type | Description |
|-------|----------|------|-------------|
| Time | 1 | string | UTC timestamp |
| Lat | 2 | llll.ll | Latitude |
| N/S | 3 | char | 'N' or 'S' |
| Lon | 4 | yyyy.yy | Longitude |
| E/W | 5 | char | 'E' or 'W' |
| Quality | 6 | int | 0=invalid, 1=GPS, 2=DGPS |
| Sats | 7 | int | Number of satellites |
| HDOP | 8 | float | Horizontal dilution of precision |
| Alt | 9 | float | Altitude above sea level |

**HDOP interpretation:**
- ≤ 2.0: Excellent (K_s = 77/256)
- 2.1-3.0: Good (K_s = 51/256)
- 3.1-5.0: Fair (K_s = 26/256)
- > 5.0: Poor (K_s = 13/256)
```

- [ ] **Step 2: Write semantic types section**

```markdown
## Core Data Types

Source: `crates/shared/src/lib.rs`

```rust
pub type DistCm   = i32;  // Distance in centimeters (±214 km)
pub type SpeedCms = i32;  // Speed in cm/s (0..214 km/h)
pub type HeadCdeg = i16;  // Heading in 0.01° (-180°..+180°)
pub type GeoCdeg  = i16;  // Lat/lon in 0.01°
pub type Prob8    = u8;   // Probability × 255 (0..255)
pub type Dist2    = i64;  // Distance² (cm²)
```

| Type | Unit | Range | Purpose |
|------|------|-------|---------|
| DistCm | cm | ±21,474,836 cm | All distances |
| SpeedCms | cm/s | 0..21,474,836 | Speed calculations |
| HeadCdeg | 0.01° | -18000..18000 | Heading/direction |
| GeoCdeg | 0.01° | -18000..18000 | GPS coordinates |
| Prob8 | 1/256 | 0..255 | Probability values |
| Dist2 | cm² | ±4.6×10¹⁸ | Intermediate distance calc |
```

- [ ] **Step 3: Write binary route format section**

```markdown
## Binary Route Format

### Version 5 Structure (v8.8)

```
[Header: 16 bytes]
  magic: "PICO2RT" (8 bytes)
  version: u32 = 5
  node_count: u32
  stop_count: u32

[Grid Origin: 8 bytes]
  x0_cm: i32  (min x of route)
  y0_cm: i32  (min y of route)

[Spatial Grid: variable size]
  grid_width: u16
  grid_height: u16
  bitmask: [u8] (ceil(width × height / 8))
  cell_offsets: [u16] (non-empty cell count)
  cell_data: [u16] (segment indices per cell)

[Route Nodes: node_count × 24 bytes]
  For each node (repr(C), 24 bytes):
    x_cm: i32          (0)
    y_cm: i32          (4)
    cum_dist_cm: i32   (8)
    seg_len_mm: i32    (12)
    dx_cm: i16         (16)
    dy_cm: i16         (18)
    heading_cdeg: i16  (20)
    _pad: i16          (22)

[Stops: stop_count × 16 bytes]
  For each stop (repr(C), 16 bytes):
    index: u8              (0)
    s_cm: i32              (1, padded to 4)
    corridor_start_cm: i32 (5)
    corridor_end_cm: i32   (9)
    _pad: [u8; 3]         (13)
```

### Memory Layout Example

```
Offset  | Size    | Content
--------|---------|------------------------
0       | 16      | Header
16      | 8       | Grid Origin
24      | 2       | Grid Width
26      | 2       | Grid Height
28      | ~450    | Bitmask (for 60×60 grid)
478     | ~1.5K   | Cell Offsets
~2K     | ~4K     | Cell Data
~6K     | ~15K    | Route Nodes (600 × 24)
~21K    | ~1K     | Stops (40 × 16)
~22K    |         | Total
```

### XIP Considerations
- All structures are repr(C) for Flash access
- No pointer indirection (pure offsets)
- Alignment: 4-byte for i32, 2-byte for i16
```

- [ ] **Step 4: Write trace_v2.jsonl format section**

```markdown
## trace_v2.jsonl Output Format

### Per-Tick JSON Line

Each line is a complete JSON object representing one GPS tick (1 Hz):

```json
{
  "time": 12345,
  "gps_state": {
    "x_cm": 27564320,
    "y_cm": 27678910,
    "heading_cdeg": 2750,
    "speed_cms": 555,
    "hdop_x10": 18,
    "fix_quality": 1
  },
  "kalman_state": {
    "s_cm": 1234567,
    "v_cms": 520,
    "z_cm": 1234500
  },
  "detection_state": {
    "stop_states": [
      {
        "index": 5,
        "fsm_state": "Approaching",
        "d_to_stop_cm": 6500,
        "probability": 128,
        "dwell_time_s": 3
      }
    ],
    "active_stop_count": 1
  }
}
```

### Field Definitions

**gps_state:**
- `time`: Unix timestamp or tick counter
- `x_cm`, `y_cm`: Projected local coordinates (cm)
- `heading_cdeg`: GPS heading × 100
- `speed_cms`: GPS speed (cm/s)
- `hdop_x10`: HDOP × 10 (180 = HDOP 18.0)
- `fix_quality`: 0=none, 1=GPS, 2=DGPS

**kalman_state:**
- `s_cm`: Filtered route progress (cm)
- `v_cms`: Filtered speed (cm/s)
- `z_cm`: Raw GPS projection (cm)

**detection_state.stop_states[]:**
- `index`: Stop index (0-based)
- `fsm_state`: "Idle" | "Approaching" | "Arriving" | "AtStop" | "Departed"
- `d_to_stop_cm`: |s_hat - s_stop| (cm)
- `probability`: Arrival probability (0-255)
- `dwell_time_s`: Time in "AtStop" state (seconds)
```

- [ ] **Step 5: Write ground truth format section**

```markdown
## Ground Truth JSON Format

For validation testing:

```json
{
  "route_name": "ty225",
  "stops": [
    {
      "stop_idx": 0,
      "arrivals": [
        {
          "time": 12345,
          "dwell_s": 15
        }
      ]
    },
    {
      "stop_idx": 5,
      "arrivals": [
        {
          "time": 23456,
          "dwell_s": 20
        },
        {
          "time": 56789,
          "dwell_s": 8
        }
      ]
    }
  ]
}
```

### Validation Rules
- `time`: First tick where stop enters "AtStop" state
- `dwell_s`: Duration in "AtStop" before "Departed"
- Multiple arrivals possible per stop (detours, loops)
```

- [ ] **Step 6: Commit**

```bash
git add docs/porting/01-data-formats.md
git commit -m "docs(porting): add data format specifications"
```

---

### Task 3: Create component testing documentation

**Files:**
- Create: `docs/porting/02-component-testing.md`
- Source: `docs/specs/*.md` (module specs)
- Source: `crates/*/tests/` (existing test vectors)

- [ ] **Step 1: Write header and map matching tests**

```markdown
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
```

- [ ] **Step 2: Write Kalman filter tests**

```markdown
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
```

- [ ] **Step 3: Write detection tests**

```markdown
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
```

- [ ] **Step 4: Commit**

```bash
git add docs/porting/02-component-testing.md
git commit -m "docs(porting): add component testing guide"
```

---

### Task 4: Create golden tests documentation

**Files:**
- Create: `docs/porting/03-golden-tests.md`
- Source: `test_data/` (ty225_normal, ty225_jump, ty225_drift)
- Source: `crates/trace_validator/` (validation logic)

- [ ] **Step 1: Write header and test catalog**

```markdown
# Golden Tests - End-to-End Validation

## Overview

Golden tests feed the same NMEA input to both the reference implementation (Rust) and the ported implementation, then compare outputs.

## Test Data Catalog

### ty225_normal
**Scenario:** Baseline normal operation
**Duration:** ~30 minutes
**Characteristics:**
- Normal GPS quality (HDOP 1.5-3.0)
- All stops detected correctly
- No GPS jumps or outages

**Files:**
- `ty225_normal_nmea.txt` - NMEA input
- `ty225_normal_gt.json` - Ground truth arrivals
- `ty225_normal_trace_v2.jsonl` - Reference output

**Expected Results:**
- 22 stops detected
- 0 false positives
- 0 false negatives
- 97.8% accuracy

### ty225_jump
**Scenario:** GPS jump recovery
**Duration:** ~5 minutes
**Characteristics:**
- Single GPS jump of ~80m at tick 123
- Speed constraint filter should reject
- Kalman filter maintains smooth estimate

**Files:**
- `ty225_jump_nmea.txt` - NMEA input (with jump)
- `ty225_jump_gt.json` - Ground truth
- `ty225_jump_trace_v2.jsonl` - Reference output

**Expected Results:**
- Jump rejected (no spurious arrival)
- Position error < 10m after 3 ticks
- All arrivals detected correctly

### ty225_drift
**Scenario:** GPS drift handling
**Duration:** ~10 minutes
**Characteristics:**
- Urban canyon conditions
- GPS position drifts ±30m
- Kalman filter reduces noise

**Files:**
- `ty225_drift_nmea.txt` - NMEA input
- `ty225_drift_gt.json` - Ground truth
- `ty225_drift_trace_v2.jsonl` - Reference output

**Expected Results:**
- Smoothed position estimate
- No arrival triggers from drift alone
- Probability model prevents false positives
```

- [ ] **Step 2: Write validation procedure**

```markdown
## Validation Procedure

### Step 1: Run Reference Implementation

```bash
# Using Rust pipeline
cargo run --release -- \
  ROUTE_NAME=ty225 \
  SCENARIO=normal \
  INPUT=test_data/ty225_normal_nmea.txt
```

Output: `trace_v2.jsonl` (reference)

### Step 2: Run Ported Implementation

```bash
# Example: iOS implementation
./ios_pipeline \
  --route test_data/ty225_route.json \
  --input test_data/ty225_normal_nmea.txt \
  --output ported_trace.jsonl
```

### Step 3: Compare Arrivals

Extract arrival events from both traces:

```python
import json

def extract_arrivals(trace_file):
    arrivals = []
    with open(trace_file) as f:
        for line in f:
            tick = json.loads(line)
            for stop in tick['detection_state']['stop_states']:
                if stop['fsm_state'] == 'AtStop' and stop.get('just_arrived'):
                    arrivals.append({
                        'stop_idx': stop['index'],
                        'time': tick['time'],
                        'dwell_s': stop.get('dwell_time_s', 0)
                    })
    return arrivals

ref_arrivals = extract_arrivals('reference_trace.jsonl')
port_arrivals = extract_arrivals('ported_trace.jsonl')
```

Compare against ground truth:

```python
def validate(arrivals, gt_file):
    with open(gt_file) as f:
        gt = json.load(f)

    gt_by_stop = {s['stop_idx']: s['arrivals'] for s in gt['stops']}

    correct = 0
    false_pos = 0
    false_neg = 0

    for stop_idx, stop_arrivals in enumerate(arrivals_by_stop):
        expected = gt_by_stop.get(stop_idx, [])

        # Match arrivals by time (±5 seconds)
        for arr in stop_arrivals:
            matched = False
            for exp in expected:
                if abs(arr['time'] - exp['time']) <= 5:
                    correct += 1
                    matched = True
                    break
            if not matched:
                false_pos += 1

        for exp in expected:
            matched = any(abs(arr['time'] - exp['time']) <= 5
                         for arr in stop_arrivals)
            if not matched:
                false_neg += 1

    total = correct + false_pos + false_neg
    accuracy = correct / total if total > 0 else 0

    return {
        'accuracy': accuracy,
        'correct': correct,
        'false_positives': false_pos,
        'false_negatives': false_neg
    }
```

### Step 4: Debug Trace Diff

When validation fails, compare per-tick state:

```python
def compare_traces(ref_trace, port_trace):
    with open(ref_trace) as rf, open(port_trace) as pf:
        for r_line, p_line in zip(rf, pf):
            r_tick = json.loads(r_line)
            p_tick = json.loads(p_line)

            # Compare Kalman state
            s_diff = abs(r_tick['kalman_state']['s_cm'] -
                        p_tick['kalman_state']['s_cm'])
            if s_diff > 1000:  # 10m threshold
                print(f"Tick {r_tick['time']}: s_cm diff = {s_diff} cm")

            # Compare stop states
            for r_stop, p_stop in zip(
                r_tick['detection_state']['stop_states'],
                p_tick['detection_state']['stop_states']
            ):
                if r_stop['fsm_state'] != p_stop['fsm_state']:
                    print(f"Stop {r_stop['index']}: "
                          f"{r_stop['fsm_state']} → {p_stop['fsm_state']}")
```
```

- [ ] **Step 3: Write accuracy metrics section**

```markdown
## Accuracy Metrics

### Target Performance

| Metric | Target | Rationale |
|--------|--------|-----------|
| Arrival detection | ≥ 97% | Production threshold |
| False positive rate | < 3% | User experience |
| False negative rate | < 3% | Operational requirement |
| Position error (steady) | < 10m | Kalman effectiveness |
| Position error (drift) | < 15m | GPS noise handling |

### Per-Scenario Acceptance

| Scenario | Min Accuracy | Notes |
|----------|-------------|-------|
| ty225_normal | ≥ 97% | Baseline |
| ty225_jump | ≥ 95% | Single anomaly acceptable |
| ty225_drift | ≥ 95% | Noise tolerance |

### Failure Diagnosis

| Symptom | Likely Cause | Check |
|---------|--------------|-------|
| High false positive | Probability threshold too low | Verify P > 191 |
| High false negative | Corridor too narrow | Check corridor boundaries |
| Position drift | Kalman gain too high | Verify K_s values |
| Missed stops | Map matching error | Check heading gate |
| Duplicate arrivals | FSM reactivation | Verify one-time rule |
```

- [ ] **Step 4: Commit**

```bash
git add docs/porting/03-golden-tests.md
git commit -m "docs(porting): add golden tests validation guide"
```

---

### Task 5: Create porting checklist documentation

**Files:**
- Create: `docs/porting/04-porting-checklist.md`
- Source: `docs/specs/00-constraints.md` (platform constraints)
- Source: `docs/dev_guide.md` (embedded considerations)

- [ ] **Step 1: Write header and iOS section**

```markdown
# Platform Porting Checklist

## iOS Porting

### CoreLocation Integration

**Required:**
- [ ] Configure `CLLocationManager` with 1 Hz updates
- [ ] Request `Always` location permission (background operation)
- [ ] Enable `allowsBackgroundLocationUpdates`
- [ ] Set `desiredAccuracy = kCLLocationAccuracyBestForNavigation`

**NMEA Emulation:**
iOS doesn't provide raw NMEA. Create adapter:

```swift
extension CLLocation {
    func toNMEA() -> String {
        let time = formatter.string(from: timestamp)
        let lat = abs(coordinate.latitude)
        let latDeg = Int(lat)
        let latMin = (lat - Double(latDeg)) * 60
        let latHem = coordinate.latitude >= 0 ? "N" : "S"

        let lon = abs(coordinate.longitude)
        let lonDeg = Int(lon)
        let lonMin = (lon - Double(lonDeg)) * 60
        let lonHem = coordinate.longitude >= 0 ? "E" : "W"

        let speedKnots = speed * 1.94384
        let heading = course >= 0 ? course : 0

        return "$GPRMC,\(time),A,"
             + "\(latDeg)\(String(format: "%.4f", latMin)),\(latHem),"
             + "\(lonDeg)\(String(format: "%.4f", lonMin)),\(lonHem),"
             + "\(String(format: "%.1f", speedKnots)),\(heading),,"
             + ",,,D"
    }
}
```

### Math Implementation

**Floating-point is acceptable:**
- Use `Double` for distances (meters)
- Convert to cm for integer compatibility: `let d_cm = Int32(d_m * 100)`
- Kalman filter can use full covariance matrix

**Example:**
```swift
struct KalmanState {
    var s_cm: Int32
    var v_cms: Int32
    var P: [[Double]]  // 2×2 covariance

    mutating func update(z_cm: Int32, v_gps: Int32) {
        // Predict
        let s_pred = s_cm + v_cms
        let v_pred = v_cms

        // Update (with full covariance)
        let K = computeKalmanGain(P: P)
        s_cm = Int32(Double(s_pred) + K[0] * Double(z_cm - s_pred))
        v_cms = max(0, Int32(Double(v_pred) + K[1] * Double(v_gps - v_pred)))
    }
}
```

### Threading Model

**Option A: OperationQueue**
```swift
let queue = OperationQueue()
queue.maxConcurrentOperationCount = 1
queue.qualityOfService = .userInitiated

func processGPS(location: CLLocation) {
    queue.addOperation {
        self.pipeline.tick(location.toNMEA())
    }
}
```

**Option B: Async/Await**
```swift
actor PipelineActor {
    var pipeline: BusArrivalPipeline

    func tick(_ nmea: String) async {
        await pipeline.process(nmea)
    }
}
```

### Memory Management

- Route data: Load from JSON, convert to Swift structs
- LUTs: Precompute as `[UInt8]` arrays
- Runtime state: < 1 KB (single struct instance)
```

- [ ] **Step 2: Write ESP32 section**

```markdown
## ESP32 Porting

### GPS Driver Integration

**UART Configuration:**
```c
#define GPS_UART_NUM      UART_NUM_1
#define GPS_TX_PIN        4
#define GPS_RX_PIN        5
#define GPS_BAUD_RATE     9600

void gps_init() {
    uart_config_t uart_config = {
        .baud_rate = GPS_BAUD_RATE,
        .data_bits = UART_DATA_8_BITS,
        .parity = UART_PARITY_DISABLE,
        .stop_bits = UART_STOP_BITS_1,
        .flow_ctrl = UART_HW_FLOWCTRL_DISABLE,
    };
    uart_param_config(GPS_UART_NUM, &uart_config);
    uart_set_pin(GPS_UART_NUM, GPS_TX_PIN, GPS_RX_PIN, ...);
}
```

**NMEA Parsing:**
```c
char nmea_buffer[256];

void gps_task(void* arg) {
    while (1) {
        int len = uart_read_bytes(GPS_UART_NUM, nmea_buffer, sizeof(nmea_buffer), 100 / portTICK_PERIOD_MS);
        if (len > 0 && validate_nmea(nmea_buffer, len)) {
            pipeline_tick(nmea_buffer);
        }
    }
}
```

### Memory Constraints

**SRAM Budget (520 KB total):**
- Route data: ~12 KB (load from Flash to RAM for speed)
- Runtime state: ~1 KB
- Stack per task: ~4 KB
- Free for application: > 500 KB

**Flash Storage:**
- Store route_data.bin in SPIFFS or raw Flash partition
- Use XIP (Execute-in-Place) for read-only access

**Example:**
```c
#define ROUTE_FLASH_ADDR 0x300000  // 3MB offset

const RouteNode* load_route_nodes(size_t* count) {
    const RouteHeader* header = (const RouteHeader*)ROUTE_FLASH_ADDR;
    *count = header->node_count;
    return (const RouteNode*)(ROUTE_FLASH_ADDR + sizeof(RouteHeader));
}
```

### FreeRTOS Task Structure

```c
void pipeline_task(void* arg) {
    PipelineState pipeline;
    pipeline_init(&pipeline);

    TickType_t last_tick = xTaskGetTickCount();

    while (1) {
        // Wait for GPS data (queue-based)
        NMEAMessage msg;
        if (xQueueReceive(gps_queue, &msg, pdMS_TO_TICKS(1000)) == pdTRUE) {
            pipeline_tick(&pipeline, &msg);

            // Emit trace output (optional)
            trace_emit(&pipeline);
        }

        // 1 Hz tick handling
        vTaskDelayUntil(&last_tick, pdMS_TO_TICKS(1000));
    }
}

void app_main() {
    // Create GPS task
    xTaskCreate(gps_task, "gps", 4096, NULL, 5, NULL);

    // Create pipeline task
    xTaskCreate(pipeline_task, "pipeline", 8192, NULL, 4, NULL);
}
```

### Integer Math Verification

**All runtime calculations must be integer-only:**

```c
// ✅ Correct: integer arithmetic
int32_t dist2 = dx*dx + dy*dy;

// ❌ Wrong: floating-point
float dist2 = sqrtf(dx*dx + dy*dy);  // Don't do this!

// ✅ Correct: fixed-point Kalman
int32_t s_new = s_pred + (51 * (z_gps - s_pred)) / 256;

// ❌ Wrong: floating-point Kalman
float s_new = s_pred + 0.2f * (z_gps - s_pred);  // Don't do this!
```

**LUT Usage:**
```c
// Precomputed Gaussian LUT (generated offline)
extern const uint8_t GAUSSIAN_LUT[256];

uint8_t gaussian_lut(int32_t d_cm, int32_t sigma_cm) {
    if (sigma_cm == 0) return 255;
    int32_t idx = ((int64_t)d_cm * 64) / sigma_cm;
    if (idx < 0) idx = -idx;
    if (idx >= 256) idx = 255;
    return GAUSSIAN_LUT[idx];
}
```
```

- [ ] **Step 3: Write common tasks section**

```markdown
## Common Implementation Tasks

### NMEA Parser

**Required fields to extract:**

| Field | NMEA Source | Type | Conversion |
|-------|-------------|------|------------|
| Time | $GPRMC field 1 | string | Parse hhmmss.ss |
| Lat | $GPRMC field 3 | ddmm.mmmm | Convert to GeoCdeg |
| Lon | $GPRMC field 5 | dddmm.mmmm | Convert to GeoCdeg |
| Speed | $GPRMC field 7 | knots | × 51.44 → SpeedCms |
| Heading | $GPRMC field 8 | degrees | × 100 → HeadCdeg |
| HDOP | $GPGGA field 8 | float | × 10 → hdop_x10 |

**Validation:**
- Checksum verification (required)
- Status 'A' (valid data)
- Fix quality > 0

### Route Data Loading

**Binary format (recommended):**
```c
// C header for binary format
typedef struct __attribute__((packed)) {
    int32_t x_cm;
    int32_t y_cm;
    int32_t cum_dist_cm;
    int32_t seg_len_mm;
    int16_t dx_cm;
    int16_t dy_cm;
    int16_t heading_cdeg;
    int16_t _pad;
} RouteNode;  // 24 bytes

typedef struct __attribute__((packed)) {
    uint8_t index;
    int32_t s_cm;
    int32_t corridor_start_cm;
    int32_t corridor_end_cm;
    uint8_t _pad[3];
} Stop;  // 16 bytes
```

**JSON format (easier debugging):**
```swift
struct Route: Codable {
    let nodes: [RouteNode]
    let stops: [Stop]
    let grid: SpatialGrid
}
```

### Trace Output Generation

**Format:** One JSON line per tick

```swift
func emitTrace(_ state: PipelineState) -> String {
    let gps = [
        "time": state.time,
        "x_cm": state.gps.x_cm,
        "y_cm": state.gps.y_cm,
        "heading_cdeg": state.gps.heading_cdeg,
        "speed_cms": state.gps.speed_cms,
        "hdop_x10": state.gps.hdop_x10
    ]

    let kalman = [
        "s_cm": state.kalman.s_cm,
        "v_cms": state.kalman.v_cms,
        "z_cm": state.kalman.z_cm
    ]

    // ... build full JSON object
    return jsonString + "\n"
}
```
```

- [ ] **Step 4: Commit**

```bash
git add docs/porting/04-porting-checklist.md
git commit -m "docs(porting): add platform porting checklist"
```

---

### Task 6: Update MEMORY.md index

**Files:**
- Modify: `MEMORY.md`

- [ ] **Step 1: Add porting docs entry**

```markdown
## Documentation

- [Porting Documentation](docs/porting/00-algorithm-spec.md) — Platform-agnostic algorithm specs, data formats, and testing guides for iOS/ESP32 porting
```

- [ ] **Step 2: Commit**

```bash
git add MEMORY.md
git commit -m "docs: index porting documentation"
```

---

## Self-Review Checklist

- [ ] **Spec coverage:** All 5 docs from design spec accounted for
- [ ] **No placeholders:** Every code block and example is complete
- [ ] **Type consistency:** DistCm, SpeedCms, HeadCdeg, Prob8 used consistently
- [ ] **Cross-references:** Links to tech report v8 sections where appropriate
- [ ] **Test data:** ty225_normal, ty225_jump, ty225_drift documented
- [ ] **Platform notes:** iOS (floating-point OK) and ESP32 (integer-only) distinguished
- [ ] **Accuracy targets:** ≥97% threshold clearly stated

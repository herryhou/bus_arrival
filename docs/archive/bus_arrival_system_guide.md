# Bus Arrival Detection System

## Overview

GPS-based bus arrival detection system. Processes NMEA sentences through a 3-phase pipeline: route preprocessing → GPS localization → Bayesian arrival detection.

## Deployment Targets

| Target | Platform | Purpose |
|--------|----------|---------|
| **Android App** | Kotlin + JVM | Production deployment, real-time detection |
| **Host Tools** | Rust (std) | Route preprocessing, trace validation, testing |
| **Embedded Firmware** | Rust (no_std) | RP2350 microcontroller, resource-constrained |

## System Architecture

```
┌────────────────────────────────────────────────────────────────┐
│                       Android App (Kotlin)                     │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐ │
│  │   Location  │─▶│   Pipeline  │─▶│    DetectionService     │ │
│  │   Manager   │  │             │  │  (Foreground Service)   │ │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘ │
│                         │                    │                 │
│                         ▼                    ▼                 │
│                    ┌─────────┐         ┌──────────┐            │
│                    │  Trace  │         │ Database │            │
│                    │  Output │         │ (Room)   │            │
│                    └─────────┘         └──────────┘            │
└────────────────────────────────────────────────────────────────┘

┌───────────────────────────────────────────────────────────────┐
│                    Host Tools (Rust)                          │
│  ┌──────────────┐  ┌──────────────┐  ┌─────────────────────┐  │
│  │ Preprocessor │─▶│   Pipeline   │─▶│  Trace Validator    │  │
│  │ (Route Prep) │  │ (NMEA→Trace) │  │  (Testing/Debug)    │  │
│  └──────────────┘  └──────────────┘  └─────────────────────┘  │
└───────────────────────────────────────────────────────────────┘
```

## Phase 1: Route Preprocessing

**Binary:** `preprocessor` (Rust)

Converts GeoJSON route + stops → `route_data.bin` with spatial index.

**Steps:**
1. Douglas-Peucker simplification (ε = 5m)
2. Stop-to-segment mapping
3. Spatial grid index (v5.1 sparse bitmask)
4. Binary pack (XIP-compatible for embedded)

**Output:** `route_data.bin` (~20-40 KB for typical routes)

## Phase 2: GPS Localization

**Components:** `MapMatcher` → `KalmanFilter` → `DeadReckoning`

### Map Matching

Heading-constrained projection onto route segments.

**Algorithm:**
1. Spatial grid lookup (cell size 100m)
2. Heading validation: accept if `|Δheading| ≤ 90°`
3. Select nearest point on matched segment
4. Return `(s_cm, segment_index)`

### Kalman Filter

1D filter for route position estimation.

**State:** `[s_cm, v_cms]` (position, velocity)

**Update cycle:**
1. Predict: `s_pred = s + v × dt`
2. Measure: `z = project_to_route(gps)`
3. Update: `s = s_pred + K × (z - s_pred)`

**Gains:** Position `51/256` to `77/256` (HDOP-adaptive), Velocity `77/256`

### Dead Reckoning

Activated during GPS outages (> 10 seconds without valid fix).

**Logic:** Project position using last known velocity, decay confidence over time.

## Phase 3: Arrival Detection

**Components:** `StateMachine` + `ProbabilityModel` + `Recovery`

### Probability Model

4-feature Bayesian model for arrival confidence.

**Features:**
- F1 (Distance): `P(d|A)` using `z_gps_cm`, σ = 2750 cm
- F2 (Speed): `P(v|A)` using `v_cms`, threshold = 200 cm/s
- F3 (Progress): `P(δs|A)` using `s_cm`, σ = 2000 cm
- F4 (Dwell): Time spent at stop

**Output:** `Prob8` (0-255), threshold = 128

### State Machine

Per-stop FSM with 6 states.

**States:**
- `Idle`: Before entering corridor (bus not near stop)
- `Approaching`: In corridor, not yet close
- `Arriving`: In arrival zone (close to stop, probability ≥ 128)
- `AtStop`: Confirmed stop (distance ≤ 50m)
- `Departed`: Moved past stop
- `TripComplete`: Past last stop (terminal state)

**Transitions:**
- `Idle → Approaching`: Enter corridor (distance < threshold)
- `Approaching → Arriving`: `probability ≥ 128` for 2 consecutive ticks
- `Arriving → AtStop`: `distance ≤ 50m` AND `probability ≥ 191`
- `AtStop → Departed`: `distance > 40m` AND moved past stop
- `Departed → Approaching`: Re-entry after leaving corridor
- `Any → TripComplete`: Past last stop in route

### Recovery

Stop index recovery after GPS anomalies (jumps, detours, multipath).

**Trigger:** GPS jump > 50 m OR off-route detection

**Algorithm:** Forward projection with corridor filtering, select best match.

## Mode System

3 operating modes managed by `ModeMachine`.

| Mode | Position Source | Detection | Use Case |
|------|-----------------|-----------|----------|
| **Normal** | Kalman-filtered | Enabled | Default operation |
| **OffRoute** | Frozen (last valid) | Suppressed | Off-route detected |
| **Recovering** | Raw GPS | Search active | After jump/detour |

**Transitions:**
- `Normal → OffRoute`: divergence > 50m for 5 consecutive ticks
- `OffRoute → Recovering`: divergence ≤ 50m for 2 ticks AND displacement > 50m
- `OffRoute → Normal`: divergence ≤ 50m for 2 ticks AND displacement ≤ 50m
- `Recovering → Normal`: Recovery search succeeds

Note:
- divergence = How far is GPS from the route
- displacement = |current_z_gps_cm - frozen_s_cm|, How far has GPS moved since we froze the position.

## Data Flow

```
NMEA Sentences
      │
      ▼
┌─────────────┐
│ NmeaParser  │ → GpsPoint { lat, lon, heading, speed, hdop }
└─────────────┘
      │
      ▼
┌─────────────┐
│ MapMatcher  │ → (s_cm, segment_idx)
└─────────────┘
      │
      ▼
┌─────────────┐
│ KalmanFilter│ → KalmanState { s_cm, v_cms, last_seg_idx }
└─────────────┘
      │
      ▼
┌─────────────────────┐
│ StateMachine (per   │ → StopEvent { Arrived, Departed }
│ active stop)        │
└─────────────────────┘
      │
      ▼
┌─────────────────────┐
│ TraceWriter         │ → trace.jsonl (debug output)
│ Database            │ → Room DB (persistent events)
└─────────────────────┘
```

## Semantic Type System

All physical quantities use integer types (no floating-point in firmware).

| Type | Definition | Range | Purpose |
|------|------------|-------|---------|
| `DistCm` | `i32` | ±214 km | Distance in centimeters |
| `SpeedCms` | `i32` | 0..214 km/h | Speed in cm/s |
| `HeadCdeg` | `i16` | -180°..+180° | Heading in 0.01° |
| `GeoCdeg` | `i16` | -180°..+180° | Lat/lon in 0.01° |
| `Prob8` | `u8` | 0..255 | Probability × 255 |

## File Formats

### route_data.bin

Binary format with precomputed coefficients.

```
Header (16 bytes):
  - node_count: u32
  - stop_count: u32
  - x0_cm: i32
  - y0_cm: i32

Nodes array (node_count × 52 bytes):
  - RouteNode { x_cm, y_cm, s_cm, seg_len, heading_cdeg }

Stops array (stop_count × 12 bytes):
  - Stop { s_cm, corridor_start_cm, corridor_end_cm }

Spatial grid (variable):
  - Sparse bitmask cells

CRC32: u32 (4 bytes)
```

### trace.jsonl

JSON Lines format, one record per GPS tick.

```json
{
  "time": 1234567890,
  "lat": 25.0,
  "lon": 121.0,
  "s_cm": 123456,
  "v_cms": 150,
  "mode": "Normal",
  "active_stops": [5, 6],
  "stop_states": [{
    "stop_idx": 5,
    "distance_cm": 250,
    "fsm_state": "Arriving",
    "probability": 210
  }]
}
```

## Build & Run

### Android

```bash
cd android
./gradlew assembleDebug
./gradlew test
```

### Host (Rust)

```bash
# Build all
make build

# Generate route data
make preprocess ROUTE_NAME=ty225

# Run pipeline
make run ROUTE_NAME=ty225 SCENARIO=normal

# Validate trace
make validate-trace TRACE=test_data/ty225_normal_trace.jsonl
```

### Embedded (RP2350)

```bash
make build-firmware
make flash-firmware
```

## Component Locations

### Kotlin (Android)

```
android/app/src/main/java/com/busarrival/app/
├── service/
│   ├── DetectionPipeline.kt    # Main pipeline orchestration
│   ├── DetectionService.kt     # Foreground service
│   └── LocationManager.kt      # GPS input handling
├── data/pipeline/
│   ├── localization/
│   │   ├── MapMatcher.kt
│   │   ├── KalmanFilter.kt
│   │   └── DeadReckoning.kt
│   └── detection/
│       ├── StateMachine.kt
│       ├── ProbabilityModel.kt
│       └── Recovery.kt
└── presentation/ui/
    ├── detection/DetectionScreen.kt
    └── history/HistoryScreen.kt
```

### Rust (Host)

```
crates/
├── preprocessor/      # Route preprocessing
├── pipeline/          # Main pipeline (gps_processor + detection)
│   ├── gps_processor/ # Map matching, Kalman, DR
│   └── detection/     # State machine, probability, recovery
├── shared/            # Types, binary format, constants
└── trace_validator/   # Trace validation tool
```

## Key Constants

| Parameter | Value | Purpose |
|-----------|-------|---------|
| `V_MAX_CMS` | 1667 (60 km/h) | Speed constraint filter |
| `SIGMA_GPS_CM` | 2000 (20 m) | GPS noise margin |
| `GPS_JUMP_THRESHOLD` | 5000 (50 m) | Recovery trigger |
| `OFF_ROUTE_D2_THRESHOLD` | 25000000 | Off-route detection (50m²) |
| `SIGMA_D_CM` | 2750 | Distance likelihood (F1) |
| `SIGMA_P_CM` | 2000 | Progress difference (F3) |
| `V_STOP_CMS` | 200 (7.2 km/h) | Stop speed threshold |

## Testing

### Golden Tests

Regression tests using pre-recorded scenarios.

```bash
# Run all golden tests
cargo test -p pipeline --test scenarios -- --nocapture

# Specific scenario
cargo test -p pipeline --test scenarios -- normal
```

### Trace Validation

Compare trace output against ground truth.

```bash
cargo run -p trace_validator -- \
  trace.jsonl ground_truth.json report.json
```

## Reference Documentation

- **`docs/specs/00-constraints.md`** — Semantic types, resource budgets
- **`docs/specs/06-state_machine.md`** — FSM transitions and rules
- **`docs/specs/07-stop_recovery.md`** — Recovery algorithm details
- **`CLAUDE.md`** — Build commands and quick reference

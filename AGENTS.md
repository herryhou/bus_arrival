## Source of Truth

**MUST read before any work:**
- `bus_arrival_tech_report_v8.md` — Detailed design doc with rationale, algorithms, and data structures
- `docs/SPEC.md` — Master spec index

## Build Commands

```bash
# Build all
make build

# Run pipeline (NMEA + route_data → trace.jsonl)
make run ROUTE_NAME=ty225 SCENARIO=normal

# Generate route data from GeoJSON
cargo run -p preprocessor -- route.json stops.json output.bin

# Run pipeline directly
cargo run -p pipeline -- nmea.txt route.bin

# Extract from trace
./tools/arrival_from_trace.sh trace.jsonl > arrivals.jsonl
./tools/announce_from_trace.sh trace.jsonl > announce.jsonl

# Tests
cargo test
cargo test -p pipeline
```

## Architecture

3-phase pipeline: NMEA → GPS localization (Kalman + map matching) → Bayesian arrival detection → `trace.jsonl`

```
crates/
├── shared/           # Types, binary format
├── preprocessor/     # Route simplification
├── pipeline/         # GPS + detection
├── trace_validator/  # Validation tool
└── pico2-firmware/   # Embedded (RP2350, no_std)
```

## Architecture

The system processes GPS NMEA data to detect bus arrivals using a 3-phase pipeline:

**Phase 1 (preprocessor):** Route preprocessing and simplification (Douglas-Peucker algorithm)
- Input: GeoJSON route + stops
- Output: Binary route data (`route_data.bin`) with precomputed coefficients

**Phase 2 (gps_processor):** GPS localization with Kalman filtering and map matching
- Spatial grid index, heading-constrained map matching, 1D Kalman filter
- Dead-reckoning for GPS outages

**Phase 3 (detection):** Bayesian arrival detection with finite state machine
- 4-feature probability model (distance, speed, progress error, dwell time)
- Stop corridor filtering, state machine (Approaching → Arriving → AtStop → Departed)
- Stop index recovery after GPS anomalies

**Output:** `trace.jsonl` (complete state machine trace with arrivals, departures, and all intermediate states)

## Workspace Structure

```
crates/
├── shared/           # Shared types and binary format (RouteNode, Stop, binfile)
├── preprocessor/     # Phase 1: Route simplification and binary packing
├── pipeline/         # Phase 2 + 3: Unified pipeline (gps_processor + detection)
│   ├── gps_processor/  # GPS localization library
│   └── detection/      # Arrival detection library
├── trace_validator/  # Trace validation tool (compare vs ground truth)
└── pico2-firmware/   # Embedded firmware (RP2350, no_std, embassy-rp)
```

## Firmware (pico2-firmware)

**3-layer architecture with clear component boundaries:**

### Component Layers
- **Parser Layer:** `NmeaParser` component (NMEA → GpsPoint)
- **Estimation Layer:** `EstimationState` with isolated Kalman/DR pipeline
- **Control Layer:** `SystemState` orchestrating `ModeMachine`, estimation, detection, recovery

### Key Components
- **NmeaParser:** `feed_sentence()` → `Option<GpsPoint>` (pure state update)
- **ModeMachine:** `update(ModeInput)` → `ModeOutput` (pure state machine)
- **Estimation:** `estimate(EstimationInput)` → `EstimationOutput` (isolated pipeline)
- **Recovery:** `recover(RecoveryInput)` → `Option<usize>` (pure function)

### Principles
- **Isolation:** Estimation has no access to mode/stop state
- **Single Transition:** ModeMachine enforces one transition per tick
- **Explicit Boundaries:** Each component has well-defined inputs/outputs
- **Testability:** Components can be tested in isolation

### Mode System
- **Normal:** Kalman-filtered position, arrival detection enabled
- **OffRoute:** Position frozen, detection suppressed
- **Recovering:** Raw GPS position, recovery search active

### Entry Point
- `main.rs` uses `SystemState::tick(gps, est_state)` → `Option<ArrivalEvent>`
- Old `state::State` deprecated but still available

## Key Constraints

- **Integer-only** (no FPU on RP2350)
- **Semantic types:** `DistCm` (i32), `SpeedCms` (i32), `HeadCdeg` (i16), `Prob8` (u8)
- **XIP:** Route data in Flash, zero-copy
- **Budget:** < 8% CPU @ 150MHz (1Hz GPS), ~34 KB Flash, < 1 KB SRAM

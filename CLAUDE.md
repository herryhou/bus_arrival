# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Source of Truth

**LLM Specs (READ THESE FIRST):**
- `docs/SPEC.md` — Master spec index with task→spec mapping
- `docs/specs/00-constraints.md` — **MUST read before any work** — Integer-only, semantic types, budgets

**Implementation Details:**
- `docs/specs/*.md` — Module-specific specs (map matching, Kalman, detection, etc.)

**Background Reading (not for implementation):**
- `docs/bus_arrival_tech_report_v8.md` — Algorithm explanations (why things work)
- `docs/spatial_grid_binary_format.md` — Grid index v5.1 format details

**Related:**
- `docs/dev_guide.md` — Embedded Rust development guide for RP2350 (no_std, embassy-rp)
- `docs/arrival_detector_test.md` — BDD-style test plan and validation approach

## Build Commands

```bash
# Build all binaries (host + firmware)
cargo build --release
make build

# Run full pipeline with test data
make run ROUTE_NAME=ty225 SCENARIO=normal

# Generate route data from GeoJSON
cargo run -p preprocessor -- test_data/ty225_route.json test_data/ty225_stops.json test_data/ty225.bin

# Run pipeline (NMEA + route_data → arrivals/departures)
cargo run -p pipeline -- test_data/ty225_normal_nmea.txt test_data/ty225.bin arrivals.jsonl --trace trace.jsonl

# Build Pico 2 W firmware (no_std, RP2350)
cargo build --release --target thumbv8m.main-none-eabi -p pico2-firmware
make build-firmware
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

**Output:** `arrivals.jsonl` (events), `trace.jsonl` (debug state), `announce.jsonl` (voice events)

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

## Firmware Architecture (2-Layer Design)

The pico2-firmware crate implements a 2-layer architecture for embedded deployment:

**Control Layer** (`crates/pico2-firmware/src/control/`):
- `SystemState` — state machine managing Normal/OffRoute/Recovering modes
- `SystemMode` — mode enum with transition functions
- Unified triggers based on `divergence_d2` and `displacement`
- Recovery timeout (30s) with geometric fallback
- `tick()` orchestrator — coordinates estimation and detection

**Estimation Layer** (`crates/pico2-firmware/src/estimation/`):
- `estimate()` — isolated GPS → position pipeline
- `KalmanState` — Kalman filter (no control state)
- `DrState` — DR/EMA state (no control state)
- Returns `EstimationOutput` with confidence signal

**Recovery Module** (`crates/pico2-firmware/src/recovery/`):
- `recover()` — pure function with explicit `RecoveryInput`
- Search window: hint_idx ± 10 stops (O(20) performance)
- Spatial anchor penalty for off-route recovery
- Velocity constraint prevents physically impossible jumps

**Key Design Principles:**
- **Isolation:** Estimation layer has bounded internal state, no access to control layer
- **Unified triggers:** All mode transitions use estimation signals only
- **Single transition:** Only ONE mode change per tick (prevents race conditions)
- **First-class recovery:** Recovery is a system mode, not inline logic

## Testing

```bash
# Run all tests
cargo test

# Run specific crate tests
cargo test -p pipeline
cargo test -p shared

# Run scenario-based integration tests
cargo test -p pipeline --test integration_test -- scenarios

# Validate trace against ground truth
cargo run --release --bin trace_validator -- trace.jsonl --ground-truth gt.json -o report.html
```

Test coverage follows the plan in `docs/arrival_detector_test.md`:
- Scenario-based validation (normal, drift, jump, outage)
- Exact validation (precision/recall metrics, target ≥97%)
- Order validation (monotonically increasing stop detection)
- Position accuracy (within 50m at AtStop state)
- Edge cases (corrupt NMEA, stationary GPS, extreme jumps)

## Key Constraints

- **Integer-only arithmetic** for RP2350 (no hardware FPU)
- **Semantic type system:** `DistCm` (i32), `SpeedCms` (i32), `HeadCdeg` (i16), `Prob8` (u8)
- **XIP (Execute-in-Place)** for route data in Flash - zero-copy access
- **CPU budget:** < 8% @ 150MHz for 1Hz GPS updates
- **Memory budget:** ~34 KB Flash, < 1 KB SRAM runtime

## Off-Route Detection (feat/off-route-detection branch)

The system includes off-route detection and recovery:
- Immediate position freezing when off-route suspected
- Re-acquisition detection for recovery
- Arrival suppression during off-route episodes
- Off-route state exposed in trace output

# Crates - Rust Workspace

Core Rust implementation: preprocessing, GPS pipeline, embedded firmware.

## Workspace Structure

```
crates/
├── shared/           # Types, binary format (RouteNode, Stop, binfile)
├── preprocessor/     # Phase 1: Route simplification (Douglas-Peucker)
├── pipeline/         # Phase 2 + 3: GPS + detection
│   ├── gps_processor/  # GPS localization (Kalman + map matching)
│   └── detection/      # Bayesian arrival detection
├── trace_validator/  # Trace validation tool
└── pico2-firmware/   # Embedded firmware (RP2350, no_std, embassy-rp)
```

## Build Commands

```bash
# Build all
cargo build

# Build specific crate
cargo build -p preprocessor
cargo build -p pipeline
cargo build -p pico2-firmware

# Run pipeline (NMEA + route_data → trace_v2.jsonl)
cargo run -p pipeline -- nmea.txt route.bin

# Generate route data from GeoJSON
cargo run -p preprocessor -- route.json stops.json output.bin

# Tests
cargo test
cargo test -p pipeline
```

## Key Types (shared/)

- `RouteNode` — Route point with precomputed coefficients
- `Stop` — Bus stop with corridor definition
- `GpsPoint` — NMEA-derived position
- `Estimate` — Kalman-filtered position
- `ArrivalEvent` — Arrival/departure event

## Semantic Types

- `DistCm` (i32) — Distance in centimeters
- `SpeedCms` (i32) — Speed in cm/s
- `HeadCdeg` (i16) — Heading in centidegrees
- `Prob8` (u8) — Probability (0-255)

## Firmware (pico2-firmware)

3-layer architecture:
- **Parser:** `NmeaParser` (NMEA → GpsPoint)
- **Estimation:** `EstimationState` (Kalman/DR pipeline)
- **Control:** `SystemState` (mode machine, detection, recovery)

Entry point: `SystemState::tick(gps, est_state) → Option<ArrivalEvent>`

Mode system:
- `Normal` — Kalman position, detection enabled
- `OffRoute` — Position frozen, detection suppressed
- `Recovering` — Raw GPS, recovery search

Constraints: Integer-only, XIP, < 8% CPU @ 150MHz, ~34 KB Flash, < 1 KB SRAM

## Related Docs

See `docs/SPEC.md` and `docs/specs/` for algorithms and rationale.

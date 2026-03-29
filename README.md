# Bus Arrival Detection System

GPS-based bus arrival detection system for RP2350 microcontroller with web-based debugging visualizer.

## Overview

This system processes GPS NMEA data to detect bus arrivals and departures at predefined stops using:
- **Phase 1**: Route preprocessing and simplification (Douglas-Peucker algorithm)
- **Phase 2**: GPS localization with Kalman filtering and map matching
- **Phase 3**: Bayesian arrival detection with finite state machine

## Architecture

```
┌─────────────┐     ┌──────────────────┐     ┌─────────────────┐
│ NMEA Log    │────▶│ Pipeline         │────▶│ Arrivals/       │
│ (GPS data)  │     │ (Phase 2 + 3)    │     │ Departures      │
└─────────────┘     └──────────────────┘     └─────────────────┘
                            │
                            │ --trace --▶ Debug Output
                            │ --announce --▶ Voice Events
                            ▼
                     ┌─────────────┐
                     │ Visualizer  │
                     │ (Web UI)    │
                     └─────────────┘
```

## Quick Start

### 1. Build the Project

```bash
cargo build --release
```

### 2. Process GPS Data

```bash
# Generate route data from GeoJSON
cargo run -p preprocessor -- tools/data/ty225_route.json tools/data/ty225_stops.json route_data.bin

# Run pipeline to detect arrivals/departures
cargo run -p pipeline -- gps.nmea route_data.bin arrivals.jsonl --trace trace.jsonl
```

### 3. Visualize Results

```bash
cd visualizer
npm install
npm run dev
```

Open http://localhost:5173/ and upload `route_data.bin` and `trace.jsonl`.

## Pipeline Usage

```bash
pipeline [OPTIONS] <nmea> <route_data> <output>

Arguments:
  <nmea>       NMEA log file (GPS data)
  <route_data> Route data binary file
  <output>     Output JSONL file for arrivals/departures

Options:
  --trace <file>    Enable trace output to file (for debugging)
  --announce <file> Enable announce event output to file
  -h, --help        Show this help message
```

### Output Files

| File | Description |
|------|-------------|
| `arrivals.jsonl` | Arrival and departure events (one per line) |
| `trace.jsonl` | Debug trace with internal state (if `--trace` specified) |
| `announce.jsonl` | Voice announcement events (if `--announce` specified) |

## Generating Test Data

### Step 1: Prepare GeoJSON Files

Create route and stop GeoJSON files in `tools/data/`:

```bash
# Example: Generate NMEA test data
node tools/gen_nmea/gen_nmea.js generate \
  --route tools/data/ty225_route.json \
  --stops tools/data/ty225_stops.json \
  --scenario normal \
  --out_nmea ty225.nmea \
  --out_gt ty225_gt.json
```

### Step 2: Generate Route Data

```bash
cargo run -p preprocessor -- \
  tools/data/ty225_route.json \
  tools/data/ty225_stops.json \
  route_data.bin
```

**Output:**
```
Loaded 638 waypoints, 58 stops
Computed average latitude: 24.99°
Simplified route: 638 → 805 nodes
Built route graph with 805 nodes
Built spatial grid: 38x36 cells
Successfully wrote route_data.bin
```

### Step 3: Run Pipeline

```bash
cargo run -p pipeline -- ty225.nmea route_data.bin arrivals.jsonl \
  --trace trace.jsonl --announce announce.jsonl
```

## Data Formats

### `route_data.bin` (Binary)

Binary format with precomputed route coefficients:

```
Header (16 bytes):
  - node_count: u32
  - stop_count: u32
  - x0_cm: i32
  - y0_cm: i32

Nodes array (node_count × 52 bytes):
  - RouteNode with precomputed segment coefficients

Stops array (stop_count × 12 bytes):
  - Stop: progress_cm, corridor_start_cm, corridor_end_cm

CRC32: u32 (4 bytes)
```

### `arrivals.jsonl` (JSON Lines)

One JSON object per line:
```json
{"time":1234567900,"stop_idx":5,"s_cm":123500,"v_cms":10,"probability":210}
```

### `trace.jsonl` (JSON Lines)

Full internal state for debugging:
```json
{
  "time": 1234567890,
  "lat": 25.0,
  "lon": 121.0,
  "s_cm": 123456,
  "v_cms": 150,
  "active_stops": [5, 6],
  "stop_states": [{
    "stop_idx": 5,
    "distance_cm": 250,
    "fsm_state": "Arriving",
    "dwell_time_s": 0,
    "probability": 210,
    "features": {"p1": 200, "p2": 180, "p3": 190, "p4": 100},
    "just_arrived": false
  }],
  "gps_jump": false,
  "recovery_idx": null
}
```

## Project Structure

```
bus_arrival/
├── crates/                  # Rust workspace
│   ├── shared/              # Shared types and binary format
│   ├── preprocessor/        # Phase 1: Route simplification
│   ├── pipeline/            # Phase 2 + 3: Unified pipeline
│   │   ├── gps_processor/   # GPS localization library
│   │   └── detection/       # Arrival detection library
│   └── trace_validator/     # Trace validation tool
├── tools/                   # Test data generation
│   ├── data/                # Sample route/stop GeoJSON files
│   └── gen_nmea/            # NMEA test data generator
├── docs/                    # Technical documentation
├── test_data/               # Test fixtures
├── scripts/                 # Utility scripts
└── visualizer/              # Web-based debugging UI
```

## Available Binaries

| Binary | Description |
|--------|-------------|
| `pipeline` | **Recommended** - Unified NMEA → Arrivals/Departures processor |
| `preprocessor` | Generate `route_data.bin` from GeoJSON |
| `trace_validator` | Validate trace output against ground truth |

## Makefile Targets

```bash
make build              # Build all crates
make run                # Run pipeline with test data
make validate-trace     # Validate trace against ground truth
make clean              # Clean build artifacts
```

## Visualizer Features

- **Route Map** - Interactive map with route line and stop markers
- **Timeline Charts** - Speed, probability, and distance over time
- **FSM Inspector** - State machine details per stop
- **Feature Breakdown** - Bayesian probability feature scores (p1-p4)

## Development

### Build

```bash
cargo build              # Debug build
cargo build --release    # Release build
```

### Test

```bash
cargo test               # Run all tests
cargo test -p pipeline   # Test specific crate
```

### Format

```bash
cargo fmt                # Format code
cargo clippy             # Lint code
```

## Technical Documentation

See `docs/` for detailed technical documentation:
- `bus_arrival_tech_report_v8.md` - Complete technical specification
- `core_data_flow.md` - Data flow overview
- `dev_guide.md` - Development guide

## License

MIT

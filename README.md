# Bus Arrival Detection System

GPS-based bus arrival detection system for RP2350 microcontroller with web-based debugging visualizer.

## Architecture

```
┌─────────────┐     ┌──────────────┐     ┌─────────────────┐     ┌─────────────┐
│ NMEA Log    │────▶│ Simulator    │────▶│ Arrival         │────▶│ Arrivals    │
│ (GPS data)  │     │ (Phase 2)    │     │ Detector        │     │ Output     │
└─────────────┘     └──────────────┘     │ (Phase 3)       │     └─────────────┘
                                          │                 │
                                          │ ┌─────────────┐ │
                                          │ │ Trace Output │ │
                                          │ └─────────────┘ │
                                          └─────────────────┘
                                                 │
                                                 ▼
                                          ┌─────────────┐
                                          │ Visualizer  │
                                          │ (Web UI)    │
                                          └─────────────┘
```

## Quick Start

### Generate Test Data Files

The visualizer requires two input files:

1. **`route_data.bin`** - Binary route data with precomputed coefficients
2. **`trace.jsonl`** - Debug trace from arrival detector with internal state

#### Step 1: Generate `route_data.bin`

```bash
# From GeoJSON files (route and stops)
cargo run -p preprocessor -- tools/data/route.json tools/data/stops.json route_data.bin
```

**Output:**
```
Loaded 638 waypoints, 23 stops
Simplified to 255 nodes
Built spatial grid: 9x8 cells
Packed 255 RouteNodes (52 bytes each) = 13260 bytes
Packed 23 Stops (12 bytes each) = 276 bytes
CRC32: 0x12345678
Wrote route_data.bin (13908 bytes)
```

#### Step 2: Generate `phase2.jsonl` (from NMEA log)

```bash
# From GPS NMEA log
cargo run -p simulator -- test.nmea route_data.bin phase2.jsonl
```

**Output:**
```
Phase 2: Localization Pipeline
  NMEA input:   test.nmea
  Route data:   route_data.bin
  Output:       phase2.jsonl

Loaded 255 nodes, 23 stops
Processed 1234 GPS updates
```

#### Step 3: Generate `trace.jsonl` (for visualizer)

```bash
# Run arrival detector with --trace flag
cargo run -p arrival_detector -- phase2.jsonl route_data.bin arrivals.jsonl --trace trace.jsonl
```

**Output:**
```
Phase 3: Arrival Detection
  Input:  phase2.jsonl
  Route:  route_data.bin
  Output: arrivals.jsonl
  Trace:  trace.jsonl

Processed 1234 records, detected 23 arrivals
```

### Run the Visualizer

```bash
cd visualizer
npm install  # First time only
npm run dev
```

Open http://localhost:5173/ and upload:
- `route_data.bin`
- `trace.jsonl`

## Data Formats

### `route_data.bin` (Binary)

```
Header (16 bytes):
  - node_count: u32
  - stop_count: u32
  - x0_cm: i32
  - y0_cm: i32

Nodes array (node_count × 52 bytes):
  - RouteNode: repr(C, packed) struct
  - Contains precomputed segment coefficients

Stops array (stop_count × 12 bytes):
  - Stop: progress_cm, corridor_start_cm, corridor_end_cm

CRC32: u32 (4 bytes)
```

### `phase2.jsonl` (JSON Lines)

One JSON object per line, GPS update:
```json
{"time":1234567890,"s_cm":123456,"v_cms":150,"seg_idx":42,"valid":true}
{"time":1234567891,"s_cm":123606,"v_cms":150,"seg_idx":42,"valid":true}
...
```

### `trace.jsonl` (JSON Lines - Visualizer Input)

Full internal state for debugging:
```json
{
  "time": 1234567890,
  "s_cm": 123456,
  "v_cms": 150,
  "active_stops": [5, 6],
  "stop_states": [
    {
      "stop_idx": 5,
      "distance_cm": 250,
      "fsm_state": "Arriving",
      "dwell_time_s": 0,
      "probability": 210,
      "features": {"p1": 200, "p2": 180, "p3": 190, "p4": 100},
      "just_arrived": false
    }
  ],
  "gps_jump": false,
  "recovery_idx": null
}
```

### `arrivals.jsonl` (JSON Lines - Final Output)

Detected arrival events:
```json
{"time":1234567900,"stop_idx":5,"s_cm":123500,"v_cms":10,"probability":210}
...
```

## Project Structure

```
bus_arrival/
├── shared/           # Shared types and binary format
├── preprocessor/     # Phase 1: Route simplification & binary packing
├── simulator/        # Phase 2: GPS localization (Kalman filter)
├── arrival_detector/ # Phase 3: Bayesian arrival detection
├── visualizer/       # Web-based debugging UI
├── tools/            # Test data generation
│   ├── data/         # Sample route/stop GeoJSON files
│   └── gen_nmea/     # NMEA test data generator
└── target/           # Compiled binaries
```

## Binaries

After `cargo build`, binaries are in `target/debug/`:
- `preprocessor` - Generate `route_data.bin`
- `simulator` - Generate `phase2.jsonl` from NMEA
- `arrival_detector` - Generate arrivals and trace output

## Development

### Build All

```bash
cargo build
```

### Run Tests

```bash
cargo test
```

### Build Visualizer

```bash
cd visualizer
npm run build  # Static output in build/
```

## Visualizer Features

- **Route Map** - MapLibre GL JS map with route line and stop markers
- **Timeline Charts** - Speed, probability, distance over time (Chart.js)
- **FSM Inspector** - State machine details per stop
- **Feature Breakdown** - Bayesian probability feature scores (p1-p4)

## License

MIT

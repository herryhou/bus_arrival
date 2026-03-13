# Bus Arrival Detection Visualizer — Design Spec

**Date:** 2026-03-13
**Status:** Draft
**Type:** Web-based debugging and demonstration tool

---

## Overview

A highly interactive web-based visual tool that displays the internal state of the RP2350 bus arrival detection algorithm as it processes GPS data. The visualizer shows the complete detection pipeline including preprocessing, localization, Bayesian feature fusion, and state machine transitions.

**Goal:** Provide a senior-engineer-level debugging and demonstration tool for understanding the arrival detection algorithm internals.

**Input:** `route_data.bin` (preprocessor output) + `trace.jsonl` (arrival detector with `--trace` flag)
**Output:** Interactive web visualization (static HTML/JS/CSS)

---

## Architecture

### Technology Stack

| Layer | Technology | Purpose |
|-------|-----------|---------|
| **Framework** | SvelteKit | App framework with static adapter |
| **Maps** | Leaflet | Route visualization (lightweight) |
| **Charts** | Chart.js | Time-series graphs |
| **Styling** | Tailwind CSS | Utility-first CSS |
| **Build** | Vite | Fast dev server & optimizer |
| **Language** | TypeScript | Type safety for data structures |

### Project Structure

```
bus_arrival/
├── visualizer/                    # NEW SvelteKit project
│   ├── src/
│   │   ├── routes/
│   │   │   └── +page.svelte       # Main visualizer page
│   │   ├── lib/
│   │   │   ├── components/
│   │   │   │   ├── MapView.svelte         # Route map with bus position
│   │   │   │   ├── TimelineCharts.svelte  # Time-series graphs
│   │   │   │   ├── FeatureBreakdown.svelte # Bayesian feature display
│   │   │   │   ├── FsmInspector.svelte     # State machine visualization
│   │   │   │   ├── ControlPanel.svelte     # Playback controls
│   │   │   │   └── Header.svelte           # File upload & title
│   │   │   ├── stores/
│   │   │   │   └── data.ts        # Writable stores for shared state
│   │   │   ├── parsers/
│   │   │   │   ├── routeData.ts   # route_data.bin parser
│   │   │   │   └── traceData.ts   # Trace JSONL parser
│   │   │   ├── utils/
│   │   │   │   └── projection.ts  # Grid to Lat/Lon conversion
│   │   │   └── types.ts           # TypeScript interfaces
│   │   ├── app.css
│   │   └── app.d.ts
│   ├── static/
│   │   └── samples/               # Sample data files for demo
│   ├── svelte.config.js           # Static adapter config
│   ├── vite.config.ts
│   ├── package.json
│   ├── tsconfig.json
│   └── tailwind.config.js
└── arrival_detector/              # MODIFIED: Add trace output
    └── src/
        └── trace.rs               # NEW: Trace record emission
```

---

## Data Flow

### Overall Architecture

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  arrival_       │────▶│  trace.jsonl    │────▶│  Visualizer     │
│  detector       │     │  (Rust output)  │     │  (displays)     │
│  (Rust)         │     └─────────────────┘     └─────────────────┘
└─────────────────┘                                       │
         │                                                 ▼
         │                                          ┌─────────────────┐
         └─────────────────────────────────────────▶│  All Views      │
                                                    │  (read-only)    │
                                                    └─────────────────┘
```

### Rust Side: Extended Trace Output

A new `--trace` flag for `arrival_detector` that emits detailed debugging state.

**IMPORTANT:** The Rust trace feature must be implemented BEFORE the visualizer can be built. This is a prerequisite dependency.

#### Implementation: `arrival_detector/src/trace.rs`

```rust
// arrival_detector/src/trace.rs

use serde::Serialize;
use shared::{DistCm, SpeedCms, Prob8, FsmState};
use std::io::{BufWriter, Write};

/// Trace record for debugging visualization
#[derive(Serialize)]
#[serde(tag = "fsm_state")]  // Ensures consistent serialization
pub struct TraceRecord {
    /// Input: GPS timestamp (seconds since epoch)
    pub time: u64,

    /// Input: Route progress (cm)
    pub s_cm: DistCm,

    /// Input: Velocity (cm/s)
    pub v_cms: SpeedCms,

    /// Corridor filter: which stops are active
    pub active_stops: Vec<u8>,

    /// Per-stop detailed state (only for active stops)
    pub stop_states: Vec<StopTraceState>,

    /// GPS jump detected?
    pub gps_jump: bool,

    /// Recovery: new stop index if jumped
    pub recovery_idx: Option<u8>,
}

#[derive(Serialize)]
pub struct StopTraceState {
    pub stop_idx: u8,

    /// Distance to stop (cm)
    pub distance_cm: DistCm,

    /// FSM state serialized as string
    pub fsm_state: &'static str,  // "Approaching" | "Arriving" | "AtStop" | "Departed"

    /// Dwell time (seconds)
    pub dwell_time_s: u16,

    /// Arrival probability (0-255)
    pub probability: Prob8,

    /// Individual feature scores
    pub features: FeatureScores,

    /// Just arrived this frame?
    pub just_arrived: bool,
}

#[derive(Serialize)]
pub struct FeatureScores {
    pub p1: u8,  // Distance likelihood (Gaussian)
    pub p2: u8,  // Speed likelihood (Logistic)
    pub p3: u8,  // Progress likelihood (Gaussian)
    pub p4: u8,  // Dwell time likelihood (Linear)
}

/// Write a trace record to the output file
pub fn write_trace_record<W: Write>(
    output: &mut BufWriter<W>,
    record: &TraceRecord,
) -> std::io::Result<()> {
    let json = serde_json::to_string(record)?;
    writeln!(output, "{}", json)
}

/// Convert FsmState to string for consistent serialization
pub fn fsm_state_as_string(state: FsmState) -> &'static str {
    match state {
        FsmState::Approaching => "Approaching",
        FsmState::Arriving => "Arriving",
        FsmState::AtStop => "AtStop",
        FsmState::Departed => "Departed",
    }
}
```

#### Implementation: Modified `arrival_detector/src/main.rs`

Add trace output support to the main loop:

```rust
use std::env;
use std::fs::File;
use std::io::{BufWriter, Read};
use arrival_detector::trace::{TraceRecord, StopTraceState, FeatureScores, write_trace_record, fsm_state_as_string};

fn main() {
    let args: Vec<String> = env::args().collect();

    // Check for --trace flag
    let mut trace_path: Option<PathBuf> = None;
    let mut input_idx = 1;
    for i in 1..args.len() {
        if args[i] == "--trace" && i + 1 < args.len() {
            trace_path = Some(PathBuf::from(&args[i + 1]));
            input_idx = if i > 1 { 1 } else { 3 };
            break;
        }
    }

    let input_path = PathBuf::from(&args[input_idx]);
    let route_path = PathBuf::from(&args[input_idx + 1]);
    let output_path = PathBuf::from(&args[input_idx + 2]);

    // Open trace file if specified
    let mut trace_writer: Option<BufWriter<File>> = trace_path.map(|p| {
        BufWriter::new(File::create(&p).expect("Failed to create trace file"))
    });

    // ... existing route loading and initialization ...

    for record in input::parse_input(&input_path) {
        if !record.valid {
            continue;
        }

        // ... existing GPS jump and corridor filter logic ...

        // Build trace record if trace output enabled
        let trace_record = if let Some(ref mut tw) = trace_writer {
            let stop_trace_states: Vec<StopTraceState> = active_indices.iter()
                .map(|&idx| {
                    let stop = &stops[idx];
                    let state = &stop_states[idx];

                    // Compute probability (same logic as main loop)
                    let prob = probability::arrival_probability(
                        record.s_cm, record.v_cms, stop,
                        state.dwell_time_s, &gaussian_lut, &logistic_lut,
                    );

                    // Re-compute feature scores for trace
                    let d_cm = (record.s_cm - stop.progress_cm).abs();
                    let idx1 = ((d_cm as i64 * 64) / 2750).min(255) as usize;
                    let p1 = gaussian_lut[idx1];
                    let idx2 = (record.v_cms / 10).max(0).min(127) as usize;
                    let p2 = logistic_lut[idx2];
                    let idx3 = ((d_cm as i64 * 64) / 2000).min(255) as usize;
                    let p3 = gaussian_lut[idx3];
                    let p4 = ((state.dwell_time_s as u32) * 255 / 10).min(255) as u8;

                    StopTraceState {
                        stop_idx: idx as u8,
                        distance_cm: d_cm,
                        fsm_state: fsm_state_as_string(state.fsm_state),
                        dwell_time_s: state.dwell_time_s,
                        probability: prob,
                        features: FeatureScores { p1, p2, p3, p4 },
                        just_arrived: false,  // Will be set below if arrival occurs
                    }
                })
                .collect();

            Some(TraceRecord {
                time: record.time,
                s_cm: record.s_cm,
                v_cms: record.v_cms,
                active_stops: active_indices.iter().map(|&i| i as u8).collect(),
                stop_states: stop_trace_states,
                gps_jump: false,  // Will be set below
                recovery_idx: None,  // Will be set below
            })
        } else {
            None
        };

        // Check for GPS jump - trigger recovery
        let gps_jump_detected = (record.s_cm - last_s_cm).abs() > 20000;
        let recovery_result: Option<u8> = if gps_jump_detected {
            recovery::find_stop_index(record.s_cm, &stops, current_stop_idx)
                .map(|idx| {
                    current_stop_idx = idx as u8;
                    idx as u8
                })
        } else {
            None
        };
        last_s_cm = record.s_cm;

        // Find active stops (corridor filter)
        let active_indices = corridor::find_active_stops(record.s_cm, &stops);

        // Build trace record AFTER state initialization, BEFORE updates
        let mut trace_record = trace_writer.as_ref().map(|_| {
            let stop_trace_states: Vec<StopTraceState> = active_indices.iter()
                .map(|&idx| {
                    let stop = &stops[idx];
                    let state = &stop_states[idx];

                    // Compute probability (same as main loop)
                    let prob = probability::arrival_probability(
                        record.s_cm, record.v_cms, stop,
                        state.dwell_time_s, &gaussian_lut, &logistic_lut,
                    );

                    // Compute feature scores
                    let d_cm = (record.s_cm - stop.progress_cm).abs();
                    let idx1 = ((d_cm as i64 * 64) / 2750).min(255) as usize;
                    let p1 = gaussian_lut[idx1];
                    let idx2 = (record.v_cms / 10).max(0).min(127) as usize;
                    let p2 = logistic_lut[idx2];
                    let idx3 = ((d_cm as i64 * 64) / 2000).min(255) as usize;
                    let p3 = gaussian_lut[idx3];
                    let p4 = ((state.dwell_time_s as u32) * 255 / 10).min(255) as u8;

                    StopTraceState {
                        stop_idx: idx as u8,
                        distance_cm: d_cm,
                        fsm_state: fsm_state_as_string(state.fsm_state),
                        dwell_time_s: state.dwell_time_s,
                        probability: prob,
                        features: FeatureScores { p1, p2, p3, p4 },
                        just_arrived: false,  // Will update below if arrival occurs
                    }
                })
                .collect();

            TraceRecord {
                time: record.time,
                s_cm: record.s_cm,
                v_cms: record.v_cms,
                active_stops: active_indices.iter().map(|&i| i as u8).collect(),
                stop_states: stop_trace_states,
                gps_jump: gps_jump_detected,
                recovery_idx: recovery_result,
            }
        });

        // Now process each active stop and update state machines
        let mut arrivals_this_frame: Vec<u8> = Vec::new();

        for &stop_idx in &active_indices {
            let stop = &stops[stop_idx];
            let state = &mut stop_states[stop_idx];

            // Handle re-entry after departure
            if state.fsm_state == FsmState::Departed {
                if state.can_reactivate(record.s_cm, stop.progress_cm) {
                    state.reset();
                }
            }

            // Compute probability
            let prob = probability::arrival_probability(
                record.s_cm, record.v_cms, stop,
                state.dwell_time_s, &gaussian_lut, &logistic_lut,
            );

            // Update state machine and detect arrival
            if state.update(record.s_cm, record.v_cms, stop.progress_cm, prob) {
                // Just arrived!
                arrivals_this_frame.push(state.index);

                let event = ArrivalEvent {
                    time: record.time,
                    stop_idx: state.index,
                    s_cm: record.s_cm,
                    v_cms: record.v_cms,
                    probability: prob,
                };
                output::write_event(&mut output_writer, &event).expect("Failed to write arrival event");
                arrivals += 1;
                current_stop_idx = state.index;
            }
        }

        // Write trace record after state updates (now we know which stops arrived)
        if let (Some(mut tw), Some(mut tr)) = (trace_writer.as_mut(), trace_record) {
            // Update just_arrived flags for stops that arrived this frame
            for arrived_idx in &arrivals_this_frame {
                if let Some(trace_state) = tr.stop_states.iter_mut().find(|s| s.stop_idx == *arrived_idx) {
                    trace_state.just_arrived = true;
                }
            }

            write_trace_record(&mut tw, &tr).expect("Failed to write trace");
        }

        processed += 1;
    }

    // Flush trace writer
    if let Some(mut tw) = trace_writer {
        tw.flush().expect("Failed to flush trace file");
    }
}
```

#### CLI Interface

```bash
# Normal mode: just arrivals
cargo run --bin arrival_detector -- input.jsonl route_data.bin output.jsonl

# Trace mode: full debugging info
cargo run --bin arrival_detector -- input.jsonl route_data.bin output.jsonl --trace trace.jsonl
```

#### Output Format

Each line in `trace.jsonl` is a JSON-encoded `TraceRecord` with fsm_state as string:

```json
{"time":1768214400,"s_cm":12345,"v_cms":150,"active_stops":[0,1],"stop_states":[{"stop_idx":0,"distance_cm":500,"fsm_state":"Arriving","dwell_time_s":5,"probability":200,"features":{"p1":180,"p2":220,"p3":190,"p4":120},"just_arrived":false}],"gps_jump":false,"recovery_idx":null}
```

### Visualizer Side: Data Parsing

#### TypeScript Interfaces

```typescript
// visualizer/src/lib/types.ts

// Matches RouteNode from shared/src/lib.rs (52 bytes, #[repr(C, packed)])
export interface RouteNode {
    // i64 fields (8 bytes each, offset 0-16)
    len2_cm2: bigint;          // Squared segment length (cm²)
    line_c: bigint;            // Line constant: -(line_a × x₀ + line_b × y₀)

    // i16 fields (2 bytes each, offset 16-20)
    heading_cdeg: number;      // Segment heading in 0.01°
    _pad: number;              // Padding for alignment

    // i32 fields (4 bytes each, offset 20-52)
    x_cm: number;              // X coordinate (relative to grid origin) in cm
    y_cm: number;              // Y coordinate (relative to grid origin) in cm
    cum_dist_cm: number;       // Cumulative distance from route start in cm
    dx_cm: number;             // Segment vector X: x[i+1] - x[i] in cm
    dy_cm: number;             // Segment vector Y: y[i+1] - y[i] in cm
    seg_len_cm: number;        // Segment length in cm (offline sqrt)
    line_a: number;            // Line coefficient A: = -dy
    line_b: number;            // Line coefficient B: = dx
}

export interface RouteData {
    version: number;
    grid_origin: { x0_cm: number; y0_cm: number };
    nodes: RouteNode[];
    stops: Stop[];
}

// Matches Stop from shared/src/lib.rs (12 bytes, #[repr(C)])
export interface Stop {
    progress_cm: number;       // Position along route in cm
    corridor_start_cm: number; // progress_cm - 8000 cm (80m before stop)
    corridor_end_cm: number;   // progress_cm + 4000 cm (40m after stop)
}

export interface TraceRecord {
    time: number;
    s_cm: number;
    v_cms: number;
    active_stops: number[];
    stop_states: StopTraceState[];
    gps_jump: boolean;
    recovery_idx: number | null;
}

export interface StopTraceState {
    stop_idx: number;
    distance_cm: number;
    fsm_state: 'Approaching' | 'Arriving' | 'AtStop' | 'Departed';
    dwell_time_s: number;
    probability: number;
    features: {
        p1: number;
        p2: number;
        p3: number;
        p4: number;
    };
    just_arrived: boolean;
}

// Parse errors
export class RouteDataError extends Error {
    constructor(message: string, public offset?: number) {
        super(message);
        this.name = 'RouteDataError';
    }
}

export class TraceDataError extends Error {
    constructor(message: string, public line?: number) {
        super(message);
        this.name = 'TraceDataError';
    }
}
```

---

## UI Layout

### Overall Layout

```
┌─────────────────────────────────────────────────────────────────┐
│  Header: Bus Arrival Visualizer                    [Load Files] │
├─────────────────────────────────────────────────────────────────┤
│  ┌───────────────────────────────────────────────────────────┐  │
│  │  Route Map View                                    [A] │  │
│  │  Leaflet map with route, stops, corridors, bus position │  │
│  └───────────────────────────────────────────────────────────┘  │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │  Timeline Charts                                 [B] │  │
│  │  Multi-axis chart with progress, velocity, probability   │  │
│  └───────────────────────────────────────────────────────────┘  │
│  ┌───────────────────────┬─────────────────────────────────┐  │
│  │  Feature Breakdown  [C] │  FSM Inspector                  [D]│  │
│  │  Individual features    │  Per-stop FSM states             │  │
│  │  and combined prob       │  with state history              │  │
│  └───────────────────────┴─────────────────────────────────┘  │
├─────────────────────────────────────────────────────────────────┤
│  Control Panel: ◀◀ ▶ ▶▶ ● Speed Stop 0/15,234 ━━━━━━━━━━━━━━━━━━│
└─────────────────────────────────────────────────────────────────┘
```

### View Specifications

#### [A] Route Map View

**Purpose:** Show spatial relationship between route, stops, and bus position.

**Elements:**
- **Base map:** Leaflet with OpenStreetMap tiles
- **Route:** Gray polyline from `route_data.bin` nodes
- **Stop markers:** Circles color-coded by FSM state:
  - Gray (`#9CA3AF`): Approaching
  - Yellow (`#FBBF24`): Arriving
  - Green (`#10B981`): AtStop
  - Red (`#EF4444`): Departed
- **Corridors:** Semi-transparent blue rectangles (±80m before, ±40m after stop)
- **Bus position:** Animated marker on route based on `s_cm`

#### [B] Timeline Charts

**Purpose:** Show time-series evolution of key variables.

**Elements:**
- **X-axis:** GPS timestamp (scrollable, zoomable)
- **Left Y-axis:** Distance (cm) and Speed (cm/s)
- **Right Y-axis:** Probability (0-255)
- **Series:**
  - Progress (`s_cm`): Blue line
  - Velocity (`v_cms`): Green line
  - Probability: Orange area fill
  - FSM State: Background colored bands
- **Playhead:** Vertical red draggable line

#### [C] Feature Breakdown

**Purpose:** Show how Bayesian features combine into arrival probability.

**Elements:**
- Horizontal bar chart for selected stop at current timestamp
- 4 feature bars (p1-p4) normalized 0-255
- Combined probability bar
- Threshold line at 191 (75%)

#### [D] FSM Inspector

**Purpose:** Show state machine state for all stops.

**Elements:**
- List of all stops with current FSM state icon
- Click to expand:
  - State transition timeline
  - Probability over time mini-chart
  - Distance to stop over time mini-chart
  - Dwell time counter

### Control Panel

**Elements:**
- **Playback:** `◀◀` (start), `▶` (step), `▶▶` (play/pause)
- **Speed:** 0.5x, 1x, 2x, 5x dropdown
- **Position:** Draggable slider, frame counter, timestamp
- **Stop selector:** Dropdown to jump to specific stop

---

## State Management

### Svelte Stores

```typescript
// visualizer/src/lib/stores/data.ts

import { writable, derived } from 'svelte/store';

// ===== Raw Data (loaded from files) =====
export const routeData = writable<RouteData | null>(null);
export const traceData = writable<TraceRecord[] | null>(null);

// ===== Playback State =====
export const currentFrame = writable<number>(0);
export const isPlaying = writable<boolean>(false);
export const playbackSpeed = writable<number>(1.0);

// ===== Selection State =====
export const selectedStopIdx = writable<number | null>(null);

// ===== Derived Values =====

export const currentRecord = derived(
    [traceData, currentFrame],
    ([$traceData, $currentFrame]) => {
        if (!$traceData || $currentFrame >= $traceData.length) return null;
        return $traceData[$currentFrame];
    }
);

export const busPosition = derived(
    currentRecord,
    ($currentRecord) => {
        if (!$currentRecord) return null;
        return { s_cm: $currentRecord.s_cm, v_cms: $currentRecord.v_cms };
    }
);

export const stopStatesMap = derived(
    currentRecord,
    ($currentRecord) => {
        if (!$currentRecord) return new Map();
        return new Map(
            $currentRecord.stop_states.map(s => [s.stop_idx, s])
        );
    }
);

export const totalFrames = derived(
    traceData,
    ($traceData) => $traceData?.length || 0
);
```

---

## Component Architecture

```
App (+page.svelte)
├── Header
│   ├── Title
│   └── LoadButton
├── MainContent
│   ├── MapView.svelte
│   │   ├── LeafletMap (wrapper)
│   │   ├── RouteLayer
│   │   ├── StopMarkers
│   │   ├── CorridorLayer
│   │   └── BusMarker
│   ├── TimelineCharts.svelte
│   │   └── Chart.js canvas
│   ├── SplitView
│   │   ├── FeatureBreakdown.svelte
│   │   │   └── FeatureBar × 4
│   │   └── FsmInspector.svelte
│   │       ├── StopList
│   │       └── StopDetail
│   └── ControlPanel.svelte
│       ├── PlaybackButtons
│       ├── SpeedSelector
│       └── PositionSlider
└── Footer
```

---

## Implementation Details

### Binary File Parsing: `route_data.bin`

The binary format is defined in `shared/src/binfile.rs` and `shared/src/lib.rs`. Parsing requires careful handling of packed structs and endianness.

#### Binary Format Structure

```
Offset  Size    Field
------  ----    -----
0       4       Magic bytes (0x0210) + version (1)
4       4       CRC32 checksum (of rest of file)
8       4       Grid origin x0_cm (i32, little-endian)
12      4       Grid origin y0_cm (i32, little-endian)
16      4       Node count (u32, little-endian)
20      4       Stop count (u32, little-endian)
24      -       Nodes array (52 bytes each, packed)
-       -       Stops array (12 bytes each, packed)
-       -       Spatial grid data (variable)
```

#### Parser Implementation

```typescript
// visualizer/src/lib/parsers/routeData.ts

import { RouteData, RouteNode, Stop, RouteDataError } from '$lib/types';

const MAGIC: number = 0x0210;
const NODE_SIZE: number = 52;  // Verified at compile time in Rust
const STOP_SIZE: number = 12;

// CRC32 table for validation (standard polynomial 0xEDB88320)
const CRC_TABLE: number[] = new Array(256);
for (let i = 0; i < 256; i++) {
    let crc = i;
    for (let j = 0; j < 8; j++) {
        crc = (crc >>> 1) ^ ((crc & 1) ? 0xEDB88320 : 0);
    }
    CRC_TABLE[i] = crc;
}

function crc32(data: Uint8Array): number {
    let crc = 0xFFFFFFFF;
    for (let i = 0; i < data.length; i++) {
        crc = (crc >>> 8) ^ CRC_TABLE[(crc ^ data[i]) & 0xFF];
    }
    return (crc ^ 0xFFFFFFFF) >>> 0;
}

export async function parseRouteData(file: File): Promise<RouteData> {
    const buffer = await file.arrayBuffer();
    const view = new DataView(buffer);
    const bytes = new Uint8Array(buffer);

    // Verify magic and version
    const magicVersion = view.getUint16(0, true);  // little-endian
    if (magicVersion !== MAGIC) {
        throw new RouteDataError(`Invalid magic: expected 0x${MAGIC.toString(16)}, got 0x${magicVersion.toString(16)}`);
    }

    const version = view.getUint16(2, true);
    if (version !== 1) {
        throw new RouteDataError(`Unsupported version: ${version}`);
    }

    // Verify CRC32
    const storedCrc = view.getUint32(4, true);
    const dataCrc = crc32(bytes.subarray(8));  // CRC covers everything after offset 8
    if (storedCrc !== dataCrc) {
        throw new RouteDataError(`CRC mismatch: expected 0x${storedCrc.toString(16)}, got 0x${dataCrc.toString(16)}`);
    }

    // Read header
    let offset = 8;
    const x0_cm = view.getInt32(offset, true);
    const y0_cm = view.getInt32(offset + 4, true);
    offset += 8;

    const nodeCount = view.getUint32(offset, true);
    const stopCount = view.getUint32(offset + 4, true);
    offset += 8;

    // Parse nodes (52 bytes each, packed)
    const nodes: RouteNode[] = [];
    for (let i = 0; i < nodeCount; i++) {
        // RouteNode has i64 fields which need special handling in JS
        // We read them as two i32 values and combine
        const len2_low = view.getUint32(offset, true);
        const len2_high = view.getInt32(offset + 4, true);
        const len2_cm2 = (BigInt(len2_high) << 32n) | BigInt(len2_low);

        const line_c_low = view.getUint32(offset + 8, true);
        const line_c_high = view.getInt32(offset + 12, true);
        const line_c = (BigInt(line_c_high) << 32n) | BigInt(line_c_low);

        const heading_cdeg = view.getInt16(offset + 16, true);
        const _pad = view.getInt16(offset + 18, true);

        const x_cm = view.getInt32(offset + 20, true);
        const y_cm = view.getInt32(offset + 24, true);
        const cum_dist_cm = view.getInt32(offset + 28, true);
        const dx_cm = view.getInt32(offset + 32, true);
        const dy_cm = view.getInt32(offset + 36, true);
        const seg_len_cm = view.getInt32(offset + 40, true);
        const line_a = view.getInt32(offset + 44, true);
        const line_b = view.getInt32(offset + 48, true);

        nodes.push({
            len2_cm2, line_c,
            heading_cdeg, _pad,
            x_cm, y_cm, cum_dist_cm,
            dx_cm, dy_cm, seg_len_cm,
            line_a, line_b
        });

        offset += NODE_SIZE;
    }

    // Parse stops (12 bytes each)
    const stops: Stop[] = [];
    for (let i = 0; i < stopCount; i++) {
        const progress_cm = view.getInt32(offset, true);
        const corridor_start_cm = view.getInt32(offset + 4, true);
        const corridor_end_cm = view.getInt32(offset + 8, true);

        stops.push({ progress_cm, corridor_start_cm, corridor_end_cm });
        offset += STOP_SIZE;
    }

    // Spatial grid parsing (optional - skip if not needed for visualization)
    // The grid data is after the stops array. We calculate its offset by reading
    // the grid metadata that follows the stops.
    //
    // Grid format (from shared/src/binfile.rs):
    //   - grid_size_cm: u32
    //   - cols: u32
    //   - rows: u32
    //   - x0_cm: i32
    //   - y0_cm: i32
    //   - cells: variable (list of lists, flattened)
    //
    // For the visualizer, we typically don't need the spatial grid since
    // we're not doing map matching - just displaying pre-computed results.
    // We skip parsing it here to keep the parser simple.
    //
    // If you need the grid data for advanced visualizations:
    // const grid_size_cm = view.getUint32(offset, true);
    // const cols = view.getUint32(offset + 4, true);
    // const rows = view.getUint32(offset + 8, true);
    // offset += 12;
    // ... parse cells ...

    return {
        version,
        grid_origin: { x0_cm, y0_cm },
        nodes,
        stops
    };
}
```

### Trace File Parsing: `trace.jsonl`

```typescript
// visualizer/src/lib/parsers/traceData.ts

import { TraceRecord, StopTraceState, TraceDataError } from '$lib/types';

export async function parseTraceFile(file: File): Promise<TraceRecord[]> {
    const text = await file.text();
    const lines = text.split('\n').filter(line => line.trim());
    const records: TraceRecord[] = [];

    for (let i = 0; i < lines.length; i++) {
        try {
            const obj = JSON.parse(lines[i]);
            records.push({
                time: obj.time,
                s_cm: obj.s_cm,
                v_cms: obj.v_cms,
                active_stops: obj.active_stops,
                stop_states: obj.stop_states.map((s: any) => ({
                    stop_idx: s.stop_idx,
                    distance_cm: s.distance_cm,
                    fsm_state: validateFsmState(s.fsm_state),
                    dwell_time_s: s.dwell_time_s,
                    probability: s.probability,
                    features: s.features,
                    just_arrived: s.just_arrived
                })),
                gps_jump: obj.gps_jump,
                recovery_idx: obj.recovery_idx
            });
        } catch (e) {
            throw new TraceDataError(`Failed to parse line ${i + 1}: ${e}`, i + 1);
        }
    }

    return records;
}

function validateFsmState(value: string): 'Approaching' | 'Arriving' | 'AtStop' | 'Departed' {
    const valid = ['Approaching', 'Arriving', 'AtStop', 'Departed'];
    if (!valid.includes(value)) {
        throw new TraceDataError(`Invalid fsm_state: ${value}`);
    }
    return value as any;
}
```

### Coordinate Projection

**IMPORTANT:** Use the grid origin from the binary file, not hardcoded constants. The projection MUST match the Rust code exactly.

**IMPORTANT:** Use the grid origin from the binary file, not hardcoded constants. The projection MUST match the Rust code exactly.

```typescript
// visualizer/src/lib/utils/projection.ts

// Constants MUST match shared/src/lib.rs exactly
const EARTH_R_CM: number = 637_100_000.0;
const PROJECTION_LAT_AVG: number = 25.0;

// Default origin from Rust (used for validation)
const DEFAULT_ORIGIN_LAT_DEG: number = 20.0;
const DEFAULT_ORIGIN_LON_DEG: number = 120.0;

/**
 * Convert grid coordinates (cm) to lat/lon for Leaflet.
 *
 * Uses the inverse of the projection in shared/src/lib.rs:
 *   x_cm = R * cos(lat_avg) * (lon - lon_0) * pi/180
 *   y_cm = R * (lat - lat_0) * pi/180
 *
 * @param x_cm - Grid X coordinate in cm (relative to grid origin)
 * @param y_cm - Grid Y coordinate in cm (relative to grid origin)
 * @param origin - Grid origin from route_data.bin {x0_cm, y0_cm}
 * @returns [lat, lon] in degrees
 */
export function gridToLatLng(
    x_cm: number,
    y_cm: number,
    origin: { x0_cm: number; y0_cm: number }
): [number, number] {
    // The grid origin in the binary file is in cm, relative to (lat_0, lon_0)
    // We need to convert it to lat/lon first, then apply the grid offset

    // Convert grid origin to lat/lon (inverse of forward projection)
    const origin_lat = (origin.y0_cm / EARTH_R_CM) * (180 / Math.PI) + DEFAULT_ORIGIN_LAT_DEG;
    const origin_lon = (origin.x0_cm / (EARTH_R_CM * Math.cos(PROJECTION_LAT_AVG * Math.PI / 180)))
                       * (180 / Math.PI) + DEFAULT_ORIGIN_LON_DEG;

    // Now convert the point (x_cm, y_cm) to lat/lon relative to origin
    const lat = (y_cm / EARTH_R_CM) * (180 / Math.PI) + origin_lat;
    const lon = (x_cm / (EARTH_R_CM * Math.cos(PROJECTION_LAT_AVG * Math.PI / 180)))
                * (180 / Math.PI) + origin_lon;

    return [lat, lon];
}

/**
 * Project entire route nodes to lat/lng for Leaflet polyline.
 */
export function projectRoute(
    nodes: RouteNode[],
    origin: { x0_cm: number; y0_cm: number }
): Array<[number, number]> {
    return nodes.map(node => gridToLatLng(node.x_cm, node.y_cm, origin));
}

/**
 * Project stop position to lat/lng for Leaflet marker.
 *
 * Uses linear interpolation between nodes for accuracy.
 * Stops are typically between route nodes, so we find the
 * segment containing the stop and interpolate.
 *
 * @param stop - Stop with progress_cm along route
 * @param nodes - Route nodes from binary file
 * @param origin - Grid origin from route_data.bin
 * @returns [lat, lon] in degrees
 */
export function projectStop(
    stop: Stop,
    nodes: RouteNode[],
    origin: { x0_cm: number; y0_cm: number }
): [number, number] {
    // Find the segment containing this stop's progress
    const segment = findSegmentForProgress(stop.progress_cm, nodes);
    if (!segment) {
        // Fallback: use nearest node
        const node = findNearestNode(stop.progress_cm, nodes);
        return gridToLatLng(node.x_cm, node.y_cm, origin);
    }

    // Interpolate position along the segment
    const { startNode, endNode, offsetAlongSegment } = segment;

    // Linear interpolation in grid coordinates
    const t = offsetAlongSegment / (endNode.cum_dist_cm - startNode.cum_dist_cm);
    const x_cm = startNode.x_cm + t * (endNode.x_cm - startNode.x_cm);
    const y_cm = startNode.y_cm + t * (endNode.y_cm - startNode.y_cm);

    return gridToLatLng(x_cm, y_cm, origin);
}

/**
 * Find the route segment containing a given progress distance.
 *
 * Returns the start/end nodes and offset from the start node.
 */
function findSegmentForProgress(
    progress_cm: number,
    nodes: RouteNode[]
): { startNode: RouteNode; endNode: RouteNode; offsetAlongSegment: number } | null {
    // Binary search for the segment
    let left = 0, right = nodes.length - 1;

    while (left < right) {
        const mid = Math.floor((left + right + 1) / 2);
        if (nodes[mid].cum_dist_cm <= progress_cm) {
            left = mid;
        } else {
            right = mid - 1;
        }
    }

    // left is now the index of the node at or before the stop
    if (left >= nodes.length - 1) {
        // Stop is at or past the last node
        return null;
    }

    const startNode = nodes[left];
    const endNode = nodes[left + 1];
    const offsetAlongSegment = progress_cm - startNode.cum_dist_cm;

    return { startNode, endNode, offsetAlongSegment };
}

/**
 * Find the nearest node to a progress distance.
 * Used as fallback when segment finding fails.
 */
function findNearestNode(progress_cm: number, nodes: RouteNode[]): RouteNode {
    let left = 0, right = nodes.length - 1;
    while (left < right) {
        const mid = Math.floor((left + right + 1) / 2);
        if (nodes[mid].cum_dist_cm <= progress_cm) {
            left = mid;
        } else {
            right = mid - 1;
        }
    }

    // Check which of left or left+1 is closer
    if (left >= nodes.length - 1) return nodes[left];
    if (left === 0) return nodes[0];

    const distLeft = Math.abs(progress_cm - nodes[left].cum_dist_cm);
    const distNext = Math.abs(progress_cm - nodes[left + 1].cum_dist_cm);

    return distNext < distLeft ? nodes[left + 1] : nodes[left];
}
```

### Playback Loop

```typescript
let playbackInterval: number;

isPlaying.subscribe(($isPlaying) => {
    clearInterval(playbackInterval);

    if ($isPlaying) {
        playbackInterval = setInterval(() => {
            const frame = get(currentFrame);
            const total = get(totalFrames);
            const speed = get(playbackSpeed);

            if (frame >= total - 1) {
                isPlaying.set(false);
                return;
            }

            currentFrame.update(n => n + 1);
        }, 1000 / (speed * 10)); // Base: 10 fps
    }
});
```

### Performance Optimizations

1. **Lazy rendering:** Only render visible map markers within bounds
2. **Chart downsampling:** Decimate to max 1000 points for performance
3. **Virtual scrolling:** For stop list if 20+ stops

---

### Error Handling Strategy

The visualizer must handle errors gracefully and provide clear feedback to users.

#### File Validation

```typescript
// visualizer/src/lib/parsers/validation.ts

export async function validateFiles(
    routeFile: File,
    traceFile: File
): Promise<{ valid: boolean; errors: string[] }> {
    const errors: string[] = [];

    // Check file sizes (reject unreasonably large files)
    const MAX_ROUTE_SIZE = 10 * 1024 * 1024;  // 10 MB
    const MAX_TRACE_SIZE = 500 * 1024 * 1024; // 500 MB

    if (routeFile.size > MAX_ROUTE_SIZE) {
        errors.push(`Route file too large: ${(routeFile.size / 1024 / 1024).toFixed(1)} MB`);
    }
    if (traceFile.size > MAX_TRACE_SIZE) {
        errors.push(`Trace file too large: ${(traceFile.size / 1024 / 1024).toFixed(1)} MB`);
    }

    // Check file extensions
    if (!routeFile.name.endsWith('.bin')) {
        errors.push('Route file must be .bin');
    }
    if (!traceFile.name.endsWith('.jsonl')) {
        errors.push('Trace file must be .jsonl');
    }

    return { valid: errors.length === 0, errors };
}
```

#### Parsing Error Recovery

```typescript
// In component with error handling

let loadError: string | null = null;
let loadProgress = 0;

async function loadFiles() {
    loadError = null;
    loadProgress = 0;

    try {
        // Validate first
        const validation = await validateFiles(routeFile!, traceFile!);
        if (!validation.valid) {
            loadError = validation.errors.join('; ');
            return;
        }

        // Parse with progress indication
        const [route, trace] = await Promise.all([
            parseRouteDataWithProgress(routeFile!, p => loadProgress = p * 0.3),
            parseTraceFileWithProgress(traceFile!, p => loadProgress = 30 + p * 0.7)
        ]);

        routeData.set(route);
        traceData.set(trace);
        currentFrame.set(0);
    } catch (e) {
        loadError = e instanceof RouteDataError || e instanceof TraceDataError
            ? e.message
            : 'Failed to load files';
    }
}
```

#### User Feedback

```svelte
<!-- Header.svelte with error display -->

{#if $loadError}
    <div class="bg-red-100 border border-red-400 text-red-700 px-4 py-3 rounded">
        <strong>Error:</strong> {$loadError}
    </div>
{/if}

{#if loadProgress > 0 && loadProgress < 100}
    <div class="w-full bg-gray-200 rounded-full h-2.5">
        <div class="bg-blue-600 h-2.5 rounded-full" style="width: {loadProgress}%"></div>
    </div>
{/if}
```

---

### Performance & Data Strategy

#### Data Volume Considerations

Real-world GPS data volumes:
- 1 update/second = 86,400 records/day
- Typical 8-hour bus run = 28,800 records
- Trace.jsonl size ≈ 100 bytes/record = 2.9 MB for 8 hours

#### Optimizations for Large Datasets

**1. Chunked Loading**
```typescript
// Load trace data in chunks for progressive rendering
export async function parseTraceInChunks(
    file: File,
    chunkSize: number = 5000,
    onChunk: (records: TraceRecord[]) => void
): Promise<TraceRecord[]> {
    const text = await file.text();
    const lines = text.split('\n');
    const allRecords: TraceRecord[] = [];

    for (let i = 0; i < lines.length; i += chunkSize) {
        const chunk = lines.slice(i, i + chunkSize);
        const records = chunk
            .filter(line => line.trim())
            .map(line => JSON.parse(line));
        allRecords.push(...records);
        onChunk(records);
    }

    return allRecords;
}
```

**2. Timeline Windowing**
```typescript
// Only render chart data for visible time window
export const visibleTraceData = derived(
    [traceData, timelineWindow],
    ([$traceData, $window]) => {
        if (!$traceData) return [];
        return $traceData.slice($window.start, $window.end);
    }
);
```

**3. Web Worker for Parsing**
```typescript
// visualizer/src/lib/workers/parser.worker.ts

self.onmessage = (e) => {
    const { file } = e.data;
    // Parse in worker thread to avoid blocking UI
    const records = parseTraceFileSync(file);
    self.postMessage({ records });
};
```

**4. Memory Limits**
- Warn user if file > 100 MB
- Offer to load only first N records for preview
- Implement memory monitoring and cleanup

---

### Testing Strategy

#### Unit Tests
- Binary parser with valid/invalid files
- Projection accuracy against Rust output
- Trace parser with malformed JSON

#### Integration Tests
- Load sample data and verify all views render
- Playback controls function correctly
- Synchronized state updates

#### Performance Tests
- Load time for 10k, 50k, 100k records
- Playback frame rate at 1x, 2x, 5x speed
- Memory usage over time

---

## Validation Criteria

### Prerequisites (Rust Side)
- [ ] `arrival_detector` emits `trace.jsonl` with `--trace` flag
- [ ] Trace records include all required fields
- [ ] FSM state serializes as string ("Approaching", "Arriving", "AtStop", "Departed")
- [ ] Feature scores (p1-p4) computed and included in trace

### Visualizer (Frontend)
- [ ] Loads and parses `route_data.bin` correctly with CRC validation
- [ ] Loads and parses `trace.jsonl` correctly with error recovery
- [ ] Map displays route polyline from binary nodes
- [ ] Stop markers display and change color based on FSM state
- [ ] Timeline charts show progress, velocity, probability over time
- [ ] Playhead is draggable and syncs all views (map, charts, breakdown, FSM)
- [ ] Playback controls work (play, pause, step, speed)
- [ ] Feature breakdown shows individual scores for selected stop
- [ ] FSM inspector shows all stops with state history
- [ ] Performance acceptable for 50k+ trace records (typical 8-hour run)
- [ ] File upload shows progress and error messages
- [ ] Coordinate projection matches Rust code exactly

### Testing
- [ ] Unit tests for binary parser
- [ ] Unit tests for trace parser
- [ ] Integration test with sample data
- [ ] Performance benchmark with 50k records

---

## Prerequisites & Dependencies

### Rust Dependencies (to add)

```toml
# arrival_detector/Cargo.toml

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
```

### Node.js Dependencies

```json
{
  "name": "bus-arrival-visualizer",
  "version": "0.1.0",
  "type": "module",
  "devDependencies": {
    "@sveltejs/adapter-static": "^3.0.0",
    "@sveltejs/kit": "^2.0.0",
    "@sveltejs/vite-plugin-svelte": "^3.0.0",
    "svelte": "^4.2.0",
    "svelte-check": "^3.6.0",
    "typescript": "^5.3.0",
    "vite": "^5.0.0",
    "tailwindcss": "^3.4.0"
  },
  "dependencies": {
    "chart.js": "^4.4.0",
    "leaflet": "^1.9.0"
  }
}
```

### Build Tools
- Node.js 18+
- Rust 1.70+ (for trace generation)
- Modern web browser (Chrome, Firefox, Safari, Edge)

---

## Deployment

### Build Instructions

```bash
cd visualizer
npm install
npm run build
```

The static adapter will produce optimized files in `visualizer/build/`.

### Hosting Requirements

**Fully static** - Can be hosted anywhere:
- GitHub Pages
- Netlify
- Vercel
- Local file system (`file://`)
- Any static web server

No server-side processing required.

### Build Output Structure

```
visualizer/build/
├── _app/
│   ├── immutable/
│   │   ├── assets/     # JS, CSS chunks
│   │   └── nodes/      # Component code
│   └── version.json    # Cache busting
└── index.html          # Entry point
```

### Sample Data Generation

```bash
# Generate trace file from NMEA input
cd /Users/herry/project/pico2w/bus_arrival

# 1. Run preprocessor to create route_data.bin
cargo run --bin preprocessor -- tools/data/TY_225_stops.csv test.nmea route_data.bin

# 2. Run simulator to create Phase 2 output
cargo run --bin simulator -- test.nmea route_data.bin phase2.jsonl

# 3. Run arrival detector with trace
cargo run --bin arrival_detector -- phase2.jsonl route_data.bin arrivals.jsonl --trace trace.jsonl

# 4. Copy files to visualizer static folder
cp route_data.bin visualizer/static/samples/
cp trace.jsonl visualizer/static/samples/
```

---

- Export current view as image
- Share playback state via URL (deep linking)
- Compare multiple runs side-by-side
- Add/export annotations
- Real-time mode with WebSocket server

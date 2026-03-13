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

#### CLI Interface

```bash
# Normal mode: just arrivals
cargo run --bin arrival_detector -- input.jsonl route_data.bin output.jsonl

# Trace mode: full debugging info
cargo run --bin arrival_detector -- input.jsonl route_data.bin output.jsonl --trace trace.jsonl
```

#### Trace Record Format

```rust
// arrival_detector/src/trace.rs

use serde::Serialize;
use shared::{DistCm, SpeedCms, Prob8, FsmState};

#[derive(Serialize)]
pub struct TraceRecord {
    /// Input: GPS timestamp
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

    /// Current FSM state
    pub fsm_state: FsmState,

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
```

#### Output Format

Each line in `trace.jsonl` is a JSON-encoded `TraceRecord`:

```json
{"time": 1768214400, "s_cm": 12345, "v_cms": 150, "active_stops": [0, 1], "stop_states": [...], "gps_jump": false, "recovery_idx": null}
```

### Visualizer Side: Data Parsing

#### TypeScript Interfaces

```typescript
// visualizer/src/lib/types.ts

export interface RouteData {
    version: number;
    grid_origin: { x0_cm: number; y0_cm: number };
    nodes: RouteNode[];
    stops: Stop[];
}

export interface RouteNode {
    x_cm: number;
    y_cm: number;
    cum_dist_cm: number;
    heading_cdeg: number;
    dx_cm: number;
    dy_cm: number;
}

export interface Stop {
    progress_cm: number;
    corridor_start_cm: number;
    corridor_end_cm: number;
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

### Coordinate Projection

Route data is stored in grid coordinates (cm). Must convert to lat/lon for Leaflet.

```typescript
// visualizer/src/lib/utils/projection.ts

const ORIGIN_LAT_DEG = 20.0;
const ORIGIN_LON_DEG = 120.0;
const EARTH_R_CM = 637_100_000.0;
const PROJECTION_LAT_AVG = 25.0;

export function gridToLatLng(x_cm: number, y_cm: number): [number, number] {
    const lat = (y_cm / EARTH_R_CM) * (180 / Math.PI) + ORIGIN_LAT_DEG;
    const lon = (x_cm / (EARTH_R_CM * Math.cos(PROJECTION_LAT_AVG * Math.PI / 180)))
                * (180 / Math.PI) + ORIGIN_LON_DEG;
    return [lat, lon];
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

## Validation Criteria

- [ ] Loads and parses `route_data.bin` correctly
- [ ] Loads and parses `trace.jsonl` correctly
- [ ] Map displays route polyline and stop markers
- [ ] Stop markers change color based on FSM state
- [ ] Timeline charts show progress, velocity, probability
- [ ] Playhead is draggable and syncs all views
- [ ] Playback controls work (play, pause, speed)
- [ ] Feature breakdown shows individual scores
- [ ] FSM inspector shows all stops with state history
- [ ] Performance acceptable for 10k+ trace records

---

## Future Enhancements

- Export current view as image
- Share playback state via URL (deep linking)
- Compare multiple runs side-by-side
- Add/export annotations
- Real-time mode with WebSocket server

# Bus Arrival Detection Visualizer — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a web-based visual tool that displays the internal state of the RP2350 bus arrival detection algorithm as it processes GPS data.

**Architecture:**
- Rust side: Add `--trace` flag to `arrival_detector` to emit detailed JSONL trace files
- Frontend: SvelteKit app that parses binary route data and trace files, displays interactive visualizations

**Tech Stack:**
- Rust: serde, serde_json (for trace serialization)
- SvelteKit with static adapter
- TypeScript, MapLibre GL JS (maps), Chart.js (charts), Tailwind CSS

---

## Scope

This plan has two independent phases:

1. **Phase 1 (Rust):** Add trace output to `arrival_detector` (existing bugs already fixed)
2. **Phase 2 (Frontend):** Build SvelteKit visualizer

Phase 2 can proceed once Phase 1 is complete, as it depends on the trace format.

---

## File Structure

### New Files (Rust)
```
arrival_detector/src/
└── trace.rs              # Trace record structures and emission
```

### Modified Files (Rust)
```
arrival_detector/src/
├── main.rs               # Add --trace flag and trace writing
├── lib.rs                # Export trace module
└── Cargo.toml            # Add serde dependency (if not already)

shared/src/
└── lib.rs                # Verify FsmState serialization format
```

### New Files (Frontend)
```
visualizer/
├── src/
│   ├── routes/
│   │   └── +page.svelte
│   ├── lib/
│   │   ├── components/
│   │   │   ├── MapView.svelte
│   │   │   ├── TimelineCharts.svelte
│   │   │   ├── FeatureBreakdown.svelte
│   │   │   ├── FsmInspector.svelte
│   │   │   ├── ControlPanel.svelte
│   │   │   └── Header.svelte
│   │   ├── stores/
│   │   │   └── data.ts
│   │   ├── parsers/
│   │   │   ├── routeData.ts
│   │   │   └── traceData.ts
│   │   ├── utils/
│   │   │   └── projection.ts
│   │   └── types.ts
│   ├── app.css
│   └── app.d.ts
├── static/
│   └── samples/
├── svelte.config.js
├── vite.config.ts
├── package.json
├── tsconfig.json
└── tailwind.config.js
```

---

## Chunk 1: Rust Trace Output

**Goal:** Add `--trace` flag to `arrival_detector` to emit detailed JSONL trace files.

---

### Task 0: Add serde to shared crate

**Note:** `FsmState` needs to derive `Serialize` for consistent JSON serialization. We use string conversion in trace.rs for compatibility, but serde is useful for other potential uses.

**Files:**
- Modify: `shared/Cargo.toml`

- [ ] **Step 1: Check current shared/Cargo.toml**

Run: `cat shared/Cargo.toml`
Expected: Verify current dependencies

- [ ] **Step 2: Add serde as optional dependency with derive feature**

```toml
[dependencies]
serde = { workspace = true, optional = true, features = ["derive"] }
crc32fast = { workspace = true, default-features = false }

[features]
default = ["std"]
std = ["crc32fast/std", "serde"]  # Add serde to std feature
```

**Note:** Explicitly adding `features = ["derive"]` ensures the Serialize derive macro is available.

- [ ] **Step 3: Verify workspace has serde**

Run: `cat Cargo.toml | grep serde`
Expected: `serde = { version = "1.0", workspace = true }` or similar

- [ ] **Step 4: Derive Serialize on FsmState**

Modify `shared/src/lib.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]  // Add Serialize
pub enum FsmState {
    Approaching,
    Arriving,
    AtStop,
    Departed,
}
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check -p shared`
Expected: No errors

- [ ] **Step 6: Commit**

```bash
git add shared/Cargo.toml shared/src/lib.rs
git commit -m "feat(shared): add serde support for FsmState serialization"
```

---

### Task 1: Create trace.rs Module

**Files:**
- Create: `arrival_detector/src/trace.rs`

- [ ] **Step 1: Write trace.rs module**

```rust
//! Trace record emission for debugging visualization

use serde::Serialize;
use shared::{DistCm, SpeedCms, Prob8, FsmState};
use std::io::{BufWriter, Write};

/// Trace record for debugging visualization
#[derive(Serialize)]
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

    /// FSM state - using FsmState directly lets serde handle serialization
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

/// Write a trace record to the output file
pub fn write_trace_record<W: Write>(
    output: &mut BufWriter<W>,
    record: &TraceRecord,
) -> std::io::Result<()> {
    let json = serde_json::to_string(record)?;
    writeln!(output, "{}", json)
}
```

**Note:** Using `FsmState` directly with serde's `Serialize` derive produces cleaner JSON and eliminates the need for manual string conversion functions. Serde will serialize the enum variants as their string names automatically.

- [ ] **Step 2: Verify module compiles**

Run: `cargo check --bin arrival_detector`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add arrival_detector/src/trace.rs
git commit -m "feat(arrival_detector): add trace module for debugging output"
```

---

### Task 2: Export trace Module from lib.rs

**Files:**
- Modify: `arrival_detector/src/lib.rs`

- [ ] **Step 1: Add trace module export**

```rust
pub mod corridor;
pub mod probability;
pub mod state_machine;
pub mod recovery;
pub mod input;
pub mod output;
pub mod trace;  // ADD THIS LINE
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check --bin arrival_detector`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add arrival_detector/src/lib.rs
git commit -m "feat(arrival_detector): export trace module"
```

---

### Task 3: Verify FsmState Serialization

**Files:**
- Check: `shared/src/lib.rs`

- [ ] **Step 1: Verify FsmState derives serde traits**

The `FsmState` enum should derive `Serialize`. Check:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum FsmState {
    Approaching,
    Arriving,
    AtStop,
    Departed,
}
```

Note: The `Serialize` derive produces JSON as `{"Approaching":{}}` by default. We use string conversion in trace.rs for consistent format.

- [ ] **Step 2: Run test**

Run: `cargo test -p shared`
Expected: Tests pass

---

### Task 4: Add --trace Flag to main.rs

**Files:**
- Modify: `arrival_detector/src/main.rs`

- [ ] **Step 1: Add trace argument parsing**

Find the current args parsing section (starts with `let args: Vec<String> = env::args().collect();`) and replace it with:

```rust
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

if args.len() < input_idx + 3 {
    eprintln!("Usage: arrival_detector <input.jsonl> <route_data.bin> <output.jsonl> [--trace trace.jsonl]");
    std::process::exit(1);
}

let input_path = PathBuf::from(&args[input_idx]);
let route_path = PathBuf::from(&args[input_idx + 1]);
let output_path = PathBuf::from(&args[input_idx + 2]);

// Print usage info
println!("Phase 3: Arrival Detection");
if let Some(ref tp) = trace_path {
    println!("  Mode: Trace output enabled");
}
println!("  Input:  {}", input_path.display());
println!("  Route:  {}", route_path.display());
println!("  Output: {}", output_path.display());
if let Some(ref tp) = trace_path {
    println!("  Trace:  {}", tp.display());
}
println!();
```

- [ ] **Step 2: Open trace writer if specified**

Find the `output_writer` initialization and add after it:

```rust
let mut output_writer = BufWriter::new(File::create(&output_path).expect("Failed to create output file"));

// Open trace writer if --trace specified
let mut trace_writer: Option<BufWriter<File>> = trace_path.as_ref().map(|p| {
    BufWriter::new(File::create(p).expect("Failed to create trace file"))
});
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check --bin arrival_detector`
Expected: No errors

- [ ] **Step 4: Commit**

```bash
git add arrival_detector/src/main.rs
git commit -m "feat(arrival_detector): add --trace flag CLI parsing"
```

---

### Task 5: Integrate Trace Writing in Main Loop

**Files:**
- Modify: `arrival_detector/src/main.rs`

**Note:** This approach computes probability ONCE per active stop and reuses it for both trace and state update. This avoids code duplication while ensuring consistency between trace output and actual algorithm behavior.

- [ ] **Step 1: Add trace imports to top of main.rs**

Add to existing imports:

```rust
use arrival_detector::state_machine::StopState;
use arrival_detector::{input, corridor, probability, recovery, output, trace};
use arrival_detector::trace::{TraceRecord, StopTraceState, FeatureScores, write_trace_record};
```

- [ ] **Step 2: Build trace record in main loop**

Find the inner loop that processes active stops (`for &stop_idx in &active_indices {`) and replace the entire section (from GPS jump check to end of that loop) with:

```rust
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

// Pre-compute probability and features for all active stops
// We do this ONCE to avoid duplication between trace and state updates
struct CalculatedState {
    stop_idx: usize,
    prob: Prob8,
    features: FeatureScores,
    distance_cm: DistCm,
    pre_update_fsm_state: FsmState,
    dwell_time_s: u16,
}

let calculated_states: Vec<CalculatedState> = active_indices.iter()
    .map(|&idx| {
        let stop = &stops[idx];
        let state = &stop_states[idx];

        // Compute probability ONCE
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

        CalculatedState {
            stop_idx: idx,
            prob,
            features: FeatureScores { p1, p2, p3, p4 },
            distance_cm: d_cm,
            pre_update_fsm_state: state.fsm_state,
            dwell_time_s: state.dwell_time_s,
        }
    })
    .collect();

// Build trace record with pre-update state
let mut trace_record: Option<TraceRecord> = trace_writer.as_ref().map(|_| {
    let stop_trace_states: Vec<StopTraceState> = calculated_states.iter()
        .map(|cs| StopTraceState {
            stop_idx: cs.stop_idx as u8,
            distance_cm: cs.distance_cm,
            fsm_state: cs.pre_update_fsm_state,  // Use FsmState directly, serde handles serialization
            dwell_time_s: cs.dwell_time_s,
            probability: cs.prob,
            features: cs.features,
            just_arrived: false,  // Will update below if arrival occurs
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
// Use the PRE-COMPUTED probability values for consistency
let mut arrivals_this_frame: Vec<u8> = Vec::new();

for cs in &calculated_states {
    let stop = &stops[cs.stop_idx];
    let state = &mut stop_states[cs.stop_idx];

    // Handle re-entry after departure
    if state.fsm_state == FsmState::Departed {
        if state.can_reactivate(record.s_cm, stop.progress_cm) {
            state.reset();
        }
    }

    // Update state machine with PRE-COMPUTED probability
    if state.update(record.s_cm, record.v_cms, stop.progress_cm, cs.prob) {
        // Just arrived!
        arrivals_this_frame.push(state.index);

        let event = ArrivalEvent {
            time: record.time,
            stop_idx: state.index,
            s_cm: record.s_cm,
            v_cms: record.v_cms,
            probability: cs.prob,
        };
        output::write_event(&mut output_writer, &event).expect("Failed to write arrival event");
        arrivals += 1;
        current_stop_idx = state.index;
    }
}

// Write trace record after state updates
if let (Some(mut tw), Some(mut tr)) = (trace_writer.as_mut(), trace_record) {
    // Update just_arrived flags for stops that arrived this frame
    for arrived_idx in arrivals_this_frame.iter() {
        if let Some(ts) = tr.stop_states.iter_mut().find(|s| s.stop_idx == *arrived_idx) {
            ts.just_arrived = true;
        }
    }

    write_trace_record(&mut tw, &tr).expect("Failed to write trace");
}
```

- [ ] **Step 3: Flush trace writer at end**

Find the final `println!` statement and add before it:

```rust
// Flush output writers
output_writer.flush().expect("Failed to flush output file");

if let Some(mut tw) = trace_writer {
    tw.flush().expect("Failed to flush trace file");
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check --bin arrival_detector`
Expected: No errors

- [ ] **Step 5: Test trace output**

**Note:** This test requires phase2.jsonl. Generate it first if needed:

```bash
# Generate phase2.jsonl from existing test.nmea
cargo run --bin simulator -- test.nmea route_data.bin phase2.jsonl
```

Now test trace output:

Run: `cargo run --bin arrival_detector -- phase2.jsonl route_data.bin arrivals.jsonl --trace trace.jsonl`
Expected: Creates `trace.jsonl` with valid JSON lines

- [ ] **Step 6: Verify trace output format**

Run: `head -5 trace.jsonl | jq`
Expected: Valid JSON with fields: time, s_cm, v_cms, active_stops, stop_states, gps_jump, recovery_idx

- [ ] **Step 7: Commit**

```bash
git add arrival_detector/src/main.rs
git commit -m "feat(arrival_detector): integrate trace writing in main loop"
```

---

### Task 6: Add Integration Test for Trace Output

**Files:**
- Create: `arrival_detector/tests/trace_output.rs`

- [ ] **Step 1: Write integration test**

```rust
//! Integration test for trace output

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

#[test]
fn test_trace_output_valid_json() {
    // This test requires running the binary first with sample data
    // For now, just verify the trace module compiles

    use arrival_detector::trace::{TraceRecord, fsm_state_as_string};
    use shared::FsmState;

    // Verify string conversion works
    assert_eq!(fsm_state_as_string(FsmState::Approaching), "Approaching");
    assert_eq!(fsm_state_as_string(FsmState::Arriving), "Arriving");
    assert_eq!(fsm_state_as_string(FsmState::AtStop), "AtStop");
    assert_eq!(fsm_state_as_string(FsmState::Departed), "Departed");
}
```

- [ ] **Step 2: Run test**

Run: `cargo test -p arrival_detector trace_output`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add arrival_detector/tests/trace_output.rs
git commit -m "test(arrival_detector): add trace output integration test"
```

---

**Chunk 1 Complete:** Rust trace output is now functional. Verify with:

```bash
cargo run --bin arrival_detector -- phase2.jsonl route_data.bin arrivals.jsonl --trace trace.jsonl
head -n 1 trace.jsonl | jq
```

---

## Chunk 2: SvelteKit Project Setup

**Goal:** Initialize the visualizer as a SvelteKit project with static adapter.

---

### Task 1: Initialize SvelteKit Project

**Files:**
- Create: `visualizer/` directory structure

- [ ] **Step 1: Create SvelteKit project**

Run from project root:
```bash
cd /Users/herry/project/pico2w/bus_arrival
npx create-svelte-kit@latest visualizer
# Select: Skeleton project, TypeScript, ESLint, Prettier
cd visualizer
npm install
```

- [ ] **Step 2: Install static adapter**

Run: `npm install -D @sveltejs/adapter-static`

- [ ] **Step 3: Configure static adapter**

Edit `svelte.config.js`:
```javascript
import adapter from '@sveltejs/adapter-static';

/** @type {import('@sveltejs/kit').Config} */
const config = {
    kit: {
        adapter: adapter({
            pages: 'build',
            assets: 'build',
            fallback: 'index.html',
            strict: true
        })
    }
};

export default config;
```

- [ ] **Step 4: Install dependencies**

**Note:** Using MapLibre GL JS instead of Leaflet for better vector rendering performance and native bearing support.

Run: `npm install maplibre-gl chart-js`
Run: `npm install -D @types/maplibre-gl`
Run: `npm install -D tailwindcss postcss autoprefixer`
Run: `npx tailwindcss init -p`

- [ ] **Step 5: Configure Tailwind**

Edit `tailwind.config.js`:
```javascript
/** @type {import('tailwindcss').Config} */
export default {
  content: ['./src/**/*.{html,js,svelte,ts}'],
  theme: { extend: {} },
  plugins: [],
};
```

- [ ] **Step 6: Add Tailwind to app.css**

Edit `src/app.css`:
```css
@tailwind base;
@tailwind components;
@tailwind utilities;
html, body { @apply h-full w-full margin-0 padding-0; }
```

- [ ] **Step 7: Commit**

```bash
git add visualizer/
git commit -m "feat(visualizer): initialize SvelteKit project"
```

---

### Task 2: Create TypeScript Types

**Files:**
- Create: `visualizer/src/lib/types.ts`

**Note on FsmState:**
- Rust now derives `Serialize` on `FsmState` (via Task 0)
- Serde serializes enum variants as their string names ("Approaching", "Arriving", etc.)
- TypeScript interface should use: `fsm_state: 'Approaching' | 'Arriving' | 'AtStop' | 'Departed'`

- [ ] **Step 1: Write types.ts** (See design spec for complete interface definitions)
- [ ] **Step 2: Commit**

```bash
git add visualizer/src/lib/types.ts
git commit -m "feat(visualizer): add TypeScript types"
```

---

**Chunk 2 Complete:** SvelteKit project initialized.

---

## Chunk 3: Parsers & Utilities

**Goal:** Implement binary file parser and coordinate projection.

---

### Task 1: Implement routeData.ts Parser

**Files:**
- Create: `visualizer/src/lib/parsers/routeData.ts`

**Important Notes:**
- **Binary Layout:** RouteNode is `#[repr(C, packed)]` in Rust (52 bytes total)
- **Offset 0-7:** len2_cm2 (i64) - read as Uint32 low + Int32 high, combine to BigInt
- **Offset 8-15:** line_c (i64) - same treatment
- **Offset 16-17:** heading_cdeg (i16, little-endian)
- **Offset 18-19:** _pad (i16)
- **Offset 20+:** i32 fields (x_cm, y_cm, cum_dist_cm, etc.) - little-endian
- **Byte Order:** Rust uses `to_le_bytes()`, so DataView must use `littleEndian: true`

- [ ] **Step 1: Write binary parser** (See design spec: `docs/superpowers/specs/2026-03-13-visualizer-design.md` section "Binary File Parsing")
- [ ] **Step 2: Write unit test**
- [ ] **Step 3: Commit**

```bash
git add visualizer/src/lib/parsers/routeData.ts
git commit -m "feat(visualizer): add binary route data parser"
```

---

### Task 2: Implement Coordinate Projection

**Files:**
- Create: `visualizer/src/lib/utils/projection.ts`

**Important Notes:**
- **Constants MUST match `shared/src/lib.rs` exactly:**
  - `FIXED_ORIGIN_LON_DEG = 120.0`
  - `FIXED_ORIGIN_LAT_DEG = 20.0`
  - `EARTH_R_CM = 637_100_000.0`
  - `PROJECTION_LAT_AVG = routeData.lat_avg_deg` (from binary)
- **Inverse projection formula:**
  - `lat = y_cm / EARTH_R_CM * (180/π) + FIXED_ORIGIN_LAT_DEG`
  - `lon = x_cm / (EARTH_R_CM * cos(PROJECTION_LAT_AVG * π/180)) * (180/π) + FIXED_ORIGIN_LON_DEG`

- [ ] **Step 1: Write projection utilities** (See design spec for complete implementation)
- [ ] **Step 2: Commit**

```bash
git add visualizer/src/lib/utils/projection.ts
git commit -m "feat(visualizer): add coordinate projection"
```

---

### Task 3: Implement traceData.ts Parser

**Files:**
- Create: `visualizer/src/lib/parsers/traceData.ts`

- [ ] **Step 1: Write trace parser** (See spec)
- [ ] **Step 2: Commit**

```bash
git add visualizer/src/lib/parsers/traceData.ts
git commit -m "feat(visualizer): add trace file parser"
```

---

**Chunk 3 Complete:** Parsers and utilities implemented.

---

## Chunk 4: State & Components

**Goal:** Create Svelte stores and core UI components.

---

### Task 1: Create Svelte Stores

**Files:**
- Create: `visualizer/src/lib/stores/data.ts`

- [ ] **Step 1: Write stores** (See spec)
- [ ] **Step 2: Commit**

---

### Task 2: Create Components

**Files:**
- Create: `visualizer/src/lib/components/Header.svelte`
- Create: `visualizer/src/lib/components/MapView.svelte`
- Create: `visualizer/src/lib/components/ControlPanel.svelte`

**Note for MapView.svelte:**
- Use MapLibre GL JS (not Leaflet) for better vector rendering
- Initialize map with: `new maplibregl.Map({ container: 'map', style: 'https://demotiles.maplibre.org/style.json' })`
- Add route as GeoJSON line source
- Add stops as circle layer with color-coded FSM states

- [ ] **Step 1: Write each component** (See design spec for component details)
- [ ] **Step 2: Commit each**

---

### Task 3: Create Main Page

**Files:**
- Modify: `visualizer/src/routes/+page.svelte`

- [ ] **Step 1: Write main page** (See spec)
- [ ] **Step 2: Test locally:** `npm run dev`
- [ ] **Step 3: Commit**

---

**Chunk 4 Complete:** State management and UI components implemented.

---

## Chunk 5: Build & Deploy

**Goal:** Build static files and verify.

---

- [ ] **Step 1: Build for production**

Run: `npm run build`

- [ ] **Step 2: Test static build**

Run: `npx serve visualizer/build`

- [ ] **Step 3: Generate sample data**

```bash
cd /Users/herry/project/pico2w/bus_arrival
cargo run --bin simulator -- test.nmea route_data.bin phase2.jsonl
cargo run --bin arrival_detector -- phase2.jsonl route_data.bin arrivals.jsonl --trace trace.jsonl
cp route_data.bin trace.jsonl visualizer/static/samples/
```

- [ ] **Step 4: Final commit**

```bash
git add visualizer/
git commit -m "feat(visualizer): complete implementation"
```

---

## Implementation Complete!

**Usage:**
1. Generate trace: `cargo run --bin arrival_detector -- phase2.jsonl route_data.bin arrivals.jsonl --trace trace.jsonl`
2. Open visualizer, load `route_data.bin` and `trace.jsonl`
3. Use controls to step through GPS updates


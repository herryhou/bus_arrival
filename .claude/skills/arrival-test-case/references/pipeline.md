# Bus Arrival Detection Pipeline

## Architecture Overview

```
NMEA GPS Data → Simulator → Arrival Detector → Visualizer
     (raw)          (phase 2)      (phase 3)        (UI)
```

## Phase 1: Binary Route Data

**Input:** Route JSON with waypoints and stops
**Output:** `.bin` file with pre-computed route segments

**Command:** ( handled by separate build tool )

**Content:**
- Route points (lat/lon)
- Stop locations
- Segment bearings and distances
- Spatial index for fast map matching

## Phase 2: Simulator (Map Matching)

**Input:** NMEA file, route data binary
**Output:** JSONL trace with map-matched positions

**Command:**
```bash
cargo run -p simulator -- input.nmea route.bin output.jsonl
```

**Output fields:**
- `time` - Timestamp
- `lat`, `lon` - Map-matched position
- `s_cm` - Distance along route (cm)
- `v_cms` - Velocity (cm/s)
- `heading_cdeg` - Heading in centidegrees (-18000 to 18000)
- `status` - Map matching status
- `seg_idx` - Current route segment
- `active_stops` - Nearby stop indices
- `stop_states` - Per-stop distance and FSM state
- `gps_jump` - GPS jump detected
- `recovery_idx` - Recovery mode index

**Key responsibilities:**
- Parse NMEA sentences
- Validate checksums
- Project GPS onto route segments
- Detect GPS jumps
- Compute distances to stops

## Phase 3: Arrival Detector

**Input:** Simulator JSONL, route data binary
**Output:** Arrivals JSONL, trace JSONL (optional)

**Command:**
```bash
cargo run -p arrival_detector -- \
  sim.jsonl route.bin arrivals.jsonl --trace trace.jsonl
```

**Output fields (trace):**
- All simulator fields, plus:
- `stop_states[].fsm_state` - FSM state per stop
- `stop_states[].dwell_time_s` - Time at stop
- `stop_states[].probability` - Arrival probability
- `stop_states[].just_arrived` - Arrival flag

**FSM States:**
- `Approaching` - Moving toward stop
- `Arriving` - Decelerating to stop
- `AtStop` - Dwelling at stop
- `Departed` - Left the stop

## Phase 4: Visualizer

**Input:** Trace JSONL, route data binary
**Output:** Web UI

**Load method:**
1. Copy files to `visualizer/static/`
2. Run `npm run dev`
3. Open browser and select trace file

**What it shows:**
- Route path and stops
- Bus position and heading
- FSM state transitions
- Event log (ARRIVAL, DEPARTURE, JUMP, etc.)

## Data Flow Example

```bash
# Step 1: Generate NMEA from route
node gen_nmea.js generate --route route.json --out-nmea test.nmea

# Step 2: Run simulator
cargo run -p simulator -- test.nmea route.bin test_sim.jsonl

# Step 3: Run arrival detector
cargo run -p arrival_detector -- \
  test_sim.jsonl route.bin test_arrivals.jsonl \
  --trace test_trace.jsonl

# Step 4: Visualize
cp test_trace.jsonl visualizer/static/
cd visualizer && npm run dev
```

## Common Issues

| Issue | Cause | Fix |
|-------|-------|-----|
| 0 GPS updates | Coordinates don't match route | Use gen_nmea.js with same route |
| Missing active_stops | Old simulator version | Rebuild simulator |
| Wrong heading | Overflow bug not fixed | Check nmea.rs conversion |
| No events | Wrong trace file | Use arrival_detector trace |

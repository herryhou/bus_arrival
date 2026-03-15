---
name: arrival-test-case
description: when user asks to "create test case for bus arrival", "generate NMEA test data", "create heading overflow test", "build GPS simulation test". Provides workflow for creating and validating test cases for the bus arrival detection system.
version: 0.1.0
---

# Arrival Detection Test Case Creation

Create and validate test cases for the bus arrival detection system, including NMEA GPS data generation, heading overflow scenarios, and end-to-end pipeline verification.

## Overview

The bus arrival detection system uses NMEA GPS sentences processed through a simulator and arrival detector. Test cases require proper route geometry, valid NMEA checksums, and verification that heading values don't overflow i16 bounds.

## When to Use This Skill

Use this skill when:
- Creating new test routes for arrival detection
- Generating NMEA GPS data with specific heading scenarios
- Testing the full simulator → arrival detector → visualizer pipeline

## Test Case Creation Workflow

### Step 1: Design Route Geometry

Create a route JSON file with waypoints that generate desired headings:

```json
{
  "route_points": [
    [lat1, lon1],
    [lat2, lon2]
  ]
}
```

**Location:** `test_data/test_name_route.json`

**Key considerations:**
- Heading between consecutive points is calculated automatically via bearing()

### Step 2: Create Stops File

```json
{"stops": [
  {"lat": 25.xxxx, "lon": 121.xxxx}
]}
```

**Location:** `test_data/test_name_stops.json`

### Step 3: Generate NMEA Data

Use `gen_nmea.js` tool to generate valid NMEA sentences:

```bash
cd tools/gen_nmea
node gen_nmea.js generate \
  --route ../../test_data/test_name_route.json \
  --stops ../../test_data/test_name_stops.json \
  --out-nmea ../../test_data/test_name.nmea \
  --out-gt ../../test_data/test_name_gt.json \
  --json
```

**What gen_nmea.js does:**
- Calculates headings from route geometry automatically
- Adds headings noise (AR1 process)
- Generates proper NMEA checksums
- Adds GPS noise (AR1 process)
- Simulates bus motion with acceleration/deceleration

**Available scenarios:** normal, drift, jump, outage

### Step 4: Generate `test_name.bin`

```bash
cargo run -p preprocessor -- test_data/test_name_route.json test_data/test_name_stops.json test_data/test_name.bin
```
pack all route nodes and stops into a binary format for the arrival detector.

### Step 5: Run Simulator

```bash
cargo run -p simulator -- test_data/test_name.nmea test_data/test_name.bin test_data/test_name_sim.jsonl
```

### Step 6: Verify Simulator Output

```bash
# Check for overflow sentinel (should be 0)
# grep -c '32767' test_name_sim.jsonl

# Check heading range (valid: -18000 to 18000)
# jq -r '.heading_cdeg' test_name_sim.jsonl | sort -n | grep -v null | head -1
# jq -r '.heading_cdeg' test_name_sim.jsonl | sort -n | grep -v null | tail -1
```

### Step 7: Run Arrival Detector

```bash
cargo run -p arrival_detector -- \
  test_data/test_name_sim.jsonl \
  test_data/test_name.bin \
  test_data/test_name_arrivals.jsonl \
  --trace test_data/test_name_trace.jsonl
```

### Step 8: Test in Visualizer

```bash
# cp test_data/test_name_trace.jsonl visualizer/static/
# cp test_data/test_name.bin visualizer/static/
cd visualizer && npm run dev
```

**What to verify:**
- Bus appears on route path
- Heading arrow points in travel direction
- Events appear (ARRIVAL, DEPARTURE, etc.)
- ARRIVAL Events amount and timing match ground truth 

## Common Issues

**"Processed 0 GPS updates"**: Use gen_nmea.js with the same route JSON used to create test_name.bin

**"missing or invalid 'active_stops' field"**: Ensure simulator output includes active_stops, stop_states, gps_jump, recovery_idx, heading_cdeg

**Bus showing wrong heading (32767)**: Verify nmea.rs applies the conversion: heading > 18000 → subtract 36000

**No events in visualizer**: Use arrival_detector trace (--trace output), not simulator output

## Quick Verification Commands

```bash
# Count unique headings in NMEA
# grep GPRMC test.nmea | awk -F',' '{print $9}' | sort -u

```

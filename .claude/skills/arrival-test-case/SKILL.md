---
name: arrival-test-case
description: when user asks to "create test case for bus arrival", "generate NMEA test data", "create heading overflow test", "build GPS simulation test". Provides workflow for creating and validating test cases for the bus arrival detection system.
version: 0.2.0
---

# Arrival Detection Test Case Creation

Create and validate test cases for the bus arrival detection system, including NMEA GPS data generation, heading overflow scenarios, and end-to-end pipeline verification.

## Overview

The bus arrival detection system uses NMEA GPS sentences processed through a simulator and arrival detector. Test cases require proper route geometry, valid NMEA checksums, and verification that heading values don't overflow i16 bounds.

**CRITICAL**: The coordinate system must match between preprocessor and simulator. Route nodes are stored relative to the spatial grid origin (x0_cm, y0_cm), NOT the fixed origin (120°E, 20°N).

## When to Use This Skill

Use this skill when:
- Creating new test routes for arrival detection
- Generating NMEA GPS data with specific heading scenarios
- Testing the full simulator → arrival detector → visualizer pipeline
- Debugging arrival detection failures

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
- Route should be 2-5 km for normal scenarios
- Include enough points (15-25) for realistic geometry
- For best results, center route around latitude 25.0°N (Taiwan average)

### Step 2: Create Stops File

```json
{
  "stops": [
    {"lat": 25.xxxx, "lon": 121.xxxx, "name": "Stop Name"}
  ]
}
```

**Location:** `test_data/test_name_stops.json`

**Key considerations:**
- Stop coordinates must match route point coordinates exactly for best results
- Place stops at specific route point indices for predictable behavior
- Use 3-5 stops for comprehensive testing

### Step 3: Generate NMEA Data

Use `gen_nmea.js` tool to generate valid NMEA sentences:

```bash
cd tools/gen_nmea
node gen_nmea.js generate \
  --route ../../test_data/test_name_route.json \
  --stops ../../test_data/test_name_stops.json \
  --out-nmea ../../test_data/test_name.nmea \
  --out-gt ../../test_data/test_name_gt.json \
  --scenario normal \
  --json
```

**What gen_nmea.js does:**
- Calculates headings from route geometry automatically
- Adds heading noise (AR1 process)
- Generates proper NMEA checksums
- Adds GPS noise (AR1 process)
- Simulates bus motion with acceleration/deceleration
- Creates ground truth with expected arrival times

**Available scenarios:** normal, drift, jump, outage

### Step 4: Generate `test_name.bin`

```bash
cargo run -p preprocessor -- \
  test_data/test_name_route.json \
  test_data/test_name_stops.json \
  test_data/test_name.bin
```

**What to look for in output:**
- `Transformed route nodes to grid origin` - confirms coordinate fix is applied
- `Computed average latitude` - should be close to route's actual average
- `Projected N stops with corridors` - confirms stop projection succeeded

### Step 5: Run Simulator

```bash
cargo run -p simulator -- \
  test_data/test_name.nmea \
  test_data/test_name.bin \
  test_data/test_name_sim.jsonl
```

### Step 6: Verify Simulator Output

**CRITICAL VERIFICATION** - Run these checks to detect coordinate system issues:

```bash
# Check s_cm range - should be ~80% of expected route distance
MAX_S_CM=$(jq -r '.s_cm' test_data/test_name_sim.jsonl | sort -n | tail -1)
echo "Max s_cm: $MAX_S_CM cm = $(($MAX_S_CM / 100)) meters"

# Check if active_stops are populated (should see 0 and 1)
jq -r '.active_stops | length' test_data/test_name_sim.jsonl | sort -u

# Count GPS updates processed
# Should match NMEA record count (~2x NMEA lines for GPRMC+GPGGA)
wc -l test_data/test_name_sim.jsonl
```

**Expected results:**
- Max s_cm: Should be close to route length (e.g., 250000-300000 cm for 2.5-3km route)
- active_stops: Should show both 0 and 1 (empty + populated)
- GPS updates: ~400-500 for normal 8-minute simulation

**FAILURE INDICATORS:**
- Max s_cm < 10000 cm: Coordinate system mismatch bug
- active_stops shows only 0: No stops being detected
- Processed 0 GPS updates: Route/NMEA mismatch

### Step 7: Run Arrival Detector

```bash
cargo run -p arrival_detector -- \
  test_data/test_name_sim.jsonl \
  test_data/test_name.bin \
  test_data/test_name_arrivals.jsonl \
  --trace test_data/test_name_trace.jsonl
```

**Verification:**
```bash
# Check arrivals detected
cat test_data/test_name_arrivals.jsonl

# Compare with ground truth
echo "=== Detected arrivals ===" && wc -l test_data/test_name_arrivals.jsonl
echo "=== Ground truth arrivals ===" && jq '. | length' test_data/test_name_gt.json
```

### Step 8: Test in Visualizer

```bash
cp test_data/test_name_trace.jsonl visualizer/static/
cp test_data/test_name.bin visualizer/static/
cd visualizer && npm run dev
```

**What to verify:**
- Bus appears on route path
- Heading arrow points in travel direction
- Events appear (ARRIVAL, DEPARTURE, etc.)
- ARRIVAL Events amount and timing match ground truth

## Troubleshooting

### Coordinate System Mismatch (CRITICAL BUG - FIXED)

**Symptoms:**
- Max s_cm < 10000 cm for multi-km route
- active_stops always empty
- 0 arrivals detected despite valid NMEA data

**Root Cause:**
Preprocessor was storing route nodes relative to fixed origin (120°E, 20°N) but simulator expected them relative to spatial grid origin (x0_cm, y0_cm).

**Fix Status:** FIXED in preprocessor v0.2.0+
- Check for "Transformed route nodes to grid origin" message in preprocessor output
- If missing, rebuild preprocessor: `cargo build -p preprocessor`

**Verification:**
```bash
# Should show route nodes were transformed
cargo run -p preprocessor -- test_data/test_name_route.json test_data/test_name_stops.json test_data/test_name.bin 2>&1 | grep "Transformed"
```

### "Processed 0 GPS Updates"

**Cause:** NMEA file doesn't match route geometry
**Fix:** Regenerate NMEA with same route JSON used for binary file

### "missing or invalid 'active_stops' field"

**Cause:** Old simulator output format or coordinate system mismatch
**Fix:** Check simulator output includes: active_stops, stop_states, gps_jump, recovery_idx, heading_cdeg

### Bus Showing Wrong Heading (32767)

**Cause:** Heading overflow - NMEA heading not converted from 0-360° to -18000-18000 centidegrees
**Fix:** Verify simulator/nmea.rs applies: `heading > 18000 → subtract 36000`

### No Events in Visualizer

**Cause:** Using simulator output instead of arrival detector trace
**Fix:** Use `--trace` output from arrival_detector, not simulator output

### Missing Arrivals (detected < ground truth)

**Possible causes:**
1. Short dwell times (< 5s) may be missed
2. High speed through stop (check v_cms in simulator output)
3. GPS noise causing position jumps

**Debug:**
```bash
# Check speed at stop location
jq -r 'select(.active_stops | index(STOP_IDX)) | {time, s_cm, v_cms}' test_data/test_name_sim.jsonl
```

## Common Issues

| Issue | Cause | Solution |
|-------|-------|----------|
| Max s_cm < 10km | Coordinate system mismatch | Rebuild preprocessor |
| 0 arrivals | active_stops empty | Check coordinate transform |
| "Processed 0 GPS" | NMEA/route mismatch | Regenerate with same route |
| Wrong heading (32767) | Overflow bug | Check nmea.rs conversion |
| No visualizer events | Wrong input file | Use arrival_detector trace |

## Quick Verification Commands

```bash
# Full test pipeline verification
echo "=== Route length ===" && \
  MAX_S=$(jq -r '.s_cm' test_data/test_name_sim.jsonl | sort -n | tail -1) && \
  echo "$MAX_S cm = $(($MAX_S / 100)) meters"

echo "=== Active stops check ===" && \
  jq -r '.active_stops | length' test_data/test_name_sim.jsonl | sort -u

echo "=== Arrival detection ===" && \
  echo "Detected: $(wc -l < test_data/test_name_arrivals.jsonl)" && \
  echo "Expected: $(jq '. | length' test_data/test_name_gt.json)"

echo "=== Heading range ===" && \
  echo "Min: $(jq -r '.heading_cdeg' test_data/test_name_sim.jsonl | sort -n | head -1) cdeg" && \
  echo "Max: $(jq -r '.heading_cdeg' test_data/test_name_sim.jsonl | sort -n | tail -1) cdeg"
```

## Test Case Best Practices

1. **Route Design:**
   - Use 15-25 waypoints for realistic geometry
   - Center around latitude 25.0°N for Taiwan
   - Make routes 2-5 km long
   - Place stops at explicit route point indices

2. **Stop Configuration:**
   - Use 3-5 stops for good coverage
   - Match stop coordinates exactly to route points
   - Space stops evenly along route

3. **Scenario Selection:**
   - `normal`: Standard GPS, 8 sats, HDOP 3.5
   - `drift`: Urban canyon, 5 sats, HDOP 7.0
   - `jump`: GPS with 100m+ position jump
   - `outage`: GPS signal outage for testing

4. **Verification:**
   - Always check s_cm range after simulator
   - Verify active_stops are populated
   - Compare detected arrivals with ground truth
   - Test in visualizer for final validation

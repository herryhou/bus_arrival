# Plan: Improve gen_nmea.js GPS Simulator

## Context

The `gen_nmea.js` tool generates NMEA test data for the bus arrival detection system. Current limitations identified:

1. **No loop route support** - Routes where start/end meet cause issues (confirmed bug in `bdd_localization.rs:771`)
2. **Hardcoded parameters** - Speeds, acceleration, noise params are constants
3. **Limited route validation** - Only basic checks exist
4. **Basic noise model** - AR(1) with drift, no $GNGSA sentences
5. **No multi-visit stop support** - Can't simulate routes where same stop is visited multiple times

## Critical Files

| File | Purpose |
|------|---------|
| `/workspace/tools/gen_nmea/gen_nmea.js` | Main generator (JavaScript) |
| `/workspace/simulator/src/nmea.rs` | NMEA parser (Rust - already handles $GNGSA) |
| `/workspace/simulator/src/kalman.rs` | Kalman filter with monotonicity constraint |
| `/workspace/simulator/src/map_match.rs` | Map matcher (projection logic) |
| `/workspace/shared/src/binfile.rs` | Binary route format (VERSION=2) |
| `/workspace/simulator/tests/bdd_localization.rs` | Loop closure test (ignored at line 771) |

## Architecture Analysis

### Current Binary Format (v2)
```
[Header: 28 bytes]
  magic: u32          // "BUSA"
  version: u16        // 2
  node_count: u16
  stop_count: u8
  padding: u8[3]
  x0_cm: i32
  y0_cm: i32
  lat_avg_deg: f64
[Nodes...]
[Stops...]
[Grid...]
[LUTs...]
[CRC32]
```

**Missing**: No flags for route topology (circular, linear)

### Loop Route Problem
When a circular route completes, GPS returns to start coordinates:
- Map matcher projects to segment 0 (geometrically closest)
- Progress snaps to 0 instead of route_length
- Kalman filter produces intermediate value (~6024 instead of 20000)

The fix requires **coordinated changes**:
1. Binary format: Add route flags + route_length
2. Map matcher: Use route_length for projection clamping
3. Generator: Detect and flag circular routes

---

## Implementation Plan (Phased)

### Phase 1: Low-Risk, High-Value Improvements
**Goal**: Incremental improvements with no simulator changes

#### 1.1 Make Parameters Configurable

**Problem**: Speed, acceleration, noise parameters are hardcoded (lines 223-234)

**Solution**: Add optional `config` section to route JSON:

```json
{
  "route_points": [[25.0, 121.0], [25.01, 121.01]],
  "stops": [0, 50],
  "config": {
    "cruise_speed_kmh": 28,
    "max_speed_kmh": 50,
    "accel_ms2": 1.2,
    "decel_ms2": 1.8,
    "stop_dwell_s": 8,
    "ar1_alpha": 0.7,
    "drift_decay": 0.98,
    "hdop_base": 3.5
  }
}
```

**Changes to `gen_nmea.js`**:
1. Add `mergeConfig(base, override)` helper
2. Update `cmdGenerate()` to merge `route.config` with `SCENARIOS`
3. Add `--config` CLI flag for external config file
4. Add config validation (speeds > 0, alpha in [0,1])

**Files modified**: `/workspace/tools/gen_nmea/gen_nmea.js`

#### 1.2 Add Route Validation

**Problem**: Current validation (lines 649-658) only checks array presence

**Solution**: Add geometric validation:

```javascript
const validations = {
  // Coordinate bounds
  coordinates: { lat: [-90, 90], lon: [-180, 180] },

  // Segment constraints
  max_segment_m: 500,
  min_segment_m: 0.1,

  // Stop constraints
  max_stop_distance_m: 200,
  min_stop_spacing_m: 20,

  // Route topology
  detect_loops: true,
  detect_self_cross: true
}
```

**Implementation**:
1. Add `validateRoute(route)` function
2. Calculate segment lengths, detect anomalies
3. Project each stop to route, check distance
4. Detect circular geometry (start/end within 10m)
5. Emit warnings (not errors) for non-critical issues
6. Add `--strict` flag to promote warnings to errors

**Files modified**: `/workspace/tools/gen_nmea/gen_nmea.js`

#### 1.3 Add $GNGSA Sentence Generation

**Problem**: Current noise model is adequate but doesn't output $GNGSA

**Good news**: `nmea.rs` already parses $GNGSA (lines 73-87)

**Implementation**:
1. Add `makeGNGSA(ts, hdop, vdop, pdop, sats)` function
2. Generate 12 dummy PRN numbers (e.g., "01,03,05,07,09,11,13,15,17,19,21,22")
3. Call after GPRMC/GGA in simulation loop
4. Add `--no-gsa` flag to disable

**Fix syntax error from original plan**:
```javascript
function makeGNGSA(ts, hdop, vdop, pdop, sats) {
  const activeSatsStr = "01,03,05,07,09,11,13,15,17,19,21,22";
  const body = `GNGSA,A,3,${activeSatsStr},${pdop.toFixed(1)},${hdop.toFixed(1)},${vdop.toFixed(1)}`;
  return `$${body}*${nmeaChecksum(body)}`;
}
```

**Note**: GSA field order is PDOP, HDOP, VDOP (lines 80-82 in nmea.rs confirm HDOP is at index -2)

**Files modified**: `/workspace/tools/gen_nmea/gen_nmea.js`

#### 1.4 Remove Debug Statement

**Bug found**: Line 643 has `console.log(route.stops);`

**Fix**: Remove or replace with `--verbose` logging

---

### Phase 2: Loop Route Support (Coordinated JS+Rust Changes)
**Goal**: Fix the documented bug at `bdd_localization.rs:771`

**This requires coordinated changes across 3 components:**

#### 2.1 Extend Binary Route Format

**Proposed format change (v3)**:
```
[Header: 32 bytes]  // +4 bytes for flags
  magic: u32
  version: u16        // 3
  node_count: u16
  stop_count: u8
  flags: u8          // NEW: bit 0 = circular
  padding: u8[2]     // was 3, now 2
  x0_cm: i32
  y0_cm: i32
  lat_avg_deg: f64
  route_length_cm: u32  // NEW: total route length for clamping
```

**Files modified**:
- `/workspace/shared/src/binfile.rs` (VERSION → 3, add flags, route_length)
- `/workspace/simulator/src/binfile.rs` (update load logic)

#### 2.2 Update Map Matcher for Circular Routes

**Problem**: `project_to_route()` doesn't clamp to route_length

**Solution**: Add route_length-aware projection

```rust
// In map_match.rs
pub fn project_to_route_circular(
    gps_x: DistCm,
    gps_y: DistCm,
    seg_idx: usize,
    route_data: &RouteData,
    route_length_cm: DistCm,
    expected_progress: DistCm,  // From Kalman state
) -> DistCm {
    let raw = project_to_route(gps_x, gps_y, seg_idx, route_data);

    // If GPS projects to near start (0) but expected is near end,
    // assume we've completed a loop and return route_length
    if raw < 1000 && expected_progress > route_length_cm - 10000 {
        route_length_cm
    } else {
        raw
    }
}
```

**Alternative (simpler)**: Use heading continuity - if heading doesn't match segment 0's heading, don't snap to segment 0.

**Files modified**:
- `/workspace/simulator/src/map_match.rs`
- `/workspace/simulator/src/kalman.rs` (use new projection function)

#### 2.3 Update Generator for Circular Routes

**Implementation**:
1. Add `detectRouteType(route_points)` function
2. If first/last points within 10m → set `circular` flag
3. When generating binary data, include flags and route_length
4. For circular routes, ensure final point has correct heading

**Files modified**:
- `/workspace/tools/gen_nmea/gen_nmea.js`
- Route preprocessor (if it generates the .bin file)

#### 2.4 Enable Ignored Test

**File**: `/workspace/simulator/tests/bdd_localization.rs`

**Action**: Remove `#[ignore]` from `scenario_loop_closure_full_route_completion` after fixes are complete

---

### Phase 3: Multi-Visit Stops (Depends on Phase 2)
**Goal**: Support routes where stops are visited multiple times

**This depends on loop route support because**:
- Multi-visit stops require lap counting
- Lap counting requires circular route handling

**Implementation** (Phase 2 complete):
1. Extend stops format:
```json
{
  "stops": [
    { "lat": 25.001, "lon": 121.001, "visit_count": 2, "name": "Central Station" }
  ],
  "route_type": "circular"
}
```

2. In ground_truth, add `lap` field:
```json
{ "stop_idx": 0, "seg_idx": 10, "timestamp": 1000, "dwell_s": 8, "lap": 0 }
{ "stop_idx": 0, "seg_idx": 10, "timestamp": 5000, "dwell_s": 8, "lap": 1 }
```

3. Add validation: total_laps >= max(visit_count)

**Files modified**: `/workspace/tools/gen_nmea/gen_nmea.js`

---

## Verification

### Phase 1 Testing

```bash
# Test config override
./tools/gen_nmea/gen_nmea.js generate \
  --route route.json \
  --config custom_config.json \
  --json

# Test validation with strict mode
./tools/gen_nmea/gen_nmea.js generate \
  --route route.json \
  --strict

# Test $GNGSA generation
./tools/gen_nmea/gen_nmea.js generate \
  --route route.json \
  --scenario normal
grep GNGSA test.nmea  # Should find $GNGSA sentences

# Test --no-gsa flag
./tools/gen_nmea/gen_nmea.js generate \
  --route route.json \
  --no-gsa
grep GNGSA test.nmea  # Should NOT find $GNGSA
```

### Phase 2 Testing

```bash
# Generate circular route test data
./tools/gen_nmea/gen_nmea.js generate \
  --route tools/data/circular_test/route.json \
  --out-nmea test_loop.nmea

# Run through simulator
cargo run --bin simulator -- test_loop.nmea output.jsonl

# Check output: final progress should be ~20000, not ~6000
jq '.s_cm' output.jsonl | tail -1

# Run BDD tests
cargo test -p simulator bdd_localization
```

### Phase 3 Testing

```bash
# Generate multi-visit stop route
./tools/gen_nmea/gen_nmea.js generate \
  --route tools/data/multi_visit/route.json \
  --out-nmea test_multi.nmea

# Verify ground_truth has lap field
jq '.lap' ground_truth.json | sort -u
# Should show: 0, 1, 2, ...
```

---

## Files to Modify Summary

| Phase | File | Changes |
|-------|------|---------|
| 1.1 | `tools/gen_nmea/gen_nmea.js` | Config merging, validation, $GNGSA |
| 2.1 | `shared/src/binfile.rs` | VERSION→3, add flags, route_length |
| 2.2 | `simulator/src/map_match.rs` | Circular-aware projection |
| 2.2 | `simulator/src/kalman.rs` | Use new projection, pass route_length |
| 2.3 | `tools/gen_nmea/gen_nmea.js` | Detect circular, pack v3 format |
| 2.4 | `simulator/tests/bdd_localization.rs` | Remove #[ignore] |
| 3 | `tools/gen_nmea/gen_nmea.js` | Multi-visit stops |

---

## Backward Compatibility

- Phase 1: Fully backward compatible (new fields optional)
- Phase 2: Binary format v2 → v3 bump required
  - Old routes continue to work (flags=0, route_length=0)
  - Simulator must handle both v2 and v3
- Phase 3: New feature, no compatibility impact

---

## Risk Assessment

| Phase | Risk | Mitigation |
|-------|------|------------|
| 1.1 | Low | Pure JS change, existing SCENARIOS as fallback |
| 1.2 | Low | Warnings only, --strict flag for CI |
| 1.3 | Low | Parser already supports $GNGSA |
| 2.1 | Medium | Version bump requires coordinated rollout |
| 2.2 | Medium | Map matcher changes need thorough testing |
| 2.3 | Low | Generator-only change |
| 3 | Low | Depends on Phase 2 stability |

---

## Success Criteria

### Phase 1
- [ ] Config overrides work via JSON and CLI flag
- [ ] Validation emits warnings for anomalous routes
- [ ] $GNGSA sentences appear in output (unless --no-gsa)
- [ ] Debug statement removed

### Phase 2
- [ ] Binary format v3 with flags and route_length
- [ ] Circular route test passes (`scenario_loop_closure_full_route_completion`)
- [ ] Progress clamps to route_length on loop completion
- [ ] Linear routes unaffected

### Phase 3
- [ ] Multi-visit stops generate correct ground_truth
- [ ] Lap field increments correctly
- [ ] Validation catches visit_count > laps mismatch

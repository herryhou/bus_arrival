# Phase 2: Localization Pipeline — Design Spec

**Date:** 2026-03-12
**Status:** Approved
**Phase:** 2 of 3 — Localization Pipeline

---

## Overview

Phase 2 implements the GPS localization pipeline that converts NMEA sentences into smoothed route progress (ŝ) and speed (v̂). This phase reads the output of Phase 1 (route_data.bin) and processes 1 Hz GPS updates through map matching, Kalman filtering, and dead-reckoning.

**Goal:** Transform raw GPS input → accurate route progress and velocity
**Input:** NMEA file or live GPS stream + route_data.bin
**Output:** JSON per GPS update with ŝ(t), v̂(t)

---

## Project Structure

```
bus_arrival/
├── shared/              # Existing, add new types
│   └── src/lib.rs       # Add GpsPoint, KalmanState, SpatialGrid, DrState
├── preprocessor/        # Existing (Phase 1)
│   └── src/grid.rs      # NEW: grid index builder for route_data.bin
└── simulator/           # NEW: host simulation binary
    ├── Cargo.toml
    └── src/
        ├── main.rs      # CLI entry point
        ├── nmea.rs      # NMEA parser
        ├── route_data.rs # Binary reader
        ├── grid.rs      # Spatial index query
        ├── map_match.rs # Heading-constrained map matching
        ├── kalman.rs    # Kalman filter
        └── output.rs    # JSON output
```

---

## Core Data Structures

### GpsPoint (shared/src/lib.rs)

```rust
/// Parsed GPS data from NMEA sentences
pub struct GpsPoint {
    pub lat: f64,            // degrees WGS84
    pub lon: f64,            // degrees WGS84
    pub heading_cdeg: HeadCdeg,  // 0.01° (0-36000)
    pub speed_cms: SpeedCms,     // cm/s
    pub hdop_x10: u16,      // HDOP × 10 (e.g., 15 = 1.5)
    pub has_fix: bool,      // valid GPS fix
}
```

### KalmanState (shared/src/lib.rs)

```rust
/// 1D Kalman filter state for route progress
pub struct KalmanState {
    pub s_cm: DistCm,     // route progress estimate (cm)
    pub v_cms: SpeedCms,  // speed estimate (cm/s)
}

impl KalmanState {
    /// Fixed-point update: Ks = 51/256 ≈ 0.20, Kv = 77/256 ≈ 0.30
    pub fn update(&mut self, z_cm: DistCm, v_gps_cms: SpeedCms) {
        let s_pred = self.s_cm + self.v_cms;  // dt = 1s
        let v_pred = self.v_cms;
        self.s_cm = s_pred + (51 * (z_cm - s_pred)) / 256;
        self.v_cms = v_pred + (77 * (v_gps_cms - v_pred)) / 256;
    }

    /// HDOP-adaptive update
    pub fn update_adaptive(&mut self, z_cm: DistCm, v_gps_cms: SpeedCms, hdop_x10: u16) {
        let ks = ks_from_hdop(hdop_x10);
        let s_pred = self.s_cm + self.v_cms;
        let v_pred = self.v_cms;
        self.s_cm = s_pred + (ks * (z_cm - s_pred)) / 256;
        self.v_cms = v_pred + (77 * (v_gps_cms - v_pred)) / 256;
    }
}

fn ks_from_hdop(hdop_x10: u16) -> i32 {
    match hdop_x10 {
        0..=20  => 77,   // HDOP ≤ 2.0 → Ks ≈ 0.30
        21..=30 => 51,   // HDOP ≤ 3.0 → Ks ≈ 0.20
        31..=50 => 26,   // HDOP ≤ 5.0 → Ks ≈ 0.10
        _       => 13,   // HDOP  > 5.0 → Ks ≈ 0.05
    }
}
```

### SpatialGrid (shared/src/lib.rs)

```rust
/// Spatial grid for O(k) map matching
pub struct SpatialGrid {
    pub cells: Vec<Vec<usize>>,  // 2D grid flattened
    pub grid_size_cm: DistCm,    // 10000 cm (100m)
    pub cols: u32,
    pub rows: u32,
    pub x0_cm: DistCm,           // grid origin X
    pub y0_cm: DistCm,           // grid origin Y
}

impl SpatialGrid {
    /// Query grid for candidate segments around a point
    pub fn query(&self, x_cm: DistCm, y_cm: DistCm) -> Vec<usize> {
        // Return segments in 3×3 cell neighborhood
        // Reduces O(N) → O(k) where k ≈ 5-15
    }
}
```

### DrState (shared/src/lib.rs)

```rust
/// Dead-reckoning state for GPS outage compensation
pub struct DrState {
    pub last_gps_time: Option<u32>,  // seconds
    pub last_valid_s: DistCm,
    pub filtered_v: SpeedCms,         // EMA smoothed speed
}

impl DrState {
    /// Update during GPS outage (max 10 seconds)
    pub fn update(&mut self, state: &mut KalmanState, dt_s: u32) {
        // ŝ(t) = ŝ(t-1) + v_filtered × dt
    }
}
```

---

## Module Specifications

### 1. NMEA Parser (simulator/src/nmea.rs)

**Supported Sentences:**
- `$GPRMC` — position, heading (true north), speed (knots), date/time
- `$GNGSA` — HDOP (horizontal dilution of precision)
- `$GPGGA` — backup (fix status, HDOP)

**Parser Functions:**
```rust
/// Parse single NMEA sentence, returns Some(GpsPoint) when complete
pub fn parse_nmea(sentence: &str, state: &mut NmeaState) -> Option<GpsPoint> {
    // Verify checksum
    // Parse sentence type
    // Extract relevant fields
    // Accumulate data across sentences (RMC + GSA)
}

/// Verify NMEA checksum
fn verify_checksum(sentence: &str) -> bool {
    // XOR all bytes between $ and *
    // Compare to hex checksum
}

/// Convert knots to cm/s: 1 knot = 0.514444 m/s = 51.44 cm/s
fn knots_to_cms(knots: f64) -> SpeedCms {
    (knots * 51.44).round() as SpeedCms
}
```

**Validation:**
- Checksum must match
- Valid fix status (not "V" in RMC)
- HDOP range 0-99

---

### 2. route_data.bin Reader (simulator/src/route_data.rs)

```rust
/// Load route data from binary file
pub fn load_route_data(path: &Path) -> Result<RouteData, io::Error> {
    // 1. Open file
    // 2. Verify magic: 0x42555341
    // 3. Read version (expect 1)
    // 4. Read header: node_count, stop_count, x0_cm, y0_cm
    // 5. Read RouteNode[] (52 bytes each)
    // 6. Read Stop[] (12 bytes each)
    // 7. Read CRC32
    // 8. Verify CRC32
}

pub struct RouteData {
    pub nodes: Vec<RouteNode>,
    pub stops: Vec<Stop>,
    pub grid_origin: GridOrigin,
    pub grid: SpatialGrid,  // built from nodes
}
```

---

### 3. Spatial Grid Index (simulator/src/grid.rs)

```rust
/// Build spatial grid from route nodes
pub fn build_spatial_grid(nodes: &[RouteNode], origin: &GridOrigin) -> SpatialGrid {
    let x_min = origin.x0_cm;
    let y_min = origin.y0_cm;
    let grid_size = 10000;  // 100m in cm

    // Determine grid dimensions from node bounds
    let x_max = nodes.iter().map(|n| n.x_cm).max().unwrap();
    let y_max = nodes.iter().map(|n| n.y_cm).max().unwrap();
    let cols = ((x_max - x_min) / grid_size + 1) as u32;
    let rows = ((y_max - y_min) / grid_size + 1) as u32;

    let mut cells = vec![vec![]; (cols * rows) as usize];

    // For each segment, add to all cells it intersects
    for i in 0..nodes.len()-1 {
        let p0 = &nodes[i];
        let p1 = &nodes[i+1];

        // Bounding box of segment
        let x_start = (p0.x_cm.min(p1.x_cm) - x_min) / grid_size;
        let x_end = (p0.x_cm.max(p1.x_cm) - x_min) / grid_size;
        let y_start = (p0.y_cm.min(p1.y_cm) - y_min) / grid_size;
        let y_end = (p0.y_cm.max(p1.y_cm) - y_min) / grid_size;

        // Add segment index to each cell in bounding box
        for gy in y_start..=y_end {
            for gx in x_start..=x_end {
                let idx = (gy * cols + gx) as usize;
                cells[idx].push(i);
            }
        }
    }

    SpatialGrid { cells, grid_size_cm: grid_size, cols, rows, x0_cm: x_min, y0_cm: y_min }
}
```

---

### 4. Heading-Constrained Map Matching (simulator/src/map_match.rs)

```rust
/// Find best route segment for GPS point
pub fn find_best_segment(
    gps: &GpsPoint,
    grid: &SpatialGrid,
    nodes: &[RouteNode],
) -> (usize, DistCm) {
    // 1. Grid query → candidate segments
    let candidates = grid.query(gps.x_cm, gps.y_cm);

    // 2. Find segment with minimum score
    let mut best_idx = 0;
    let mut best_score = i64::MAX;

    for &idx in &candidates {
        let score = segment_score(gps, &nodes[idx]);
        if score < best_score {
            best_score = score;
            best_idx = idx;
        }
    }

    (best_idx, best_score)
}

/// Heading-weighted segment score
fn segment_score(gps: &GpsPoint, seg: &RouteNode) -> i64 {
    // Distance squared to segment
    let dist2 = distance_to_segment_squared(gps, seg);

    // Heading penalty with speed ramp
    let heading_diff = heading_diff_cdeg(gps.heading_cdeg, seg.heading_cdeg);
    let w = heading_weight(gps.speed_cms);
    let penalty = (heading_diff.pow(2) as i64 * w) >> 8;

    dist2 + penalty
}

/// Heading weight: 0 at v=0, 256 at v≥83 cm/s (3 km/h)
fn heading_weight(v_cms: SpeedCms) -> i32 {
    ((v_cms * 256) / 83).min(256)
}
```

---

### 5. Segment Projection (simulator/src/map_match.rs)

```rust
/// Project GPS onto selected segment → route progress
pub fn project_to_route(gps_x: DistCm, gps_y: DistCm, seg: &RouteNode) -> DistCm {
    // t = dot(gps - P[i], segment) / len2
    let dx = gps_x - seg.x_cm;
    let dy = gps_y - seg.y_cm;
    let t_num = (dx as i64 * seg.dx_cm as i64 + dy as i64 * seg.dy_cm as i64);

    // Clamp t to [0, 1]
    let t = if t_num < 0 { 0 } else if t_num > seg.len2_cm2 { seg.len2_cm2 } else { t_num };

    // z = cum_dist[i] + t × seg_len / len2
    let base = seg.cum_dist_cm - seg.seg_len_cm;
    base + ((t as i64 * seg.seg_len_cm as i64 / seg.len2_cm2) as DistCm)
}
```

---

### 6. Speed Constraint Filter (simulator/src/map_match.rs)

```rust
/// Maximum feasible distance in 1 second
const D_MAX_CM: DistCm = 3667;  // V_max(1667 cm/s) + σ_gps(2000 cm)

/// Reject GPS updates that exceed physical limits
pub fn check_speed_constraint(z_new: DistCm, z_prev: DistCm) -> bool {
    (z_new - z_prev).unsigned_abs() <= D_MAX_CM
}

/// Monotonicity constraint with noise tolerance
pub fn check_monotonic(z_new: DistCm, z_prev: DistCm) -> bool {
    z_new >= z_prev - 1000  // allow -10m GPS noise
}
```

---

### 7. Kalman Filter (simulator/src/kalman.rs)

```rust
/// Main processing pipeline for each GPS update
pub fn process_gps_update(
    state: &mut KalmanState,
    dr: &mut DrState,
    gps: &GpsPoint,
    route_data: &RouteData,
) -> ProcessResult {
    // 1. Check for GPS outage
    if !gps.has_fix {
        return handle_outage(state, dr);
    }

    // 2. Map matching + projection
    let (seg_idx, _) = find_best_segment(gps, &route_data.grid, &route_data.nodes);
    let z_raw = project_to_route(gps.x_cm, gps.y_cm, &route_data.nodes[seg_idx]);

    // 3. Speed constraint filter
    if !check_speed_constraint(z_raw, state.s_cm) {
        return ProcessResult::Rejected("speed constraint");
    }

    // 4. Monotonicity filter
    if !check_monotonic(z_raw, state.s_cm) {
        return ProcessResult::Rejected("monotonicity");
    }

    // 5. Kalman update (HDOP-adaptive)
    state.update_adaptive(z_raw, gps.speed_cms, gps.hdop_x10);

    // 6. Update DR state
    dr.last_gps_time = Some(current_time());
    dr.last_valid_s = state.s_cm;
    dr.filtered_v = state.v_cms;

    ProcessResult::Valid { s_cm: state.s_cm, v_cms: state.v_cms }
}

pub enum ProcessResult {
    Valid { s_cm: DistCm, v_cms: SpeedCms },
    Rejected(&'static str),
    Outage,
}
```

---

### 8. Dead-Reckoning (simulator/src/kalman.rs)

```rust
/// Handle GPS outage (max 10 seconds)
fn handle_outage(state: &mut KalmanState, dr: &mut DrState) -> ProcessResult {
    let now = current_time();
    let dt = match dr.last_gps_time {
        Some(t) => now - t,
        None => return ProcessResult::Rejected("no previous fix"),
    };

    if dt > 10 {
        return ProcessResult::Outage;  // Exceeded max DR duration
    }

    // Dead-reckoning: ŝ(t) = ŝ(t-1) + v_filtered × dt
    state.s_cm = dr.last_valid_s + dr.filtered_v * (dt as DistCm);
    // Speed decays during outage
    dr.filtered_v = dr.filtered_v * 9 / 10;

    ProcessResult::Dr { s_cm: state.s_cm, v_cms: state.v_cms }
}
```

---

## Output Format

**JSON per GPS update:**
```jsonl
{"time": 1234567890, "s_cm": 123456, "v_cms": 567, "status": "valid", "seg_idx": 42}
{"time": 1234567891, "s_cm": 124023, "v_cms": 580, "status": "valid", "seg_idx": 42}
{"time": 1234567892, "s_cm": 124590, "v_cms": 0, "status": "at_stop", "seg_idx": 43}
{"time": 1234567893, "s_cm": 124590, "v_cms": 0, "status": "dr_outage", "seg_idx": null}
```

**Status values:**
- `valid` — Normal GPS update accepted
- `rejected` — GPS update filtered (speed/monotonicity constraint)
- `dr_outage` — Dead-reckoning active

---

## CLI Interface

```bash
# Run localization on NMEA file
cargo run --bin simulator -- localize test.nmea route_data.bin output.jsonl

# With verbose mode
cargo run --bin simulator -- localize test.nmea route_data.bin output.jsonl --verbose
```

---

## Validation Criteria

- [ ] Parses all NMEA sentences in test.nmea
- [ ] Loads route_data.bin with correct magic/headers
- [ ] Grid index returns 5-15 candidates per query
- [ ] Map matching selects correct segment
- [ ] Kalman output is smooth (no jumps > 1m between valid updates)
- [ ] Dead-reckoning works for up to 10 seconds
- [ ] Output JSON matches ground_truth.json format (when available)

---

## Next Phase

Phase 3 will consume ŝ(t), v̂(t) and implement:
- Stop Corridor Filter
- Stop Probability Model (4-feature Bayesian)
- Stop State Machine (FSM)
- Arrival Event Output

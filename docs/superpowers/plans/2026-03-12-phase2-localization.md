# Phase 2: Localization Pipeline — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Rust host simulator that reads NMEA sentences and route_data.bin, processes them through map matching and Kalman filtering, and outputs JSON with ŝ(t), v̂(t).

**Architecture:** Pipeline: NMEA → GPS Point → Grid Index Lookup → Map Matching → Projection → Filtering → Kalman → Dead-Reckoning → JSON Output

**Tech Stack:** Rust 2021 edition, existing shared crate, new simulator binary

---

## Chunk 1: Shared Types & Simulator Setup

**Goal:** Add new types to shared crate and create simulator binary scaffold.

---

### Task 1: Add GpsPoint and Related Types to Shared

**Files:**
- Modify: `shared/src/lib.rs`

- [ ] **Step 1: Add GpsPoint struct to shared/src/lib.rs**

```rust
/// Parsed GPS data from NMEA sentences
#[derive(Debug, Clone)]
pub struct GpsPoint {
    pub lat: f64,              // degrees WGS84
    pub lon: f64,              // degrees WGS84
    pub heading_cdeg: HeadCdeg, // 0.01° (0-36000)
    pub speed_cms: SpeedCms,   // cm/s
    pub hdop_x10: u16,         // HDOP × 10
    pub has_fix: bool,         // valid GPS fix
}

impl GpsPoint {
    pub fn new() -> Self {
        GpsPoint {
            lat: 0.0,
            lon: 0.0,
            heading_cdeg: 0,
            speed_cms: 0,
            hdop_x10: 0,
            has_fix: false,
        }
    }
}
```

- [ ] **Step 2: Run cargo test to verify**

Run: `cargo test -p shared`
Expected: Compiles without errors

- [ ] **Step 3: Commit**

```bash
git add shared/src/lib.rs && git commit -m "feat(shared): add GpsPoint struct for NMEA data"
```

---

### Task 2: Add KalmanState to Shared

**Files:**
- Modify: `shared/src/lib.rs`

- [ ] **Step 1: Add KalmanState struct with update methods**

```rust
/// 1D Kalman filter state for route progress
#[derive(Debug, Clone)]
pub struct KalmanState {
    pub s_cm: DistCm,     // route progress estimate (cm)
    pub v_cms: SpeedCms,  // speed estimate (cm/s)
}

impl KalmanState {
    pub fn new() -> Self {
        KalmanState { s_cm: 0, v_cms: 0 }
    }

    /// Fixed-point update: Ks = 51/256 ≈ 0.20, Kv = 77/256 ≈ 0.30
    pub fn update(&mut self, z_cm: DistCm, v_gps_cms: SpeedCms) {
        let s_pred = self.s_cm + self.v_cms;
        let v_pred = self.v_cms;
        self.s_cm = s_pred + (51 * (z_cm - s_pred)) / 256;
        self.v_cms = v_pred + (77 * (v_gps_cms - v_pred)) / 256;
    }

    fn ks_from_hdop(hdop_x10: u16) -> i32 {
        match hdop_x10 {
            0..=20  => 77,   // HDOP ≤ 2.0 → Ks ≈ 0.30
            21..=30 => 51,   // HDOP ≤ 3.0 → Ks ≈ 0.20
            31..=50 => 26,   // HDOP ≤ 5.0 → Ks ≈ 0.10
            _       => 13,   // HDOP  > 5.0 → Ks ≈ 0.05
        }
    }

    /// HDOP-adaptive Kalman update
    pub fn update_adaptive(&mut self, z_cm: DistCm, v_gps_cms: SpeedCms, hdop_x10: u16) {
        let ks = Self::ks_from_hdop(hdop_x10);
        let s_pred = self.s_cm + self.v_cms;
        let v_pred = self.v_cms;
        self.s_cm = s_pred + (ks * (z_cm - s_pred)) / 256;
        self.v_cms = v_pred + (77 * (v_gps_cms - v_pred)) / 256;
    }
}
```

- [ ] **Step 2: Add tests for KalmanState**

```rust
#[cfg(test)]
mod tests {
    // ... existing tests ...

    #[test]
    fn kalman_initial_state() {
        let state = KalmanState::new();
        assert_eq!(state.s_cm, 0);
        assert_eq!(state.v_cms, 0);
    }

    #[test]
    fn kalman_update_basic() {
        let mut state = KalmanState::new();
        state.update(10000, 500); // z=100cm, v=5m/s
        assert!(state.s_cm > 0);
        assert!(state.v_cms > 0);
    }

    #[test]
    fn kalman_smoothing() {
        let mut state = KalmanState::new();
        state.update(10000, 500);
        let s1 = state.s_cm;
        state.update(10100, 500); // +100cm raw
        let s2 = state.s_cm;
        // Smoothed increase should be less than raw increase
        assert!((s2 - s1) < 100);
    }
}
```

- [ ] **Step 3: Run tests to verify**

Run: `cargo test -p shared`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add shared/src/lib.rs && git commit -m "feat(shared): add KalmanState with adaptive update"
```

---

### Task 3: Add SpatialGrid and DrState to Shared

**Files:**
- Modify: `shared/src/lib.rs`

- [ ] **Step 1: Add SpatialGrid and DrState structs**

```rust
/// Spatial grid for O(k) map matching
#[derive(Debug)]
pub struct SpatialGrid {
    pub cells: Vec<Vec<usize>>,  // 2D grid flattened
    pub grid_size_cm: DistCm,    // 10000 cm (100m)
    pub cols: u32,
    pub rows: u32,
    pub x0_cm: DistCm,           // grid origin X
    pub y0_cm: DistCm,           // grid origin Y
}

/// Dead-reckoning state for GPS outage compensation
#[derive(Debug)]
pub struct DrState {
    pub last_gps_time: Option<u64>,  // seconds since epoch
    pub last_valid_s: DistCm,
    pub filtered_v: SpeedCms,         // EMA smoothed speed
}

impl DrState {
    pub fn new() -> Self {
        DrState {
            last_gps_time: None,
            last_valid_s: 0,
            filtered_v: 0,
        }
    }
}
```

- [ ] **Step 2: Export new types from shared**

```rust
pub use {RouteNode, Stop, GridOrigin};
pub use {DistCm, SpeedCms, HeadCdeg, Prob8, Dist2};
pub use {GpsPoint, KalmanState, SpatialGrid, DrState};
```

- [ ] **Step 3: Run tests to verify**

Run: `cargo test -p shared`
Expected: All tests pass, types are exported

- [ ] **Step 4: Commit**

```bash
git add shared/src/lib.rs && git commit -m "feat(shared): add SpatialGrid and DrState types"
```

---

### Task 4: Create Simulator Binary Scaffold

**Files:**
- Create: `simulator/Cargo.toml`
- Create: `simulator/src/main.rs`

- [ ] **Step 1: Create simulator/Cargo.toml**

```toml
[package]
name = "simulator"
version.workspace = true
edition.workspace = true

[[bin]]
name = "simulator"
path = "src/main.rs"

[dependencies]
shared = { path = "../shared" }
serde = { workspace = true }
serde_json = { workspace = true }
crc32fast = { workspace = true }
```

- [ ] **Step 2: Create simulator/src/main.rs with CLI**

```rust
use std::env;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 4 {
        eprintln!("Usage: simulator <nmea_file> <route_data.bin> <output.jsonl>");
        eprintln!();
        eprintln!("Arguments:");
        eprintln!("  nmea_file       - NMEA input file");
        eprintln!("  route_data.bin  - Binary route data from Phase 1");
        eprintln!("  output.jsonl   - JSON output (one line per GPS update)");
        std::process::exit(1);
    }

    let nmea_path = PathBuf::from(&args[1]);
    let route_path = PathBuf::from(&args[2]);
    let output_path = PathBuf::from(&args[3]);

    println!("Phase 2: Localization Pipeline");
    println!("  NMEA input:   {}", nmea_path.display());
    println!("  Route data:   {}", route_path.display());
    println!("  Output:       {}", output_path.display());
    println!();
    println!("TODO: Implement pipeline");
}
```

- [ ] **Step 3: Run to verify CLI**

Run: `cargo run --bin simulator -- test.nmea route_data.bin output.jsonl`
Expected: Prints info without error

- [ ] **Step 4: Commit**

```bash
git add simulator/ && git commit -m "feat(simulator): add CLI scaffold"
```

---

## Chunk 2: route_data.bin Reader

**Goal:** Load binary route data with validation.

---

### Task 5: Implement Binary Route Data Reader

**Files:**
- Create: `simulator/src/route_data.rs`

- [ ] **Step 1: Add binary reader functions**

```rust
//! Binary route data reader

use shared::{RouteNode, Stop, GridOrigin, SpatialGrid};
use std::io::{self, Read};
use std::fs::File;
use std::path::Path;

pub const MAGIC: u32 = 0x42555341;
pub const VERSION: u16 = 1;

pub struct RouteData {
    pub nodes: Vec<RouteNode>,
    pub stops: Vec<Stop>,
    pub grid_origin: GridOrigin,
    pub grid: SpatialGrid,
}

/// Load route data from binary file
pub fn load_route_data(path: &Path) -> Result<RouteData, io::Error> {
    let mut file = File::open(path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    // Verify magic
    let magic = u32::from_le_bytes(buffer[0..4].try_into().unwrap());
    if magic != MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Invalid magic: {:08x}", magic),
        ));
    }

    // Read version
    let version = u16::from_le_bytes(buffer[4..6].try_into().unwrap());
    if version != VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Unsupported version: {}", version),
        ));
    }

    // Read header
    let node_count = u16::from_le_bytes(buffer[6..8].try_into().unwrap()) as usize;
    let stop_count = buffer[8] as usize;
    let x0_cm = i32::from_le_bytes(buffer[9..13].try_into().unwrap());
    let y0_cm = i32::from_le_bytes(buffer[13..17].try_into().unwrap());

    let grid_origin = GridOrigin { x0_cm, y0_cm };

    // Read route nodes (52 bytes each)
    let mut nodes = Vec::with_capacity(node_count);
    let mut offset = 17;
    for _ in 0..node_count {
        let node_bytes = &buffer[offset..offset + 52];
        unsafe {
            let node_ptr = node_bytes.as_ptr() as *const RouteNode;
            nodes.push(std::ptr::read(node_ptr));
        }
        offset += 52;
    }

    // Read stops (12 bytes each)
    let mut stops = Vec::with_capacity(stop_count);
    for _ in 0..stop_count {
        let stop_bytes = &buffer[offset..offset + 12];
        unsafe {
            let stop_ptr = stop_bytes.as_ptr() as *const Stop;
            stops.push(std::ptr::read(stop_ptr));
        }
        offset += 12;
    }

    // TODO: Read CRC32 and verify
    // TODO: Build spatial grid from nodes

    Ok(RouteData {
        nodes,
        stops,
        grid_origin,
        grid: SpatialGrid::empty(),
    })
}

impl SpatialGrid {
    fn empty() -> Self {
        SpatialGrid {
            cells: vec![],
            grid_size_cm: 10000,
            cols: 0,
            rows: 0,
            x0_cm: 0,
            y0_cm: 0,
        }
    }
}
```

- [ ] **Step 2: Add mod route_data to main.rs**

```rust
mod route_data;
```

- [ ] **Step 3: Run cargo check to verify**

Run: `cargo check -p simulator`
Expected: Compiles without errors

- [ ] **Step 4: Commit**

```bash
git add simulator/src/route_data.rs simulator/src/main.rs && git commit -m "feat(simulator): add route_data.bin reader"
```

---

## Chunk 3: NMEA Parser

**Goal:** Parse NMEA sentences into GpsPoint.

---

### Task 6: Implement NMEA Parser

**Files:**
- Create: `simulator/src/nmea.rs`

- [ ] **Step 1: Write NMEA parser module**

```rust
//! NMEA sentence parser

use shared::{GpsPoint, SpeedCms, HeadCdeg};

/// NMEA parser state (accumulates data across sentences)
pub struct NmeaState {
    point: GpsPoint,
}

impl NmeaState {
    pub fn new() -> Self {
        NmeaState {
            point: GpsPoint::new(),
        }
    }

    /// Parse NMEA sentence, returns Some(GpsPoint) when complete
    pub fn parse_sentence(&mut self, sentence: &str) -> Option<GpsPoint> {
        if !verify_checksum(sentence) {
            return None;
        }

        let parts: Vec<&str> = sentence.split(',').collect();

        if parts.is_empty() {
            return None;
        }

        match parts[0] {
            "$GPRMC" => self.parse_rmc(&parts),
            "$GNGSA" => self.parse_gsa(&parts),
            "$GPGGA" => self.parse_gga(&parts),
            _ => None,
        }
    }

    fn parse_rmc(&mut self, parts: &[&str]) -> Option<GpsPoint> {
        // $GPRMC,123519,v,ddmm.mm,s,dddmm.mm,a,hhh.h,V,V,ddmmyy,,,A*hh
        if parts.len() < 12 {
            return None;
        }

        // Status 'V' = Warning, 'A' = Valid
        let status = parts[2];
        if status != "A" {
            return None;
        }

        // Parse position
        let lat = parse_lat(parts[3], parts[4])?;
        let lon = parse_lon(parts[5], parts[6])?;
        let speed_knots: f64 = parts[7].parse().unwrap_or(0.0);
        let heading_deg: f64 = parts[8].parse().unwrap_or(0.0);

        self.point.lat = lat;
        self.point.lon = lon;
        self.point.speed_cms = knots_to_cms(speed_knots);
        self.point.heading_cdeg = (heading_deg * 100.0).round() as HeadCdeg;
        self.point.has_fix = true;

        None // Not complete yet (need HDOP)
    }

    fn parse_gsa(&mut self, parts: &[&str]) -> Option<GpsPoint> {
        // $GNGSA,A,3,04,05,07,08,09,10,11,12,1.0,1.0,1.0*hh
        if parts.len() < 16 {
            return None;
        }

        // HDOP is at index 15
        let hdop: f64 = parts[15].parse().unwrap_or(99.0);
        self.point.hdop_x10 = (hdop * 10.0).round() as u16;

        // Return complete point
        Some(std::mem::replace(&mut self.point, GpsPoint::new()))
    }

    fn parse_gga(&mut self, parts: &[&str]) -> Option<GpsPoint> {
        // $GPGGA,123519,v,ddmm.mm,s,dddmm.mm,a,xx,yy,z.z,h.h,M*hh
        if parts.len() < 7 {
            return None;
        }

        // Quality indicator
        if parts[6] != "1" && parts[6] != "2" {
            return None;
        }

        let lat = parse_lat(parts[2], parts[3])?;
        let lon = parse_lon(parts[4], parts[5])?;

        self.point.lat = lat;
        self.point.lon = lon;
        self.point.has_fix = true;

        None
    }
}

/// Verify NMEA checksum
fn verify_checksum(sentence: &str) -> bool {
    if let Some(star_pos) = sentence.find('*') {
        let data = &sentence[1..star_pos];
        let checksum_str = &sentence[star_pos + 1..star_pos + 3];
        if let Ok(checksum) = u8::from_str_radix(checksum_str, 16) {
            let calculated = data.bytes().fold(0u8, |acc, b| acc ^ b);
            calculated == checksum
        } else {
            false
        }
    } else {
        false
    }
}

/// Parse latitude from NMEA format (ddmm.mmmm)
fn parse_lat(deg_min: &str, ns: &str) -> Option<f64> {
    let dm: f64 = deg_min.parse().ok()?;
    let degrees = (dm / 100.0).trunc() + (dm % 100.0) / 60.0;
    Some(if ns == "N" { degrees } else { -degrees })
}

/// Parse longitude from NMEA format (dddmm.mmmm)
fn parse_lon(deg_min: &str, ew: &str) -> Option<f64> {
    let dm: f64 = deg_min.parse().ok()?;
    let degrees = (dm / 100.0).trunc() + (dm % 100.0) / 60.0;
    Some(if ew == "E" { degrees } else { -degrees })
}

/// Convert knots to cm/s: 1 knot = 51.44 cm/s
fn knots_to_cms(knots: f64) -> SpeedCms {
    (knots * 51.44).round() as SpeedCms
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_checksum_valid() {
        assert!(verify_checksum("$GPRMC,123519,V,0000.0000,N,00000.0000,E,000.0,000.0,030311,,,A*57"));
    }

    #[test]
    fn parse_lat_north() {
        assert_eq!(parse_lat("2502.5434", "N").unwrap(), 25.04239);
    }

    #[test]
    fn knots_to_cms_conversion() {
        assert_eq!(knots_to_cms(10.0), 514); // ~5.1 m/s = 514 cm/s
    }
}
```

- [ ] **Step 2: Add mod nmea to main.rs**

```rust
mod nmea;
```

- [ ] **Step 3: Run tests to verify**

Run: `cargo test -p simulator`
Expected: All NMEA parser tests pass

- [ ] **Step 4: Commit**

```bash
git add simulator/src/nmea.rs simulator/src/main.rs && git commit -m "feat(simulator): add NMEA parser"
```

---

## Chunk 4: Spatial Grid Index

**Goal:** Build spatial grid for O(k) map matching.

---

### Task 7: Implement Spatial Grid Builder

**Files:**
- Create: `simulator/src/grid.rs`

- [ ] **Step 1: Implement spatial grid builder**

```rust
//! Spatial grid index for O(k) map matching

use shared::{RouteNode, GridOrigin, DistCm, SpatialGrid};

pub const GRID_SIZE_CM: DistCm = 10000;  // 100m

/// Build spatial grid from route nodes
pub fn build_spatial_grid(nodes: &[RouteNode], origin: &GridOrigin) -> SpatialGrid {
    if nodes.is_empty() {
        return SpatialGrid::empty();
    }

    let x_min = origin.x0_cm;
    let y_min = origin.y0_cm;

    // Calculate grid dimensions
    let x_max = nodes.iter().map(|n| n.x_cm).max().unwrap();
    let y_max = nodes.iter().map(|n| n.y_cm).max().unwrap();

    let cols = ((x_max - x_min) / GRID_SIZE_CM + 1) as u32;
    let rows = ((y_max - y_min) / GRID_SIZE_CM + 1) as u32;
    let total_cells = (cols * rows) as usize;

    let mut cells: Vec<Vec<usize>> = vec![Vec::new(); total_cells];

    // For each segment, add to intersecting cells
    for i in 0..nodes.len().saturating_sub(1) {
        let p0 = &nodes[i];
        let p1 = &nodes[i + 1];

        // Segment bounding box
        let x_start = ((p0.x_cm.min(p1.x_cm) - x_min) / GRID_SIZE_CM) as usize;
        let x_end = ((p0.x_cm.max(p1.x_cm) - x_min) / GRID_SIZE_CM) as usize;
        let y_start = ((p0.y_cm.min(p1.y_cm) - y_min) / GRID_SIZE_CM) as usize;
        let y_end = ((p0.y_cm.max(p1.y_cm) - y_min) / GRID_SIZE_CM) as usize;

        // Add segment to each cell in bounding box
        for gy in y_start..=y_end.min(rows as usize - 1) {
            for gx in x_start..=x_end.min(cols as usize - 1) {
                let idx = gy * (cols as usize) + gx;
                cells[idx].push(i);
            }
        }
    }

    SpatialGrid {
        cells,
        grid_size_cm: GRID_SIZE_CM,
        cols,
        rows,
        x0_cm: x_min,
        y0_cm: y_min,
    }
}

impl SpatialGrid {
    pub fn empty() -> Self {
        SpatialGrid {
            cells: vec![vec![]],
            grid_size_cm: GRID_SIZE_CM,
            cols: 0,
            rows: 0,
            x0_cm: 0,
            y0_cm: 0,
        }
    }

    /// Query grid for candidate segments around a point
    pub fn query(&self, x_cm: DistCm, y_cm: DistCm) -> Vec<usize> {
        if self.cols == 0 || self.rows == 0 {
            return Vec::new();
        }

        let gx = ((x_cm - self.x0_cm) / self.grid_size_cm) as usize;
        let gy = ((y_cm - self.y0_cm) / self.grid_size_cm) as usize;

        let mut candidates = Vec::new();

        // 3×3 neighborhood
        for dy in 0..=2 {
            for dx in 0..=2 {
                let ny = (gy as i32 + dy - 1) as usize;
                let nx = (gx as i32 + dx - 1) as usize;

                if ny < self.rows as usize && nx < self.cols as usize {
                    let idx = ny * (self.cols as usize) + nx;
                    candidates.extend_from_slice(&self.cells[idx]);
                }
            }
        }

        candidates
    }
}
```

- [ ] **Step 2: Integrate grid builder into route_data.rs**

```rust
use crate::grid::build_spatial_grid;

// In load_route_data, after reading stops:
// Build spatial grid from nodes
let grid = build_spatial_grid(&nodes, &grid_origin);

Ok(RouteData {
    nodes,
    stops,
    grid_origin,
    grid,
})
```

- [ ] **Step 3: Add tests for spatial grid**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_grid() {
        let grid = SpatialGrid::empty();
        assert!(grid.query(0, 0).is_empty());
    }

    #[test]
    fn grid_single_segment() {
        let origin = GridOrigin { x0_cm: 0, y0_cm: 0 };
        let nodes = vec![
            RouteNode::test_node(0, 0, 0),
            RouteNode::test_node(10000, 0, 10000),
        ];

        let grid = build_spatial_grid(&nodes, &origin);
        assert!(!grid.query(5000, 0).is_empty());
    }
}
```

- [ ] **Step 4: Run tests to verify**

Run: `cargo test -p simulator`
Expected: All grid tests pass

- [ ] **Step 5: Commit**

```bash
git add simulator/src/grid.rs simulator/src/route_data.rs && git commit -m "feat(simulator): add spatial grid index"
```

---

## Chunk 5: Map Matching & Projection

**Goal:** Find best segment and project GPS to route progress.

---

### Task 8: Implement Map Matching

**Files:**
- Create: `simulator/src/map_match.rs`

- [ ] **Step 1: Implement map matching logic**

```rust
//! Heading-constrained map matching

use shared::{RouteNode, GpsPoint, HeadCdeg, Dist2, DistCm};

/// Find best route segment for GPS point
pub fn find_best_segment(
    gps_x: DistCm,
    gps_y: DistCm,
    gps_heading: HeadCdeg,
    gps_speed: SpeedCms,
    grid: &shared::SpatialGrid,
    nodes: &[RouteNode],
) -> usize {
    let candidates = grid.query(gps_x, gps_y);

    if candidates.is_empty() {
        return 0;
    }

    let mut best_idx = candidates[0];
    let mut best_score = i64::MAX;

    for &idx in &candidates {
        let seg = &nodes[idx];
        let score = segment_score(gps_x, gps_y, gps_heading, gps_speed, seg);
        if score < best_score {
            best_score = score;
            best_idx = idx;
        }
    }

    best_idx
}

/// Heading-weighted segment score
fn segment_score(
    gps_x: DistCm,
    gps_y: DistCm,
    gps_heading: HeadCdeg,
    gps_speed: SpeedCms,
    seg: &RouteNode,
) -> i64 {
    // Distance squared to segment
    let dist2 = distance_to_segment_squared(gps_x, gps_y, seg);

    // Heading penalty with speed ramp
    let heading_diff = heading_diff_cdeg(gps_heading, seg.heading_cdeg);
    let w = heading_weight(gps_speed);
    let penalty = (heading_diff.pow(2) as i64 * w as i64) >> 8;

    dist2 + penalty
}

/// Heading weight: 0 at v=0, 256 at v≥83 cm/s (3 km/h)
fn heading_weight(v_cms: SpeedCms) -> i32 {
    ((v_cms * 256) / 83).min(256)
}

/// Calculate heading difference (shortest around 360°)
fn heading_diff_cdeg(a: HeadCdeg, b: HeadCdeg) -> HeadCdeg {
    let diff = (a as i32 - b as i32).unsigned_abs() % 36000;
    if diff > 18000 { 36000 - diff } else { diff as HeadCdeg }
}

/// Distance squared from point to segment (clamped projection)
fn distance_to_segment_squared(x: DistCm, y: DistCm, seg: &RouteNode) -> Dist2 {
    let dx = x - seg.x_cm;
    let dy = y - seg.y_cm;

    // t = dot(point - P[i], segment) / |segment|²
    let t_num = (dx as i64 * seg.dx_cm as i64 + dy as i64 * seg.dy_cm as i64);
    let t = if t_num < 0 { 0 } else if t_num > seg.len2_cm2 { seg.len2_cm2 } else { t_num };

    // Projected point
    let px = seg.x_cm + ((t * seg.dx_cm as i64 / seg.len2_cm2) as DistCm);
    let py = seg.y_cm + ((t * seg.dy_cm as i64 / seg.len2_cm2) as DistCm);

    // Distance squared
    ((x - px) as i64).pow(2) + ((y - py) as i64).pow(2)
}
```

- [ ] **Step 2: Add segment projection function**

```rust
/// Project GPS point onto segment → route progress
pub fn project_to_route(
    gps_x: DistCm,
    gps_y: DistCm,
    seg_idx: usize,
    nodes: &[RouteNode],
) -> DistCm {
    let seg = &nodes[seg_idx];

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

/// Convert lat/lon to relative cm coordinates
pub fn latlon_to_cm_relative(lat: f64, lon: f64, x0_cm: i64, y0_cm: i64) -> (DistCm, DistCm) {
    const R_CM: f64 = 637_100_000.0;
    const FIXED_ORIGIN_LAT_DEG: f64 = 20.0;

    let lat_rad = lat.to_radians();
    let lon_rad = lon.to_radians();
    let lat_avg_rad = FIXED_ORIGIN_LAT_DEG.to_radians();

    let y_abs = (lat_rad * R_CM).round() as i64;
    let x_abs = (lon_rad * lat_avg_rad.cos() * R_CM).round() as i64;

    ((x_abs - x0_cm) as DistCm, (y_abs - y0_cm) as DistCm)
}
```

- [ ] **Step 3: Add mod map_match to main.rs and test**

```rust
mod map_match;
```

- [ ] **Step 4: Run tests to verify**

Run: `cargo test -p simulator`
Expected: All map matching tests pass

- [ ] **Step 5: Commit**

```bash
git add simulator/src/map_match.rs simulator/src/main.rs && git commit -m "feat(simulator): add map matching and projection"
```

---

## Chunk 6: Filtering & Kalman

**Goal:** Apply speed/monotonicity constraints and Kalman smoothing.

---

### Task 9: Implement Kalman Filter Module

**Files:**
- Create: `simulator/src/kalman.rs`

- [ ] **Step 1: Implement filtering and Kalman processing**

```rust
//! Kalman filter and GPS processing pipeline

use shared::{KalmanState, DrState, GpsPoint, RouteData, DistCm, SpeedCms};

/// Maximum feasible distance in 1 second
pub const D_MAX_CM: DistCm = 3667;  // V_max(1667 cm/s) + σ_gps(2000 cm)

/// ProcessResult from GPS update
pub enum ProcessResult {
    Valid { s_cm: DistCm, v_cms: SpeedCms, seg_idx: usize },
    Rejected(&'static str),
    Outage,
}

/// Main processing pipeline for each GPS update
pub fn process_gps_update(
    state: &mut KalmanState,
    dr: &mut DrState,
    gps: &GpsPoint,
    route_data: &RouteData,
    current_time: u64,
) -> ProcessResult {
    // 1. Check for GPS outage
    if !gps.has_fix {
        return handle_outage(state, dr, current_time);
    }

    // 2. Convert GPS to relative coordinates
    let (gps_x, gps_y) = crate::map_match::latlon_to_cm_relative(
        gps.lat,
        gps.lon,
        route_data.grid_origin.x0_cm as i64,
        route_data.grid_origin.y0_cm as i64,
    );

    // 3. Map matching
    let seg_idx = crate::map_match::find_best_segment(
        gps_x,
        gps_y,
        gps.heading_cdeg,
        gps.speed_cms,
        &route_data.grid,
        &route_data.nodes,
    );

    // 4. Projection
    let z_raw = crate::map_match::project_to_route(
        gps_x,
        gps_y,
        seg_idx,
        &route_data.nodes,
    );

    // 5. Speed constraint filter
    if !check_speed_constraint(z_raw, state.s_cm) {
        return ProcessResult::Rejected("speed constraint");
    }

    // 6. Monotonicity filter
    if !check_monotonic(z_raw, state.s_cm) {
        return ProcessResult::Rejected("monotonicity");
    }

    // 7. Kalman update (HDOP-adaptive)
    state.update_adaptive(z_raw, gps.speed_cms, gps.hdop_x10);

    // 8. Update DR state
    dr.last_gps_time = Some(current_time);
    dr.last_valid_s = state.s_cm;
    dr.filtered_v = state.v_cms;

    ProcessResult::Valid {
        s_cm: state.s_cm,
        v_cms: state.v_cms,
        seg_idx,
    }
}

/// Reject GPS updates that exceed physical limits
fn check_speed_constraint(z_new: DistCm, z_prev: DistCm) -> bool {
    (z_new - z_prev).unsigned_abs() <= D_MAX_CM
}

/// Monotonicity constraint with noise tolerance
fn check_monotonic(z_new: DistCm, z_prev: DistCm) -> bool {
    z_new >= z_prev - 1000  // allow -10m GPS noise
}

/// Handle GPS outage (max 10 seconds)
fn handle_outage(
    state: &mut KalmanState,
    dr: &mut DrState,
    current_time: u64,
) -> ProcessResult {
    let dt = match dr.last_gps_time {
        Some(t) => current_time.saturating_sub(t),
        None => return ProcessResult::Rejected("no previous fix"),
    };

    if dt > 10 {
        return ProcessResult::Outage;
    }

    // Dead-reckoning: ŝ(t) = ŝ(t-1) + v_filtered × dt
    state.s_cm = dr.last_valid_s + dr.filtered_v * (dt as DistCm);
    // Speed decays during outage
    dr.filtered_v = dr.filtered_v * 9 / 10;

    ProcessResult::DrOutage {
        s_cm: state.s_cm,
        v_cms: state.v_cms,
    }
}
```

- [ ] **Step 2: Add mod kalman to main.rs**

```rust
mod kalman;
```

- [ ] **Step 3: Run tests to verify**

Run: `cargo test -p simulator`
Expected: All Kalman tests pass

- [ ] **Step 4: Commit**

```bash
git add simulator/src/kalman.rs simulator/src/main.rs && git commit -m "feat(simulator): add Kalman filter and processing pipeline"
```

---

## Chunk 7: Output & Integration

**Goal:** Complete pipeline integration and JSON output.

---

### Task 10: Implement Output Module

**Files:**
- Create: `simulator/src/output.rs`

- [ ] **Step 1: Implement JSON output**

```rust
//! JSON output for localization results

use serde::Serialize;
use std::io::{self, Write};

#[derive(Serialize)]
struct OutputRecord {
    time: u64,
    s_cm: i64,
    v_cms: i32,
    status: String,
    seg_idx: Option<usize>,
}

pub fn write_output<W: Write>(
    output: &mut W,
    time: u64,
    result: &super::kalman::ProcessResult,
) -> io::Result<()> {
    let record = match result {
        super::kalman::ProcessResult::Valid { s_cm, v_cms, seg_idx } => OutputRecord {
            time,
            s_cm: *s_cm as i64,
            v_cms: *v_cms as i32,
            status: "valid".to_string(),
            seg_idx: Some(*seg_idx),
        },
        super::kalman::ProcessResult::Rejected(reason) => OutputRecord {
            time,
            s_cm: 0,
            v_cms: 0,
            status: format!("rejected_{}", reason),
            seg_idx: None,
        },
        super::kalman::ProcessResult::Outage => OutputRecord {
            time,
            s_cm: 0,
            v_cms: 0,
            status: "dr_outage".to_string(),
            seg_idx: None,
        },
        super::kalman::ProcessResult::DrOutage { s_cm, v_cms } => OutputRecord {
            time,
            s_cm: *s_cm as i64,
            v_cms: *v_cms as i32,
            status: "dr_outage".to_string(),
            seg_idx: None,
        },
    };

    writeln!(output, "{}", serde_json::to_string(&record).unwrap())
}
```

- [ ] **Step 2: Integrate full pipeline in main.rs**

```rust
use kalman::{process_gps_update, ProcessResult, DrState, KalmanState};
use route_data::load_route_data;
use nmea::NmeaState;
use output::write_output;
use std::fs::File;
use std::io::{BufRead, BufWriter};

fn main() {
    // ... CLI parsing ...

    // Load route data
    let route_data = load_route_data(&route_path)
        .expect("Failed to load route_data.bin");

    // Initialize state
    let mut kalman = KalmanState::new();
    let mut dr = DrState::new();
    let mut nmea_state = NmeaState::new();

    // Open NMEA file
    let nmea_file = File::open(&nmea_path).expect("Failed to open NMEA file");
    let reader = BufReader::new(nmea_file);
    let mut output = BufWriter::new(File::create(&output_path).expect("Failed to create output"));

    let mut time = 0u64;

    // Process each line
    for line in reader.lines() {
        let line = line.expect("Failed to read line");

        if let Some(gps) = nmea_state.parse_sentence(&line) {
            let result = process_gps_update(&mut kalman, &mut dr, &gps, &route_data, time);
            write_output(&mut output, time, &result).expect("Failed to write output");
        }

        time += 1;
    }

    println!("Processed {} GPS updates", time);
}
```

- [ ] **Step 3: Run to verify**

Run: `cargo build --release --bin simulator`
Expected: Compiles without errors

- [ ] **Step 4: Test with sample data**

Run: `cargo run --bin simulator -- test.nmea route_data.bin output.jsonl`
Expected: Creates output.jsonl with results

- [ ] **Step 5: Commit**

```bash
git add simulator/src/output.rs simulator/src/main.rs && git commit -m "feat(simulator): add JSON output and full pipeline integration"
```

---

## Completion Checklist

After all tasks complete:

- [ ] `cargo test --workspace` passes (all modules)
- [ ] `cargo run --bin simulator -- test.nmea route_data.bin output.jsonl` produces output
- [ ] Output JSON has correct format (time, s_cm, v_cms, status, seg_idx)
- [ ] All commits follow conventional commit format
- [ ] Final `git log` shows clean progression

---

## Testing with Real Data

To test with actual NMEA data:

```bash
# Use existing test.nmea from tools/
cargo run --bin simulator -- ../../test.nmea route_data.bin output.jsonl

# Verify output
head output.jsonl
```

Expected output format:
```json
{"time":0,"s_cm":123456,"v_cms":567,"status":"valid","seg_idx":42}
{"time":1,"s_cm":124023,"v_cms":580,"status":"valid","seg_idx":42}
...
```

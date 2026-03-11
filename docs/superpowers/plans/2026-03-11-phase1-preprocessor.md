# Phase 1: Offline Preprocessor — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Rust offline preprocessor that converts route.json + stops.json into route_data.bin with all precomputed geometric coefficients.

**Architecture:** 7-stage pipeline (RDP simplify → lat_avg/bbox → coordinate conversion → linearization → stop projection → grid index → binary packing). All geometric coefficients precomputed offline to minimize runtime cost on embedded target.

**Tech Stack:** Rust 2021 edition, serde/serde_json for input parsing, bincode for binary serialization, CRC32 for integrity.

---

## Chunk 1: Workspace Setup & Shared Types

**Goal:** Establish workspace structure and define core data structures used across all phases.

---

### Task 1: Create Workspace Root

**Files:**
- Create: `Cargo.toml` (workspace root)

- [ ] **Step 1: Write workspace Cargo.toml**

```toml
[workspace]
resolver = "2"
members = ["shared", "preprocessor"]

[workspace.package]
version = "0.1.0"
edition = "2021"
rust-version = "1.75"

[workspace.dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
bincode = "1.3"
crc32fast = "1.4"
```

- [ ] **Step 2: Run `cargo check` to verify workspace**

Run: `cargo check`
Expected: No errors (workspace with 0 members is valid)

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "feat(workspace): initialize Cargo workspace"
```

---

### Task 2: Create Shared Crate — Semantic Types

**Files:**
- Create: `shared/Cargo.toml`
- Create: `shared/src/lib.rs`

- [ ] **Step 1: Write shared/Cargo.toml**

```toml
[package]
name = "shared"
version.workspace = true
edition.workspace = true

[dependencies]
```

- [ ] **Step 2: Write shared/src/lib.rs — semantic type aliases**

```rust
//! Shared types for bus arrival detection system.
//!
//! All physical quantities use semantic integer types to prevent unit confusion
//! and enable zero-cost runtime behavior on no_std targets.

/// Distance in centimeters.
/// Range: ±21,474,836 cm ≈ ±214 km — sufficient for bus routes.
pub type DistCm = i32;

/// Speed in centimeters per second.
/// Range: 0..21,474,836 cm/s ≈ 0..214 km/h — covers bus speeds.
pub type SpeedCms = i32;

/// Heading in hundredths of a degree.
/// Range: -18000..18000 = -180°..+180°
pub type HeadCdeg = i16;

/// Probability scaled 0..255 (u8 = probability × 255).
/// Precision: 1/256 ≈ 0.004 — sufficient for arrival decisions.
pub type Prob8 = u8;

/// Squared distance (cm²) for intermediate calculations.
/// Prevents overflow in dot products: (2×10⁶)² ≈ 4×10¹² < i64::MAX.
pub type Dist2 = i64;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_sizes() {
        assert_eq!(core::mem::size_of::<DistCm>(), 4);
        assert_eq!(core::mem::size_of::<SpeedCms>(), 4);
        assert_eq!(core::mem::size_of::<HeadCdeg>(), 2);
        assert_eq!(core::mem::size_of::<Prob8>(), 1);
        assert_eq!(core::mem::size_of::<Dist2>(), 8);
    }
}
```

- [ ] **Step 3: Run tests to verify types**

Run: `cd shared && cargo test`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add shared/ && git commit -m "feat(shared): add semantic type aliases"
```

---

### Task 3: Add RouteNode Structure

**Files:**
- Modify: `shared/src/lib.rs`

- [ ] **Step 1: Write RouteNode struct with repr(C)**

```rust
/// Route node with ALL precomputed segment coefficients.
///
/// Field ordering: i64 fields placed first to satisfy 8-byte alignment
/// without compiler-inserted padding on ARM Cortex-M33.
/// Total size = 52 bytes (verified at compile time).
///
/// # Layout
/// ```text
/// offset  0: len2_cm2     i64   8 bytes  (|P[i+1]-P[i]|², cm²)
/// offset  8: line_c       i64   8 bytes  (= -(A·x₀ + B·y₀))
/// offset 16: x_cm         i32   4 bytes
/// offset 20: y_cm         i32   4 bytes
/// offset 24: cum_dist_cm  i32   4 bytes
/// offset 28: dx_cm        i32   4 bytes  (segment vector x)
/// offset 32: dy_cm        i32   4 bytes  (segment vector y)
/// offset 36: seg_len_cm   i32   4 bytes  (offline sqrt, not used runtime)
/// offset 40: line_a       i32   4 bytes  (= -dy)
/// offset 44: line_b       i32   4 bytes  (= dx)
/// offset 48: heading_cdeg i16   2 bytes
/// offset 50: _pad         i16   2 bytes
/// total: 52 bytes (no padding gaps)
/// ```
#[repr(C)]
pub struct RouteNode {
    // ── i64 fields first ──────────────────────────────────────────
    /// Squared segment length: |P[i+1] - P[i]|² in cm²
    pub len2_cm2: Dist2,
    /// Line constant: -(line_a × x₀ + line_b × y₀)
    pub line_c: Dist2,

    // ── i32 fields ────────────────────────────────────────────────
    /// X coordinate (relative to grid origin) in cm
    pub x_cm: DistCm,
    /// Y coordinate (relative to grid origin) in cm
    pub y_cm: DistCm,
    /// Cumulative distance from route start in cm
    pub cum_dist_cm: DistCm,
    /// Segment vector X: x[i+1] - x[i] in cm
    pub dx_cm: DistCm,
    /// Segment vector Y: y[i+1] - y[i] in cm
    pub dy_cm: DistCm,
    /// Segment length in cm (sqrt computed offline only)
    pub seg_len_cm: DistCm,
    /// Line coefficient A: = -dy_cm (for distance calculation)
    pub line_a: DistCm,
    /// Line coefficient B: = dx_cm (for distance calculation)
    pub line_b: DistCm,

    // ── i16 fields ────────────────────────────────────────────────
    /// Segment heading in 0.01° (e.g., 9000 = 90°)
    pub heading_cdeg: HeadCdeg,
    /// Padding to align struct size
    pub _pad: i16,
}

// Compile-time assertion — fails if field reordering changes size
const _: () = assert!(core::mem::size_of::<RouteNode>() == 52);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_node_size() {
        assert_eq!(core::mem::size_of::<RouteNode>(), 52);
        assert_eq!(core::mem::align_of::<RouteNode>(), 8);
    }

    #[test]
    fn route_node_field_offsets() {
        let node = RouteNode {
            len2_cm2: 1,
            line_c: 2,
            x_cm: 3,
            y_cm: 4,
            cum_dist_cm: 5,
            dx_cm: 6,
            dy_cm: 7,
            seg_len_cm: 8,
            line_a: 9,
            line_b: 10,
            heading_cdeg: 11,
            _pad: 0,
        };

        // Verify i64 fields come first
        let base = &node as *const _ as usize;
        assert_eq!(&node.len2_cm2 as *const _ as usize - base, 0);
        assert_eq!(&node.line_c as *const _ as usize - base, 8);
        assert_eq!(&node.x_cm as *const _ as usize - base, 16);
    }
}
```

- [ ] **Step 2: Run tests to verify RouteNode layout**

Run: `cd shared && cargo test`
Expected: All tests pass, size == 52, alignment == 8

- [ ] **Step 3: Commit**

```bash
git add shared/src/lib.rs && git commit -m "feat(shared): add RouteNode struct with 52-byte layout"
```

---

### Task 4: Add Stop and GridOrigin Structures

**Files:**
- Modify: `shared/src/lib.rs`

- [ ] **Step 1: Add Stop and GridOrigin structs**

```rust
/// Bus stop with precomputed corridor boundaries.
#[repr(C)]
pub struct Stop {
    /// Position along route in cm
    pub progress_cm: DistCm,
    /// Corridor start: progress_cm - 8000 cm (80m before stop)
    pub corridor_start_cm: DistCm,
    /// Corridor end: progress_cm + 4000 cm (40m after stop)
    pub corridor_end_cm: DistCm,
}

/// Grid origin for spatial indexing.
///
/// All coordinates are stored relative to this origin to prevent
/// i32 overflow at high longitudes.
#[repr(C)]
pub struct GridOrigin {
    /// Minimum X of all route nodes (cm, relative to Earth origin)
    pub x0_cm: DistCm,
    /// Minimum Y of all route nodes (cm, relative to Earth origin)
    pub y0_cm: DistCm,
}

#[cfg(test)]
mod tests {
    // ... existing tests ...

    #[test]
    fn stop_size() {
        assert_eq!(core::mem::size_of::<Stop>(), 12);
    }

    #[test]
    fn stop_corridor_monotonic() {
        let stop = Stop {
            progress_cm: 10000,
            corridor_start_cm: 2000,
            corridor_end_cm: 14000,
        };
        assert!(stop.corridor_start_cm < stop.progress_cm);
        assert!(stop.progress_cm < stop.corridor_end_cm);
    }
}
```

- [ ] **Step 2: Run tests to verify Stop layout**

Run: `cd shared && cargo test`
Expected: All tests pass, Stop size == 12

- [ ] **Step 3: Export types from lib.rs**

Make sure lib.rs has:
```rust
pub use {RouteNode, Stop, GridOrigin};
pub use {DistCm, SpeedCms, HeadCdeg, Prob8, Dist2};
```

- [ ] **Step 4: Run cargo test to verify exports**

Run: `cargo test -p shared`
Expected: All tests pass, types are public

- [ ] **Step 5: Commit**

```bash
git add shared/src/lib.rs && git commit -m "feat(shared): add Stop and GridOrigin structs"
```

---

## Chunk 2: Preprocessor CLI Skeleton

**Goal:** Create the preprocessor binary with argument parsing and file I/O skeleton.

---

### Task 5: Create Preprocessor Crate

**Files:**
- Create: `preprocessor/Cargo.toml`
- Create: `preprocessor/src/main.rs`

- [ ] **Step 1: Write preprocessor/Cargo.toml**

```toml
[package]
name = "preprocessor"
version.workspace = true
edition.workspace = true

[[bin]]
name = "preprocessor"
path = "src/main.rs"

[dependencies]
shared = { path = "../shared" }
serde = { workspace = true }
serde_json = { workspace = true }
bincode = { workspace = true }
crc32fast = { workspace = true }
```

- [ ] **Step 2: Write basic main.rs with CLI args**

```rust
use std::env;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 4 {
        eprintln!("Usage: {} <route.json> <stops.json> <route_data.bin>", args[0]);
        eprintln!();
        eprintln!("Arguments:");
        eprintln!("  route.json     Input: route polyline as [[lat, lon], ...]");
        eprintln!("  stops.json     Input: stops as [{\"lat\": ..., \"lon\": ...}, ...]");
        eprintln!("  route_data.bin Output: binary route data file");
        std::process::exit(1);
    }

    let route_path = PathBuf::from(&args[1]);
    let stops_path = PathBuf::from(&args[2]);
    let output_path = PathBuf::from(&args[3]);

    println!("Phase 1: Offline Preprocessor");
    println!("  Route input:  {}", route_path.display());
    println!("  Stops input:  {}", stops_path.display());
    println!("  Output:       {}", output_path.display());
    println!();
    println!("TODO: Implement pipeline");
}
```

- [ ] **Step 3: Run to verify CLI parsing**

Run: `cargo run --bin preprocessor -- route.json stops.json route_data.bin`
Expected: Prints usage info without error

Run: `cargo run --bin preprocessor --`
Expected: Prints usage message and exits

- [ ] **Step 4: Commit**

```bash
git add preprocessor/ && git commit -m "feat(preprocessor): add CLI skeleton with argument parsing"
```

---

## Chunk 3: Input Data Structures & JSON Parsing

**Goal:** Parse route.json and stops.json into Rust structures.

---

### Task 6: Define Input JSON Structures

**Files:**
- Create: `preprocessor/src/input.rs`

- [ ] **Step 1: Write input.rs with JSON deserialization types**

```rust
//! Input data structures for JSON parsing.

use serde::{Deserialize, Serialize};

/// Input route JSON format.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RouteInput {
    /// Array of [lat, lon] pairs
    #[serde(rename = "route_points")]
    pub points: Vec<[f64; 2]>,
}

/// Input stops JSON format.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StopsInput {
    /// Array of stop locations
    pub stops: Vec<StopLocation>,
}

/// Single stop location.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StopLocation {
    pub lat: f64,
    pub lon: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_route_json() {
        let json = r#"{"route_points": [[25.0, 121.0], [25.1, 121.1]]}"#;
        let route: RouteInput = serde_json::from_str(json).unwrap();
        assert_eq!(route.points.len(), 2);
        assert_eq!(route.points[0], [25.0, 121.0]);
    }

    #[test]
    fn parse_stops_json() {
        let json = r#"{"stops": [{"lat": 25.0, "lon": 121.0}]}"#;
        let stops: StopsInput = serde_json::from_str(json).unwrap();
        assert_eq!(stops.stops.len(), 1);
    }
}
```

- [ ] **Step 2: Add `mod input;` to main.rs**

```rust
mod input;

use std::env;
use std::path::PathBuf;
use std::fs;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 4 {
        eprintln!("Usage: {} <route.json> <stops.json> <route_data.bin>", args[0]);
        std::process::exit(1);
    }

    let route_path = PathBuf::from(&args[1]);
    let stops_path = PathBuf::from(&args[2]);
    let _output_path = PathBuf::from(&args[3]);

    // Parse inputs
    let route_json = fs::read_to_string(&route_path)
        .expect("Failed to read route.json");
    let _route: input::RouteInput = serde_json::from_str(&route_json)
        .expect("Failed to parse route.json");

    let stops_json = fs::read_to_string(&stops_path)
        .expect("Failed to read stops.json");
    let _stops: input::StopsInput = serde_json::from_str(&stops_json)
        .expect("Failed to parse stops.json");

    println!("Parsed route with {} points", _route.points.len());
    println!("Parsed {} stops", _stops.stops.len());
}
```

- [ ] **Step 3: Run tests to verify JSON parsing**

Run: `cd preprocessor && cargo test`
Expected: All JSON parsing tests pass

- [ ] **Step 4: Test with actual files**

First, create test input files:
```bash
echo '{"route_points": [[25.00425, 121.28645], [25.00566, 121.28619]]}' > /tmp/test_route.json
echo '{"stops": [{"lat": 25.004283, "lon": 121.286559}]}' > /tmp/test_stops.json
```

Run: `cargo run --bin preprocessor -- /tmp/test_route.json /tmp/test_stops.json /tmp/test_output.bin`
Expected: "Parsed route with 2 points" and "Parsed 1 stops"

- [ ] **Step 5: Commit**

```bash
git add preprocessor/ && git commit -m "feat(preprocessor): add JSON input parsing"
```

---

## Chunk 4: Coordinate Conversion (lat/lon → cm)

**Goal:** Convert lat/lon to centimeter coordinates with relative offset support.

---

### Task 7: Implement Coordinate Conversion Module

**Files:**
- Create: `preprocessor/src/coord.rs`

- [ ] **Step 1: Write coord.rs with latlon_to_cm functions**

```rust
//! Coordinate conversion: lat/lon → centimeter coordinates.

use shared::{DistCm};

/// Earth radius in centimeters.
const R_CM: f64 = 637_100_000.0;

/// Convert lat/lon to absolute cm coordinates.
///
/// Returns (x_cm, y_cm) where:
/// - x_cm: longitude × R × cos(lat_avg)
/// - y_cm: latitude × R
pub fn latlon_to_cm(lat: f64, lon: f64) -> (i64, i64) {
    let lat_rad = lat.to_radians();
    let lon_rad = lon.to_radians();

    let y_abs = (lat_rad * R_CM).round() as i64;
    let x_abs = (lon_rad * R_CM).round() as i64; // simplified: cos(lat_avg)≈1 for now

    (x_abs, y_abs)
}

/// Convert lat/lon to relative cm coordinates.
///
/// # Arguments
/// * `lat` - Latitude in degrees
/// * `lon` - Longitude in degrees
/// * `lat_avg` - Average latitude of route (for x scale correction)
/// * `x0_cm` - Grid origin X (absolute cm)
/// * `y0_cm` - Grid origin Y (absolute cm)
///
/// # Returns
/// Relative (x_cm, y_cm) offset from origin, safe from i32 overflow.
pub fn latlon_to_cm_relative(lat: f64, lon: f64, lat_avg: f64,
                              x0_cm: i64, y0_cm: i64) -> (DistCm, DistCm) {
    let lat_rad = lat.to_radians();
    let lon_rad = lon.to_radians();
    let lat_avg_rad = lat_avg.to_radians();

    // Calculate absolute positions
    let y_abs = (lat_rad * R_CM).round() as i64;
    let x_abs = (lon_rad * lat_avg_rad.cos() * R_CM).round() as i64;

    // Return offset from origin (safe from overflow)
    let x_rel = (x_abs - x0_cm) as DistCm;
    let y_rel = (y_abs - y0_cm) as DistCm;

    (x_rel, y_rel)
}

/// Calculate bounding box origin from a set of absolute coordinates.
pub fn compute_bbox_origin(coords: &[(i64, i64)]) -> (i64, i64) {
    let min_x = coords.iter().map(|(x, _)| *x).min().unwrap_or(0);
    let min_y = coords.iter().map(|(_, y)| *y).min().unwrap_or(0);
    (min_x, min_y)
}

/// Calculate average latitude from a set of points.
pub fn compute_lat_avg(points: &[[f64; 2]]) -> f64 {
    if points.is_empty() {
        return 25.0; // Taiwan average
    }
    let sum: f64 = points.iter().map(|p| p[0]).sum();
    sum / points.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latlon_to_cm_basic() {
        let (x, y) = latlon_to_cm(25.0, 121.0);
        // At equator: 1° ≈ 111 km ≈ 11,100,000 cm
        // At 25° lat: lon scale reduced by cos(25°) ≈ 0.906
        assert!(x.abs() > 10_000_000); // order of magnitude check
        assert!(y.abs() > 10_000_000);
    }

    #[test]
    fn latlon_to_cm_relative() {
        let lat_avg = 25.0;
        let (x0, y0) = latlon_to_cm(25.0, 121.0);

        let (x, y) = latlon_to_cm_relative(25.0, 121.0, lat_avg, x0, y0);
        assert_eq!(x, 0); // origin point should be (0, 0) relative
        assert_eq!(y, 0);
    }

    #[test]
    fn relative_coords_fit_in_i32() {
        // Taiwan coordinates should fit in i32 after offset
        let lat_avg = 25.0;
        let (x0, y0) = latlon_to_cm(25.0, 121.0);

        // Points within ~100km should fit
        for lat_off in -1.0..=1.0 {
            for lon_off in -1.0..=1.0 {
                let (x, y) = latlon_to_cm_relative(
                    25.0 + lat_off * 0.1,
                    121.0 + lon_off * 0.1,
                    lat_avg, x0, y0
                );
                assert!(x < i32::MAX as i64);
                assert!(y < i32::MAX as i64);
            }
        }
    }

    #[test]
    fn compute_lat_avg() {
        let points = [[25.0, 121.0], [25.5, 121.0], [24.5, 121.0]];
        let avg = compute_lat_avg(&points);
        assert!((avg - 25.0).abs() < 0.01);
    }

    #[test]
    fn compute_bbox_origin() {
        let coords = vec![(100, 200), (50, 150), (75, 175)];
        let (x0, y0) = compute_bbox_origin(&coords);
        assert_eq!(x0, 50);
        assert_eq!(y0, 150);
    }
}
```

- [ ] **Step 2: Add `mod coord;` to main.rs**

- [ ] **Step 3: Run tests to verify coordinate conversion**

Run: `cd preprocessor && cargo test`
Expected: All coordinate tests pass

- [ ] **Step 4: Commit**

```bash
git add preprocessor/src/coord.rs && git commit -m "feat(preprocessor): add coordinate conversion module"
```

---

## Chunk 5: Douglas-Peucker Simplification

**Goal:** Implement polyline simplification to reduce node count while preserving geometry.

---

### Task 8: Implement Douglas-Peucker Algorithm

**Files:**
- Create: `preprocessor/src/simplify.rs`

- [ ] **Step 1: Write simplify.rs with RDP algorithm**

```rust
//! Douglas-Peucker polyline simplification.

/// Simplified point with original index tracking.
#[derive(Debug, Clone)]
struct IndexedPoint {
    index: usize,
    lat: f64,
    lon: f64,
}

/// Douglas-Peucker simplification.
///
/// # Arguments
/// * `points` - Original [[lat, lon], ...] points
/// * `epsilon_cm` - Tolerance in cm (distance threshold)
/// * `protected_indices` - Indices that must NOT be removed (e.g., near stops)
///
/// # Returns
/// Indices of points to keep after simplification.
pub fn douglas_peucker(
    points: &[[f64; 2]],
    epsilon_cm: i32,
    protected_indices: &[usize],
) -> Vec<usize> {
    if points.len() <= 2 {
        return (0..points.len()).collect();
    }

    let mut keep = vec![false; points.len()];
    douglas_peucker_recursive(points, epsilon_cm, protected_indices, 0, points.len() - 1, &mut keep);

    // Always keep first and last
    keep[0] = true;
    keep[points.len() - 1] = true;

    // Always keep protected points
    for &idx in protected_indices {
        if idx < keep.len() {
            keep[idx] = true;
        }
    }

    keep.iter()
        .enumerate()
        .filter(|(_, &k)| k)
        .map(|(i, _)| i)
        .collect()
}

fn douglas_peucker_recursive(
    points: &[[f64; 2]],
    epsilon_cm: i32,
    protected_indices: &[usize],
    start: usize,
    end: usize,
    keep: &mut [bool],
) {
    if end <= start + 1 {
        return;
    }

    // Find point with maximum distance from line (start, end)
    let (max_dist_idx, max_dist) = find_furthest_point(points, start, end);

    if max_dist > epsilon_cm as f64 || is_protected(max_dist_idx, protected_indices) {
        keep[max_dist_idx] = true;
        douglas_peucker_recursive(points, epsilon_cm, protected_indices, start, max_dist_idx, keep);
        douglas_peucker_recursive(points, epsilon_cm, protected_indices, max_dist_idx, end, keep);
    }
}

fn find_furthest_point(points: &[[f64; 2]], start: usize, end: usize) -> (usize, f64) {
    let p1 = points[start];
    let p2 = points[end];

    let mut max_dist = 0.0;
    let mut max_idx = start;

    for i in (start + 1)..end {
        let dist = perpendicular_distance(points[i], p1, p2);
        if dist > max_dist {
            max_dist = dist;
            max_idx = i;
        }
    }

    (max_idx, max_dist)
}

/// Calculate perpendicular distance from point to line (in degrees, approximate).
/// Simplified haversine-free version suitable for short distances.
fn perpendicular_distance(point: [f64; 2], line_start: [f64; 2], line_end: [f64; 2]) -> f64 {
    let (x0, y0) = (point[1], point[0]);
    let (x1, y1) = (line_start[1], line_start[0]);
    let (x2, y2) = (line_end[1], line_end[0]);

    // Line equation: Ax + By + C = 0
    let A = y1 - y2;
    let B = x2 - x1;
    let C = x1 * y2 - x2 * y1;

    let numerator = (A * x0 + B * y0 + C).abs();
    let denominator = (A * A + B * B).sqrt();

    if denominator == 0.0 {
        0.0
    } else {
        // Convert to approximate cm (1 degree ≈ 111 km at equator)
        (numerator / denominator) * 11_100_000.0
    }
}

fn is_protected(idx: usize, protected: &[usize]) -> bool {
    protected.contains(&idx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn douglas_peucker_basic() {
        let points = [
            [0.0, 0.0],
            [0.001, 0.0],  // should be removed
            [0.002, 0.0],
        ];
        let result = douglas_peucker(&points, 1000, &[]);
        // With epsilon=1000cm, middle point is far enough to keep
        assert!(result.contains(&0));
        assert!(result.contains(&2));
    }

    #[test]
    fn douglas_peucker_preserves_endpoints() {
        let points = [[0.0, 0.0], [0.001, 0.0], [0.002, 0.0]];
        let result = douglas_peucker(&points, 100_000_000, &[]);
        assert!(result.contains(&0));
        assert!(result.contains(&2));
    }

    #[test]
    fn douglas_peucker_respects_protected() {
        let points = [[0.0, 0.0], [0.001, 0.0], [0.002, 0.0]];
        let result = douglas_peucker(&points, 100_000_000, &[1]);
        assert!(result.contains(&1)); // protected point always kept
    }
}
```

- [ ] **Step 2: Add `mod simplify;` to main.rs**

- [ ] **Step 3: Run tests to verify RDP algorithm**

Run: `cd preprocessor && cargo test`
Expected: All simplification tests pass

- [ ] **Step 4: Commit**

```bash
git add preprocessor/src/simplify.rs && git commit -m "feat(preprocessor): add Douglas-Peucker simplification"
```

---

## Chunk 6: Route Linearization

**Goal:** Compute all segment coefficients (dx, dy, len2, seg_len, line_a/b/c, heading).

---

### Task 9: Implement Route Linearization

**Files:**
- Create: `preprocessor/src/linearize.rs`

- [ ] **Step 1: Write linearize.rs with coefficient computation**

```rust
//! Route linearization: compute all geometric coefficients.

use shared::{RouteNode, DistCm, Dist2, HeadCdeg};

/// Linearize route: compute cumulative distance and all segment coefficients.
///
/// # Arguments
/// * `nodes_cm` - Route nodes as (x_cm, y_cm) relative coordinates
///
/// # Returns
/// Vector of RouteNode with all coefficients precomputed.
pub fn linearize_route(nodes_cm: Vec<(DistCm, DistCm)>) -> Vec<RouteNode> {
    let n = nodes_cm.len();
    if n < 2 {
        return Vec::new();
    }

    let mut result = Vec::with_capacity(n);

    // First node: cum_dist = 0, segment fields default to 0
    result.push(RouteNode {
        len2_cm2: 0,
        line_c: 0,
        x_cm: nodes_cm[0].0,
        y_cm: nodes_cm[0].1,
        cum_dist_cm: 0,
        dx_cm: 0,
        dy_cm: 0,
        seg_len_cm: 0,
        line_a: 0,
        line_b: 0,
        heading_cdeg: 0,
        _pad: 0,
    });

    let mut cum_dist: DistCm = 0;

    for i in 0..n-1 {
        let (x0, y0) = nodes_cm[i];
        let (x1, y1) = nodes_cm[i + 1];

        let dx_cm = x1 - x0;
        let dy_cm = y1 - y0;

        // Use i64 for squared length to prevent overflow
        let dx_64 = dx_cm as i64;
        let dy_64 = dy_cm as i64;
        let len2_cm2 = dx_64 * dx_64 + dy_64 * dy_64;

        // Segment length (only computed offline)
        let seg_len_cm = (len2_cm2 as f64).sqrt().round() as DistCm;

        cum_dist += seg_len_cm;

        // Line coefficients for distance calculation: A·x + B·y + C = 0
        let line_a = -dy_cm;  // = -dy
        let line_b = dx_cm;   // = dx
        let line_c = -((line_a as i64 * x0 as i64) + (line_b as i64 * y0 as i64));

        // Heading in 0.01° units
        let heading_cdeg = ((dy_cm as f64).atan2(dx_cm as f64).to_degrees() * 100.0).round() as HeadCdeg;

        result.push(RouteNode {
            len2_cm2,
            line_c,
            x_cm: x1,
            y_cm: y1,
            cum_dist_cm: cum_dist,
            dx_cm,
            dy_cm,
            seg_len_cm,
            line_a,
            line_b,
            heading_cdeg,
            _pad: 0,
        });
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linearize_simple_route() {
        // Right triangle: (0,0) → (300,0) → (300,400)
        let nodes = vec![(0, 0), (300, 0), (300, 400)];
        let result = linearize_route(nodes);

        assert_eq!(result.len(), 3);

        // First node
        assert_eq!(result[0].x_cm, 0);
        assert_eq!(result[0].y_cm, 0);
        assert_eq!(result[0].cum_dist_cm, 0);

        // Second node (after first segment of 300cm)
        assert_eq!(result[1].x_cm, 300);
        assert_eq!(result[1].y_cm, 0);
        assert_eq!(result[1].cum_dist_cm, 300);
        assert_eq!(result[1].dx_cm, 300);
        assert_eq!(result[1].dy_cm, 0);
        assert_eq!(result[1].seg_len_cm, 300);
        assert_eq!(result[1].len2_cm2, 90_000); // 300²

        // Third node (after second segment of 400cm)
        assert_eq!(result[2].x_cm, 300);
        assert_eq!(result[2].y_cm, 400);
        assert_eq!(result[2].cum_dist_cm, 700); // 300 + 400
    }

    #[test]
    fn linearize_heading_east() {
        let nodes = vec![(0, 0), (100, 0)];
        let result = linearize_route(nodes);
        // Heading east = 0°
        assert!((result[1].heading_cdeg - 0).abs() < 100); // within 1°
    }

    #[test]
    fn linearize_heading_north() {
        let nodes = vec![(0, 0), (0, 100)];
        let result = linearize_route(nodes);
        // Heading north = 90° = 9000 (in 0.01° units)
        assert!((result[1].heading_cdeg - 9000).abs() < 100);
    }

    #[test]
    fn route_node_size_is_52_bytes() {
        let nodes = vec![(0, 0), (100, 0)];
        let result = linearize_route(nodes);
        assert_eq!(std::mem::size_of_val(&result[0]), 52);
    }
}
```

- [ ] **Step 2: Add `mod linearize;` to main.rs**

- [ ] **Step 3: Run tests to verify linearization**

Run: `cd preprocessor && cargo test`
Expected: All linearization tests pass

- [ ] **Step 4: Commit**

```bash
git add preprocessor/src/linearize.rs && git commit -m "feat(preprocessor): add route linearization"
```

---

## Chunk 7: Stop Projection

**Goal:** Project stops onto route and compute corridor boundaries.

---

### Task 10: Implement Stop Projection

**Files:**
- Create: `preprocessor/src/stops.rs`

- [ ] **Step 1: Write stops.rs with projection and corridor calculation**

```rust
//! Stop projection: map stops onto route and compute corridors.

use shared::{Stop, RouteNode, DistCm, Dist2};

/// Project stops onto route and compute corridor boundaries.
///
/// # Arguments
/// * `stops_latlon` - Stop locations as [(lat, lon), ...]
/// * `route_nodes` - Linearized route nodes
/// * `lat_avg` - Average latitude for coordinate conversion
/// * `x0_cm`, `y0_cm` - Grid origin (absolute cm)
///
/// # Returns
/// Vector of Stop with progress and corridor boundaries.
pub fn project_stops(
    stops_latlon: &[(f64, f64)],
    route_nodes: &[RouteNode],
    lat_avg: f64,
    x0_cm: i64,
    y0_cm: i64,
) -> Vec<Stop> {
    use crate::coord::latlon_to_cm_relative;

    let mut stops = Vec::with_capacity(stops_latlon.len());

    for &(lat, lon) in stops_latlon {
        let (sx_cm, sy_cm) = latlon_to_cm_relative(lat, lon, lat_avg, x0_cm, y0_cm);

        // Find closest segment
        let (seg_idx, t) = find_closest_segment(sx_cm, sy_cm, route_nodes);

        // Calculate progress: cum_dist[seg] + t * seg_len
        let progress_cm = if seg_idx < route_nodes.len() {
            let base_cum = if seg_idx == 0 {
                0
            } else {
                route_nodes[seg_idx - 1].cum_dist_cm
            };
            let seg_len = route_nodes[seg_idx].seg_len_cm;
            base_cum + ((t * seg_len as f64).round() as DistCm)
        } else {
            0
        };

        // Compute corridor boundaries
        let corridor_start_cm = progress_cm - 8000; // L_pre = 80m
        let mut corridor_end_cm = progress_cm + 4000;  // L_post = 40m

        // Protect overlap with previous stop
        if let Some(prev) = stops.last() {
            let min_start = prev.corridor_end_cm + 2000; // δ_sep = 20m
            if corridor_start_cm < min_start {
                corridor_start_cm = min_start;
            }
        }

        stops.push(Stop {
            progress_cm,
            corridor_start_cm,
            corridor_end_cm,
        });
    }

    stops
}

/// Find closest route segment to a point.
///
/// Returns (segment_index, t) where t ∈ [0, 1] is the projection parameter.
fn find_closest_segment(x: DistCm, y: DistCm, nodes: &[RouteNode]) -> (usize, f64) {
    if nodes.len() < 2 {
        return (0, 0.0);
    }

    let mut best_idx = 0;
    let mut best_t = 0.0;
    let mut best_dist2: Dist2 = i64::MAX;

    for i in 0..nodes.len()-1 {
        let (seg_idx, t, dist2) = distance_to_segment(x, y, i, nodes);
        if dist2 < best_dist2 {
            best_dist2 = dist2;
            best_idx = seg_idx;
            best_t = t;
        }
    }

    (best_idx, best_t)
}

/// Calculate distance from point to segment and projection parameter t.
///
/// Returns (segment_index, t, distance_squared)
fn distance_to_segment(x: DistCm, y: DistCm, seg_idx: usize, nodes: &[RouteNode]) -> (usize, f64, Dist2) {
    let p0 = &nodes[seg_idx];
    let dx = x - p0.x_cm;
    let dy = y - p0.y_cm;

    // t = dot(point - p0, segment) / |segment|²
    let t_num = (dx as i64 * p0.dx_cm as i64 + dy as i64 * p0.dy_cm as i64) as f64;
    let t = (t_num / p0.len2_cm2 as f64).clamp(0.0, 1.0);

    // Projected point on segment
    let px = p0.x_cm + (t * p0.dx_cm as f64).round() as DistCm;
    let py = p0.y_cm + (t * p0.dy_cm as f64).round() as DistCm;

    // Distance squared
    let dist2 = ((x - px) as i64).pow(2) + ((y - py) as i64).pow(2);

    (seg_idx, t, dist2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_stop_onto_segment() {
        // Route: (0,0) → (1000, 0)
        let nodes = vec![
            RouteNode {
                x_cm: 0, y_cm: 0, cum_dist_cm: 0,
                dx_cm: 1000, dy_cm: 0, seg_len_cm: 1000,
                len2_cm2: 1_000_000, line_c: 0,
                line_a: 0, line_b: 1000, heading_cdeg: 0, _pad: 0,
            },
            RouteNode {
                x_cm: 1000, y_cm: 0, cum_dist_cm: 1000,
                dx_cm: 0, dy_cm: 0, seg_len_cm: 0,
                len2_cm2: 0, line_c: 0,
                line_a: 0, line_b: 0, heading_cdeg: 0, _pad: 0,
            },
        ];

        let stops_latlon = vec![(25.0, 121.0)]; // Will convert to (500, 0) after offset
        let result = project_stops(&stops_latlon, &nodes, 25.0, 0, 0);

        assert_eq!(result.len(), 1);
        // Progress should be ~500 (midpoint of segment)
        assert!((result[0].progress_cm - 500).abs() < 10);
    }

    #[test]
    fn corridor_overlap_protection() {
        use crate::coord::latlon_to_cm_relative;

        let nodes = vec![
            RouteNode {
                x_cm: 0, y_cm: 0, cum_dist_cm: 0,
                dx_cm: 1000, dy_cm: 0, seg_len_cm: 1000,
                len2_cm2: 1_000_000, line_c: 0,
                line_a: 0, line_b: 1000, heading_cdeg: 0, _pad: 0,
            },
            RouteNode {
                x_cm: 1000, y_cm: 0, cum_dist_cm: 1000,
                dx_cm: 0, dy_cm: 0, seg_len_cm: 0,
                len2_cm2: 0, line_c: 0,
                line_a: 0, line_b: 0, heading_cdeg: 0, _pad: 0,
            },
        ];

        // Two stops very close (100cm apart)
        let stops_latlon = vec![(25.0, 121.0), (25.0, 121.0)];
        let result = project_stops(&stops_latlon, &nodes, 25.0, 0, 0);

        // Second stop's corridor_start should be pushed out
        assert!(result[1].corridor_start_cm >= result[0].corridor_end_cm + 2000);
    }
}
```

- [ ] **Step 2: Add `mod stops;` to main.rs**

- [ ] **Step 3: Run tests to verify stop projection**

Run: `cd preprocessor && cargo test`
Expected: All stop projection tests pass

- [ ] **Step 4: Commit**

```bash
git add preprocessor/src/stops.rs && git commit -m "feat(preprocessor): add stop projection with corridor calculation"
```

---

## Chunk 8: Binary Packing & Integration

**Goal:** Serialize all data to route_data.bin with magic bytes and CRC32.

---

### Task 11: Implement Binary Packing

**Files:**
- Create: `preprocessor/src/pack.rs`

- [ ] **Step 1: Write pack.rs with binary serialization**

```rust
//! Binary packing: serialize route data to route_data.bin format.

use shared::{RouteNode, Stop, GridOrigin};
use std::io::{self, Write};

/// Magic bytes for route_data.bin: "BUSA" (BUS Arrival)
pub const MAGIC: u32 = 0x42555341;

/// Format version
pub const VERSION: u16 = 1;

/// Binary format header (excluding magic and version).
#[repr(C)]
struct Header {
    node_count: u16,
    stop_count: u8,
    x0_cm: i32,
    y0_cm: i32,
}

/// Pack route data into binary format.
///
/// # Format
/// ```text
/// [4B] magic (0x42555341)
/// [2B] version (1)
/// [2B] node_count
/// [1B] stop_count
/// [4B] x0_cm (grid origin)
/// [4B] y0_cm (grid origin)
/// [N×52B] route_nodes
/// [M×12B] stops
/// [var] grid_index (TODO)
/// [4B] crc32
/// ```
pub fn pack_route_data(
    route_nodes: &[RouteNode],
    stops: &[Stop],
    grid_origin: &GridOrigin,
    output: &mut impl Write,
) -> io::Result<()> {
    let node_count = route_nodes.len() as u16;
    let stop_count = stops.len() as u8;

    // Write header
    output.write_all(&MAGIC.to_le_bytes())?;
    output.write_all(&VERSION.to_le_bytes())?;
    output.write_all(&node_count.to_le_bytes())?;
    output.write_all(&stop_count.to_le_bytes())?;
    output.write_all(&grid_origin.x0_cm.to_le_bytes())?;
    output.write_all(&grid_origin.y0_cm.to_le_bytes())?;

    // Write route nodes (raw bytes, repr(C) ensures layout)
    for node in route_nodes {
        let bytes = unsafe {
            std::slice::from_raw_parts(
                node as *const RouteNode as *const u8,
                std::mem::size_of::<RouteNode>(),
            )
        };
        output.write_all(bytes)?;
    }

    // Write stops (raw bytes)
    for stop in stops {
        let bytes = unsafe {
            std::slice::from_raw_parts(
                stop as *const Stop as *const u8,
                std::mem::size_of::<Stop>(),
            )
        };
        output.write_all(bytes)?;
    }

    // TODO: Write grid index

    // Calculate and write CRC32
    use crc32fast::Hasher;
    let mut hasher = Hasher::new();
    // For now, CRC of 0 (will recalculate properly when reading)
    output.write_all(&0u32.to_le_bytes())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magic_bytes() {
        let magic_str = std::str::from_utf8(&MAGIC.to_be_bytes()).unwrap();
        assert_eq!(magic_str, "BUSA");
    }

    #[test]
    fn pack_empty_route() {
        let mut output = Vec::new();
        let nodes: Vec<RouteNode> = vec![];
        let stops: Vec<Stop> = vec![];
        let origin = GridOrigin { x0_cm: 0, y0_cm: 0 };

        pack_route_data(&nodes, &stops, &origin, &mut output).unwrap();

        // Verify magic and version
        assert_eq!(&output[0..4], &MAGIC.to_le_bytes());
        assert_eq!(&output[4..6], &VERSION.to_le_bytes());
    }
}
```

- [ ] **Step 2: Add `mod pack;` to main.rs and integrate full pipeline**

- [ ] **Step 3: Run tests to verify packing**

Run: `cd preprocessor && cargo test`
Expected: All packing tests pass

- [ ] **Step 4: Test full pipeline with sample data**

Run: `cargo run --bin preprocessor -- /tmp/test_route.json /tmp/test_stops.json /tmp/test_output.bin`
Expected: Creates /tmp/test_output.bin without errors

- [ ] **Step 5: Verify output file exists and has magic bytes**

Run: `xxd /tmp/test_output.bin | head -n 2`
Expected: First 4 bytes show "BUSA" or 41 53 55 42

- [ ] **Step 6: Commit**

```bash
git add preprocessor/src/pack.rs preprocessor/src/main.rs && git commit -m "feat(preprocessor): add binary packing and full pipeline integration"
```

---

## Completion Checklist

After all tasks complete:

- [ ] `cargo test --workspace` passes
- [ ] `cargo run --bin preprocessor -- route.json stops.json route_data.bin` produces output file
- [ ] Output file starts with magic bytes `0x42555341`
- [ ] Output file size ≈ 34 KB for typical route
- [ ] All commits follow conventional commit format

---

## Next Phase

Phase 2 will consume `route_data.bin` and implement:
- NMEA parsing
- Map matching (grid index + heading constraint)
- Kalman filtering
- Localization pipeline: GPS → (ŝ, v̂)

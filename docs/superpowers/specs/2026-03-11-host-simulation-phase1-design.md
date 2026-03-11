# Phase 1: Offline Preprocessor — Design Spec

**Date:** 2026-03-11
**Status:** Approved
**Phase:** 1 of 3 — Offline Preprocessing

---

## Overview

Phase 1 implements the offline preprocessing pipeline that converts raw route data (GeoJSON-style lat/lon coordinates) into a compact, precomputed binary format (`route_data.bin`) suitable for embedded runtime execution. All geometric coefficients are calculated once here — the runtime firmware performs zero geometric recomputation.

## Goals

- Convert route.json + stops.json → route_data.bin (~34KB Flash)
- Apply Douglas-Peucker simplification (7200 nodes → ~640 nodes)
- Precompute ALL segment coefficients (len2, line_a/b/c, heading, etc.)
- Build spatial grid index for O(k) map matching (k ≈ 5-15)
- Output validatable binary with magic bytes + CRC32

## Project Structure

```
bus_arrival/
├── Cargo.toml                  # Workspace root
├── shared/
│   ├── Cargo.toml
│   └── src/lib.rs              # RouteNode, Stop, GridOrigin, semantic types
├── preprocessor/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs             # CLI entry point
│       ├── coord.rs            # lat/lon → x_cm/y_cm conversion
│       ├── simplify.rs         # Douglas-Peucker + protections
│       ├── linearize.rs        # Cumulative distance, all coefficients
│       ├── stops.rs            # Stop projection + corridor calc
│       ├── grid.rs             # Spatial grid index
│       └── pack.rs             # Serialize to route_data.bin
```

## Data Structures

### Semantic Type Aliases (shared/src/lib.rs)

```rust
pub type DistCm   = i32;  // distance in centimeters
pub type SpeedCms = i32;  // speed in cm/s
pub type HeadCdeg = i16;  // heading in 0.01° (±18000 = ±180°)
pub type Prob8    = u8;   // probability 0..255 (0.0..1.0)
pub type Dist2    = i64;  // squared distance (cm²)
```

### RouteNode (shared/src/lib.rs)

```rust
#[repr(C)]
pub struct RouteNode {
    // i64 fields first — prevents padding on ARM Cortex-M33
    pub len2_cm2:     i64,   // |P[i+1] - P[i]|² (cm²)
    pub line_c:       i64,   // = -(line_a·x₀ + line_b·y₀)

    // i32 fields
    pub x_cm:         i32,
    pub y_cm:         i32,
    pub cum_dist_cm:  i32,   // cumulative distance from route start
    pub dx_cm:        i32,   // segment vector x
    pub dy_cm:        i32,   // segment vector y
    pub seg_len_cm:   i32,   // segment length (sqrt offline only)
    pub line_a:       i32,   // = -dy
    pub line_b:       i32,   // = dx

    // i16 fields
    pub heading_cdeg: i16,
    pub _pad:         i16,
}

const _: () = assert!(core::mem::size_of::<RouteNode>() == 52);
```

Size: 52 bytes × 640 nodes = 33.3 KB (Flash)

### Stop (shared/src/lib.rs)

```rust
#[repr(C)]
pub struct Stop {
    pub progress_cm:       DistCm,
    pub corridor_start_cm: DistCm,  // L_pre = 8000 cm behind
    pub corridor_end_cm:   DistCm,  // L_post = 4000 cm ahead
}
```

Size: 12 bytes × 50 stops = 600 bytes (Flash)

## Processing Pipeline

### 1. Coordinate Conversion (coord.rs)

**Input:** lat/lon (WGS84)
**Output:** x_cm, y_cm (planar approximation)

```rust
fn latlon_to_cm(lat: f64, lon: f64, lat_avg: f64) -> (i32, i32) {
    const R_CM: f64 = 637_100_000.0;  // Earth radius in cm
    let y_cm = (lat.to_radians() * R_CM).round() as i32;
    let x_cm = (lon.to_radians() * lat_avg.to_radians().cos() * R_CM).round() as i32;
    (x_cm, y_cm)
}
```

### 2. Douglas-Peucker Simplification (simplify.rs)

**Parameters:**
- ε_general = 700 cm (default tolerance)
- ε_curve = 250 cm (for turns > 20°)
- Stop protection radius = ±3000 cm
- Max segment length = 3000 cm (insert node if exceeded)

**Algorithm:** Recursive RDP with curve detection and stop protection.

### 3. Route Linearization (linearize.rs)

**For each segment (P[i], P[i+1]):**
```rust
dx_cm = x[i+1] - x[i]
dy_cm = y[i+1] - y[i]
len2_cm2 = dx² + dy²  // i64 to prevent overflow
seg_len_cm = sqrt(len2_cm2)  // ONLY here, offline
cum_dist_cm[i+1] = cum_dist_cm[i] + seg_len_cm
line_a = -dy_cm
line_b = dx_cm
line_c = -(line_a × x[i] + line_b × y[i])  // i64
heading_cdeg = atan2(dy, dx) × 100  // 0.01° units
```

### 4. Stop Projection (stops.rs)

**For each stop:**
1. Convert lat/lon → x_cm/y_cm
2. Find closest segment
3. Project onto route: `progress_cm = cum_dist[seg] + t × seg_len[seg]`
4. Compute corridors with overlap protection:
   - `corridor_start = progress_cm - 8000`
   - `corridor_end = progress_cm + 4000`
   - If overlap: `start[i] = max(start[i], end[i-1] + 2000)`

### 5. Spatial Grid Index (grid.rs)

**Parameters:**
- Grid cell size Δg = 10,000 cm (100 m)
- Grid origin = (min x, min y) of all nodes

**Output:** List of segment indices per grid cell (~1.2 KB Flash)

### 6. Binary Packing (pack.rs)

**Output format (route_data.bin):**

| Offset | Field          | Type    | Size    |
|--------|----------------|---------|---------|
| 0      | magic          | u32     | 4 B     |
| 4      | version        | u16     | 2 B     |
| 6      | node_count     | u16     | 2 B     |
| 8      | stop_count     | u8      | 1 B     |
| 9      | x0_cm          | i32     | 4 B     |
| 13     | y0_cm          | i32     | 4 B     |
| 17     | route_nodes[]  | RouteNode[] | N×52 B |
| ...    | stops[]        | Stop[]  | M×12 B  |
| ...    | grid_index     | bytes   | ~1.2 KB |
| -4     | crc32          | u32     | 4 B     |

**Magic:** 0x42555341 ("BUSA")
**Version:** 1

## CLI Interface

```bash
# Run preprocessor
cargo run --bin preprocessor -- route.json stops.json route_data.bin

# With verbose output
cargo run --bin preprocessor -- route.json stops.json route_data.bin --verbose
```

## Validation Criteria

- [ ] route_data.bin file created
- [ ] Magic bytes verified (0x42555341)
- [ ] CRC32 check passes on read-back
- [ ] Node count ≈ 640 (±10%)
- [ ] Stop count ≈ 50
- [ ] Total size ≈ 34 KB
- [ ] All corridor_start < corridor_end (monotonic)
- [ ] All heading_cdeg in valid range (-18000..18000)

## Dependencies

```toml
[workspace]
members = ["shared", "preprocessor"]

[workspace.dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

## Next Phase

Phase 2 will consume route_data.bin and implement the localization pipeline (map matching → Kalman filter).

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

## processing_steps
```txt
原始 polyline (lat/lon)
    ↓
① Douglas-Peucker 簡化        ← 直接在 lat/lon 操作
    ↓
② 計算 lat_avg                ← 簡化後節點的均值
    ↓
③ latlon → x_cm/y_cm（相對）  ← 統一用固定原點 (120.0°E, 20.0°N)
    ↓
④ Route Linearization          ← 預算所有幾何係數
    ↓
⑤ Stop Projection
    ↓
⑥ Grid Index
    ↓
⑦ Binary Packing
```

**Fixed Origin**: All routes use a unified fixed origin at (120.0°E, 20.0°N) instead of computing bbox-specific origin. This ensures:
- All routes share the same coordinate system
- Simpler implementation (no bbox computation needed)
- Consistent behavior across routes
- Safe from i32 overflow (Taiwan coordinates fit in ±2,000 km range from this origin)

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

**NOTE:** All coordinates (x_cm, y_cm, dx_cm, dy_cm) are stored as **relative offsets from grid origin**, not absolute Earth coordinates. This prevents i32 overflow and ensures safe arithmetic.

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

### 1. Douglas-Peucker Simplification (simplify.rs)

**Input:** Raw lat/lon polyline
**Output:** Simplified lat/lon polyline

**IMPORTANT:** Operates directly on lat/lon coordinates, not converted coordinates.

**Parameters:**
- ε_general = 700 cm (default tolerance)
- ε_curve = 250 cm (for turns > 20°)
- Stop protection radius = ±3000 cm
- Max segment length = 3000 cm (insert node if exceeded)

**Algorithm:** Recursive RDP with curve detection and stop protection.

### 2. Compute lat_avg

**After simplification:**
- `lat_avg = mean(all simplified node lats)` — used for x-coordinate scaling

**NOTE:** Origin is FIXED at (120.0°E, 20.0°N) for all routes. No bbox computation needed.

### 3. Coordinate Conversion (coord.rs)

**Input:** lat/lon (WGS84), lat_avg (shared)
**Output:** Relative x_cm, y_cm (offset from fixed origin)

**CRITICAL:** Use relative coordinates to avoid i32 overflow. Fixed origin at (120.0°E, 20.0°N) ensures Taiwan routes fit in i32 range.

```rust
const FIXED_ORIGIN_LON: f64 = 120.0;  // degrees East
const FIXED_ORIGIN_LAT: f64 = 20.0;   // degrees North

fn latlon_to_cm_relative(lat: f64, lon: f64, lat_avg: f64) -> (i32, i32) {
    const R_CM: f64 = 637_100_000.0;

    // Pre-computed fixed origin in cm
    let x0_cm = fixed_origin_x_cm();  // ~1.33×10^9 cm
    let y0_cm = fixed_origin_y_cm();  // ~2.22×10^8 cm

    let y_abs = (lat.to_radians() * R_CM).round() as i64;
    let x_abs = (lon.to_radians() * lat_avg.to_radians().cos() * R_CM).round() as i64;
    // Return offset from fixed origin (safe from overflow)
    ((x_abs - x0_cm) as i32,
     (y_abs - y0_cm as i64) as i32)
}
```

### 4. Route Linearization (linearize.rs)

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

### 5. Stop Projection (stops.rs)

**For each stop:**
1. Convert lat/lon → relative x_cm/y_cm (using same lat_avg and origin)
2. Find closest segment
3. Project onto route with t clamping:

```rust
// t ∈ [0.0, 1.0], clamp to prevent projection outside segment
let t_num = dot_i64(sx - p_i.x, sy - p_i.y, dx, dy);
let t = (t_num as f64 / len2_cm2 as f64).clamp(0.0, 1.0);
let progress_cm = cum_dist[seg] + (t * seg_len[seg] as f64).round() as i32;
```

4. Compute corridors with overlap protection:
   - `corridor_start = progress_cm - 8000`
   - `corridor_end = progress_cm + 4000`
   - If overlap: `start[i] = max(start[i], end[i-1] + 2000)`

### 5. Spatial Grid Index (grid.rs)

**Parameters:**
- Grid cell size Δg = 10,000 cm (100 m)
- Grid origin = (min x, min y) of all nodes

**Output:** List of segment indices per grid cell (~1.2 KB Flash)

### 7. Binary Packing (pack.rs)

**Output format (route_data.bin):**

| Offset | Field          | Type    | Size    | Description |
|--------|----------------|---------|---------|-------------|
| 0      | magic          | u32     | 4 B     | 0x42555341 ("BUSA") |
| 4      | version        | u16     | 2 B     | Format version = 1 |
| 6      | node_count     | u16     | 2 B     | Number of RouteNodes |
| 8      | stop_count     | u8      | 1 B     | Number of Stops |
| 9      | x0_cm          | i32     | 4 B     | Grid origin x (cm, relative) |
| 13     | y0_cm          | i32     | 4 B     | Grid origin y (cm, relative) |
| 17     | route_nodes[]  | RouteNode[] | N×52 B | Route nodes (relative coords) |
| ...    | stops[]        | Stop[]  | M×12 B  | Stops with corridors |
| ...    | grid_index     | bytes   | ~1.2 KB | Spatial grid index |
| -4     | crc32          | u32     | 4 B     | CRC32 of entire file |

**Magic value:** `0x42555341` (ASCII "BUSA" for "BUS Arrival")
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

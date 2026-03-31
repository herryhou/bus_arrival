# RouteNode Struct Optimization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Optimize RouteNode struct from 40 bytes to 28 bytes by removing len2_cm2, upgrading seg_len_cm to seg_len_mm (i64), and reducing dx_cm/dy_cm from i32 to i16.

**Architecture:** 
1. Modify RouteNode struct definition (shared crate)
2. Update preprocessor to generate new format (linearize.rs)
3. Increment binary format version (binfile.rs)
4. Update runtime map matching to compute len2 on-the-fly (map_match.rs)
5. Update visualizer parser (TypeScript)
6. Update all tests and documentation

**Tech Stack:** Rust (no_std embedded), TypeScript, binary file format

---

### Task 1: Update RouteNode Struct Definition

**Files:**
- Modify: `crates/shared/src/lib.rs`

- [ ] **Step 1: Modify RouteNode struct - remove len2_cm2, change seg_len_cm to seg_len_mm, dx_cm/dy_cm to i16**

```rust
/// Route node with precomputed segment coefficients for runtime GPS matching.
///
/// Field ordering: i64 fields placed first to satisfy 8-byte alignment
/// without compiler-inserted padding on ARM Cortex-M33.
/// Total size = 28 bytes (with repr(C) alignment).
///
/// # Layout (v8.7 - 28 bytes)
/// ```text
/// offset  0: seg_len_mm   i64   8 bytes  (|P[i+1]-P[i]|, mm)
/// offset  8: x_cm         i32   4 bytes
/// offset 12: y_cm         i32   4 bytes
/// offset 16: cum_dist_cm  i32   4 bytes
/// offset 20: dx_cm        i16   2 bytes  (segment vector x)
/// offset 22: dy_cm        i16   2 bytes  (segment vector y)
/// offset 24: heading_cdeg i16   2 bytes
/// offset 26: _pad         i16   2 bytes  (alignment padding to 8-byte boundary)
/// total: 28 bytes (aligned to 8-byte boundary for i64 field)
/// ```
///
/// # Changes from v8.5
/// - Removed `len2_cm2` (i64) - computed at runtime as (seg_len_mm / 10)^2
/// - Changed `seg_len_cm` (i32) to `seg_len_mm` (i64) for 10x precision
/// - Changed `dx_cm`, `dy_cm` from i32 to i16 (max segment length 100m = 10,000 cm fits in i16)
/// - Reordered fields for optimal packing
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RouteNode {
    // ── i64 fields first (8-byte aligned) ──────────────────────────
    /// Segment length: |P[i+1] - P[i]| in millimeters
    pub seg_len_mm: i64,

    // ── i32 fields (4-byte aligned) ────────────────────────────────
    /// X coordinate (relative to grid origin) in cm
    pub x_cm: DistCm,
    /// Y coordinate (relative to grid origin) in cm
    pub y_cm: DistCm,
    /// Cumulative distance from route start in cm
    pub cum_dist_cm: DistCm,

    // ── i16 fields (2-byte aligned) ────────────────────────────────
    /// Segment vector X: x[i+1] - x[i] in cm
    pub dx_cm: i16,
    /// Segment vector Y: y[i+1] - y[i] in cm
    pub dy_cm: i16,
    /// Segment heading in 0.01° (e.g., 9000 = 90°)
    pub heading_cdeg: HeadCdeg,
    /// Padding to align struct size to 8-byte boundary
    pub _pad: i16,
}
```

- [ ] **Step 2: Update compile-time size assertion**

```rust
// Compile-time assertion — v8.7: 28 bytes
const _: () = assert!(core::mem::size_of::<RouteNode>() == 28);
const _: () = assert!(core::mem::size_of::<Stop>() == 12);
```

- [ ] **Step 3: Run tests to verify compilation**

Run: `cargo test --package shared`
Expected: PASS (but some tests may fail - we'll fix them in subsequent tasks)

- [ ] **Step 4: Commit**

```bash
git add crates/shared/src/lib.rs
git commit -m "feat(shared): redesign RouteNode struct - 40→28 bytes

- Remove len2_cm2 (i64) - compute at runtime
- Change seg_len_cm (i32) to seg_len_mm (i64) for 10x precision
- Reduce dx_cm, dy_cm from i32 to i16 (max 100m segment fits)
- Reorder fields for optimal packing
- Total size: 40 → 28 bytes (30% reduction)"
```

---

### Task 2: Update Binary Format Version

**Files:**
- Modify: `crates/shared/src/binfile.rs`

- [ ] **Step 1: Increment VERSION constant and update documentation**

```rust
/// Format version
/// v2: Removed line_a, line_b, line_c from RouteNode (52 → 36 bytes)
/// v3 (v8.5): Changed repr(C, packed) to repr(C) to fix UB with field references
///             Size now 40 bytes on platforms with 8-byte i64 alignment
/// v4 (v8.7): RouteNode optimization - remove len2_cm2, seg_len_cm→seg_len_mm (i64),
///             dx_cm/dy_cm i32→i16. Size now 28 bytes.
pub const VERSION: u16 = 4;
```

- [ ] **Step 2: Run tests to verify compilation**

Run: `cargo test --package shared`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/shared/src/binfile.rs
git commit -m "feat(shared): increment binary format version to 4 (v8.7)"
```

---

### Task 3: Update Preprocessor Linearize Module

**Files:**
- Modify: `crates/preprocessor/src/linearize.rs`

- [ ] **Step 1: Update linearize_route function for new struct**

```rust
/// Linearize a route by computing all geometric coefficients
///
/// Processes a sequence of (x, y) coordinates and computes all geometric
/// properties needed for real-time GPS matching.
///
/// # Algorithm (v8.7)
/// For each segment (i to i+1):
/// 1. Compute segment vector: dx = x[i+1] - x[i], dy = y[i+1] - y[i]
/// 2. Compute actual length in mm: seg_len_mm = sqrt(dx² + dy²) × 10
/// 3. Update cumulative distance: cum_dist[i+1] = cum_dist[i] + seg_len
/// 4. Compute heading: heading = atan2(dx, dy) × 100 (in 0.01° units)
///
/// # Arguments
/// * `nodes_cm` - Slice of (x, y) coordinates in centimeters
///
/// # Returns
/// * `Vec<RouteNode>` - Route nodes with all geometric coefficients
pub fn linearize_route(nodes_cm: &[(i64, i64)]) -> Vec<RouteNode> {
    if nodes_cm.is_empty() {
        return vec![];
    }

    let n = nodes_cm.len();
    if n == 1 {
        return vec![RouteNode {
            seg_len_mm: 0,
            x_cm: nodes_cm[0].0 as i32,
            y_cm: nodes_cm[0].1 as i32,
            cum_dist_cm: 0,
            dx_cm: 0,
            dy_cm: 0,
            heading_cdeg: 0,
            _pad: 0,
        }];
    }

    let mut route = Vec::with_capacity(n);
    let mut cum_dist_cm = 0i32;

    for i in 0..n {
        let (x0, y0) = nodes_cm[i];

        // Last node has no outgoing segment
        if i == n - 1 {
            route.push(RouteNode {
                seg_len_mm: 0,
                x_cm: x0 as i32,
                y_cm: y0 as i32,
                cum_dist_cm,
                dx_cm: 0,
                dy_cm: 0,
                heading_cdeg: 0,
                _pad: 0,
            });
            continue;
        }

        let (x1, y1) = nodes_cm[i + 1];

        // Segment vector in cm (truncated to i16 - max 100m segment constraint)
        let dx_cm = (x1 - x0) as i32;
        let dy_cm = (y1 - y0) as i32;

        // Validate segment length constraint (100m = 10,000 cm)
        let dx_abs = dx_cm.abs();
        let dy_abs = dy_cm.abs();
        if dx_abs > 10_000 || dy_abs > 10_000 {
            // Log warning but continue - values will be truncated
            eprintln!("Warning: Segment {} exceeds 100m constraint: dx={}, dy={}", i, dx_cm, dy_cm);
        }

        // Squared length in cm²
        let len2_cm2 = (dx_cm as i64 * dx_cm as i64) + (dy_cm as i64 * dy_cm as i64);

        // Actual length in mm (10x precision)
        let seg_len_mm = ((len2_cm2 as f64).sqrt() * 10.0).round() as i64;

        // Heading in centidegrees (0.01° units)
        let heading_rad = (dx_cm as f64).atan2(dy_cm as f64);
        let heading_cdeg = (heading_rad.to_degrees() * 100.0).round() as i16;

        route.push(RouteNode {
            seg_len_mm,
            x_cm: x0 as i32,
            y_cm: y0 as i32,
            cum_dist_cm,
            dx_cm: dx_cm as i16,
            dy_cm: dy_cm as i16,
            heading_cdeg,
            _pad: 0,
        });

        // Cumulative distance in cm
        let seg_len_cm = (seg_len_mm / 10) as i32;
        cum_dist_cm += seg_len_cm;
    }

    route
}
```

- [ ] **Step 2: Update test assertion**

```rust
#[test]
fn test_route_node_size() {
    // v8.7: Optimized struct layout - 28 bytes
    assert_eq!(std::mem::size_of::<RouteNode>(), 28);
}
```

- [ ] **Step 3: Run tests to verify**

Run: `cargo test --package preprocessor --lib linearize`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/preprocessor/src/linearize.rs
git commit -m "feat(preprocessor): update linearize for new RouteNode layout

- Store segment length in mm (seg_len_mm: i64) instead of cm
- Truncate dx_cm, dy_cm to i16 (100m max segment constraint)
- Remove len2_cm2 storage (computed at runtime)
- Add warning for segments exceeding 100m constraint"
```

---

### Task 4: Update Map Matching Module

**Files:**
- Modify: `crates/pipeline/gps_processor/src/map_match.rs`

- [ ] **Step 1: Update distance_to_segment_squared to compute len2 from seg_len_mm**

```rust
/// Distance squared from point to segment (clamped projection)
/// v8.7: Computes len2 from seg_len_mm: (seg_len_mm / 10)^2
fn distance_to_segment_squared(x: DistCm, y: DistCm, seg: &RouteNode) -> Dist2 {
    let dx = x - seg.x_cm;
    let dy = y - seg.y_cm;

    // Compute len2_cm2 from seg_len_mm: (mm / 10)^2 = cm^2
    let seg_len_cm = seg.seg_len_mm / 10;
    let len2_cm2 = seg_len_cm * seg_len_cm;

    // t = dot(point - P[i], segment) / |segment|²
    let t_num = dx as i64 * seg.dx_cm as i64 + dy as i64 * seg.dy_cm as i64;

    if len2_cm2 == 0 {
        return ((x - seg.x_cm) as i64).pow(2) + ((y - seg.y_cm) as i64).pow(2);
    }

    let t = if t_num < 0 { 0 } else if t_num > len2_cm2 { len2_cm2 } else { t_num };

    // Projected point
    let px = seg.x_cm + ((t * seg.dx_cm as i64 / len2_cm2) as DistCm);
    let py = seg.y_cm + ((t * seg.dy_cm as i64 / len2_cm2) as DistCm);

    // Distance squared
    ((x - px) as i64).pow(2) + ((y - py) as i64).pow(2)
}
```

- [ ] **Step 2: Update project_to_route to use seg_len_mm**

```rust
/// Project GPS point onto segment → route progress
/// v8.7: Uses seg_len_mm for length computation
pub fn project_to_route(
    gps_x: DistCm,
    gps_y: DistCm,
    seg_idx: usize,
    route_data: &RouteData,
) -> DistCm {
    let seg = route_data.get_node(seg_idx).unwrap_or_else(|| {
        // Fallback to first node if index is invalid
        route_data.get_node(0).unwrap()
    });

    let dx = gps_x - seg.x_cm;
    let dy = gps_y - seg.y_cm;
    let t_num = dx as i64 * seg.dx_cm as i64 + dy as i64 * seg.dy_cm as i64;

    // Compute len2_cm2 from seg_len_mm: (mm / 10)^2 = cm^2
    let seg_len_cm = seg.seg_len_mm / 10;
    let len2_cm2 = seg_len_cm * seg_len_cm;

    if len2_cm2 == 0 {
        return seg.cum_dist_cm;
    }

    let t = if t_num < 0 { 0 } else if t_num > len2_cm2 { len2_cm2 } else { t_num };

    // z = cum_dist[i] + t × seg_len_cm / len2_cm2
    let base = seg.cum_dist_cm;
    base + ((t * seg_len_cm / len2_cm2) as DistCm)
}
```

- [ ] **Step 3: Run tests to verify**

Run: `cargo test --package gps_processor`
Expected: PASS (may need to regenerate test data after implementation)

- [ ] **Step 4: Commit**

```bash
git add crates/pipeline/gps_processor/src/map_match.rs
git commit -m "feat(gps_processor): update map matching for new RouteNode layout

- Compute len2_cm2 at runtime from seg_len_mm: (seg_len_mm / 10)^2
- Update distance_to_segment_squared function
- Update project_to_route function
- No API changes - transparent to callers"
```

---

### Task 5: Update Visualizer TypeScript Parser

**Files:**
- Modify: `visualizer/src/lib/parsers/routeData.ts`
- Modify: `visualizer/src/lib/types.ts`

- [ ] **Step 1: Update types.ts RouteNode interface**

```typescript
/** Route node with precomputed segment coefficients (v8.7 format - 28 bytes) */
export interface RouteNode {
	/** Segment length: |P[i+1] - P[i]| in millimeters */
	seg_len_mm: number;
	/** X coordinate (absolute, from fixed origin 120°E, 20°N) in cm */
	x_cm: number;
	/** Y coordinate (absolute, from fixed origin 120°E, 20°N) in cm */
	y_cm: number;
	/** Cumulative distance from route start in cm */
	cum_dist_cm: number;
	/** Segment vector X: x[i+1] - x[i] in cm (i16) */
	dx_cm: number;
	/** Segment vector Y: y[i+1] - y[i] in cm (i16) */
	dy_cm: number;
	/** Segment heading in 0.01° */
	heading_cdeg: number;
	/** Padding for alignment */
	_pad: number;
}
```

- [ ] **Step 2: Update routeData.ts constants and parser**

```typescript
/**
 * Binary route data parser
 *
 * Parses route_data.bin files generated by preprocessor (v8.7 format).
 * Binary format must match Rust RouteData struct in shared/src/binfile.rs.
 *
 * RouteNode layout (#[repr(C)], 28 bytes):
 * offset  0: seg_len_mm   i64   8 bytes
 * offset  8: x_cm         i32   4 bytes
 * offset 12: y_cm         i32   4 bytes
 * offset 16: cum_dist_cm  i32   4 bytes
 * offset 20: dx_cm        i16   2 bytes
 * offset 22: dy_cm        i16   2 bytes
 * offset 24: heading_cdeg i16   2 bytes
 * offset 26: _pad         i16   2 bytes (alignment padding to 8-byte boundary)
 *
 * Stop layout (12 bytes):
 * offset  0: progress_cm        u32   4 bytes
 * offset  4: corridor_start_cm  u32   4 bytes
 * offset  8: corridor_end_cm    u32   4 bytes
 *
 * File layout (v8.7 format):
 * - Header (28 bytes):
 *   - MAGIC: u32 (0x42555341 = "BUSA")
 *   - VERSION: u16 (4) - v8.7 format
 *   - node_count: u16
 *   - stop_count: u8
 *   - padding: u8[3] (for 4-byte alignment)
 *   - x0_cm: i32
 *   - y0_cm: i32
 *   - lat_avg_deg: f64 (average latitude for projection)
 * - Nodes array: node_count × 28 bytes
 * - Stops array: stop_count × 12 bytes
 * - Grid data (cols, rows, grid_size_cm, offsets, cells)
 * - LUTs: 256 bytes (gaussian) + 128 bytes (logistic)
 * - CRC32: u32 = 4 bytes
 */

const ROUTE_NODE_SIZE = 28; // v8.7: optimized layout
const STOP_SIZE = 12;
const HEADER_SIZE_V8 = 28; // + lat_avg_deg(8) for total of 28 bytes
const MAGIC = 0x42555341; // "BUSA" in little-endian
const VERSION = 4; // v8.7 format: 28-byte optimized nodes

/**
 * Parse RouteNode from binary data (v8.7 format - 28 bytes)
 */
function parseRouteNode(dataView: DataView, offset: number): RouteNode {
	return {
		seg_len_mm: Number(readI64(dataView, offset)),
		x_cm: dataView.getInt32(offset + 8, true),
		y_cm: dataView.getInt32(offset + 12, true),
		cum_dist_cm: dataView.getInt32(offset + 16, true),
		dx_cm: dataView.getInt16(offset + 20, true),
		dy_cm: dataView.getInt16(offset + 22, true),
		heading_cdeg: dataView.getInt16(offset + 24, true),
		_pad: dataView.getInt16(offset + 26, true)
	};
}
```

- [ ] **Step 3: Update version check error message**

```typescript
if (version !== VERSION) {
	throw new Error(`Unsupported version: ${version}, expected ${VERSION} (v8.7 format). Please regenerate route_data.bin with the latest preprocessor.`);
}
```

- [ ] **Step 4: Run TypeScript type check**

Run: `cd visualizer && npm run check`
Expected: PASS (no type errors)

- [ ] **Step 5: Commit**

```bash
git add visualizer/src/lib/types.ts visualizer/src/lib/parsers/routeData.ts
git commit -m "feat(visualizer): update parser for v8.7 binary format

- Update RouteNode interface: seg_len_mm (was len2_cm2), dx_cm/dy_cm as i16
- Update ROUTE_NODE_SIZE: 40 → 28 bytes
- Update VERSION: 3 → 4
- Update parseRouteNode for new layout"
```

---

### Task 6: Regenerate Test Data

**Files:**
- Test data files in `crates/preprocessor/tests/` and `crates/pipeline/gps_processor/tests/`

- [ ] **Step 1: Regenerate route_data.bin with new format**

Run: `cargo run --bin preprocessor -- --input data/ty225.gpx --output route_data.bin`
Expected: Success, generates v8.7 format binary

- [ ] **Step 2: Run all preprocessor tests**

Run: `cargo test --package preprocessor`
Expected: PASS (after test data regeneration)

- [ ] **Step 3: Run all GPS processor tests**

Run: `cargo test --package gps_processor`
Expected: PASS (after test data regeneration)

- [ ] **Step 4: Commit updated test data**

```bash
git add crates/preprocessor/tests/ crates/pipeline/gps_processor/tests/
git commit -m "test: regenerate test data for v8.7 format"
```

---

### Task 7: Update Documentation

**Files:**
- Modify: `docs/bus_arrival_tech_report_v8.md`
- Modify: `docs/dev_guide.md`
- Modify: `docs/shrink_struct_RouteNode_proposal.md` (mark as implemented)

- [ ] **Step 1: Update tech report with new struct layout**

Add/update section in tech report:

```markdown
## RouteNode Structure (v8.7)

The RouteNode struct stores precomputed geometric coefficients for each route node.
v8.7 optimized the layout from 40 to 28 bytes (30% reduction):

| Offset | Field | Type | Description |
|--------|-------|------|-------------|
| 0 | seg_len_mm | i64 | Segment length in millimeters |
| 8 | x_cm | i32 | X coordinate in cm |
| 12 | y_cm | i32 | Y coordinate in cm |
| 16 | cum_dist_cm | i32 | Cumulative distance in cm |
| 20 | dx_cm | i16 | Segment vector X (cm) |
| 22 | dy_cm | i16 | Segment vector Y (cm) |
| 24 | heading_cdeg | i16 | Heading in 0.01° |
| 26 | _pad | i16 | Alignment padding |

**Key Changes from v8.5:**
- Removed `len2_cm2` (i64) - computed at runtime as `(seg_len_mm / 10)^2`
- Upgraded `seg_len_cm` (i32) to `seg_len_mm` (i64) for 10x precision
- Reduced `dx_cm`, `dy_cm` from i32 to i16 (100m max segment constraint)
```

- [ ] **Step 2: Update dev guide with new binary format**

Update binary format section to reflect VERSION=4 and new layout.

- [ ] **Step 3: Mark proposal as implemented**

```markdown
## **技術建議書：RouteNode 結構體深度優化方案 (v8.7 - IMPLEMENTED)**
```

- [ ] **Step 4: Commit**

```bash
git add docs/
git commit -m "docs: update for v8.7 RouteNode optimization

- Document new 28-byte struct layout
- Update binary format version to 4
- Mark optimization proposal as implemented"
```

---

### Task 8: Verification and Integration Testing

**Files:**
- All crates

- [ ] **Step 1: Run full test suite**

Run: `cargo test --workspace`
Expected: All tests PASS

- [ ] **Step 2: Build embedded target**

Run: `cargo build --target thumbv6m-none-eabi --release`
Expected: Build succeeds without warnings

- [ ] **Step 3: Verify Flash savings**

Compare route_data.bin file sizes:
```bash
ls -lh route_data.bin.old route_data.bin  # Compare sizes
```
Expected: ~30% reduction in route node section

- [ ] **Step 4: Test visualizer with new format**

Run: `cd visualizer && npm run dev`
Expected: Loads and displays route correctly

- [ ] **Step 5: Create summary commit**

```bash
git add .
git commit -m "test: verify v8.7 RouteNode optimization complete

- All tests pass
- Embedded target builds successfully
- Visualizer parses new format correctly
- ~30% Flash savings confirmed"
```

---

### Task 9: Create Migration Guide

**Files:**
- Create: `docs/v8.7_migration_guide.md`

- [ ] **Step 1: Write migration guide**

```markdown
# v8.7 Migration Guide: RouteNode Optimization

## Summary
v8.7 optimizes the RouteNode struct from 40 to 28 bytes (30% reduction).

## Breaking Changes
- Binary format version changed from 3 to 4
- Old `route_data.bin` files are incompatible

## Migration Steps
1. Regenerate all `route_data.bin` files using the updated preprocessor
2. Update visualizer to VERSION=4
3. Rebuild embedded firmware with new shared crate

## Compatibility
- No API changes to Rust code
- Visualizer must be updated to parse new format
```

- [ ] **Step 2: Commit**

```bash
git add docs/v8.7_migration_guide.md
git commit -m "docs: add v8.7 migration guide"
```

---

## Summary

This plan implements the RouteNode struct optimization from the proposal document, reducing the size from 40 to 28 bytes (30% savings) through:

1. **Struct redesign** - Remove len2_cm2, upgrade seg_len to mm, reduce dx/dy to i16
2. **Binary format version bump** - VERSION 3 → 4
3. **Preprocessor updates** - Generate new format with mm precision
4. **Runtime updates** - Compute len2 on-the-fly from seg_len_mm
5. **Visualizer updates** - Parse new 28-byte format
6. **Documentation updates** - Reflect all changes

**Expected benefits:**
- ~30% Flash savings for route data (24 KB → 16.8 KB for 600 nodes)
- 10x precision improvement for segment lengths
- Minimal runtime performance impact (<0.1 ms per map match)

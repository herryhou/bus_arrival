# Binary Format Migration Guide

## Summary
This guide covers binary format changes from v8.5 through v8.8, including RouteNode optimization (v8.7), sparse grid optimization (v8.8), and XIP alignment fix (v8.8.1).

## Current Version: 5.1 (v8.8.1)

## Breaking Changes

### v5.1 (v8.8.1) - XIP Alignment Fix
- **Behavior change:** Grid `get_cell()` now handles misaligned flash addresses
- **No breaking change:** Binary format unchanged, still VERSION 5
- **Runtime impact:** Misaligned access incurs heap allocation (rare)
- **Recommendation:** Ensure linker places bin file at even address

### v5 (v8.8) - Sparse Grid Optimization
- Binary format version changed from 4 to 5
- Old `route_data.bin` files are incompatible
- Grid format changed from dense to sparse (bitmask + u16 offsets)
- **Flash savings:** ~16 KB → ~5 KB (60-70% reduction)

### v4 (v8.7) - RouteNode Optimization
- Binary format version changed from 3 to 4
- Old `route_data.bin` files are incompatible
- Field types changed (though API remains compatible)

## Technical Details of Field Changes

### Field Type Changes
1. **`seg_len_cm` (i32) → `seg_len_mm` (i32)**
   - Increased precision from centimeters to millimeters (10x improvement)
   - Remains signed 32-bit (sufficient for 100km+ routes)
   - Allows for more precise distance calculations

2. **`dx_cm`, `dy_cm` (i32 → i16)**
   - Reduced from signed 32-bit to signed 16-bit
   - Constraint: Maximum segment length is now 100 meters
   - This is sufficient for bus route segments (stops are typically closer)
   - Saves 4 bytes (2 bytes × 2 fields)

3. **Removed `len2_cm2` field**
   - Previously stored pre-computed squared length
   - Now computed at runtime as: `(seg_len_mm / 10)^2`
   - Saves 8 bytes of storage
   - Computation is trivial and performed infrequently

### Memory Layout (v8.7)
```
Total size: 24 bytes (16 bytes i32 + 8 bytes i16)
Field breakdown:
offset  0: x_cm         i32   4 bytes  (X coordinate, cm)
offset  4: y_cm         i32   4 bytes  (Y coordinate, cm)
offset  8: cum_dist_cm  i32   4 bytes  (cumulative distance, cm)
offset 12: seg_len_mm   i32   4 bytes  (|P[i+1]-P[i]|, mm)
offset 16: dx_cm        i16   2 bytes  (segment vector X, cm)
offset 18: dy_cm        i16   2 bytes  (segment vector Y, cm)
offset 20: heading_cdeg i16   2 bytes  (segment heading, 0.01°)
offset 22: _pad         i16   2 bytes  (alignment padding)
```

### Field Descriptions
- **x_cm**: X coordinate (relative to grid origin) in centimeters
- **y_cm**: Y coordinate (relative to grid origin) in centimeters
- **cum_dist_cm**: Cumulative distance from route start in centimeters
- **seg_len_mm**: Segment length |P[i+1] - P[i]| in millimeters
- **dx_cm**: Segment vector X: x[i+1] - x[i] in centimeters
- **dy_cm**: Segment vector Y: y[i+1] - y[i] in centimeters
- **heading_cdeg**: Segment heading in hundredths of a degree (e.g., 9000 = 90°)
- **_pad**: Padding to align struct size to 4-byte boundary

## Migration Steps

### For v5 (v8.8) - Sparse Grid
1. Regenerate all `route_data.bin` files using the updated preprocessor
2. Update visualizer to VERSION=5
3. Rebuild embedded firmware with new shared crate

### For v5.1 (v8.8.1) - XIP Alignment Fix
1. Update `shared` crate to latest version
2. Rebuild embedded firmware (no `route_data.bin` regeneration needed)
3. **Optional:** Configure linker script to place bin file at even address for optimal performance

## Verification Steps

### Compile-time Assertion
Ensure the struct size is correct:
```rust
assert!(size_of::<RouteNode>() == 24);
```

### Binary Inspection
Verify the version byte in generated `route_data.bin` files:
```bash
xxd -l 6 route_data.bin
# Bytes 4-5 should show: 05 00 (VERSION=5 in little-endian)
```

### XIP Alignment Test
The `test_grid_misaligned_access` test verifies grid can be read from misaligned addresses:
```bash
cargo test -p shared test_grid_misaligned_access
```

## Compatibility
- No API changes to Rust code (field access remains the same)
- Visualizer must be updated to parse new format
- All embedded firmware must be rebuilt with the new shared crate

## Version History

### v2
- Removed line coefficients (no longer needed after DP optimization)
- Size reduction: 52 → 36 bytes

### v3
- Changed from `repr(C, packed)` to `repr(C)`
- Removed packed attribute to fix alignment issues
- Size increase: 36 → 40 bytes (due to padding)

### v4 (v8.7)
- RouteNode optimization
- Field type refinement and computed value removal
- Size reduction: 40 → 24 bytes

### v5 (v8.8) - Sparse Grid Optimization
- **Bitmask indexing:** 1 bit per cell to mark non-empty cells
- **u16 offsets:** Only non-empty cells store offset (max 65,535 bytes)
- **Flash savings:** Grid ~16 KB → ~5 KB (60-70% reduction)
- **Performance:** +1 popcount operation, negligible CPU impact

### v5.1 (v8.8.1) - XIP Alignment Fix
- **Problem:** Bin file at odd flash address causes misaligned grid data access
- **Solution:** `get_cell()` detects misalignment and falls back to element-by-element unaligned reads
- **Performance:** Aligned (fast path) = zero-copy; Misaligned (slow path) = heap alloc + copy
- **Mitigation:** Ensure linker places bin file at even address for production

# Spatial Grid Index Specification (v5.1)

## Overview

Sparse grid index for O(k) segment queries during map matching. Uses bitmask + u16 offsets for 60-70% size reduction vs v4.

## Format Version

**Current:** v5.1 (XIP support for misaligned flash addresses)

## On-Disk Format

```
[Header: 12 bytes]
  cols: u32 (4 bytes)
  rows: u32 (4 bytes)
  grid_size_cm: i32 (4 bytes)

[Bitmask: ceil(cols × rows / 8) bytes]
  1 bit per cell: 1 = has data, 0 = empty

[Offsets: 2 × non_empty_count bytes]
  u16 offset per non-empty cell → points into data section

[Padding: 0-1 byte]
  Align to 2-byte boundary

[Data Section: variable]
  per cell: count (u16) + indices (u16[])

[CRC: 4 bytes]
  CRC32 checksum of all data before CRC
```

## Key Operations

### Cell Lookup

```
1. Bounds check: col < cols && row < rows
2. Check bitmask bit for cell index
3. If bit = 0 → empty cell, return []
4. Count set bits before this cell → offset index
5. Read u16 offset from offsets table
6. Read count (u16) from data section at offset
7. Return slice of u16 indices
```

### Set Bit Counting

The offsets table is dense (only non-empty cells), so we count set bits in the bitmask to find the correct offset index:

```
Cell:     0  1  2  3  4  5  6  7  8  9 10 11 12 13 14 15
Bitmask:  0  0  1  0  0  0  0  1  0  0  0  0  1  0  0  0

Offset index for cell 7 = popcount(0..7) = 2
offsets[2] → data offset for cell 7
```

## Invariants (MUST)

- [ ] Grid size: 10000 cm (100 m) cells
- [ ] Max data section: 64 KB (u16::MAX)
- [ ] CRC32: covers all data before CRC field
- [ ] XIP: handle misaligned addresses via element-by-element reads

## XIP Support

### Problem

When loaded at odd flash addresses, the data section pointer becomes misaligned even though file offset is even:

```
File offset:     0x2C (even) → padding → 0x2D (odd) → data section
Flash address:   0x80001 (odd) + 0x2C = 0x8002D (odd)
```

### Solution

**v5.1 (XIP fix):** `visit_cell()` detects misaligned `indices_ptr` and falls back to element-by-element unaligned reads:

```rust
// Fast path: aligned, zero-copy slice
Ok(unsafe { core::slice::from_raw_parts(indices_ptr, count) })

// Slow path: misaligned (XIP at odd address)
for i in 0..count {
    let idx = unsafe { core::ptr::read_unaligned(indices_ptr.add(i)) };
    f(idx);
}
```

### Performance Impact

| Scenario | Path | Cost |
|----------|------|------|
| Aligned (typical) | Zero-copy slice | None |
| Misaligned (XIP at odd address) | Element-by-element reads | Unaligned LDRH per element |

**Mitigation:** Ensure linker places bin file at even address for production.

## Design Trade-offs

### Why u16 offsets?

- **Maximum data section size:** 64KB (u16::MAX)
- **Typical route:** ~5000 segments × ~4 cells/segment × 2 bytes = ~40KB
- **Overflow protection:** Returns `BusError::GridDataOverflow` if exceeded

### Why not compress?

- **Zero-copy reads:** Directly access Flash memory without decompression
- **Deterministic access:** O(1) cell lookup, no seeking
- **Small enough:** 5KB is acceptable for embedded Flash

## Version History

| Version | Change | Size Reduction |
|---------|--------|----------------|
| v4 | Dense grid (u32 offset per cell) | - |
| v5 | Sparse grid (bitmask + u16 offsets) | 60-70% |
| v5.1 | XIP support: handle misaligned flash addresses | - |

## Related Files

- `docs/spatial_grid_binary_format.md` — Complete format documentation
- `crates/shared/src/binfile.rs` — Read/write implementation

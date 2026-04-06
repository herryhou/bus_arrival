# Grid Binary Format (v5)

## Overview

The spatial grid is an index for O(k) segment queries during map matching. It divides the route bounding box into fixed-size cells (default: 100m × 100m) and stores which route segments intersect each cell.

**v5 optimization (v8.8):** Sparse grid representation using bitmask + u16 offsets reduces grid size from ~16KB to ~5KB (60-70% savings).

---

## In-Memory Representation

```rust
// crates/shared/src/lib.rs
pub struct SpatialGrid {
    pub cells: Vec<Vec<usize>>,  // Dense: one Vec per cell
    pub grid_size_cm: i32,
    pub cols: u32,
    pub rows: u32,
    pub x0_cm: i32,  // Grid origin (bounding box min)
    pub y0_cm: i32,
}
```

- **Dense format:** Every cell has a Vec, even if empty (most are)
- **Cell content:** Segment indices (`usize`) that intersect the cell
- **Used by:** Preprocessor only (std environment)

---

## On-Disk Format (v5)

### Structure Layout

```
[Header: 12 bytes]
  cols:         u32  (4 bytes)
  rows:         u32  (4 bytes)
  grid_size_cm: i32  (4 bytes)

[Bitmask: ceil(cols × rows / 8) bytes]
  1 bit per cell: 1 = has data, 0 = empty

[Offsets Table: 2 × non_empty_count bytes]
  u16 offset for each non-empty cell → points into data section

[Padding: 0-1 byte]
  Align data section to 2-byte boundary

[Data Section: variable]
  For each non-empty cell:
    count: u16          (2 bytes) - number of segments in this cell
    indices: u16[count] (2×count bytes) - segment indices

[CRC: 4 bytes]
  CRC32 checksum of all data before CRC
```

### Byte-Level Example

For a 4×4 grid with 2 non-empty cells:

```
Offset  Content               Description
------- -------------------- ----------------------------------------
0x00    04 00 00 00          cols = 4
0x04    04 00 00 00          rows = 4
0x08    10 27 00 00          grid_size_cm = 10000
0x0C    30 00 00 00          bitmask (bits: 00110000...) - cells 4,5 have data
0x10    00 00                offset[0] = 0 (cell 4 data starts at 0)
0x12    08 00                offset[1] = 8 (cell 5 data starts at byte 8)
0x14    -- padding --        (if offsets_size % 2 != 0)
0x15    02 00 00 00 01 00    cell 4: count=2, indices=[0,1]
0x1C    01 00 05 00          cell 5: count=1, indices=[5]
```

---

## Reading the Grid

### Key Function: `SpatialGridView::get_cell()`

```rust
// crates/shared/src/binfile.rs
pub fn get_cell(&self, col: u32, row: u32) -> Result<&'a [u16], BusError> {
    // 1. Bounds check
    let cell_idx = (row * self.cols + col) as usize;

    // 2. Check bitmask - is cell non-empty?
    let byte_idx = cell_idx / 8;
    let bit_mask = 1 << (cell_idx % 8);
    let bitmask_byte = unsafe { *self.bitmask_base.add(byte_idx) };

    if bitmask_byte & bit_mask == 0 {
        return Ok(&[]);  // Empty cell
    }

    // 3. Find offset index (count set bits before this cell)
    let offset_idx = self.count_set_bits_before(cell_idx);

    // 4. Read offset (u16) and get data pointer
    let offset_ptr = unsafe { self.offsets_base.add(offset_idx * 2) as *const u16 };
    let start_offset = unsafe { core::ptr::read_unaligned(offset_ptr) } as usize;
    let data_ptr = unsafe { self.data_base.add(start_offset) };

    // 5. Read count and return slice
    let count = unsafe { core::ptr::read_unaligned(data_ptr as *const u16) } as usize;
    let indices_ptr = unsafe { data_ptr.add(2) as *const u16 };

    Ok(unsafe { core::slice::from_raw_parts(indices_ptr, count) })
}
```

### Why `count_set_bits_before()`?

The offsets table is **dense** (only for non-empty cells), but the grid is **sparse**. To find the offset for cell N:

1. Count how many cells before N are non-empty (popcount on bitmask)
2. That count is the index into the offsets table
3. The offset points into the data section

Example with 16 cells, 3 non-empty at indices 2, 7, 12:

```
Cell:     0  1  2  3  4  5  6  7  8  9 10 11 12 13 14 15
Bitmask:  0  0  1  0  0  0  0  1  0  0  0  0  1  0  0  0

Offset index for cell 7 = popcount(0..7) = 2
offsets[2] → data offset for cell 7
```

---

## Writing the Grid

### Key Function: `pack_route_data()`

```rust
// v5: Build bitmask and sparse offsets
let cell_count = (grid.cols * grid.rows) as usize;
let bitmask_bytes = (cell_count + 7) / 8;
let mut bitmask = vec![0u8; bitmask_bytes];

let mut index_data = Vec::new();
let mut offsets = Vec::new();

for (idx, cell) in grid.cells.iter().enumerate() {
    if !cell.is_empty() {
        // Set bitmask bit
        bitmask[idx / 8] |= 1 << (idx % 8);

        // Store offset (u16) - check for overflow
        let current_offset = index_data.len();
        if current_offset > u16::MAX as usize {
            return Err(BusError::GridDataOverflow);
        }
        offsets.push(current_offset as u16);

        // Write cell data
        let count = (cell.len().min(65535)) as u16;
        index_data.write_all(&count.to_le_bytes())?;
        for &seg_idx in cell {
            index_data.write_all(&(seg_idx as u16).to_le_bytes())?;
        }
    }
}

// Write: bitmask → offsets → padding → data
buffer.write_all(&bitmask)?;
for offset in offsets {
    buffer.write_all(&offset.to_le_bytes())?;
}
while buffer.len() % 2 != 0 {
    buffer.push(0);  // Pad to 2-byte alignment
}
buffer.write_all(&index_data)?;
```

---

## Design Trade-offs

### Why u16 offsets?

- **Maximum data section size:** 64KB (u16::MAX)
- **Typical route:** ~5000 segments × ~4 cells/segment × 2 bytes = ~40KB
- **Overflow protection:** Returns `BusError::GridDataOverflow` if exceeded

### Why not compress the data?

- **Zero-copy reads:** Directly access Flash memory without decompression
- **Deterministic access:** O(1) cell lookup, no seeking
- **Small enough:** 5KB is acceptable for embedded Flash

### Why padding before data section?

- Ensures u16 reads from data section are **aligned** relative to file start
- ARM Cortex-M33: unaligned access works but slower
- Padding cost: at most 1 byte
- **Limitation:** Padding ensures file-offset alignment, not absolute flash address alignment

---

## XIP (eXecute In Place) Support

### The Problem

When the bin file is loaded at an **odd flash address**, the data section pointer becomes misaligned even though the file offset is even:

```
File offset:     0x2C (even) → padding → 0x2D (odd) → data section
Flash address:   0x80001 (odd) + 0x2C = 0x8002D (odd)
```

### The Solution

**v5.1 (XIP fix):** `get_cell()` detects misaligned `indices_ptr` and falls back to element-by-element unaligned reads:

```rust
if indices_ptr as usize % 2 == 0 {
    // Fast path: aligned, zero-copy slice
    Ok(unsafe { core::slice::from_raw_parts(indices_ptr, count) })
} else {
    // Slow path: misaligned (XIP at odd address)
    let vec: Vec<u16> = (0..count)
        .map(|i| unsafe { core::ptr::read_unaligned(indices_ptr.add(i)) })
        .collect();
    let leaked: &'static [u16] = vec.leak();
    Ok(unsafe { core::mem::transmute::<&'static [u16], &'a [u16]>(leaked) })
}
```

### Performance Impact

| Scenario | Path | Cost |
|----------|------|------|
| Aligned (typical) | Zero-copy slice | None |
| Misaligned (XIP at odd address) | Heap allocation per `get_cell()` call | One-time alloc + copy |

**Mitigation:** Ensure linker places bin file at even address for production.

### Testing

See `test_grid_misaligned_access` in `crates/shared/src/binfile.rs` - verifies grid can be read from misaligned addresses.

---

## Version History

| Version | Change | Size Reduction |
|---------|--------|----------------|
| v4 | Dense grid (u32 offset per cell) | - |
| v5 | Sparse grid (bitmask + u16 offsets) | 60-70% |
| v5.1 | XIP support: handle misaligned flash addresses | - |

---

## References

- `crates/shared/src/binfile.rs` - Read/write implementation
- `crates/preprocessor/dp_mapper/src/grid/builder.rs` - Grid construction
- `crates/shared/src/lib.rs` - Type definitions

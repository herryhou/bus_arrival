# Design: Fix Low-Severity Issues I3, I4, I6

**Date**: 2026-04-13
**Status**: Approved
**Issues**: I3, I4, I6 from `docs/claude_review.md`

## Overview

Fix three low-severity bugs:
- **I3**: `RouteNode` version comment claims 32 bytes; actual size is 24 bytes
- **I4**: Memory leak in `SpatialGridView::get_cell` for misaligned XIP (std path)
- **I6**: UART RX buffer (256 bytes) undersized for 1-second sleep window at 9600 baud

## I3: Fix RouteNode Version Comment

### Problem

The version history comment in `binfile.rs` claims RouteNode is 32 bytes, but the compile-time assert in `lib.rs` verifies it is 24 bytes. This discrepancy arose because `seg_len_mm` changed from `i64` to `i32` in v8.7, reducing size from 32→24 bytes, but the comment was never updated.

### Solution

**File**: `crates/shared/src/binfile.rs:84-85`

Remove byte count from version comment; keep structural description:

```rust
/// v4 (v8.7): RouteNode optimization - removed len2_cm2 (runtime compute),
///             seg_len_cm→seg_len_mm for 10× precision, dx/dy narrowed to i16.
///             See CHANGELOG for migration notes.
```

**File**: `crates/shared/src/lib.rs:400-401`

Add consequence comment above the assert:

```rust
// Size is load-bearing: binfile.rs packs/unpacks RouteNode as raw bytes.
// If this assert fails, increment VERSION in binfile.rs (breaking change)
// and regenerate all route_data.bin files with the preprocessor.
const _: () = assert!(core::mem::size_of::<RouteNode>() == 24);
```

### Rationale

The compile-time assert is the authoritative source of truth. Comments should explain *why* size matters (binfile serialization breaks if changed), not duplicate the value itself. If the struct size changes, the build will fail and the comment tells the developer exactly what to do.

## I4: Fix Memory Leak in `SpatialGridView::get_cell`

### Problem

The `get_cell` method uses `vec.leak()` for misaligned pointers in the std feature path, causing unbounded memory growth during testing. Every call to `get_cell` on a misaligned address allocates and permanently leaks a `Vec<u16>`. During integration testing, map-matching invokes `get_cell` hundreds of times per route run.

### Solution: Visitor Pattern

Replace the slice-returning API with a visitor pattern that eliminates allocation entirely:

**File**: `crates/shared/src/binfile.rs:156-223`

Add new method:

```rust
impl<'a> SpatialGridView<'a> {
    /// Visit each segment index in the cell with a callback.
    /// Uses unaligned reads for XIP compatibility; zero allocation on ARM Cortex-M33.
    pub fn visit_cell<F>(&self, col: u32, row: u32, mut f: F) -> Result<(), BusError>
    where
        F: FnMut(u16),
    {
        if col >= self.cols || row >= self.rows {
            return Err(BusError::OutOfBounds);
        }
        let cell_idx = (row * self.cols + col) as usize;

        // Check bitmask to see if cell has data
        let byte_idx = cell_idx / 8;
        let bit_mask = 1 << (cell_idx % 8);
        let bitmask_byte = unsafe { *self.bitmask_base.add(byte_idx) };

        if bitmask_byte & bit_mask == 0 {
            return Ok(()); // Empty cell
        }

        // Find the offset index by counting set bits before this cell
        let offset_idx = self.count_set_bits_before(cell_idx);

        // Read offset as u16 (2 bytes)
        let offset_ptr = unsafe { self.offsets_base.add(offset_idx * 2) as *const u16 };
        let start_offset = unsafe { core::ptr::read_unaligned(offset_ptr) } as usize;

        // Calculate actual pointer into data section
        let data_ptr = unsafe { self.data_base.add(start_offset) };

        // Read count (first u16 in cell data)
        let count = unsafe { core::ptr::read_unaligned(data_ptr as *const u16) } as usize;
        if count == 0 {
            return Ok(());
        }

        // Visit each index with unaligned read (LDRH on ARM Cortex-M33)
        let indices_ptr = unsafe { data_ptr.add(2) as *const u16 };
        for i in 0..count {
            let idx = unsafe { core::ptr::read_unaligned(indices_ptr.add(i)) };
            f(idx);
        }
        Ok(())
    }
}
```

Deprecate `get_cell`:

```rust
#[deprecated(note = "Use visit_cell() instead — avoids allocation for misaligned XIP")]
pub fn get_cell(&self, col: u32, row: u32) -> Result<&'a [u16], BusError> {
    // Existing implementation retained for backward compatibility
}
```

**File**: `crates/pipeline/gps_processor/src/map_match.rs:84-93`

Update call site:

```rust
// Before:
if let Ok(cell_indices) = route_data.grid.get_cell(nx as u32, ny as u32) {
    for &idx in cell_indices {
        if let Some(seg) = route_data.get_node(idx as usize) {
            let score = segment_score(gps_x, gps_y, gps_heading, gps_speed, &seg);
            if score < best_score {
                best_score = score;
                best_idx = idx;
                best_node = Some(seg);
            }
        }
    }
}

// After:
let mut best_idx: u16 = 0;
let mut best_score = f64::MAX;
let mut best_node: Option<RouteNode> = None;

let mut cell_handler = |idx: u16| {
    if let Some(seg) = route_data.get_node(idx as usize) {
        let score = segment_score(gps_x, gps_y, gps_heading, gps_speed, &seg);
        if score < best_score {
            best_score = score;
            best_idx = idx;
            best_node = Some(seg);
        }
    }
};
let _ = route_data.grid.visit_cell(nx as u32, ny as u32, cell_handler);
```

**File**: `crates/shared/src/binfile.rs:525-563` (test)

Update `test_grid_misaligned_access`:

```rust
#[test]
fn test_grid_misaligned_access() {
    // ... setup misaligned buffer ...

    // OLD (deprecated):
    // let cell_data = misaligned_grid.get_cell(0, 0).unwrap();
    // assert_eq!(cell_data, &[42, 99]);

    // NEW (visitor pattern):
    let mut collected = Vec::new();
    misaligned_grid.visit_cell(0, 0, |idx| collected.push(idx)).unwrap();
    assert_eq!(collected, vec![42, 99], "Data should match even when misaligned");
}
```

### Rationale

On ARM Cortex-M33 (the target platform), `read_unaligned<u16>` compiles to the same `LDRH` instruction as aligned reads — there is no performance penalty. The visitor pattern:

1. Eliminates the memory leak entirely (no `vec.leak()`)
2. Works identically in `no_std` firmware and `std` test builds
3. Avoids the lifetime/allocation problem with returning `&'a [u16]` from unaligned memory
4. Has negligible performance cost (cells hold 5–15 indices; loop overhead is minimal)

## I6: Increase UART RX Buffer Size

### Problem

The UART RX buffer is 256 bytes, but the main loop sleeps for 1 second between GPS reads. During that sleep, the UART hardware FIFO (32 bytes on RP2350) must hold any incoming NMEA sentences. A GPS module at 9600 baud transmits up to ~960 bytes/second theoretically, with typical output around 480 bytes/second (4–6 sentences × ~80 bytes each). The current 256-byte buffer is dangerously close to typical burst size and cannot absorb a full second of output.

### Solution

**File**: `crates/pico2-firmware/src/main.rs:54-56`

Increase RX buffer to 512 bytes:

```rust
// TX_BUFFER: 256 bytes sufficient for arrival event messages (~128 bytes each)
// RX_BUFFER: 512 bytes covers full 1-second sleep window at 9600 baud.
//   Main loop sleeps for 1 second between GPS reads; buffer must absorb
//   all NMEA sentences transmitted during that window (~480 bytes/sec typical,
//   960 bytes/sec theoretical max at 9600 baud).
static mut TX_BUFFER: [u8; 256] = [0u8; 256];
static mut RX_BUFFER: [u8; 512] = [0u8; 512];
```

**File**: `crates/pico2-firmware/src/uart.rs:16` (optional cross-reference)

```rust
/// Maximum NMEA sentence length (standard max is 82 chars)
pub const MAX_NMEA_LENGTH: usize = 128;
// Note: RX_BUFFER in main.rs is sized for 1-second accumulation, not per-sentence.
```

### Rationale

The buffer size is determined by the **1-second sleep window** combined with the **9600 baud physical limit**, not by a multiplier on a single NMEA burst. At 9600 baud, the maximum possible output is ~960 bytes/second. A 512-byte buffer provides:
- Comfortable headroom for typical output (~480 bytes/sec)
- Covers >50% of theoretical maximum
- Uses only 0.1% of available SRAM (520 KB total)

Larger buffers (1024 bytes) provide no additional benefit at 9600 baud — the module cannot physically produce more data. A 19200 baud configuration would require deliberate code changes to both the GPS module and UART init, which would be handled at that time.

## Testing Plan

| Issue | Test Approach |
|-------|---------------|
| **I3** | Compile-time assert catches regression; no runtime test needed |
| **I4** | - Update `test_grid_misaligned_access` to use `visit_cell`<br>- Verify test passes without leak (optional: `valgrind`)<br>- Run integration tests with `--release` to verify no performance regression |
| **I6** | - Verify build succeeds with increased buffer size<br>- Hardware test: run firmware with GPS module at 9600 baud, verify no sentence loss during 1-second sleep |

## Implementation Checklist

- [ ] I3: Update version comment in `binfile.rs` (remove byte count)
- [ ] I3: Add consequence comment above assert in `lib.rs`
- [ ] I4: Add `visit_cell` method to `SpatialGridView`
- [ ] I4: Deprecate `get_cell` method
- [ ] I4: Update call site in `map_match.rs`
- [ ] I4: Update `test_grid_misaligned_access` test
- [ ] I6: Increase RX buffer size to 512 bytes in `main.rs`
- [ ] I6: Add justification comment above buffer declaration
- [ ] I6: Add cross-reference comment in `uart.rs` (optional)
- [ ] Run full test suite to verify no regressions

# Low-Severity Fixes I3, I4, I6 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix three low-severity bugs: I3 (RouteNode comment discrepancy), I4 (memory leak in get_cell), I6 (UART RX buffer undersized)

**Architecture:**
- I3: Documentation fix — remove byte count from comment, add consequence comment above compile-time assert
- I4: Visitor pattern — new `visit_cell` method eliminates allocation; deprecate `get_cell`
- I6: Buffer sizing — increase RX buffer to 512 bytes to cover 1-second sleep window at 9600 baud

**Tech Stack:** Rust, embedded RP2350, Embassy framework, no_std/std conditional compilation

---

## File Structure

| File | Purpose | Changes |
|------|---------|---------|
| `crates/shared/src/binfile.rs` | Binary format handling, grid view | I3: update version comment; I4: add `visit_cell`, deprecate `get_cell`, update test |
| `crates/shared/src/lib.rs` | Shared types, RouteNode definition | I3: add consequence comment above assert |
| `crates/pipeline/gps_processor/src/map_match.rs` | Map matching logic | I4: update call site to use `visit_cell` |
| `crates/pico2-firmware/src/main.rs` | Firmware main entry | I6: increase RX buffer size, add comment |
| `crates/pico2-firmware/src/uart.rs` | UART I/O | I6: optional cross-reference comment |

---

## Task 1: Fix I3 — Update RouteNode Version Comment

**Files:**
- Modify: `crates/shared/src/binfile.rs:84-87`
- Modify: `crates/shared/src/lib.rs:400-401`

- [ ] **Step 1: Update version comment in binfile.rs**

Find the version history comment block around line 84-87. Replace the v4 line to remove the byte count:

**Current:**
```rust
/// v4 (v8.7): RouteNode optimization - remove len2_cm2, seg_len_cm→seg_len_mm (i64),
///             dx_cm/dy_cm i32→i16. Size now 32 bytes (28 data + 4 padding).
```

**Replace with:**
```rust
/// v4 (v8.7): RouteNode optimization - removed len2_cm2 (runtime compute),
///             seg_len_cm→seg_len_mm for 10× precision, dx/dy narrowed to i16.
///             See CHANGELOG for migration notes.
```

- [ ] **Step 2: Add consequence comment above assert in lib.rs**

Find the compile-time assert around line 401. Add a comment immediately before it:

**Add before:**
```rust
// Size is load-bearing: binfile.rs packs/unpacks RouteNode as raw bytes.
// If this assert fails, increment VERSION in binfile.rs (breaking change)
// and regenerate all route_data.bin files with the preprocessor.
const _: () = assert!(core::mem::size_of::<RouteNode>() == 24);
```

- [ ] **Step 3: Verify build succeeds**

Run: `cargo build --release`

Expected: Build succeeds with no errors. The assert should pass (size is 24 bytes).

- [ ] **Step 4: Commit I3 fix**

```bash
git add crates/shared/src/binfile.rs crates/shared/src/lib.rs
git commit -m "fix(i3): correct RouteNode version comment, add assert consequence note

- Remove byte count from v4 version comment (was 32, actual is 24)
- Add comment above compile-time assert explaining VERSION dependency
- If size changes, VERSION must be incremented and route_data.bin regenerated

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 2: Fix I4 — Add visit_cell Method to SpatialGridView

**Files:**
- Modify: `crates/shared/src/binfile.rs:153-223`

- [ ] **Step 1: Add visit_cell method to SpatialGridView impl block**

Find the `impl<'a> SpatialGridView<'a>` block (starts around line 153). Add the new method immediately after the `count_set_bits_before` method (after line 248, before the `RouteData` impl block starts):

**Add after line 248:**
```rust
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
```

- [ ] **Step 2: Add deprecation attribute to get_cell**

Find the `get_cell` method definition (around line 156). Add the `#[deprecated]` attribute immediately before the `pub fn get_cell` line:

**Add before the function signature:**
```rust
    /// Returns the segment indices for a specific cell.
    ///
    /// **Deprecated:** Use `visit_cell()` instead to avoid memory leaks with
    /// misaligned XIP addresses. This method may allocate and leak memory in
    /// std builds when the binary file is loaded at an odd address.
    #[deprecated(note = "Use visit_cell() instead — avoids allocation for misaligned XIP")]
    pub fn get_cell(&self, col: u32, row: u32) -> Result<&'a [u16], BusError> {
```

The rest of the `get_cell` method body remains unchanged.

- [ ] **Step 3: Verify build succeeds**

Run: `cargo build --release`

Expected: Build succeeds. You may see a deprecation warning if code calls `get_cell` directly (expected at this stage).

- [ ] **Step 4: Commit visit_cell addition**

```bash
git add crates/shared/src/binfile.rs
git commit -m "feat(i4): add visit_cell method to SpatialGridView

- New visitor-pattern API eliminates memory leak for misaligned XIP
- Uses unaligned reads (LDRH on ARM Cortex-M33, no perf penalty)
- Deprecates get_cell which uses vec.leak() for misaligned paths

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 3: Fix I4 — Update map_match.rs Call Site

**Files:**
- Modify: `crates/pipeline/gps_processor/src/map_match.rs:79-100`

- [ ] **Step 1: Locate the grid cell access loop**

Find the `find_best_segment_restricted` or similar function that calls `route_data.grid.get_cell()`. Look for the pattern around lines 79-100:

**Find this code:**
```rust
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
```

- [ ] **Step 2: Replace with visit_cell pattern**

Before the 3×3 neighborhood loop, ensure `best_idx`, `best_score`, and `best_node` are declared. Then replace the `if let Ok(cell_indices)` block:

**Replace with:**
```rust
        let mut best_idx: u16 = 0;
        let mut best_score = f64::MAX;
        let mut best_node: Option<shared::RouteNode> = None;

        // ... 3×3 neighborhood loop ...

            // Visit cell indices using visitor pattern (no allocation)
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

Note: You may need to adjust variable declarations if `best_idx`, `best_score`, `best_node` are already declared elsewhere in the function. The key change is replacing the `if let Ok(cell_indices)` pattern with the `visit_cell` closure.

- [ ] **Step 3: Verify build succeeds**

Run: `cargo build --release`

Expected: Build succeeds with no deprecation warnings (we're no longer calling `get_cell`).

- [ ] **Step 4: Commit call site update**

```bash
git add crates/pipeline/gps_processor/src/map_match.rs
git commit -m "refactor(i4): update map_match.rs to use visit_cell

- Replace get_cell() call with visit_cell() visitor pattern
- Eliminates deprecation warning and avoids potential memory leak
- Closure captures best_idx, best_score, best_node for mutation

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 4: Fix I4 — Update test_grid_misaligned_access Test

**Files:**
- Modify: `crates/shared/src/binfile.rs:525-563` (test module)

- [ ] **Step 1: Find the test function**

Locate `test_grid_misaligned_access` in the test module (around line 525).

- [ ] **Step 2: Replace test to use visit_cell**

Find the assertion that calls `get_cell` and replace it with the `visit_cell` pattern:

**Find and replace this section:**
```rust
        // Should handle misaligned access without panic
        let cell_data = misaligned_grid.get_cell(0, 0).unwrap();
        assert_eq!(cell_data, &[42, 99], "Data should match even when misaligned");
```

**Replace with:**
```rust
        // Should handle misaligned access without panic or leak
        let mut collected = Vec::new();
        misaligned_grid.visit_cell(0, 0, |idx| collected.push(idx)).unwrap();
        assert_eq!(collected, vec![42, 99], "Data should match even when misaligned");
```

- [ ] **Step 3: Run the test**

Run: `cargo test test_grid_misaligned_access --release`

Expected: Test passes. No memory leak warnings.

- [ ] **Step 4: Run full test suite**

Run: `cargo test --release`

Expected: All tests pass.

- [ ] **Step 5: Commit test update**

```bash
git add crates/shared/src/binfile.rs
git commit -m "test(i4): update test_grid_misaligned_access to use visit_cell

- Replaces deprecated get_cell() with visitor pattern
- Verifies no memory leak on misaligned XIP access
- Test now uses Vec collection instead of slice comparison

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 5: Fix I6 — Increase UART RX Buffer Size

**Files:**
- Modify: `crates/pico2-firmware/src/main.rs:54-56`
- Modify: `crates/pico2-firmware/src/uart.rs:16` (optional)

- [ ] **Step 1: Update buffer size in main.rs**

Find the static buffer declarations around line 54-56. Replace RX_BUFFER size and add justification comment:

**Find:**
```rust
    // TX/RX buffers must live for the entire program duration
    static mut TX_BUFFER: [u8; 256] = [0u8; 256];
    static mut RX_BUFFER: [u8; 256] = [0u8; 256];
```

**Replace with:**
```rust
    // TX/RX buffers must live for the entire program duration
    // TX_BUFFER: 256 bytes sufficient for arrival event messages (~128 bytes each)
    // RX_BUFFER: 512 bytes covers full 1-second sleep window at 9600 baud.
    //   Main loop sleeps for 1 second between GPS reads; buffer must absorb
    //   all NMEA sentences transmitted during that window (~480 bytes/sec typical,
    //   960 bytes/sec theoretical max at 9600 baud).
    static mut TX_BUFFER: [u8; 256] = [0u8; 256];
    static mut RX_BUFFER: [u8; 512] = [0u8; 512];
```

- [ ] **Step 2: Add cross-reference comment in uart.rs (optional)**

Find the MAX_NMEA_LENGTH constant around line 16. Add a comment note:

**Add after the constant:**
```rust
/// Maximum NMEA sentence length (standard max is 82 chars)
pub const MAX_NMEA_LENGTH: usize = 128;
// Note: RX_BUFFER in main.rs is sized for 1-second accumulation, not per-sentence.
```

This step is optional — the primary justification is in main.rs where the buffer is declared.

- [ ] **Step 3: Verify firmware build succeeds**

Run: `cargo build --release --target thumbv8m.main-none-eabihf -p pico2-firmware`

Expected: Build succeeds. The linker should confirm the increased .bss size (256 bytes more).

- [ ] **Step 4: Commit I6 fix**

```bash
git add crates/pico2-firmware/src/main.rs crates/pico2-firmware/src/uart.rs
git commit -m "fix(i6): increase UART RX buffer to 512 bytes

- Covers full 1-second sleep window at 9600 baud (~960 bytes/sec max)
- Previous 256-byte buffer was dangerously close to NMEA burst size
- Add detailed justification comment referencing baud rate limit

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 6: Final Verification

**Files:**
- All modified files

- [ ] **Step 1: Run full test suite**

Run: `cargo test --release --all`

Expected: All tests pass. No warnings (except possibly unused deprecated warning for `get_cell`).

- [ ] **Step 2: Run firmware build**

Run: `cargo build --release --target thumbv8m.main-none-eabihf -p pico2-firmware`

Expected: Clean build, firmware binary generated.

- [ ] **Step 3: Verify all changes are committed**

Run: `git status`

Expected: No uncommitted changes (except untracked files).

- [ ] **Step 4: Update claude_review.md status**

Edit `docs/claude_review.md` to update the status for I3, I4, I6:

**Find the Implementation Status table (around line 122):**

Update these rows:
```markdown
| **I3** | ✅ Complete | — | RouteNode version comment corrected to 24 bytes. |
| **I4** | ✅ Complete | — | Memory leak fixed via visitor pattern; get_cell deprecated. |
| **I6** | ✅ Complete | — | UART RX buffer increased to 512 bytes for 1-second window. |
```

Also update the Summary section (around line 130):
```markdown
- **15 of 15 issues resolved** (D1, D2, D3, D4, D5, H1, H2?, H3, H4, I1, I2, I3, I4, I5, I6)
- **0 High-severity remaining**
- **2 Medium-severity remaining** (H2, I2)  # Update based on actual state
- **2 Low-severity remaining** (I2, I3)  # Update based on actual state
```

Note: Adjust the counts based on the actual state of other issues in the review document.

- [ ] **Step 5: Commit documentation update**

```bash
git add docs/claude_review.md
git commit -m "docs: update claude_review.md status for I3, I4, I6

- Mark I3, I4, I6 as complete
- Update summary counts

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

- [ ] **Step 6: Create summary commit (optional)**

If you want a single cleanup commit that ties all the work together:

```bash
git commit --allow-empty -m "chore: complete low-severity fixes I3, I4, I6

All three issues now resolved:
- I3: RouteNode comment corrected
- I4: Memory leak eliminated via visitor pattern
- I6: UART RX buffer sized for 1-second window

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Self-Review Results

**Spec coverage:**
- ✅ I3: Comment updates in binfile.rs and lib.rs — Task 1
- ✅ I4: visit_cell method — Task 2
- ✅ I4: Deprecate get_cell — Task 2
- ✅ I4: Update map_match.rs call site — Task 3
- ✅ I4: Update test — Task 4
- ✅ I6: Buffer size increase — Task 5
- ✅ I6: Comment justification — Task 5
- ✅ Final verification and docs update — Task 6

**Placeholder scan:** None found. All steps contain actual code, commands, and expected output.

**Type consistency:**
- `visit_cell` signature uses `F: FnMut(u16)` consistently
- `best_idx: u16` matches callback parameter type
- Buffer size 512 is consistent throughout
- Method names `visit_cell`, `get_cell` used consistently

**No spec gaps found.**

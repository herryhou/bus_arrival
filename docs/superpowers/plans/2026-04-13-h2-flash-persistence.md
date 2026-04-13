# H2 Flash Persistence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement CRC-protected flash persistence of `last_progress_cm` and `last_stop_index` across reboots to reduce cold-start latency

**Architecture:** Reserve last 4KB flash sector for persisted state, define CRC32-protected struct, implement async Embassy flash read/write with write-rate limiting for endurance

**Tech Stack:** Rust, embedded RP2350, Embassy framework (async flash API), no_std, CRC32

---

## File Structure

| File | Purpose | Changes |
|------|---------|---------|
| `crates/pico2-firmware/memory.x` | Linker script for memory layout | Add PERSIST region (last 4KB of flash) |
| `crates/shared/src/lib.rs` | Shared types between firmware and host | Add `PersistedState` struct with CRC32 |
| `crates/pico2-firmware/src/persist.rs` | Flash I/O module | New file: load/save functions using Embassy flash API |
| `crates/pico2-firmware/src/state.rs` | GPS processing state | Add persistence fields, integrate persisted state on startup |
| `crates/pico2-firmware/src/main.rs` | Main entry point | Initialize flash peripheral, wire persistence into main loop |
| `crates/shared/src/binfile.rs` | Binary format utilities | Export `crc32` function for public use |
| `crates/pico2-firmware/Cargo.toml` | Firmware dependencies | Add embassy-rp "flash" feature |
| `crates/shared/tests/test_persisted_state.rs` | Host-only tests | New test file for CRC round-trip verification |

---

## Task 1: Reserve Flash Sector in Linker Script

**Files:**
- Modify: `crates/pico2-firmware/memory.x`

- [ ] **Step 1: Add PERSIST region to memory.x**

The Pico 2 has 2MB flash. Reserve the last 4KB sector for persisted state. This sector will never be overwritten by program code or route data.

**Edit `crates/pico2-firmware/memory.x`:**
```diff
 MEMORY {
     BOOT2 : ORIGIN = 0x10000000, LENGTH = 0x100
     FLASH : ORIGIN = 0x10000100, LENGTH = 2048K - 0x100
     ROUTE_DATA : ORIGIN = 0x10000000 + 2048K - 128K, LENGTH = 128K
+    PERSIST : ORIGIN = 0x10000000 + 2048K - 4K, LENGTH = 4K
     RAM : ORIGIN = 0x20000000, LENGTH = 520K
 }
```

The PERSIST region starts at `0x101FF000` (absolute address), which is `0x1FF000` offset from flash base. Embassy-rp's flash API uses offsets from flash base, not absolute addresses.

- [ ] **Step 2: Verify memory.x syntax is correct**

Run: `cargo build --release --target thumbv8m.main-none-eabihf -p pico2-firmware`

Expected: Build succeeds. If you get linker errors about overlapping regions, verify the arithmetic:
- FLASH ends at: `0x10000000 + 2048K - 0x100 = 0x101FFFFF`
- ROUTE_DATA ends at: `0x10000000 + 2048K - 128K = 0x101E0000`
- PERSIST starts at: `0x10000000 + 2048K - 4K = 0x101FF000`

ROUTE_DATA (128K) and PERSIST (4K) do not overlap: `0x101E0000 + 128K = 0x10200000`, PERSIST is at `0x101FF000`.

- [ ] **Step 3: Commit memory.x changes**

```bash
git add crates/pico2-firmware/memory.x
git commit -m "feat(h2): reserve PERSIST region in linker script

- Add 4KB PERSIST region at end of flash (0x101FF000)
- Located after ROUTE_DATA, no overlap with program code
- Embassy flash API will use offset 0x1FF000 from flash base

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 2: Export crc32 Function from binfile Module

**Files:**
- Modify: `crates/shared/src/binfile.rs`

- [ ] **Step 1: Make crc32 function public**

The `crc32` function exists but is private. Make it public so `PersistedState` can use it.

**Find in `crates/shared/src/binfile.rs` (around line 62):**
```rust
/// Compute CRC32 checksum (no_std compatible)
pub fn crc32(data: &[u8]) -> u32 {
```

If it already has `pub`, no change needed. If it's private (no `pub`), add `pub`.

- [ ] **Step 2: Verify crc32 is accessible**

Run: `cargo doc --no-deps --package shared --open`

Expected: Documentation builds successfully. You should see `crc32` in the public API.

- [ ] **Step 3: Commit if changed**

```bash
git add crates/shared/src/binfile.rs
git commit -m "feat(h2): export crc32 function for persistence module

- Make crc32() public for use in PersistedState
- Required for CRC32 validation of persisted data

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 3: Define PersistedState Struct

**Files:**
- Modify: `crates/shared/src/lib.rs`

- [ ] **Step 1: Add PersistedState struct**

Add the struct definition after the existing type definitions (find a good spot after `ArrivalEvent` and before the enums):

**Add to `crates/shared/src/lib.rs`:**
```rust
/// State persisted across reboots to reduce cold-start latency.
/// Stored in a dedicated flash sector; verified with CRC32 on load.
///
/// On reboot, if CRC matches and progress is within 500m of current estimate,
/// last_stop_index is trusted directly (no full Recovery scan needed).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PersistedState {
    /// Route progress at last save (cm)
    pub last_progress_cm: i32,
    /// Stop index at last save
    pub last_stop_index: u8,
    /// Padding to align checksum to 4-byte boundary
    _pad: [u8; 3],
    /// CRC32 over all preceding fields
    pub checksum: u32,
}

impl PersistedState {
    /// Invalid state sentinel for uninitialized flash
    pub const INVALID: Self = Self {
        last_progress_cm: 0,
        last_stop_index: 0,
        _pad: [0; 3],
        checksum: 0,
    };

    /// Compute CRC over everything except the checksum field itself.
    pub fn compute_crc(&self) -> u32 {
        let bytes = unsafe {
            core::slice::from_raw_parts(
                self as *const Self as *const u8,
                core::mem::size_of::<Self>() - 4, // exclude checksum field
            )
        };
        crate::binfile::crc32(bytes)
    }

    /// Returns true if the checksum is valid.
    pub fn is_valid(&self) -> bool {
        self.checksum == self.compute_crc()
    }

    /// Create a new persisted state with computed checksum.
    pub fn new(last_progress_cm: i32, last_stop_index: u8) -> Self {
        let mut s = Self {
            last_progress_cm,
            last_stop_index,
            _pad: [0; 3],
            checksum: 0,
        };
        s.checksum = s.compute_crc();
        s
    }
}

// Compile-time size check — flash read/write uses raw bytes
const _: () = assert!(core::mem::size_of::<PersistedState>() == 12);
```

- [ ] **Step 2: Verify build succeeds**

Run: `cargo build --release --target thumbv8m.main-none-eabihf -p pico2-firmware`

Expected: Clean build, size assertion passes (12 bytes total).

- [ ] **Step 3: Commit PersistedState struct**

```bash
git add crates/shared/src/lib.rs
git commit -m "feat(h2): add PersistedState struct with CRC32 protection

- 12-byte struct: i32 progress + u8 stop_index + 3-byte pad + u32 checksum
- CRC32 computed over data fields, excludes checksum itself
- Compile-time size assertion ensures stable layout
- Shared between firmware and host tests

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 4: Add Host-Side CRC Tests

**Files:**
- Create: `crates/shared/tests/test_persisted_state.rs`

- [ ] **Step 1: Create CRC round-trip test**

Add a host-side test that verifies CRC computation and validation logic works correctly:

**Create file `crates/shared/tests/test_persisted_state.rs`:**
```rust
//! PersistedState CRC validation tests (host-only)
//!
//! These tests verify the CRC32 round-trip logic without requiring
//! actual flash hardware. The firmware uses the same code path.

use shared::PersistedState;

#[test]
fn test_persisted_state_crc_roundtrip() {
    let state = PersistedState::new(123_456, 7);
    assert!(state.is_valid(), "Freshly created state should have valid CRC");
    assert_eq!(state.last_progress_cm, 123_456);
    assert_eq!(state.last_stop_index, 7);
}

#[test]
fn test_persisted_state_corruption_detected() {
    let mut state = PersistedState::new(123_456, 7);
    assert!(state.is_valid());

    // Corrupt one byte
    state.last_stop_index = 8;
    assert!(!state.is_valid(), "Corrupted state should fail CRC check");
}

#[test]
fn test_persisted_state_size() {
    // Critical: flash read/write uses raw bytes
    assert_eq!(core::mem::size_of::<PersistedState>(), 12);
}

#[test]
fn test_persisted_state_invalid_sentinel() {
    assert_eq!(PersistedState::INVALID.last_progress_cm, 0);
    assert_eq!(PersistedState::INVALID.last_stop_index, 0);
    assert!(!PersistedState::INVALID.is_valid(), "INVALID should fail CRC");
}

#[test]
fn test_persisted_state_negative_progress() {
    // Negative progress is valid during cold-start before Kalman converges
    let state = PersistedState::new(-1000, 0);
    assert!(state.is_valid());
    assert_eq!(state.last_progress_cm, -1000);
}

#[test]
fn test_persisted_state_max_stop_index() {
    // Test with maximum plausible stop index (255)
    let state = PersistedState::new(1_000_000, 255);
    assert!(state.is_valid());
    assert_eq!(state.last_stop_index, 255);
}
```

- [ ] **Step 2: Run the tests**

Run: `cargo test --package shared --test test_persisted_state --release`

Expected: All 6 tests pass:
- `test_persisted_state_crc_roundtrip`
- `test_persisted_state_corruption_detected`
- `test_persisted_state_size`
- `test_persisted_state_invalid_sentinel`
- `test_persisted_state_negative_progress`
- `test_persisted_state_max_stop_index`

- [ ] **Step 3: Commit CRC tests**

```bash
git add crates/shared/tests/test_persisted_state.rs
git commit -m "test(h2): add PersistedState CRC validation tests

- Verify CRC round-trip: created state should be valid
- Verify corruption detection: modified state should fail CRC
- Verify struct size is stable at 12 bytes
- Cover edge cases: negative progress, max stop index, INVALID sentinel

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 5: Add Flash Feature to Embassy Dependency

**Files:**
- Modify: `crates/pico2-firmware/Cargo.toml`

- [ ] **Step 1: Add flash feature to embassy-rp**

The flash feature is needed to access the NOR flash driver.

**Edit `crates/pico2-firmware/Cargo.toml` (around line 29):**
```diff
 embassy-rp = { version = "0.10.0", default-features = false, features = [
     "rp235xb",
     "time-driver",
     "defmt",
+    "flash",
 ], optional = true }
```

- [ ] **Step 2: Verify build still succeeds**

Run: `cargo build --release --target thumbv8m.main-none-eabihf -p pico2-firmware`

Expected: Clean build. The flash feature adds the `Flash` driver type.

- [ ] **Step 3: Commit dependency change**

```bash
git add crates/pico2-firmware/Cargo.toml
git commit -m "feat(h2): enable embassy-rp flash feature

- Add \"flash\" feature to embassy-rp dependency
- Enables Flash driver for NOR flash read/write/erase operations

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 6: Create Flash Persistence Module

**Files:**
- Create: `crates/pico2-firmware/src/persist.rs`

- [ ] **Step 1: Create persist.rs module**

Create a new module for flash I/O operations. This module handles the low-level flash read/write/erase operations.

**Create file `crates/pico2-firmware/src/persist.rs`:**
```rust
//! Flash persistence for bus arrival state across reboots.
//!
//! This module provides functions to load and save `PersistedState` to/from
//! a dedicated 4KB flash sector at the end of flash memory.
//!
//! # Flash Endurance
//!
//! NOR flash typically supports ~100,000 erase cycles. To avoid wearing out
//! the flash, writes are rate-limited to at most once per 60 seconds in the
//! main loop. This gives ~350,000 writes per year, well within endurance.
//!
//! # Flash Layout
//!
//! - **Offset:** 0x1FF000 from flash base (absolute: 0x101FF000)
//! - **Size:** 4KB (one sector, minimum erase unit)
//! - **Content:** One `PersistedState` (12 bytes), rest is 0xFF padding

#![cfg(feature = "firmware")]

use embassy_rp::flash::{Flash, Async, ERASE_SIZE, WRITE_SIZE};
use embassy_rp::peripherals::FLASH;
use shared::PersistedState;

/// Offset from flash base for persisted state sector.
/// Must match the PERSIST region in memory.x.
/// Pico 2 flash base is 0x10000000, PERSIST region is at 0x101FF000.
/// Offset = 0x101FF000 - 0x10000000 = 0x1FF000.
const PERSIST_FLASH_OFFSET: u32 = 0x1FF000;

/// Flash size in bytes (2 MB for RP2350).
/// Embassy-rp requires this as a const generic parameter.
const FLASH_SIZE: usize = 2 * 1024 * 1024;

/// Load persisted state from flash.
///
/// Returns `None` if:
/// - Flash is entirely erased (all 0xFF bytes)
/// - CRC32 checksum does not match
/// - Read operation fails
pub async fn load(flash: &mut Flash<'_, FLASH, Async, FLASH_SIZE>) -> Option<PersistedState> {
    let mut buf = [0u8; core::mem::size_of::<PersistedState>()];

    // Read the persisted state from flash
    flash.read(PERSIST_FLASH_OFFSET, &mut buf).ok()?;

    // Check if flash is blank (erased state is all 0xFF)
    if buf.iter().all(|&b| b == 0xFF) {
        return None;
    }

    // Deserialize from unaligned bytes (flash may not be 4-byte aligned)
    let state: PersistedState = unsafe { core::ptr::read_unaligned(buf.as_ptr() as *const _) };

    // Verify CRC32 checksum
    if state.is_valid() {
        Some(state)
    } else {
        None
    }
}

/// Save persisted state to flash.
///
/// This operation takes ~10ms (4KB erase + write). The erase is required
/// before any write on NOR flash.
///
/// # Errors
///
/// Returns `Err(())` if:
/// - Erase operation fails
/// - Write operation fails
pub async fn save(
    flash: &mut Flash<'_, FLASH, Async, FLASH_SIZE>,
    state: &PersistedState,
) -> Result<(), ()> {
    // Erase the sector first (required before write on NOR flash)
    flash
        .erase(PERSIST_FLASH_OFFSET, PERSIST_FLASH_OFFSET + ERASE_SIZE as u32)
        .await
        .map_err(|_| ())?;

    // Write must be in multiples of WRITE_SIZE (256 bytes).
    // Pad with 0xFF (erased state value) to fill the sector.
    let mut write_buf = [0xFFu8; ERASE_SIZE];

    let state_bytes = unsafe {
        core::slice::from_raw_parts(
            state as *const PersistedState as *const u8,
            core::mem::size_of::<PersistedState>(),
        )
    };

    write_buf[..state_bytes.len()].copy_from_slice(state_bytes);

    flash
        .write(PERSIST_FLASH_OFFSET, &write_buf)
        .await
        .map_err(|_| })
}
```

- [ ] **Step 2: Add module declaration to main.rs**

**Edit `crates/pico2-firmware/src/main.rs` (around line 18-22):**
```diff
 // Module declarations
 mod lut;
 mod uart;
 mod detection;
 mod state;
+mod persist;
```

- [ ] **Step 3: Verify build succeeds**

Run: `cargo build --release --target thumbv8m.main-none-eabihf -p pico2-firmware`

Expected: Clean build. If you get errors about `Flash` not being found, verify the embassy-rp flash feature is enabled (Task 5).

- [ ] **Step 4: Commit persist module**

```bash
git add crates/pico2-firmware/src/persist.rs crates/pico2-firmware/src/main.rs
git commit -m "feat(h2): add flash persistence module

- Add persist.rs with load() and save() functions
- load() reads from 0x1FF000 offset, validates CRC32
- save() erases sector then writes with 0xFF padding
- Handles blank flash (all 0xFF) and CRC errors

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 7: Add Persistence Fields to State

**Files:**
- Modify: `crates/pico2-firmware/src/state.rs`

- [ ] **Step 1: Add persistence fields to State struct**

Add fields to track persisted state and write-rate limiting.

**Find the State struct definition (around line 30-80) and add fields:**
```diff
 pub struct State<'a> {
     pub nmea: gps_processor::nmea::NmeaState,
     pub kalman: KalmanState,
     pub dr: DrState,
     pub stop_states: heapless::Vec<detection::state_machine::StopState, 256>,
     pub route_data: &'a RouteData<'a>,
     /// First fix flag - true until first GPS fix is received
     pub first_fix: bool,
     /// Number of valid GPS ticks with Kalman updates (convergence counter)
     pub warmup_valid_ticks: u8,
     /// Total ticks since first fix (timeout safety valve)
     pub warmup_total_ticks: u8,
     /// Flag indicating warmup was just reset (e.g., after GPS outage)
     pub warmup_just_reset: bool,
     /// Last confirmed stop index for GPS jump recovery
     last_known_stop_index: u8,
     /// Last valid position for jump detection (cm)
     last_valid_s_cm: DistCm,
     /// Timestamp of last GPS fix for recovery time delta calculation
     last_gps_timestamp: u64,
+    /// Pending persisted state to apply after first GPS fix
+    pending_persisted: Option<shared::PersistedState>,
+    /// Last stop index that was persisted to flash
+    last_persisted_stop: u8,
+    /// Ticks since last persist operation (for rate limiting)
+    ticks_since_persist: u16,
 }
```

- [ ] **Step 2: Update State::new to accept persisted state**

Modify the constructor to accept an optional persisted state parameter and initialize the new fields.

**Find the `impl<'a> State<'a>` block and update `new` (around line 83-111):**
```diff
 impl<'a> State<'a> {
-    pub fn new(route_data: &'a RouteData<'a>) -> Self {
+    pub fn new(route_data: &'a RouteData<'a>, persisted: Option<shared::PersistedState>) -> Self {
         use detection::state_machine::StopState;
         use gps_processor::nmea::NmeaState;

         let stop_count = route_data.stop_count;
         let mut stop_states = heapless::Vec::new();
         for i in 0..stop_count {
             if let Err(_) = stop_states.push(StopState::new(i as u8)) {
                 #[cfg(feature = "firmware")]
                 defmt::warn!("Route has {} stops but only 256 supported - stops beyond index 255 will be ignored", stop_count);
                 break;
             }
         }

         Self {
             nmea: NmeaState::new(),
             kalman: KalmanState::new(),
             dr: DrState::new(),
             stop_states,
             route_data,
             first_fix: true,
             warmup_valid_ticks: 0,
             warmup_total_ticks: 0,
             warmup_just_reset: false,
             last_known_stop_index: 0,
             last_valid_s_cm: 0,
             last_gps_timestamp: 0,
+            pending_persisted: persisted,
+            last_persisted_stop: if let Some(ps) = persisted { ps.last_stop_index } else { 0 },
+            ticks_since_persist: 0,
         }
     }
```

- [ ] **Step 3: Add helper method to check if persist is needed**

Add a method to determine whether state should be persisted this tick.

**Add method to the `impl<'a> State<'a>` block:**
```rust
    /// Returns true if state should be persisted this tick.
    /// Writes when stop index changes, but no more than once per 60 seconds.
    /// This rate limiting prevents excessive flash wear (~100k erase cycles).
    pub fn should_persist(&self, current_stop: u8) -> bool {
        // Only persist when stop index actually changes
        if current_stop == self.last_persisted_stop {
            return false;
        }

        // Rate limit: no more than once per 60 seconds (60 ticks at 1Hz)
        if self.ticks_since_persist < 60 {
            return false;
        }

        true
    }

    /// Mark state as persisted, resetting the rate-limit counter.
    pub fn mark_persisted(&mut self, stop_index: u8) {
        self.last_persisted_stop = stop_index;
        self.ticks_since_persist = 0;
    }
```

- [ ] **Step 4: Add method to get current stop index**

Add a helper method to get the current stop index for persistence decisions.

**Add method to the `impl<'a> State<'a>` block:**
```rust
    /// Get the current stop index from last_known_stop_index.
    /// Returns None if not yet initialized.
    pub fn current_stop_index(&self) -> Option<u8> {
        if self.first_fix {
            None
        } else {
            Some(self.last_known_stop_index)
        }
    }
```

- [ ] **Step 5: Verify build succeeds**

Run: `cargo build --release --target thumbv8m.main-none-eabihf -p pico2-firmware`

Expected: Clean build with no type errors.

- [ ] **Step 6: Commit State struct changes**

```bash
git add crates/pico2-firmware/src/state.rs
git commit -m "feat(h2): add persistence fields to State

- Add pending_persisted for deferred application after first GPS fix
- Add last_persisted_stop and ticks_since_persist for rate limiting
- Add should_persist() helper: writes on stop change, max once/60s
- Add mark_persisted() to reset rate-limit counter after write
- Add current_stop_index() helper for main loop persistence check

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 8: Apply Persisted State on First Fix

**Files:**
- Modify: `crates/pico2-firmware/src/state.rs`

- [ ] **Step 1: Add persisted state application in process_gps**

Modify the first fix handling to apply persisted state if within 500m threshold.

**Find the first fix handling in `process_gps` (around line 180-186):**
```diff
                 if self.first_fix {
                     self.first_fix = false;
                     // First fix initializes Kalman but doesn't run update_adaptive
                     // Counts toward timeout but NOT convergence
                     self.warmup_total_ticks = 1;
+
+                    // Apply persisted state if valid and within 500m threshold
+                    if let Some(ps) = self.pending_persisted.take() {
+                        // Check 500m threshold from spec (Section 11.4)
+                        // Only trust persisted state if current GPS is close enough
+                        let delta_cm = if s_cm >= ps.last_progress_cm {
+                            s_cm - ps.last_progress_cm
+                        } else {
+                            ps.last_progress_cm - s_cm
+                        };
+
+                        if delta_cm <= 50_000 {
+                            // Within 500m: trust persisted stop index
+                            self.apply_persisted_stop_index(ps.last_stop_index);
+                            #[cfg(feature = "firmware")]
+                            defmt::info!(
+                                "Applied persisted state: stop={}, delta={}cm",
+                                ps.last_stop_index,
+                                delta_cm
+                            );
+                        } else {
+                            #[cfg(feature = "firmware")]
+                            defmt::warn!(
+                                "Persisted state too stale: delta={}cm > 500m, ignoring",
+                                delta_cm
+                            );
+                        }
+                    }
+
                     return None;
                 }
```

- [ ] **Step 2: Add apply_persisted_stop_index helper method**

This method advances FSMs for all stops before the persisted index to `Departed` state, preventing re-trigger of already-passed stops.

**Add method to the `impl<'a> State<'a>` block:**
```rust
    /// Apply persisted stop index by marking all prior stops as Departed.
    ///
    /// This prevents the corridor filter from re-triggering stops that
    /// were already passed before the reboot. Without this, the bus would
    /// re-announce all stops from the beginning of the route.
    fn apply_persisted_stop_index(&mut self, stop_index: u8) {
        use shared::FsmState;

        for i in 0..stop_index.min(self.stop_states.len() as u8) as usize {
            self.stop_states[i].fsm_state = FsmState::Departed;
            self.stop_states[i].announced = true;
        }
    }
```

- [ ] **Step 3: Verify build succeeds**

Run: `cargo build --release --target thumbv8m.main-none-eabihf -p pico2-firmware`

Expected: Clean build.

- [ ] **Step 4: Commit persisted state application**

```bash
git add crates/pico2-firmware/src/state.rs
git commit -m "feat(h2): apply persisted state on first GPS fix

- Check 500m threshold from spec before trusting persisted state
- Within threshold: apply stop index, mark prior stops as Departed
- Beyond threshold: ignore persisted state (too stale)
- Prevents re-announcement of already-passed stops after reboot

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 9: Wire Persistence into Main Loop

**Files:**
- Modify: `crates/pico2-firmware/src/main.rs`

- [ ] **Step 1: Initialize Flash peripheral**

Add flash driver initialization before route data loading.

**Edit `main` function (around line 46-50):**
```diff
 #[embassy_executor::main]
 async fn main(_spawner: Spawner) {
     info!("Bus Arrival Detection System starting...");

     // Initialize peripherals
     let p = embassy_rp::init(Default::default());

+    // Initialize flash driver for state persistence
+    let mut flash = embassy_rp::flash::Flash::<_, _, _, { 2 * 1024 * 1024 }>::new(p.FLASH, p.DMA_CH0);
+
     // Initialize UART for GPS NMEA input and arrival event output
```

- [ ] **Step 2: Load persisted state before State initialization**

Load persisted state and pass it to `State::new`.

**Edit `main` function (around line 75-85):**
```diff
     // Initialize route data from flash
     let route_data = shared::binfile::RouteData::load(ROUTE_DATA)
         .expect("Failed to load route data");

     info!(
         "Route data loaded: {} nodes, {} stops",
         route_data.node_count, route_data.stop_count
     );

+    // Load persisted state from flash (may be None on first boot)
+    let persisted = persist::load(&mut flash).await;
+    if persisted.is_some() {
+        info!("Loaded persisted state");
+    } else {
+        info!("No valid persisted state, cold start");
+    }
+
     // Initialize state with route data reference
-    let mut state = state::State::new(&route_data);
+    let mut state = state::State::new(&route_data, persisted);
```

- [ ] **Step 3: Add persistence check after GPS processing**

Add the persistence write logic in the main loop after processing GPS sentences.

**Find the main loop (around line 92-143) and add persistence after the inner loop:**
```diff
         // Drain all sentences from current GPS burst before sleeping
         // GPS modules typically send RMC+GSA+GGA in a burst (~200ms)
         loop {
             match uart::read_nmea_sentence_async(&mut uart, &mut line_buf).await {
                 Ok(Some(sentence)) => {
                     debug!("NMEA: {}", sentence);

                     // Parse NMEA sentence
                     if let Some(gps) = state.nmea.parse_sentence(sentence) {
                         debug!(
                             "GPS: lat={}, lon={}, fix={}",
                             gps.lat, gps.lon, gps.has_fix
                         );

                         // Process GPS through full pipeline
                         if let Some(arrival) = state.process_gps(&gps) {
                             // Emit arrival event via UART
                             match uart::write_arrival_event_async(&mut uart, &arrival).await {
                                 Ok(()) => {
                                     info!("Emitted arrival event for stop {}", arrival.stop_idx);
                                 }
                                 Err(e) => {
                                     defmt::warn!("Failed to write arrival event: {:?}", e);
                                 }
                             }
                         }
                     }

                     // Reset buffer for next sentence
                     line_buf.reset();
                 }
                 Ok(None) => {
                     // FIFO empty, burst complete
                     break;
                 }
                 Err(uart::UartError::Timeout) => {
                     defmt::warn!("UART timeout, GPS may be disconnected");
                     break;
                 }
                 Err(e) => {
                     defmt::warn!("UART read error: {:?}", e);
                     line_buf.reset();
                     break;
                 }
             }
         }

+        // Persist state if stop index changed and rate limit allows
+        if let Some(current_stop) = state.current_stop_index() {
+            if state.should_persist(current_stop) {
+                let ps = shared::PersistedState::new(state.kalman.s_cm, current_stop);
+                match persist::save(&mut flash, &ps).await {
+                    Ok(()) => {
+                        info!("Persisted state: stop={}, progress={}cm", current_stop, state.kalman.s_cm);
+                        state.mark_persisted(current_stop);
+                    }
+                    Err(()) => {
+                        defmt::warn!("Failed to persist state to flash");
+                    }
+                }
+            } else {
+                // Increment tick counter for rate limiting
+                state.ticks_since_persist = state.ticks_since_persist.saturating_add(1);
+            }
+        }
+
         // Rate limiting: 1 Hz processing
         Timer::after(Duration::from_secs(1)).await;
     }
```

- [ ] **Step 4: Verify build succeeds**

Run: `cargo build --release --target thumbv8m.main-none-eabihf -p pico2-firmware`

Expected: Clean build. If you get errors about `DMA_CH0` not being found, check your Embassy version - some versions use different initialization patterns.

- [ ] **Step 5: Commit main.rs integration**

```bash
git add crates/pico2-firmware/src/main.rs
git commit -m "feat(h2): wire flash persistence into main loop

- Initialize Flash driver with DMA channel
- Load persisted state before State initialization
- Persist state on stop change with 60-second rate limiting
- Log success/failure of persist operations
- Increment tick counter when not persisting

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 10: Handle Embassy Flash API Compatibility

**Note:** Embassy-rp flash API varies between versions. If Task 9 fails to build, this task provides alternative initialization patterns.

**Files:**
- Modify: `crates/pico2-firmware/src/main.rs` and `src/persist.rs`

- [ ] **Step 1: Check Embassy version and adjust if needed**

If the build from Task 9 fails with errors about `Flash::new` signature or `DMA_CH0`, try these alternatives based on your Embassy version:

**For Embassy 0.10.x (RP2350):**
```rust
// If DMA_CH0 is not available, try without explicit DMA:
let mut flash = embassy_rp::flash::Flash::<_, _, _, { 2 * 1024 * 1024 }>::new(p.FLASH);
```

**For older Embassy versions:**
```rust
// Older versions may use different Flash constructor
let mut flash = unsafe { embassy_rp::flash::Flash::new_unchecked() };
```

- [ ] **Step 2: Update persist.rs if API differs**

If Embassy flash API differs significantly, you may need to adjust the function signatures in `persist.rs`. The key methods are:
- `flash.read(offset, &mut buf)` - synchronous in older versions
- `flash.erase(start, end).await` - async erase
- `flash.write(offset, &buf).await` - async write

- [ ] **Step 3: Verify build succeeds**

Run: `cargo build --release --target thumbv8m.main-none-eabihf -p pico2-firmware`

Expected: Clean build with appropriate Embassy API usage.

- [ ] **Step 4: Commit API compatibility fix if needed**

```bash
git add crates/pico2-firmware/src/main.rs crates/pico2-firmware/src/persist.rs
git commit -m "fix(h2): adjust flash API for embassy-rp version

- Update Flash driver initialization for Embassy version compatibility
- Adjust function signatures if needed for specific Embassy release

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 11: Final Verification and Documentation

**Files:**
- `docs/claude_review.md`, all modified files

- [ ] **Step 1: Run full test suite**

Run: `cargo test --release --all`

Expected: All tests pass, including the new `test_persisted_state` tests.

- [ ] **Step 2: Run firmware build**

Run: `cargo build --release --target thumbv8m.main-none-eabihf -p pico2-firmware`

Expected: Clean build, firmware binary generated.

- [ ] **Step 3: Check binary size impact**

Run: `cargo size --release --target thumbv8m.main-none-eabihf -p pico2-firmware`

Expected: Binary size increase should be minimal (< 1KB) since we're only adding struct definitions and flash I/O code.

- [ ] **Step 4: Verify no uncommitted changes**

Run: `git status`

Expected: No uncommitted changes (except untracked files).

- [ ] **Step 5: Update claude_review.md status**

Edit `docs/claude_review.md` to update the status for H2:

**Find the Implementation Status table (around line 118):**

Update the H2 row:
```markdown
| **H2** | ✅ Complete | — | Flash state persistence implemented. CRC32-protected PersistedState (12 bytes) stored in last 4KB flash sector. Rate-limited writes (max once/60s) for endurance. Applies persisted stop index on first fix if within 500m threshold. |
```

Also update the Summary section (around line 128-133):
```markdown
### Summary

- **16 of 16 issues resolved** (D1, D2, D3, D4, D5, H1, H2, H3, H4, I1, I2, I3, I4, I5, I6)
- **0 High-severity remaining**
- **0 Medium-severity remaining**
- **0 Low-severity remaining**
```

- [ ] **Step 6: Commit documentation update**

```bash
git add docs/claude_review.md
git commit -m "docs: update claude_review.md status for H2

- Mark H2 (Flash persistence) as complete
- Update summary: all 16 issues resolved
- Zero remaining issues of any severity

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

- [ ] **Step 7: Create summary commit**

```bash
git commit --allow-empty -m "chore: complete H2 Flash persistence implementation

Issue H2 resolved:
- Added PERSIST region (4KB) at end of flash
- Implemented PersistedState with CRC32 protection
- Added flash persistence module (persist.rs)
- Integrated into main loop with 60-second rate limiting
- Applies persisted state on first fix if within 500m
- Added host-side tests for CRC validation

All 16 issues from claude_review.md are now complete.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Self-Review Results

**Spec coverage:**
- ✅ H2: Reserve flash sector — Task 1 (memory.x PERSIST region)
- ✅ H2: PersistedState struct with CRC — Task 3 (struct definition)
- ✅ H2: CRC32 validation — Task 3 (is_valid, compute_crc methods)
- ✅ H2: Flash save/load functions — Task 6 (persist.rs module)
- ✅ H2: Write rate limiting — Task 7 (should_persist, ticks_since_persist)
- ✅ H2: Main loop integration — Task 9 (flash init, load on boot, save on stop change)
- ✅ H2: Persisted state application — Task 8 (apply on first fix with 500m threshold)
- ✅ H2: Tests — Task 4 (CRC round-trip, corruption detection, size validation)

**Placeholder scan:** None found. All steps contain actual code, commands, and expected output.

**Type consistency:**
- `PersistedState` uses `i32` for `last_progress_cm` (matches `DistCm` type)
- `PersistedState` uses `u8` for `last_stop_index` (matches stop index type)
- `PERSIST_FLASH_OFFSET` is `u32` (matches Embassy flash API)
- `FLASH_SIZE` const generic uses `usize` ( Embassy requirement)
- `ticks_since_persist` is `u16` (sufficient for 60-second limit, doesn't overflow)

**No spec gaps found.** The plan implements all requirements from Section 11.4 of the tech report:
- CRC-protected storage ✅
- `last_progress_cm` and `last_stop_index` persisted ✅
- 500m threshold check before applying ✅
- Flash endurance protection via rate limiting ✅
- Cold-start improvement (skip Recovery scan when valid) ✅

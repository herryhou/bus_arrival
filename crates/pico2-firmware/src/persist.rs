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

use embassy_rp::flash::{Flash, ERASE_SIZE};
use embassy_rp::peripherals::FLASH;
use embedded_storage_async::nor_flash::NorFlash;
use shared::PersistedState;

/// Offset from flash base for persisted state sector.
/// Must match the PERSIST region in memory.x.
/// Pico 2 flash base is 0x10000000, PERSIST region is at 0x101FF000.
/// Offset = 0x101FF000 - 0x10000000 = 0x1FF000.
///
/// Note: This offset is at the end of flash, within the ROUTE_DATA region.
/// The actual route data binary is only ~22KB, so the last 4KB is unused.
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
pub async fn load(
    flash: &mut Flash<'_, FLASH, embassy_rp::flash::Async, FLASH_SIZE>,
) -> Option<PersistedState> {
    let mut buf = [0u8; core::mem::size_of::<PersistedState>()];

    // Read the persisted state from flash
    flash.read(PERSIST_FLASH_OFFSET, &mut buf).await.ok()?;

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
    flash: &mut Flash<'_, FLASH, embassy_rp::flash::Async, FLASH_SIZE>,
    state: &PersistedState,
) -> Result<(), ()> {
    // Erase the sector first (required before write on NOR flash)
    flash
        .erase(
            PERSIST_FLASH_OFFSET,
            PERSIST_FLASH_OFFSET + ERASE_SIZE as u32,
        )
        .await
        .map_err(|_| ())?;

    // Write must be in multiples of WRITE_SIZE (1 byte for RP2350).
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
        .map_err(|_| ())
}

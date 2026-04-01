#![cfg_attr(not(feature = "dev"), no_std)]
#![cfg_attr(not(feature = "dev"), no_main)]

use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, Ordering};

#[cfg(feature = "dev")]
use std as _;

#[cfg(not(feature = "dev"))]
use panic_halt as _;

#[cfg(not(feature = "dev"))]
use rp2040_boot2::BOOT_LOADER_W25Q080;

use shared::binfile::RouteData;

#[cfg(not(feature = "dev"))]
#[link_section = ".boot2"]
#[used]
pub static BOOT2: [u8; 256] = BOOT_LOADER_W25Q080;

/// Route data embedded in flash
#[cfg(not(feature = "dev"))]
#[link_section = ".route_data"]
static ROUTE_DATA: [u8; 128 * 1024] = [0u8; 128 * 1024];

/// Global flag for initialization
static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Route data reference (initialized after boot)
#[cfg(not(feature = "dev"))]
static mut ROUTE_DATA_REF: MaybeUninit<&'static RouteData<'static>> = MaybeUninit::uninit();

#[cfg(not(feature = "dev"))]
#[rp2040_hal::entry]
fn main() -> ! {
    // Initialize route data from flash
    let _route_data = unsafe {
        ROUTE_DATA_REF.write(&RouteData::load(&ROUTE_DATA).unwrap());
        ROUTE_DATA_REF.assume_init_ref()
    };

    INITIALIZED.store(true, Ordering::SeqCst);

    // TODO: Initialize UART
    // TODO: Main loop:
    //   1. Read NMEA from UART
    //   2. Parse with NmeaState
    //   3. Process GPS with Kalman
    //   4. Update StopState machines
    //   5. Emit events to UART

    loop {
        // Main processing loop
    }
}

#[cfg(feature = "dev")]
fn main() {
    // TODO: Initialize UART
    // TODO: Main loop:
    //   1. Read NMEA from UART
    //   2. Parse with NmeaState
    //   3. Process GPS with Kalman
    //   4. Update StopState machines
    //   5. Emit events to UART
}

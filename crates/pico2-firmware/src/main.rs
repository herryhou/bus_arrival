#![cfg_attr(not(feature = "dev"), no_std)]
#![cfg_attr(not(feature = "dev"), no_main)]

#[cfg(feature = "dev")]
use std as _;

#[cfg(not(feature = "dev"))]
use panic_halt as _;

#[cfg(not(feature = "dev"))]
use rp2040_boot2::BOOT_LOADER_W25Q080;

mod uart;

use uart::{GpsInput, EventOutput};

#[cfg(not(feature = "dev"))]
#[link_section = ".boot2"]
#[used]
pub static BOOT2: [u8; 256] = BOOT_LOADER_W25Q080;

/// Route data embedded in flash
#[cfg(not(feature = "dev"))]
#[link_section = ".route_data"]
static ROUTE_DATA: [u8; 128 * 1024] = [0u8; 128 * 1024];

/// Global state
#[cfg(not(feature = "dev"))]
struct State {
    nmea: gps_processor::nmea::NmeaState,
    kalman: shared::KalmanState,
    dr: shared::DrState,
    stop_states: heapless::Vec<detection::state_machine::StopState, 256>,
}

#[cfg(not(feature = "dev"))]
impl State {
    fn new(route_data: &shared::binfile::RouteData) -> Self {
        use detection::state_machine::StopState;
        use gps_processor::nmea::NmeaState;

        let stop_count = route_data.stop_count;
        let mut stop_states = heapless::Vec::new();
        for i in 0..stop_count {
            stop_states.push(StopState::new(i as u8)).unwrap();
        }

        Self {
            nmea: NmeaState::new(),
            kalman: shared::KalmanState::new(),
            dr: shared::DrState::new(),
            stop_states,
        }
    }
}

#[cfg(not(feature = "dev"))]
#[rp2040_hal::entry]
fn main() -> ! {
    let mut pac = rp2040_hal::pac::Peripherals::take().unwrap();
    let _core = cortex_m::Peripherals::take().unwrap();

    let mut watchdog = rp2040_hal::watchdog::Watchdog::new(pac.WATCHDOG);

    // Configure clocks - use external 12MHz oscillator
    let clocks = rp2040_hal::clocks::init_clocks_and_plls(
        12_000_000, // 12 MHz crystal
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    let sio = pac.SIO;
    let pins = rp2040_hal::gpio::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    // UART0 on GPIO 0 (TX) and GPIO 1 (RX)
    let uart_pins = (
        pins.gpio0.into_function::<2>(),
        pins.gpio1.into_function::<2>(),
    );

    let mut uart = rp2040_hal::uart::UartPeripheral::new(pac.UART0, uart_pins, &mut pac.RESETS)
        .enable(
            rp2040_hal::uart::UartConfig::new(115200.bps(), rp2040_hal::uart::DataBits::Eight),
            &clocks.peripheral_clock,
            &mut pac.RESETS,
        )
        .unwrap();

    // Initialize route data from flash
    let route_data = unsafe {
        shared::binfile::RouteData::load(&ROUTE_DATA).expect("Failed to load route data")
    };

    // Initialize state
    let mut state = State::new(&route_data);

    // Main loop
    loop {
        // Create GPS input wrapper for this iteration
        let mut gps_input = GpsInput::new(&mut uart);

        // Read NMEA sentence
        if let Some(sentence) = gps_input.read_sentence() {
            // Parse NMEA
            if let Some(_gps) = state.nmea.parse_sentence(sentence) {
                // TODO: Process GPS through full pipeline
                // For now, just emit a test event to demonstrate the flow
                let test_arrival = shared::ArrivalEvent {
                    time: 0,
                    stop_idx: 0,
                    s_cm: 10000,
                    v_cms: 100,
                    probability: 200,
                };

                // Emit test arrival event
                // Note: This is a simplified version - the full implementation
                // would process GPS through Kalman filter and update stop states
                let mut event_output = EventOutput::new(&mut uart);
                let _ = event_output.emit_arrival(&test_arrival);
            }
        }
    }
}

#[cfg(feature = "dev")]
fn main() {
    println!("Bus Arrival Detection System - Development Mode");
    println!("This is a placeholder for host-based testing.");
    println!("The actual firmware runs on Raspberry Pi Pico 2 (no_std).");

    // TODO: Initialize UART for testing
    // TODO: Main loop:
    //   1. Read NMEA from UART
    //   2. Parse with NmeaState
    //   3. Process GPS with Kalman
    //   4. Update StopState machines
    //   5. Emit events to UART
}

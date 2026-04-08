#![no_std]
#![no_main]
#![cfg(feature = "firmware")]

use defmt::*;
use defmt_rtt as _;
use panic_probe as _;

// Embassy imports
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};

// HAL imports
use embassy_rp::uart::{Config as UartConfig, Uart};
use embassy_rp::block::ImageDef;

// Module declarations
mod lut;
mod uart;
mod detection;
mod state;


// Note: embassy-rp doesn't require external bootloader
// The RP2350 has built-in boot ROM

// Image definition for memory layout
#[used]
#[link_section = ".bi_entries"]
static IMAGE_DEF: ImageDef = ImageDef::secure_exe();

/// Route data embedded in flash
/// Embedded at compile time from the preprocessor-generated binary file.
/// Format: RouteData binary format (see shared::binfile)
/// Size: ty225_normal.bin is ~22KB (varies by route complexity)
static ROUTE_DATA: &[u8] = include_bytes!("../../../test_data/ty225_normal.bin");

// Embassy program entry point
#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("Bus Arrival Detection System starting...");

    // Initialize peripherals
    let p = embassy_rp::init(Default::default());

    // Initialize UART for GPS NMEA input and arrival event output
    // Using blocking UART with embedded-io traits for compatibility
    let mut uart = Uart::new_blocking(
        p.UART0,
        p.PIN_0, // TX
        p.PIN_1, // RX
        UartConfig::default(),
    );

    // Initialize route data from flash
    let route_data = shared::binfile::RouteData::load(ROUTE_DATA)
        .expect("Failed to load route data");

    info!(
        "Route data loaded: {} nodes, {} stops",
        route_data.node_count, route_data.stop_count
    );

    // Initialize state with route data reference
    let mut state = state::State::new(&route_data);

    // Initialize line buffer for NMEA data
    let mut line_buf = uart::UartLineBuffer::new();

    info!("System ready. Starting GPS processing...");

    // Main processing loop (1 Hz)
    loop {
        // Drain all sentences from current GPS burst before sleeping
        // GPS modules typically send RMC+GSA+GGA in a burst (~200ms)
        loop {
            match uart::read_nmea_sentence(&mut uart, &mut line_buf) {
                Ok(Some(sentence)) => {
                    debug!("NMEA: {}", sentence);

                    // Parse NMEA sentence
                    if let Some(gps) = state.nmea.parse_sentence(sentence) {
                        debug!(
                            "GPS: lat={:.6}°, lon={:.6}°, fix={}",
                            gps.lat, gps.lon, gps.has_fix
                        );

                        // Process GPS through full pipeline
                        if let Some(arrival) = state.process_gps(&gps) {
                            // Emit arrival event via UART
                            match uart::write_arrival_event(&mut uart, &arrival) {
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
                Err(e) => {
                    defmt::warn!("UART read error: {:?}", e);
                    line_buf.reset();
                    break;
                }
            }
        }

        // Rate limiting: 1 Hz processing
        Timer::after(Duration::from_secs(1)).await;
    }
}

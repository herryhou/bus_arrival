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
use embassy_rp::uart::{BufferedInterruptHandler, BufferedUart, Config as UartConfig};
use embassy_rp::bind_interrupts;
use embassy_rp::block::ImageDef;

// Module declarations
mod lut;
mod uart;
mod detection;
mod state;
mod persist;


// Note: embassy-rp doesn't require external bootloader
// The RP2350 has built-in boot ROM

// Image definition for memory layout
#[used]
#[link_section = ".bi_entries"]
static IMAGE_DEF: ImageDef = ImageDef::secure_exe();

// Interrupt bindings for buffered UART
bind_interrupts!(struct Irqs {
    UART0_IRQ => BufferedInterruptHandler<embassy_rp::peripherals::UART0>;
});

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
    // Using buffered UART (interrupt-based) for true async I/O without DMA requirement
    // TX/RX buffers must live for the entire program duration
    // TX_BUFFER: 256 bytes sufficient for arrival event messages (~128 bytes each)
    // RX_BUFFER: 512 bytes covers full 1-second sleep window at 9600 baud.
    //   Main loop sleeps for 1 second between GPS reads; buffer must absorb
    //   all NMEA sentences transmitted during that window (~480 bytes/sec typical,
    //   960 bytes/sec theoretical max at 9600 baud).
    static mut TX_BUFFER: [u8; 256] = [0u8; 256];
    static mut RX_BUFFER: [u8; 512] = [0u8; 512];

    let mut uart = unsafe {
        BufferedUart::new(
            p.UART0,
            p.PIN_0, // TX
            p.PIN_1, // RX
            Irqs,
            &mut TX_BUFFER,
            &mut RX_BUFFER,
            UartConfig::default(),
        )
    };

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

        // Rate limiting: 1 Hz processing
        Timer::after(Duration::from_secs(1)).await;
    }
}

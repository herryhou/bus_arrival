#![no_std]
#![no_main]

use defmt::*;
use defmt_rtt as _;
use panic_probe as _;

// Embassy imports
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};

// HAL imports
use embassy_rp::uart::{self, Config as UartConfig};
use embassy_rp::block::ImageDef;

// Local UART module for GPS I/O
mod uart;
use uart::{GpsInput, EventOutput};

// Note: embassy-rp doesn't require external bootloader
// The RP2350 has built-in boot ROM

// Image definition for memory layout
#[used]
#[link_section = ".bi_entries"]
static IMAGE_DEF: ImageDef = ImageDef::secure_exe();

/// Route data embedded in flash (128KB max)
#[link_section = ".route_data"]
static ROUTE_DATA: [u8; 128 * 1024] = [0u8; 128 * 1024];

/// Global state
struct State {
    nmea: gps_processor::nmea::NmeaState,
    kalman: shared::KalmanState,
    dr: shared::DrState,
    stop_states: heapless::Vec<detection::state_machine::StopState, 256>,
}

impl State {
    fn new(route_data: &shared::binfile::RouteData) -> Self {
        use detection::state_machine::StopState;
        use gps_processor::nmea::NmeaState;

        let stop_count = route_data.stop_count;
        let mut stop_states = heapless::Vec::new();
        for i in 0..stop_count {
            let _ = stop_states.push(StopState::new(i as u8));
        }

        Self {
            nmea: NmeaState::new(),
            kalman: shared::KalmanState::new(),
            dr: shared::DrState::new(),
            stop_states,
        }
    }
}

// Embassy program entry point
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Bus Arrival Detection System starting...");

    // Initialize peripherals
    let p = embassy_rp::init(Default::default());

    // Static UART for embassy-rp
    // TODO: Refactor to use async UART with DMA
    static mut UART: Option<embassy_rp::uart::Uart<'static, embassy_rp::uart::Blocking>> = None;

    let uart = unsafe {
        UART = Some(embassy_rp::uart::Uart::new_blocking(
            p.UART0,
            p.PIN_0, // TX
            p.PIN_1, // RX
            UartConfig::default(),
        ));
        UART.as_mut().unwrap()
    };

    // Initialize route data from flash
    let route_data = unsafe {
        shared::binfile::RouteData::load(&ROUTE_DATA)
            .expect("Failed to load route data")
    };

    // Initialize state
    let mut state = State::new(&route_data);

    info!("System ready. Starting GPS processing...");

    // Main processing loop
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
                let mut event_output = EventOutput::new(&mut uart);
                let _ = event_output.emit_arrival(&test_arrival);
            }
        }

        // Rate limiting: 1 Hz processing
        Timer::after(Duration::from_secs(1)).await;
    }
}

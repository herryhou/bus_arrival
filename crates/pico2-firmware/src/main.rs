#![no_std]
#![no_main]

use defmt::*;
use defmt_rtt as _;
use panic_probe as _;

// Embassy imports
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};

// HAL imports
use embassy_rp::uart::{Config as UartConfig, Uart, Blocking};
use embassy_rp::block::ImageDef;

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
async fn main(_spawner: Spawner) {
    info!("Bus Arrival Detection System starting...");

    // Initialize peripherals
    let p = embassy_rp::init(Default::default());

    // Static UART for embassy-rp 0.10.x
    // Using raw pointer to avoid borrow checker issues with &'static mut
    static mut UART: Option<Uart<'static, Blocking>> = None;
    let uart_ptr: *mut Uart<'static, Blocking> = {
        unsafe {
            UART = Some(Uart::new_blocking(
                p.UART0,
                p.PIN_0, // TX
                p.PIN_1, // RX
                UartConfig::default(),
            ));
            UART.as_mut().unwrap() as *mut Uart<'static, Blocking>
        }
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
        // Buffer for NMEA sentence
        let mut nmea_buffer = [0u8; 256];

        // Read NMEA sentence using raw pointer
        let has_sentence = unsafe {
            let uart = &mut *uart_ptr;
            // Stub: just return false for now
            // TODO: Implement actual UART read
            false
        };

        // Parse NMEA and potentially emit event
        if has_sentence {
            // Convert buffer to string slice for parsing
            if let Ok(sentence) = core::str::from_utf8(&nmea_buffer) {
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

                    // Emit test arrival event using raw pointer
                    unsafe {
                        let uart = &mut *uart_ptr;
                        // Stub: TODO: Implement actual UART write
                        let _ = uart;
                    }
                }
            }
        }

        // Rate limiting: 1 Hz processing
        Timer::after(Duration::from_secs(1)).await;
    }
}

//! UART driver for GPS input and JSON output

use core::fmt::Write;
use embedded_hal::uart::{Read, Write as UartWrite};
use nb::block;

const UART_BUF_SIZE: usize = 256;

/// GPS input from UART
pub struct GpsInput<UART> {
    uart: UART,
    buffer: [u8; UART_BUF_SIZE],
    pos: usize,
}

impl<UART: Read<u8>> GpsInput<UART> {
    pub fn new(uart: UART) -> Self {
        Self {
            uart,
            buffer: [0; UART_BUF_SIZE],
            pos: 0,
        }
    }

    /// Read a complete NMEA sentence (until \n)
    /// Returns Some(sentence) if complete, None otherwise
    pub fn read_sentence(&mut self) -> Option<&str> {
        loop {
            if self.pos >= UART_BUF_SIZE {
                // Buffer overflow - reset
                self.pos = 0;
                return None;
            }

            let byte = block!(self.uart.read()).ok()?;

            self.buffer[self.pos] = byte;
            self.pos += 1;

            if byte == b'\n' {
                let sentence = core::str::from_utf8(&self.buffer[..self.pos]).ok()?;
                self.pos = 0;
                return Some(sentence.trim());
            }
        }
    }
}

/// JSON event output to UART
pub struct EventOutput<UART> {
    uart: UART,
    buffer: [u8; 128],
}

impl<UART: UartWrite<u8>> EventOutput<UART> {
    pub fn new(uart: UART) -> Self {
        Self {
            uart,
            buffer: [0; 128],
        }
    }

    /// Emit arrival event as JSON
    pub fn emit_arrival(
        &mut self,
        event: &shared::ArrivalEvent,
    ) -> Result<(), &'static str> {
        use serde_json_core::ser::SliceWrite;

        let mut writer = SliceWrite::new(&mut self.buffer);
        let mut ser = serde_json_core::ser::Serializer::new(&mut writer);

        // Manual JSON serialization for arrival event
        use serde::ser::Serialize;
        event.serialize(&mut ser)
            .map_err(|_| "serialize failed")?;

        let json_bytes = writer.bytes();

        // Write to UART
        for &b in json_bytes {
            block!(self.uart.write(b)).map_err(|_| "uart write failed")?;
        }
        block!(self.uart.write(b'\n')).map_err(|_| "uart write failed")?;

        Ok(())
    }

    /// Emit departure event as JSON
    pub fn emit_departure(
        &mut self,
        event: &shared::DepartureEvent,
    ) -> Result<(), &'static str> {
        use serde_json_core::ser::SliceWrite;

        let mut writer = SliceWrite::new(&mut self.buffer);
        let mut ser = serde_json_core::ser::Serializer::new(&mut writer);

        use serde::ser::Serialize;
        event.serialize(&mut ser)
            .map_err(|_| "serialize failed")?;

        let json_bytes = writer.bytes();

        for &b in json_bytes {
            block!(self.uart.write(b)).map_err(|_| "uart write failed")?;
        }
        block!(self.uart.write(b'\n')).map_err(|_| "uart write failed")?;

        Ok(())
    }
}

//! UART driver for GPS input and JSON output

use nb::block;

const UART_BUF_SIZE: usize = 256;

/// GPS input from UART (borrows UART mutably)
pub struct GpsInput<'a, UART> {
    uart: &'a mut UART,
    buffer: [u8; UART_BUF_SIZE],
    pos: usize,
}

impl<'a, UART> GpsInput<'a, UART>
where
    UART: embedded_hal_nb::serial::Read<u8>,
{
    pub fn new(uart: &'a mut UART) -> Self {
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

/// JSON event output to UART (borrows UART mutably)
pub struct EventOutput<'a, UART> {
    uart: &'a mut UART,
    buffer: [u8; 128],
}

impl<'a, UART> EventOutput<'a, UART>
where
    UART: embedded_hal_nb::serial::Write<u8>,
{
    pub fn new(uart: &'a mut UART) -> Self {
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
        // Serialize to buffer using serde_json_core::to_slice
        let len = serde_json_core::to_slice(event, &mut self.buffer)
            .map_err(|_| "serialize failed")?;

        // Write to UART
        for &b in &self.buffer[..len] {
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
        // Serialize to buffer using serde_json_core::to_slice
        let len = serde_json_core::to_slice(event, &mut self.buffer)
            .map_err(|_| "serialize failed")?;

        // Write to UART
        for &b in &self.buffer[..len] {
            block!(self.uart.write(b)).map_err(|_| "uart write failed")?;
        }
        block!(self.uart.write(b'\n')).map_err(|_| "uart write failed")?;

        Ok(())
    }
}

//! UART driver for GPS input and JSON output
//!
//! This is a simplified implementation that uses embassy-rp's UART
//! with a minimal wrapper for byte-level I/O.

use core::fmt::Write;
use embassy_rp::uart::Uart;
use embassy_rp::uart::Blocking;
use embassy_rp::peripherals::UART0;

const UART_BUF_SIZE: usize = 256;
const JSON_BUF_SIZE: usize = 128;

/// GPS input from UART (simplified - reads single byte)
pub struct GpsInput<'a> {
    uart: &'a mut Uart<'static, UART0, Blocking>,
}

impl<'a> GpsInput<'a> {
    pub fn new(uart: &'a mut Uart<'static, UART0, Blocking>) -> Self {
        Self { uart }
    }

    /// Read a complete NMEA sentence (until \n)
    /// Returns Some(sentence) if complete, None otherwise
    pub fn read_sentence(&mut self) -> Option<&str> {
        // Simplified: use a fixed buffer for now
        let mut buffer = [0u8; UART_BUF_SIZE];
        let mut pos = 0;

        loop {
            if pos >= UART_BUF_SIZE {
                // Buffer overflow - reset
                pos = 0;
                return None;
            }

            // Read byte - simplified blocking read
            // Note: This is a placeholder - need proper embassy-rp UART read
            // For now, we'll return None to indicate the implementation is incomplete
            return None;
        }
    }
}

/// JSON event output to UART (simplified stub)
pub struct EventOutput<'a> {
    uart: &'a mut Uart<'static, UART0, Blocking>,
    buffer: [u8; JSON_BUF_SIZE],
    len: usize,
}

impl<'a> EventOutput<'a> {
    pub fn new(uart: &'a mut Uart<'static, UART0, Blocking>) -> Self {
        Self {
            uart,
            buffer: [0; JSON_BUF_SIZE],
            len: 0,
        }
    }

    /// Emit arrival event as JSON
    pub fn emit_arrival(
        &mut self,
        _event: &shared::ArrivalEvent,
    ) -> Result<(), &'static str> {
        // Stub - TODO: Implement proper embassy-rp UART write
        Ok(())
    }

    /// Emit departure event as JSON
    pub fn emit_departure(
        &mut self,
        _event: &shared::DepartureEvent,
    ) -> Result<(), &'static str> {
        // Stub - TODO: Implement proper embassy-rp UART write
        Ok(())
    }
}

/// Helper struct to write into a fixed-size buffer
struct BufferWriter<'a>(&'a mut [u8], &'a mut usize);

impl core::fmt::Write for BufferWriter<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let bytes = s.as_bytes();
        let remaining = &mut self.0[*self.1..];
        if remaining.len() < bytes.len() {
            return Err(core::fmt::Error);
        }
        remaining[..bytes.len()].copy_from_slice(bytes);
        *self.1 += bytes.len();
        Ok(())
    }
}

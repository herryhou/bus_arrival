//! UART driver for GPS input and JSON output

use core::fmt::Write;
use embedded_io::Write;
use embassy_rp::uart::Uart;
use embassy_rp::uart::Blocking;
use embassy_rp::peripherals::UART0;

const UART_BUF_SIZE: usize = 256;
const JSON_BUF_SIZE: usize = 128;

/// GPS input from UART (borrows UART mutably)
pub struct GpsInput<'a> {
    uart: &'a mut Uart<'static, UART0, Blocking>,
    buffer: [u8; UART_BUF_SIZE],
    pos: usize,
}

impl<'a> GpsInput<'a> {
    pub fn new(uart: &'a mut Uart<'static, UART0, Blocking>) -> Self {
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

            // Read byte using embassy_rp blocking API - busy wait
            let mut byte = 0u8;
            loop {
                match self.uart.read(&mut core::slice::from_mut(&mut byte)) {
                    Ok(_) => break,
                    Err(_) => continue, // Busy wait
                }
            }
            let byte = byte;

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
        event: &shared::ArrivalEvent,
    ) -> Result<(), &'static str> {
        // Manual JSON serialization: {"time":123,"stop_idx":1,"s_cm":10000,"v_cms":100,"probability":200}
        self.len = 0;
        write!(BufferWriter(&mut self.buffer, &mut self.len),
            "{{\"time\":{},\"stop_idx\":{},\"s_cm\":{},\"v_cms\":{},\"probability\":{}}}",
            event.time, event.stop_idx, event.s_cm, event.v_cms, event.probability)
            .map_err(|_| "json serialize failed")?;

        // Write to UART using embassy_rp blocking write_all
        let result = self.uart.write_all(&self.buffer[..self.len]);
        let result = self.uart.write_all(&[b'\n']);
        // For now, ignore errors in this stub
        Ok(())

        Ok(())
    }

    /// Emit departure event as JSON
    pub fn emit_departure(
        &mut self,
        event: &shared::DepartureEvent,
    ) -> Result<(), &'static str> {
        // Manual JSON serialization: {"time":123,"stop_idx":1,"s_cm":10000,"v_cms":50}
        self.len = 0;
        write!(BufferWriter(&mut self.buffer, &mut self.len),
            "{{\"time\":{},\"stop_idx\":{},\"s_cm\":{},\"v_cms\":{}}}",
            event.time, event.stop_idx, event.s_cm, event.v_cms)
            .map_err(|_| "json serialize failed")?;

        // Write to UART using embassy_rp blocking write_all
        let result = self.uart.write_all(&self.buffer[..self.len]);
        let result = self.uart.write_all(&[b'\n']);
        // For now, ignore errors in this stub
        Ok(())

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

//! UART I/O for GPS NMEA input and arrival event output
//!
//! Provides line-buffered NMEA sentence reading and formatted arrival event writing.

#![cfg(feature = "firmware")]

use embassy_rp::uart::Uart;

use shared::ArrivalEvent;

// ===== Constants =====

/// Maximum NMEA sentence length (standard max is 82 chars)
pub const MAX_NMEA_LENGTH: usize = 128;

// ===== Line Buffer for NMEA Data =====

/// Line buffer for accumulating NMEA data from UART
pub struct UartLineBuffer {
    buffer: [u8; MAX_NMEA_LENGTH],
    len: usize,
}

impl UartLineBuffer {
    pub fn new() -> Self {
        Self {
            buffer: [0u8; MAX_NMEA_LENGTH],
            len: 0,
        }
    }

    /// Reset the buffer (clear all data)
    pub fn reset(&mut self) {
        self.len = 0;
    }

    /// Add a byte to the buffer. Returns true if buffer is full.
    pub fn push(&mut self, byte: u8) -> Result<(), ()> {
        if self.len < MAX_NMEA_LENGTH {
            self.buffer[self.len] = byte;
            self.len += 1;
            Ok(())
        } else {
            Err(()) // Buffer full
        }
    }

    /// Check if buffer contains a complete line (ending with \r\n)
    pub fn has_complete_line(&self) -> bool {
        if self.len >= 2 {
            self.buffer[self.len - 2] == b'\r' && self.buffer[self.len - 1] == b'\n'
        } else {
            false
        }
    }

    /// Get the complete line as a string slice (without \r\n)
    pub fn as_str(&self) -> Result<&str, ()> {
        if self.len >= 2 {
            core::str::from_utf8(&self.buffer[..self.len - 2]).map_err(|_| ())
        } else {
            Err(())
        }
    }
}

// ===== NMEA Reading =====

/// Read NMEA sentences from UART using blocking I/O.
///
/// This function reads bytes from UART until a complete NMEA sentence
/// is received (terminated by \r\n). Returns the sentence as a string slice.
///
/// Returns:
/// - Ok(Some(sentence)) - Complete NMEA sentence received
/// - Ok(None) - No data available yet
/// - Err(...) - I/O error
pub fn read_nmea_sentence<'buf>(
    uart: &mut Uart<'_, embassy_rp::uart::Blocking>,
    line_buf: &'buf mut UartLineBuffer,
) -> Result<Option<&'buf str>, ()> {
    // Loop until we have a complete line or error
    // NMEA sentences are terminated by \r\n, so we must read all bytes
    // before returning to avoid losing data between 1Hz main loop iterations
    loop {
        let mut byte = [0u8; 1];

        // Try to read a single byte (blocking)
        // embassy-rp blocking UART provides blocking_read() method
        let result = uart.blocking_read(&mut byte);

        match result {
            Ok(_) => {
                let b = byte[0];

                // Skip leading NUL characters (common during startup)
                if b == 0 && line_buf.len == 0 {
                    continue;
                }

                // Check for start of new sentence ($)
                if b == b'$' && line_buf.len > 0 {
                    defmt::warn!("Incomplete NMEA sentence before new $, resetting buffer");
                    line_buf.reset();
                }

                // Add byte to buffer
                if line_buf.push(b).is_err() {
                    defmt::warn!("NMEA sentence too long, resetting buffer");
                    line_buf.reset();
                    return Ok(None);
                }

                // Check if we have a complete line
                if line_buf.has_complete_line() {
                    let sentence = core::str::from_utf8(&line_buf.buffer[..line_buf.len - 2])
                        .map_err(|_| ())?;
                    return Ok(Some(sentence));
                }
                // Continue loop to read next byte
            }
            Err(_) => {
                // No data available (or error) - return None
                // In a real system, we'd distinguish between no data and error
                return Ok(None);
            }
        }
    }
}

// ===== Arrival Event Writing =====

/// Write arrival event to UART.
///
/// Format: "ARRIVAL: t=TIME, stop=IDX, s=CMS, v=CMS/S, p=PROB\n"
pub fn write_arrival_event(
    uart: &mut Uart<'_, embassy_rp::uart::Blocking>,
    event: &ArrivalEvent,
) -> Result<(), ()> {
    // Build a static byte buffer for the message
    // Format: "ARRIVAL: t=12345, stop=0, s=10000cm, v=100cm/s, p=200\n"
    let mut msg_buf = [0u8; 128];
    let mut pos = 0;

    // Helper to append bytes to buffer
    macro_rules! append {
        ($data:expr) => {
            if pos + $data.len() > msg_buf.len() {
                return Err(()); // Buffer overflow
            }
            msg_buf[pos..pos + $data.len()].copy_from_slice($data);
            pos += $data.len();
        };
    }

    // Helper to append unsigned integer as decimal string
    fn append_u64(buf: &mut [u8], p: &mut usize, mut n: u64) -> Result<(), ()> {
        if n == 0 {
            if *p + 1 > buf.len() {
                return Err(());
            }
            buf[*p] = b'0';
            *p += 1;
            return Ok(());
        }

        // Convert to string (reverse order)
        let mut digits = [0u8; 20];
        let mut len = 0;
        while n > 0 {
            digits[len] = b'0' + ((n % 10) as u8);
            n /= 10;
            len += 1;
        }

        // Reverse and append
        for i in (0..len).rev() {
            if *p + 1 > buf.len() {
                return Err(());
            }
            buf[*p] = digits[i];
            *p += 1;
        }

        Ok(())
    }

    // Build message
    append!(b"ARRIVAL: t=");
    append_u64(&mut msg_buf, &mut pos, event.time)?;
    append!(b", stop=");
    append_u64(&mut msg_buf, &mut pos, event.stop_idx as u64)?;
    append!(b", s=");
    append_u64(&mut msg_buf, &mut pos, event.s_cm as u64)?;
    append!(b"cm, v=");
    append_u64(&mut msg_buf, &mut pos, event.v_cms as u64)?;
    append!(b"cm/s, p=");
    append_u64(&mut msg_buf, &mut pos, event.probability as u64)?;
    append!(b"\n");

    // Write to UART using blocking_write
    uart.blocking_write(&msg_buf[..pos]).map_err(|_| ())?;

    // Flush to ensure data is sent
    uart.blocking_flush().map_err(|_| ())?;

    Ok(())
}

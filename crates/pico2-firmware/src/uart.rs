//! UART I/O for GPS NMEA input and arrival event output
//!
//! Provides line-buffered NMEA sentence reading and formatted arrival event writing.

#![cfg(feature = "firmware")]
#![allow(dead_code)]

use embassy_rp::uart::BufferedUart;
use embassy_time::{with_timeout, Duration};
use embedded_io_async::{Read as AsyncRead, Write as AsyncWrite};

use shared::ArrivalEvent;

// ===== Constants =====

/// Maximum NMEA sentence length (standard max is 82 chars)
pub const MAX_NMEA_LENGTH: usize = 128;
// Note: RX_BUFFER in main.rs is sized for 1-second accumulation, not per-sentence.

// ===== Error Types =====

/// UART error types
#[derive(Debug, Clone, Copy, defmt::Format)]
pub enum UartError {
    Timeout,
    Io,
}

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

// ===== Async NMEA Reading with Timeout =====

/// Read a single byte with timeout using buffered async UART
async fn read_byte_with_timeout(
    uart: &mut BufferedUart,
    timeout: Duration,
) -> Result<u8, UartError> {
    with_timeout(timeout, async {
        let mut byte = [0u8; 1];
        match uart.read(&mut byte).await {
            Ok(n) if n == 1 => Ok(byte[0]),
            _ => Err(UartError::Io),
        }
    })
    .await
    .map_err(|_| UartError::Timeout)?
}

/// Read NMEA sentence with timeout (async version)
///
/// This function reads bytes from UART until a complete NMEA sentence
/// is received (terminated by \r\n) or a 5-second timeout occurs.
///
/// Returns:
/// - Ok(Some(sentence)) - Complete NMEA sentence received
/// - Ok(None) - No data available yet
/// - Err(UartError::Timeout) - Timeout occurred
/// - Err(UartError::Io) - I/O error
pub async fn read_nmea_sentence_async<'buf>(
    uart: &mut BufferedUart,
    line_buf: &'buf mut UartLineBuffer,
) -> Result<Option<&'buf str>, UartError> {
    let timeout = Duration::from_secs(5);

    loop {
        match read_byte_with_timeout(uart, timeout).await {
            Ok(b) => {
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
                        .map_err(|_| UartError::Io)?;
                    return Ok(Some(sentence));
                }
                // Continue loop to read next byte
            }
            Err(UartError::Timeout) => {
                defmt::warn!("UART read timeout");
                line_buf.reset();
                return Err(UartError::Timeout);
            }
            Err(UartError::Io) => {
                return Ok(None);
            }
        }
    }
}

// ===== Arrival Event Writing =====

/// Write arrival event to UART (async).
///
/// Format: "ARRIVAL: t=TIME, stop=IDX, s=CMS, v=CMS/S, p=PROB\n"
///         "DEPARTURE: t=TIME, stop=IDX, s=CMS, v=CMS/S, p=PROB\n"
///         "ANNOUNCE: t=TIME, stop=IDX, s=CMS, v=CMS/S\n"
pub async fn write_arrival_event_async(
    uart: &mut BufferedUart,
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

    // Helper to append signed integer as decimal string
    fn append_i64(buf: &mut [u8], p: &mut usize, n: i64) -> Result<(), ()> {
        if n < 0 {
            // Format negative numbers with leading minus sign
            if *p + 1 > buf.len() {
                return Err(());
            }
            buf[*p] = b'-';
            *p += 1;
            // Format absolute value
            let abs_n = n.wrapping_abs() as u64;
            append_u64(buf, p, abs_n)
        } else {
            // Positive numbers use the unsigned formatter
            append_u64(buf, p, n as u64)
        }
    }

    // Build message prefix based on event type
    if matches!(event.event_type, shared::ArrivalEventType::Arrival) {
        append!(b"ARRIVAL");
    } else if matches!(event.event_type, shared::ArrivalEventType::Departure) {
        append!(b"DEPARTURE");
    } else {
        append!(b"ANNOUNCE");
    }
    append!(b": t=");
    append_u64(&mut msg_buf, &mut pos, event.time)?;
    append!(b", stop=");
    append_u64(&mut msg_buf, &mut pos, event.stop_idx as u64)?;
    append!(b", s=");
    append_i64(&mut msg_buf, &mut pos, event.s_cm as i64)?;
    append!(b"cm, v=");
    append_i64(&mut msg_buf, &mut pos, event.v_cms as i64)?;
    append!(b"cm/s");

    // Only append probability for Arrival and Departure events
    if matches!(
        event.event_type,
        shared::ArrivalEventType::Arrival | shared::ArrivalEventType::Departure
    ) {
        append!(b", p=");
        append_u64(&mut msg_buf, &mut pos, event.probability as u64)?;
    }

    append!(b"\n");

    // Write to UART using async write
    uart.write(&msg_buf[..pos]).await.map_err(|_| ())?;

    // Flush to ensure data is sent
    uart.flush().await.map_err(|_| ())?;

    Ok(())
}

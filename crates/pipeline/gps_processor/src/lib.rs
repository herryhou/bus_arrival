//! GPS processor for bus arrival detection system.
//! Supports no_std embedded targets.

#![cfg_attr(not(feature = "std"), no_std)]

pub mod accumulator;
pub mod kalman;
pub mod map_match;
pub mod nmea;
pub mod route_data;

#[cfg(feature = "std")]
pub mod output;

// Re-export commonly used types
pub use kalman::{process_gps_update, ProcessResult, SIGMA_GPS_CM, V_MAX_CMS};
pub use accumulator::FixAccumulator;
pub use accumulator::FixAccumulator as NmeaState; // Backward compatibility

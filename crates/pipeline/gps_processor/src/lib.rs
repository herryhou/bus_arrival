//! GPS processor for bus arrival detection system.
//! Supports no_std embedded targets.

#![cfg_attr(not(feature = "std"), no_std)]


pub mod route_data;
pub mod nmea;
pub mod map_match;
pub mod kalman;

#[cfg(feature = "std")]
pub mod output;

// Re-export commonly used types
pub use nmea::NmeaState;
pub use kalman::{ProcessResult, process_gps_update, V_MAX_CMS, SIGMA_GPS_CM};

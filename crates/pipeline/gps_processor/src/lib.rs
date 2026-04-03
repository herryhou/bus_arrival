//! GPS processor for bus arrival detection system.
//! Supports no_std embedded targets.

#![cfg_attr(not(feature = "std"), no_std)]

// Use libm for floating-point operations in no_std
#[cfg(not(feature = "std"))]
use libm::{exp as f64_exp, round as f64_round, trunc as f64_trunc, cos as f64_cos};

pub mod route_data;
pub mod nmea;
pub mod map_match;
pub mod kalman;

#[cfg(feature = "std")]
pub mod output;

// Re-export commonly used types
pub use nmea::NmeaState;
pub use kalman::{ProcessResult, process_gps_update, V_MAX_CMS, SIGMA_GPS_CM};

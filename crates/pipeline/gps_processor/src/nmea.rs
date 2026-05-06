//! NMEA parsing utilities
//!
//! NOTE: The new `FixAccumulator` in `accumulator.rs` replaces the old
//! `NmeaState`. This module now provides helper functions and types.

pub use crate::accumulator::FixAccumulator;

// Re-export helper functions for tests
pub use crate::accumulator::{parse_lat, parse_lon, knots_to_cms};

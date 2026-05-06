//! NMEA parsing utilities
//!
//! NOTE: The new `FixAccumulator` in `accumulator.rs` replaces the old
//! `NmeaState`. This module now provides helper functions and types.

use shared::GpsPoint;

pub use crate::accumulator::FixAccumulator;

// Re-export helper functions for tests
pub use crate::accumulator::{parse_lat, parse_lon, knots_to_cms};

/// Backward compatibility wrapper for old NmeaState API.
///
/// This provides the old `parse_sentence` interface during migration.
/// Internally delegates to FixAccumulator.
pub struct NmeaState {
    acc: FixAccumulator,
}

impl Default for NmeaState {
    fn default() -> Self {
        Self::new()
    }
}

impl NmeaState {
    pub fn new() -> Self {
        NmeaState {
            acc: FixAccumulator::new(),
        }
    }

    /// Parse NMEA sentence, returns Some(GpsPoint) when complete.
    ///
    /// Note: This is the old API for backward compatibility.
    /// New code should use FixAccumulator directly.
    pub fn parse_sentence(&mut self, sentence: &str) -> Option<GpsPoint> {
        if self.acc.update(sentence) {
            // Check if we should emit (timestamp changed)
            if self.acc.should_emit() {
                if let Some((point, _quality)) = self.acc.build() {
                    self.acc.reset();
                    return Some(point);
                }
            }
        }
        None
    }
}

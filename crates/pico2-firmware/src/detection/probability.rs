//! Arrival probability computation adapter
//!
//! Wraps pipeline-probability crate, providing cached
//! probability computation for firmware.

use shared::{SpeedCms, PositionSignals, Stop};
use crate::detection::GpsStatus;
use pipeline_probability::ProbabilityEngine;

/// Probability computation wrapper
///
/// Wraps ProbabilityEngine from pipeline-probability crate.
pub struct FirmwareProbabilityEngine {
    inner: ProbabilityEngine,
}

impl FirmwareProbabilityEngine {
    pub fn new() -> Self {
        Self {
            inner: ProbabilityEngine::new(),
        }
    }

    /// Compute arrival probability (cached)
    ///
    /// Delegates to pipeline-probability crate.
    pub fn compute(
        &mut self,
        timestamp: u64,
        signals: PositionSignals,
        v_cms: SpeedCms,
        stop: &Stop,
        dwell_time_s: u16,
        gps_status: GpsStatus,
    ) -> u8 {
        // Convert firmware GpsStatus to detection::probability::GpsStatus
        let gps_status = match gps_status {
            GpsStatus::Valid => detection::probability::GpsStatus::Valid,
            GpsStatus::DrOutage => detection::probability::GpsStatus::DrOutage,
            GpsStatus::OffRoute => detection::probability::GpsStatus::OffRoute,
        };

        self.inner
            .compute(timestamp, signals, v_cms, stop, dwell_time_s, gps_status)
            .probability
    }

    /// Clear cache (call on new GPS record)
    pub fn clear(&mut self) {
        self.inner.clear();
    }
}

impl Default for FirmwareProbabilityEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_probability_compute() {
        let mut engine = FirmwareProbabilityEngine::new();
        let signals = PositionSignals::new(100, 100);
        let stop = Stop {
            progress_cm: 1000,
            corridor_start_cm: 900,
            corridor_end_cm: 1100,
        };

        let prob = engine.compute(0, signals, 50, &stop, 0, GpsStatus::Valid);
        assert!(prob <= 255);
    }

    #[test]
    fn test_probability_clear() {
        let mut engine = FirmwareProbabilityEngine::new();
        let signals = PositionSignals::new(100, 100);
        let stop = Stop {
            progress_cm: 1000,
            corridor_start_cm: 900,
            corridor_end_cm: 1100,
        };

        engine.compute(0, signals, 50, &stop, 0, GpsStatus::Valid);
        engine.clear();
        let _prob = engine.compute(0, signals, 50, &stop, 0, GpsStatus::Valid);
    }
}

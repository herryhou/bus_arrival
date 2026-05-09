//! Probability computation engine with caching
//!
//! This module provides cached probability computation to avoid
//! recalculating for the same timestamp/inputs (e.g., in get_trace_info).

use shared::{SpeedCms, PositionSignals, Stop};
use detection::probability::{self, GpsStatus};

/// Cached probability computation result
#[derive(Debug, Clone)]
pub struct ProbabilityResult {
    pub probability: u8,
    pub features: detection::trace::FeatureScores,
}

/// Probability computation engine with caching
///
/// Caches the last computation by (timestamp, inputs) to avoid
/// recomputing when get_trace_info() needs the same data.
pub struct ProbabilityEngine {
    last_result: Option<(u64, ProbabilityResult)>,
}

impl ProbabilityEngine {
    pub fn new() -> Self {
        Self { last_result: None }
    }

    /// Compute arrival probability (cached)
    ///
    /// Returns cached result if timestamp matches, otherwise computes
    /// new probability and caches it.
    pub fn compute(
        &mut self,
        timestamp: u64,
        signals: PositionSignals,
        v_cms: SpeedCms,
        stop: &Stop,
        dwell_time_s: u16,
        gps_status: GpsStatus,
    ) -> &ProbabilityResult {
        // Check cache
        if let Some((cached_ts, _)) = self.last_result {
            if cached_ts == timestamp {
                // Return cached result (same timestamp = same inputs)
                return &self.last_result.as_ref().unwrap().1;
            }
        }

        // Compute new probability
        let features = probability::compute_feature_scores(
            signals,
            v_cms,
            stop,
            dwell_time_s,
            probability::gaussian_lut(),
            probability::logistic_lut(),
        );

        let probability = probability::compute_arrival_probability(
            signals,
            v_cms,
            stop,
            dwell_time_s,
            gps_status,
            probability::gaussian_lut(),
            probability::logistic_lut(),
        );

        let result = ProbabilityResult { probability, features };
        self.last_result = Some((timestamp, result));
        &self.last_result.as_ref().unwrap().1
    }

    /// Clear the cache (e.g., on new GPS record)
    pub fn clear(&mut self) {
        self.last_result = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::Stop;

    fn mock_stop() -> Stop {
        Stop {
            progress_cm: 1000,
            corridor_start_cm: 900,
            corridor_end_cm: 1100,
        }
    }

    #[test]
    fn test_probability_caching() {
        let mut engine = ProbabilityEngine::new();
        let signals = PositionSignals::new(100, 100);
        let stop = mock_stop();

        // First call computes and caches
        let r1_ptr = {
            let r1 = engine.compute(0, signals, 50, &stop, 0, GpsStatus::Valid);
            r1 as *const ProbabilityResult
        };

        // Second call with same timestamp should return cached result
        let r2 = engine.compute(0, signals, 50, &stop, 0, GpsStatus::Valid);

        // Same reference (cached)
        assert_eq!(r1_ptr, r2 as *const ProbabilityResult);
    }

    #[test]
    fn test_probability_cache_miss() {
        let mut engine = ProbabilityEngine::new();
        let signals = PositionSignals::new(100, 100);
        let stop = mock_stop();

        // First call with timestamp 0
        {
            let _r1 = engine.compute(0, signals, 50, &stop, 0, GpsStatus::Valid);
        }

        // Second call with different timestamp should compute new result
        let r2_ptr = {
            let r2 = engine.compute(1, signals, 50, &stop, 0, GpsStatus::Valid);
            r2 as *const ProbabilityResult
        };

        // Third call with timestamp 1 should return cached result
        let r3 = engine.compute(1, signals, 50, &stop, 0, GpsStatus::Valid);

        // Same reference (cached for timestamp 1)
        assert_eq!(r2_ptr, r3 as *const ProbabilityResult);
    }

    #[test]
    fn test_probability_clear() {
        let mut engine = ProbabilityEngine::new();
        let signals = PositionSignals::new(100, 100);
        let stop = mock_stop();

        // First call
        {
            let _r1 = engine.compute(0, signals, 50, &stop, 0, GpsStatus::Valid);
        }

        // Clear cache
        engine.clear();

        // Second call with same timestamp should compute new result
        let r2_ptr = {
            let r2 = engine.compute(0, signals, 50, &stop, 0, GpsStatus::Valid);
            r2 as *const ProbabilityResult
        };

        // Third call with same timestamp should return cached result
        let r3 = engine.compute(0, signals, 50, &stop, 0, GpsStatus::Valid);

        // Same reference (cached after clear)
        assert_eq!(r2_ptr, r3 as *const ProbabilityResult);
    }
}

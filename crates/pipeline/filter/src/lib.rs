//! Corridor filtering for active stop selection
//!
//! This crate provides pure functions for finding stops within
//! the corridor of the current position. It is `no_std` compatible
//! and designed for firmware integration.
//!
//! # Example
//!
//! ```rust
//! use pipeline_filter::active_stops;
//! use shared::{DistCm, Stop};
//!
//! let stops = vec![
//!     Stop { progress_cm: 0, corridor_start_cm: 0, corridor_end_cm: 100 },
//!     Stop { progress_cm: 200, corridor_start_cm: 150, corridor_end_cm: 250 },
//! ];
//! let skip_flags = vec![false, false];
//!
//! // Find stops within corridor at position 50cm
//! let active = active_stops(50, &stops, &skip_flags);
//! assert_eq!(active, vec![0]);
//! ```

#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use shared::{DistCm, Stop};

/// Find stops within corridor of current position
///
/// Returns indices of stops where:
/// - s_cm is within [corridor_start_cm, corridor_end_cm]
/// - skip_flags[idx] is false
///
/// # Arguments
///
/// * `s_cm` - Current position along route (cm)
/// * `stops` - Slice of all stops
/// * `skip_flags` - Whether each stop should be skipped (e.g., re-entry skip)
///
/// # Returns
///
/// Vector of stop indices that are active (within corridor and not skipped)
pub fn active_stops(
    s_cm: DistCm,
    stops: &[Stop],
    skip_flags: &[bool],
) -> Vec<usize> {
    stops.iter()
        .enumerate()
        .filter(|(idx, stop)| {
            !skip_flags[*idx]
                && s_cm >= stop.corridor_start_cm
                && s_cm <= stop.corridor_end_cm
        })
        .map(|(idx, _)| idx)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use shared::Stop;

    fn mock_stops() -> Vec<Stop> {
        vec![
            Stop {
                progress_cm: 0,
                corridor_start_cm: 0,
                corridor_end_cm: 100,
            },
            Stop {
                progress_cm: 200,
                corridor_start_cm: 150,
                corridor_end_cm: 250,
            },
            Stop {
                progress_cm: 400,
                corridor_start_cm: 350,
                corridor_end_cm: 450,
            },
        ]
    }

    #[test]
    fn test_active_stops_single() {
        let stops = mock_stops();
        let skip = vec![false; stops.len()];
        let active = active_stops(50, &stops, &skip);
        assert_eq!(active, vec![0]);
    }

    #[test]
    fn test_active_stops_multiple() {
        let stops = mock_stops();
        let skip = vec![false; stops.len()];
        let active = active_stops(200, &stops, &skip);
        assert_eq!(active, vec![1]);
    }

    #[test]
    fn test_active_stops_empty() {
        let stops = mock_stops();
        let skip = vec![false; stops.len()];
        let active = active_stops(500, &stops, &skip);
        assert_eq!(active, vec![] as Vec<usize>);
    }

    #[test]
    fn test_active_stops_skip_flag() {
        let stops = mock_stops();
        let skip = vec![true, false, false];
        let active = active_stops(50, &stops, &skip);
        assert_eq!(active, vec![] as Vec<usize>); // Stop 0 is skipped
    }

    #[test]
    fn test_active_stops_corridor_boundary() {
        let stops = mock_stops();
        let skip = vec![false; stops.len()];
        // At exact boundary
        let active = active_stops(100, &stops, &skip);
        assert_eq!(active, vec![0]); // Inclusive of corridor_end_cm
    }
}

// Stop projection and corridor calculation
//
// Projects bus stops onto route segments and computes detection corridors.
// Ensures corridors don't overlap with minimum separation constraints.

pub mod validation;
pub use validation::{ValidationResult, ReversalInfo, validate_stop_sequence};

use shared::Stop;
use crate::input;

/// Project validated stops onto route and compute corridor boundaries.
///
/// # Arguments
/// * `progress_values` - Progress values in INPUT ORDER (already validated)
/// * `stops_input` - Original stops input (reserved for future use)
///
/// # Returns
/// Stops with corridor boundaries, sorted by progress (same as input order)
pub fn project_stops_validated(
    progress_values: &[i32],
    _stops_input: &input::StopsInput, // Reserved for future logging
) -> Vec<Stop> {
    let mut final_stops: Vec<Stop> = Vec::with_capacity(progress_values.len());

    for progress_cm in progress_values.iter() {
        let mut corridor_start_cm = progress_cm - 8000;
        let corridor_end_cm = progress_cm + 4000;

        // Overlap protection with previous stop
        if let Some(prev) = final_stops.last() {
            let min_separation = 2000; // 20m
            let min_start = prev.corridor_end_cm + min_separation;
            if corridor_start_cm < min_start {
                corridor_start_cm = min_start;
            }
        }

        // Final sanity check
        if corridor_start_cm >= *progress_cm {
            corridor_start_cm = *progress_cm - 1;
        }

        final_stops.push(Stop {
            progress_cm: *progress_cm,
            corridor_start_cm,
            corridor_end_cm,
        });
    }

    final_stops
}

#[cfg(test)]
mod tests;

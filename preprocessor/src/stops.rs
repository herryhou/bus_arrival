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

        // Overlap protection with previous stop (v8.5 spec: corridor_end[i] + δ_sep)
        if let Some(prev) = final_stops.last() {
            let min_start = prev.corridor_end_cm + 2000; // δ_sep = 2000 cm (20m)
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

/// Adjust corridor boundaries for closely-spaced stops.
///
/// For stops <120m apart, redistributes corridor space as:
/// - 55% before stop (pre-corridor)
/// - 10% gap between corridors
/// - 35% after stop (post-corridor)
///
/// This prevents overlap protection from compressing corridors
/// to the point where detection fails.
///
/// # Arguments
/// * `stops` - Stops with standard corridors (modified in place)
///
/// # Called by
/// main.rs after project_stops_validated(), before packing
pub fn preprocess_close_stop_corridors(stops: &mut [Stop]) {
    const CLOSE_STOP_THRESHOLD_CM: i32 = 12_000; // 120m
    const PRE_RATIO: i32 = 55;   // 0.55 × distance
    const POST_RATIO: i32 = 35;  // 0.35 × distance
    // Gap of 0.10 × distance forms naturally

    for i in 0..stops.len().saturating_sub(1) {
        let distance = stops[i + 1].progress_cm - stops[i].progress_cm;

        // Skip if distance is too small or at threshold
        if distance < 2_000 || distance >= CLOSE_STOP_THRESHOLD_CM {
            continue;
        }

        // Adjust current stop's post-corridor
        stops[i].corridor_end_cm =
            stops[i].progress_cm + (distance * POST_RATIO) / 100;

        // Adjust next stop's pre-corridor
        stops[i + 1].corridor_start_cm =
            stops[i + 1].progress_cm - (distance * PRE_RATIO) / 100;
    }
}

#[cfg(test)]
mod tests;

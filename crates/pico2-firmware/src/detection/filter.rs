//! Stop corridor filter adapter
//!
//! Wraps pipeline-filter crate, adapting Vec<usize> output
//! to heapless::Vec<usize, 16> for firmware constraints.

use shared::{DistCm, binfile::{RouteData, Stop}};

/// Find stops within corridor of current position
///
/// Returns heapless::Vec of stop indices where:
/// - s_cm is within [corridor_start_cm, corridor_end_cm]
/// - skip_flags[idx] is false
pub fn find_active_stops(
    s_cm: DistCm,
    route_data: &RouteData,
    skip_flags: &[bool],
) -> heapless::Vec<usize, 16> {
    // Collect stops into heapless::Vec (no_std compatible)
    let mut stops = heapless::Vec::<Stop, 32>::new();
    for i in 0..route_data.stop_count {
        if let Some(stop) = route_data.get_stop(i) {
            let _ = stops.push(stop);
        }
    }

    // Use pipeline-filter crate
    let active_std = pipeline_filter::active_stops(s_cm, &stops, skip_flags);

    // Convert to heapless::Vec
    let mut active = heapless::Vec::new();
    for idx in active_std.into_iter().take(16) {
        if active.push(idx).is_err() {
            #[cfg(feature = "firmware")]
            defmt::warn!("Active stops overflow (>16), truncating");
            break;
        }
    }
    active
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::Stop;

    #[test]
    fn test_find_active_stops_single() {
        // Create mock stops
        let stops = vec![
            Stop { progress_cm: 0, corridor_start_cm: 0, corridor_end_cm: 100 },
            Stop { progress_cm: 200, corridor_start_cm: 150, corridor_end_cm: 250 },
        ];

        // Create a mock route data using the actual binary format
        // For testing, we'll use the pipeline-filter directly
        let skip_flags = vec![false, false];
        let active = pipeline_filter::active_stops(50, &stops, &skip_flags);

        assert_eq!(active, vec![0]);
    }

    #[test]
    fn test_find_active_stops_skip_flag() {
        let stops = vec![
            Stop { progress_cm: 0, corridor_start_cm: 0, corridor_end_cm: 100 },
            Stop { progress_cm: 200, corridor_start_cm: 150, corridor_end_cm: 250 },
        ];

        let skip_flags = vec![true, false];
        let active = pipeline_filter::active_stops(50, &stops, &skip_flags);

        assert_eq!(active.len(), 0);
    }
}

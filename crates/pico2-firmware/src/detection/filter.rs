//! Stop corridor filter adapter
//!
//! Wraps pipeline-filter crate, adapting Vec<usize> output
//! to heapless::Vec<usize, 16> for firmware constraints.

use shared::{DistCm, Stop, binfile::RouteData};

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
    // Collect stops into heapless::Vec (owned values)
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

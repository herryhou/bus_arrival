//! Stop index recovery for GPS jump handling

use shared::{DistCm, Stop};

/// Trigger conditions
const GPS_JUMP_THRESHOLD: DistCm = 20000;  // 200 m

/// Find correct stop after GPS jump
///
/// Find closest stop within ±200m that is at or after the last known stop index.
/// For full implementation with velocity penalty and backward scoring, 
/// see tech report section 15.2.
pub fn find_stop_index(
    s_cm: DistCm,
    stops: &[Stop],
    last_index: u8,
) -> Option<usize> {
    // Candidates within ±200m and >= last_index - 1
    let mut candidates: Vec<(usize, DistCm)> = stops.iter()
        .enumerate()
        .filter(|&(i, stop)| {
            let d = (s_cm - stop.progress_cm).abs();
            d < GPS_JUMP_THRESHOLD && (i as u8) >= last_index.saturating_sub(1)
        })
        .map(|(i, stop)| (i, (s_cm - stop.progress_cm).abs()))
        .collect();

    // Sort by:
    // 1. Closest distance (primary)
    // 2. Higher index (secondary, to break ties in forward direction)
    candidates.sort_by(|a, b| {
        a.1.cmp(&b.1)  // Prefer smaller distance first
            .then_with(|| b.0.cmp(&a.0)) // Break ties with higher index
    });


    candidates.first().map(|(i, _)| *i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recovery_basic() {
        let stops = vec![
            Stop { progress_cm: 1000, corridor_start_cm: 0, corridor_end_cm: 2000 },
            Stop { progress_cm: 5000, corridor_start_cm: 4000, corridor_end_cm: 6000 },
            Stop { progress_cm: 9000, corridor_start_cm: 8000, corridor_end_cm: 10000 },
        ];

        // Jump to near second stop (idx 1)
        assert_eq!(find_stop_index(5100, &stops, 0), Some(1));
        
        // Cannot jump back too far (e.g. from idx 2 to 0 is not allowed by i >= last_index - 1)
        // From 2, it can jump to 1 or higher. 1100 is near stop 0, but stop 1 is also within 200m (3900cm away).
        // Distance to stop 1: |5000 - 1100| = 3900 < 20000. 
        // So it should pick Some(1) as it's the closest valid candidate.
        assert_eq!(find_stop_index(1100, &stops, 2), Some(1));
    }
}


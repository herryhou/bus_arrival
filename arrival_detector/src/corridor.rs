//! Stop corridor filter

use shared::{Stop, DistCm};

/// Find stops whose corridor contains the current route progress
pub fn find_active_stops(s_cm: DistCm, stops: &[Stop]) -> Vec<usize> {
    stops.iter()
        .enumerate()
        .filter(|(_, stop)| {
            s_cm >= stop.corridor_start_cm && s_cm <= stop.corridor_end_cm
        })
        .map(|(i, _)| i)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_active_stops() {
        let stops = vec![
            Stop { progress_cm: 10000, corridor_start_cm: 2000, corridor_end_cm: 14000 },
        ];
        assert!(find_active_stops(0, &stops).is_empty());
    }

    #[test]
    fn test_one_active_stop() {
        let stops = vec![
            Stop { progress_cm: 10000, corridor_start_cm: 2000, corridor_end_cm: 14000 },
        ];
        let result = find_active_stops(10000, &stops);
        assert_eq!(result, vec![0]);
    }
}

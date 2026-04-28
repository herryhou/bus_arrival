//! Recovery timeout and fallback logic
//!
//! When recovery repeatedly fails, timeout after 30 seconds and
//! fall back to geometric stop search (closest stop to current position).

use shared::DistCm;

const RECOVERING_TIMEOUT_SECONDS: u64 = 30;  // 30 seconds max

/// Check if recovery has timed out
///
/// Returns true if timeout occurred and fallback was executed
pub fn check_recovering_timeout(
    mode: super::SystemMode,
    recovering_since: Option<u64>,
    now: u64,
) -> bool {
    if mode != super::SystemMode::Recovering {
        return false;
    }

    let elapsed = recovering_since
        .map(|t| now.saturating_sub(t))
        .unwrap_or(0);

    elapsed > RECOVERING_TIMEOUT_SECONDS
}

/// Find closest stop index to current position (geometric fallback)
///
/// Used when recovery times out — finds nearest stop without
/// using hint_idx or frozen_s_cm.
pub fn find_closest_stop_index(
    s_cm: DistCm,
    stop_count: u8,
    get_stop: impl Fn(u8) -> Option<shared::Stop>,
) -> u8 {
    let mut closest_idx = 0;
    let mut closest_dist = i32::MAX;

    for i in 0..stop_count {
        if let Some(stop) = get_stop(i) {
            let dist = (s_cm - stop.progress_cm).abs();
            if dist < closest_dist {
                closest_dist = dist;
                closest_idx = i;
            }
        }
    }

    closest_idx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timeout_after_30_seconds() {
        use super::super::SystemMode;

        // Normal mode — no timeout
        assert!(!check_recovering_timeout(SystemMode::Normal, Some(0), 25));

        // Recovering for 25 seconds — no timeout
        assert!(!check_recovering_timeout(SystemMode::Recovering, Some(0), 25));

        // Recovering for 31 seconds — timeout
        assert!(check_recovering_timeout(SystemMode::Recovering, Some(0), 31));
    }

    #[test]
    fn test_find_closest_stop_index() {
        use shared::Stop;

        let stops = [
            Stop { progress_cm: 1000, corridor_start_cm: 0, corridor_end_cm: 2000 },
            Stop { progress_cm: 5000, corridor_start_cm: 4000, corridor_end_cm: 6000 },
            Stop { progress_cm: 9000, corridor_start_cm: 8000, corridor_end_cm: 10000 },
        ];

        // Position near second stop
        let idx = find_closest_stop_index(5500, 3, |i| stops.get(i as usize).copied());
        assert_eq!(idx, 1);

        // Position near first stop
        let idx = find_closest_stop_index(1500, 3, |i| stops.get(i as usize).copied());
        assert_eq!(idx, 0);
    }
}

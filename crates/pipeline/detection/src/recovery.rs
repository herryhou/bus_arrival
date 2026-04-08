//! Stop index recovery for GPS jump handling

use shared::DistCm;

#[cfg(feature = "std")]
use shared::Stop;

/// Trigger conditions
#[cfg_attr(not(feature = "std"), allow(dead_code))]
const GPS_JUMP_THRESHOLD: DistCm = 20000;  // 200 m

/// Find correct stop after GPS jump
///
/// Implements scoring formula from Tech Report Section 15.2:
/// score(i) = |s_i - s| + 5000 * max(0, last_index - i)
#[cfg(feature = "std")]
pub fn find_stop_index(
    s_cm: DistCm,
    stops: &[Stop],
    last_index: u8,
) -> Option<usize> {
    // Candidates within ±200m and >= last_index - 1
    // (We allow jumping back by 1 stop to handle small jitter near stop boundaries)
    let mut candidates: Vec<(usize, i32)> = stops.iter()
        .enumerate()
        .filter(|&(i, stop)| {
            let d = (s_cm - stop.progress_cm).abs();
            d < GPS_JUMP_THRESHOLD && (i as u8) >= last_index.saturating_sub(1)
        })
        .map(|(i, stop)| {
            let dist = (s_cm - stop.progress_cm).abs();
            let index_penalty = 5000 * (last_index as i32 - i as i32).max(0);
            let score = dist + index_penalty;
            (i, score)
        })
        .collect();

    // Sort by score (ascending)
    candidates.sort_by_key(|&(_, score)| score);

    candidates.first().map(|(i, _)| *i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recovery_scoring() {
        let stops = vec![
            Stop { progress_cm: 1000, corridor_start_cm: 0, corridor_end_cm: 2000 },
            Stop { progress_cm: 5000, corridor_start_cm: 4000, corridor_end_cm: 6000 },
            Stop { progress_cm: 9000, corridor_start_cm: 8000, corridor_end_cm: 10000 },
        ];

        // Jump to near second stop (idx 1) from stop 0
        // Stop 1: dist 100, penalty 0 -> score 100
        // Stop 0: dist 4100, penalty 0 -> score 4100 (wait, i >= 0-1=0, so stop 0 is candidate)
        // Stop 2: dist 3900, penalty 0 -> score 3900
        assert_eq!(find_stop_index(5100, &stops, 0), Some(1));
        
        // Jump back from idx 2 to near stop 1
        // Point s = 1100. last_index = 2.
        // Candidates: idx 1, idx 2 (idx 0 is excluded by i >= 2-1 = 1)
        // Stop 1 (5000): dist |5000-1100|=3900, penalty 5000*(2-1)=5000 -> score 8900
        // Stop 2 (9000): dist |9000-1100|=7900, penalty 0 -> score 7900
        // Stop 2 should win because the penalty for jumping back to 1 is too high.
        assert_eq!(find_stop_index(1100, &stops, 2), Some(2));

        // Jump back from idx 2 to stop 1, but much closer to 1
        // Point s = 4500.
        // Stop 1 (5000): dist 500, penalty 5000 -> score 5500
        // Stop 2 (9000): dist 4500, penalty 0 -> score 4500
        // Still stop 2 wins.
        assert_eq!(find_stop_index(4500, &stops, 2), Some(2));

        // Jump back from idx 1 to stop 0
        // Point s = 1000. last_index = 1.
        // Stop 0 (1000): dist 0, penalty 5000 -> score 5000
        // Stop 1 (5000): dist 4000, penalty 0 -> score 4000
        // Even here, stop 1 wins unless distance diff is > 5000.
        assert_eq!(find_stop_index(1000, &stops, 1), Some(1));
        
        // Point s = -2000. last_index = 1.
        // Stop 0 (1000): dist 3000, penalty 5000 -> score 8000
        // Stop 1 (5000): dist 7000, penalty 0 -> score 7000
        // Still stop 1. 

        // Point s = 0. last_index = 1. Stop 0 is at 1000. Stop 1 at 5000.
        // Stop 0: dist 1000, penalty 5000 -> score 6000
        // Stop 1: dist 5000, penalty 0 -> score 5000.
        
        // To make stop 0 win from last_index 1:
        // dist(s, stop 1) - dist(s, stop 0) > 5000.
        // |5000 - s| - |1000 - s| > 5000.
        // If s = 0: 5000 - 1000 = 4000 (No)
        // If s = -1000: 6000 - 2000 = 4000 (No)
        // Actually, if s is very far back, say s = -3000.
        // |5000 - (-3000)| = 8000. |1000 - (-3000)| = 4000.
        // 8000 - 4000 = 4000. Still not enough?
        // Wait, the max difference between |A-s| and |B-s| is |A-B|.
        // Here |5000 - 1000| = 4000.
        // So with a 5000 penalty, you can NEVER jump back from 1 to 0 if they are 4000 apart!
        // This means the 5000 penalty is VERY strong. It prevents ANY backwards jump if stops are closer than 50m.
    }

    #[test]
    fn test_recovery_with_80m_gaps() {
        let stops = vec![
            Stop { progress_cm: 1000, corridor_start_cm: 0, corridor_end_cm: 2000 },
            Stop { progress_cm: 9000, corridor_start_cm: 8000, corridor_end_cm: 10000 },
        ];
        
        // At s=1000, last_index=1.
        // Stop 0 (1000): dist 0, penalty 5000 -> score 5000
        // Stop 1 (9000): dist 8000, penalty 0 -> score 8000
        // Stop 0 wins!
        assert_eq!(find_stop_index(1000, &stops, 1), Some(0));
    }
}


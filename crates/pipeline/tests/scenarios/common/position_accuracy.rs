//! Position accuracy validation helpers
//!
//! Verifies that detected stops are within acceptable distance of the stop location.

/// Check if a position is acceptable for arrival detection
///
/// Tech report requirement: At AtStop, distance should be < 50m (5000cm)
/// In practice, we allow some tolerance for GPS drift.
pub const ACCEPTABLE_ARRIVAL_DISTANCE_CM: i32 = 5000; // 50m
pub const GOOD_ARRIVAL_DISTANCE_CM: i32 = 3000;       // 30m

/// Position accuracy result
#[derive(Debug)]
pub struct PositionAccuracyReport {
    pub total_arrivals: usize,
    pub within_good_threshold: usize,
    pub within_acceptable_threshold: usize,
    pub beyond_threshold: Vec<(usize, i32)>, // (stop_idx, distance_cm)
}

impl PositionAccuracyReport {
    pub fn print_report(&self) {
        println!("\n=== POSITION ACCURACY REPORT ===");
        println!("Total arrivals: {}", self.total_arrivals);
        println!("Within 30m (good): {} ({:.1}%)",
            self.within_good_threshold,
            (self.within_good_threshold as f64 / self.total_arrivals as f64) * 100.0);
        println!("Within 50m (acceptable): {} ({:.1}%)",
            self.within_acceptable_threshold,
            (self.within_acceptable_threshold as f64 / self.total_arrivals as f64) * 100.0);

        if !self.beyond_threshold.is_empty() {
            println!("\nBeyond 50m threshold (FAIL):");
            for (stop_idx, dist) in &self.beyond_threshold {
                println!("  Stop {}: {:.1}m from stop", stop_idx, *dist as f64 / 100.0);
            }
        }
    }

    /// Assert that all positions are within acceptable threshold
    pub fn assert_all_acceptable(&self) -> Result<(), String> {
        if !self.beyond_threshold.is_empty() {
            Err(format!(
                "{} stops detected beyond 50m threshold: {:?}",
                self.beyond_threshold.len(),
                self.beyond_threshold
            ))
        } else {
            Ok(())
        }
    }
}

/// Analyze position accuracy from trace data
pub fn analyze_position_accuracy(
    trace_path: &std::path::Path,
) -> PositionAccuracyReport {
    use std::io::BufRead;

    let file = std::fs::File::open(trace_path)
        .unwrap_or_else(|e| panic!("Failed to open trace: {:?}", e));
    let reader = std::io::BufReader::new(file);

    let mut at_stop_distances: Vec<(usize, i32)> = Vec::new();

    for line in reader.lines() {
        let line = line.unwrap();
        if line.trim().is_empty() {
            continue;
        }

        let record: serde_json::Value = serde_json::from_str(&line)
            .unwrap_or_else(|e| panic!("Failed to parse trace line: {:?}", e));

        if let Some(stop_states) = record.get("stop_states") {
            if let Some(states) = stop_states.as_array() {
                for state in states {
                    if state["fsm_state"] == "AtStop" {
                        let stop_idx = state["stop_idx"].as_u64().unwrap() as usize;
                        let distance_cm = state["distance_cm"].as_i64().unwrap() as i32;
                        at_stop_distances.push((stop_idx, distance_cm.abs()));
                    }
                }
            }
        }
    }

    let total = at_stop_distances.len();
    let within_good = at_stop_distances.iter()
        .filter(|(_, d)| *d <= GOOD_ARRIVAL_DISTANCE_CM)
        .count();
    let within_acceptable = at_stop_distances.iter()
        .filter(|(_, d)| *d <= ACCEPTABLE_ARRIVAL_DISTANCE_CM)
        .count();
    let beyond = at_stop_distances.into_iter()
        .filter(|(_, d)| *d > ACCEPTABLE_ARRIVAL_DISTANCE_CM)
        .collect();

    PositionAccuracyReport {
        total_arrivals: total,
        within_good_threshold: within_good,
        within_acceptable_threshold: within_acceptable,
        beyond_threshold: beyond,
    }
}

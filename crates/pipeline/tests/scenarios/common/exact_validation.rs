//! Exact validation helpers for scenario tests
//!
//! Provides functions to compare detected arrivals against ground truth
//! with precision/recall metrics and detailed mismatch reporting.
#![allow(dead_code)]

use serde_json::Value;
use std::collections::HashSet;

/// Validation result comparing detected vs expected arrivals
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Expected arrivals from ground truth
    pub expected: Vec<usize>,
    /// Detected arrivals from pipeline
    pub detected: Vec<usize>,
    /// True positives: correctly detected stops
    pub true_positives: HashSet<usize>,
    /// False positives: detected but not in ground truth
    pub false_positives: HashSet<usize>,
    /// False negatives: in ground truth but not detected
    pub false_negatives: HashSet<usize>,
    /// Precision: TP / (TP + FP)
    pub precision: f64,
    /// Recall: TP / (TP + FN)
    pub recall: f64,
    /// F1 score: harmonic mean of precision and recall
    pub f1: f64,
}

impl ValidationResult {
    /// Print detailed validation report
    pub fn print_report(&self) {
        println!("\n=== ARRIVAL VALIDATION REPORT ===");
        println!("Expected stops: {}", self.expected.len());
        println!("Detected stops: {}", self.detected.len());
        println!("True positives: {}", self.true_positives.len());
        println!("False positives: {}", self.false_positives.len());
        println!("False negatives: {}", self.false_negatives.len());
        println!("Precision: {:.2}%", self.precision * 100.0);
        println!("Recall: {:.2}%", self.recall * 100.0);
        println!("F1 Score: {:.2}", self.f1);

        let mut fps: Vec<_> = self.false_positives.iter().copied().collect();
        let mut fns: Vec<_> = self.false_negatives.iter().copied().collect();
        fps.sort();
        fns.sort();

        if !self.false_positives.is_empty() {
            println!("\nFalse positives (ghost stops): {:?}", fps);
        }
        if !self.false_negatives.is_empty() {
            println!("\nFalse negatives (missed stops): {:?}", fns);
        }
    }

    /// Assert that validation meets quality thresholds
    pub fn assert_quality(&self, min_precision: f64, min_recall: f64) -> Result<(), String> {
        if self.precision < min_precision {
            let mut fps: Vec<_> = self.false_positives.iter().copied().collect();
            fps.sort();
            return Err(format!(
                "Precision {:.2}% is below threshold {:.2}%. False positives: {:?}",
                self.precision * 100.0,
                min_precision * 100.0,
                fps
            ));
        }
        if self.recall < min_recall {
            let mut fns: Vec<_> = self.false_negatives.iter().copied().collect();
            fns.sort();
            return Err(format!(
                "Recall {:.2}% is below threshold {:.2}%. False negatives: {:?}",
                self.recall * 100.0,
                min_recall * 100.0,
                fns
            ));
        }
        Ok(())
    }
}

/// Validate detected arrivals against ground truth
pub fn validate_arrivals_exact(
    detected: &[usize],
    expected: &[usize],
) -> ValidationResult {
    let detected_set: HashSet<usize> = detected.iter().copied().collect();
    let expected_set: HashSet<usize> = expected.iter().copied().collect();

    let true_positives: HashSet<usize> = detected_set
        .intersection(&expected_set)
        .copied()
        .collect();

    let false_positives: HashSet<usize> = detected_set
        .difference(&expected_set)
        .copied()
        .collect();

    let false_negatives: HashSet<usize> = expected_set
        .difference(&detected_set)
        .copied()
        .collect();

    let tp = true_positives.len() as f64;
    let fp = false_positives.len() as f64;
    let fn_count = false_negatives.len() as f64;

    let precision = if tp + fp > 0.0 {
        tp / (tp + fp)
    } else {
        1.0
    };

    let recall = if tp + fn_count > 0.0 {
        tp / (tp + fn_count)
    } else {
        1.0
    };

    let f1 = if precision + recall > 0.0 {
        2.0 * (precision * recall) / (precision + recall)
    } else {
        0.0
    };

    ValidationResult {
        expected: expected.to_vec(),
        detected: detected.to_vec(),
        true_positives,
        false_positives,
        false_negatives,
        precision,
        recall,
        f1,
    }
}

/// Load expected arrivals from ground truth JSON
pub fn load_expected_arrivals(scenario: &str) -> Vec<usize> {
    let filename = format!("ty225_{}_arrivals.json", scenario);
    let mut path = super::test_data_dir();
    path.push(&filename);
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to load {}: {:?}", filename, e));

    content
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            let value: Value = serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("Failed to parse line in {}: {:?}", filename, e));
            value["stop_idx"].as_u64().unwrap() as usize
        })
        .collect()
}

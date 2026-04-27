//! Common utilities for scenario integration tests
#![allow(dead_code)]

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

/// Test data directory path
pub fn test_data_dir() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("../../test_data");
    path
}

/// Load ty225 route data binary
pub fn load_ty225_route(scenario: &str) -> Vec<u8> {
    let filename = format!("ty225_{}.bin", scenario);
    let mut path = test_data_dir();
    path.push(&filename);
    fs::read(&path)
        .unwrap_or_else(|e| panic!("Failed to load {}: {:?}", filename, e))
}

/// Load NMEA test data as a buffered reader
pub fn load_nmea_reader(scenario: &str) -> BufReader<fs::File> {
    let filename = format!("ty225_{}_nmea.txt", scenario);
    let mut path = test_data_dir();
    path.push(&filename);
    let file = fs::File::open(&path)
        .unwrap_or_else(|e| panic!("Failed to open {}: {:?}", filename, e));
    BufReader::new(file)
}

/// Load NMEA test data as lines
pub fn load_nmea(scenario: &str) -> Vec<String> {
    let reader = load_nmea_reader(scenario);
    reader.lines().map(|l| l.unwrap()).collect()
}

/// Load trace file as a buffered reader
pub fn load_trace_reader(scenario: &str) -> BufReader<fs::File> {
    let filename = format!("ty225_{}_trace.jsonl", scenario);
    let mut path = test_data_dir();
    path.push(&filename);
    let file = fs::File::open(&path)
        .unwrap_or_else(|e| panic!("Failed to open {}: {:?}", filename, e));
    BufReader::new(file)
}

/// Load expected arrivals from ground truth JSON
pub fn load_expected_arrivals(scenario: &str) -> Vec<usize> {
    let filename = format!("ty225_{}_arrivals.json", scenario);
    let mut path = test_data_dir();
    path.push(&filename);
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to load {}: {:?}", filename, e));

    // Parse arrivals JSONL - each line is a separate JSON object with "stop_idx"
    content
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            let value: serde_json::Value = serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("Failed to parse line in {}: {:?}", filename, e));
            value["stop_idx"].as_u64().unwrap() as usize
        })
        .collect()
}

/// Expected results for scenario validation
#[derive(Debug, Clone)]
pub struct ExpectedResults {
    pub arrivals: Vec<usize>,
    pub min_arrivals: usize,
    pub max_arrivals: usize,
}

impl ExpectedResults {
    /// Create expected results from ground truth
    pub fn from_ground_truth(scenario: &str) -> Self {
        let arrivals = load_expected_arrivals(scenario);
        let count = arrivals.len();
        Self {
            arrivals,
            min_arrivals: count.saturating_sub(1),
            max_arrivals: count + 1,
        }
    }

    /// Create expected results with custom bounds
    pub fn with_bounds(min: usize, max: usize) -> Self {
        Self {
            arrivals: Vec::new(),
            min_arrivals: min,
            max_arrivals: max,
        }
    }
}

/// Scenario test result
pub type TestResult = Result<(), String>;

pub mod exact_validation;
pub mod order_validation;
pub mod position_accuracy;

pub use exact_validation::validate_arrivals_exact;
pub use order_validation::{validate_arrival_order, validate_arrival_order_strict};

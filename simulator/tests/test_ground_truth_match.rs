//! Test to verify ground truth against actual route data
//!
//! This test checks if the ground truth stops align with the route data.

mod common;
use common::load_test_asset_bytes;
use shared::binfile::RouteData;

#[test]
fn test_ground_truth_alignment() {
    let data = load_test_asset_bytes("ty225.bin");
    let route_data = RouteData::load(&data).expect("Failed to load route data");

    // Load ground truth
    let gt_json = std::fs::read_to_string("/Users/herry/project/pico2w/bus_arrival/ground_truth.json")
        .expect("Failed to read ground_truth.json");
    let ground_truth: Vec<serde_json::Value> = serde_json::from_str(&gt_json)
        .expect("Failed to parse ground_truth.json");

    println!("Ground truth has {} stops", ground_truth.len());
    println!("Route data has {} stops", route_data.stop_count);

    // Check each ground truth stop
    for (i, gt_stop) in ground_truth.iter().enumerate() {
        let stop_idx = gt_stop["stop_idx"].as_u64().unwrap() as usize;
        let seg_idx = gt_stop["seg_idx"].as_u64().unwrap() as usize;
        let timestamp = gt_stop["timestamp"].as_u64().unwrap();

        println!("\nGround truth Stop {} (stop_idx={}, seg_idx={}, timestamp={})",
            i, stop_idx, seg_idx, timestamp);

        if let Some(stop) = route_data.get_stop(stop_idx) {
            println!("  Route Stop {}: progress_cm={}, corridor=[{}, {}]",
                stop_idx, stop.progress_cm, stop.corridor_start_cm, stop.corridor_end_cm);

            // Check what node corresponds to this seg_idx
            if let Some(node) = route_data.get_node(seg_idx) {
                let cum_dist = node.cum_dist_cm;
                let x_cm = node.x_cm;
                let y_cm = node.y_cm;
                println!("  Node {}: cum_dist_cm={}, coord=({},{})",
                    seg_idx, cum_dist, x_cm, y_cm);

                // Compare stop progress with node cumulative distance
                let diff = stop.progress_cm as i64 - cum_dist as i64;
                println!("  Difference: stop_progress - node_cum_dist = {} cm ({} m)",
                    diff, diff / 100);
            }
        }
    }

    // Also check the first few GPS readings from jsonl
    println!("\n=== Checking GPS data from jsonl ===");
    let jsonl_path = "/Users/herry/project/pico2w/bus_arrival/ty225.jsonl";
    if let Ok(file) = std::fs::File::open(jsonl_path) {
        use std::io::{BufRead, BufReader};
        let reader = BufReader::new(file);
        for (line_num, line) in reader.lines().take(10).enumerate() {
            if let Ok(json_str) = line {
                if let Ok(record) = serde_json::from_str::<serde_json::Value>(&json_str) {
                    let time = record["time"].as_u64().unwrap();
                    let s_cm = record["s_cm"].as_i64().unwrap();
                    let seg_idx = record["seg_idx"].as_u64().unwrap() as usize;
                    println!("Line {}: time={}, s_cm={}, seg_idx={}",
                        line_num, time, s_cm, seg_idx);

                    // Find which stop corridor this s_cm would be in
                    for i in 0..route_data.stop_count {
                        if let Some(stop) = route_data.get_stop(i) {
                            if s_cm >= stop.corridor_start_cm as i64 && s_cm <= stop.corridor_end_cm as i64 {
                                println!("  -> In Stop {} corridor [{}, {}]",
                                    i, stop.corridor_start_cm, stop.corridor_end_cm);
                            }
                        }
                    }
                }
            }
        }
    }
}

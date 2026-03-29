//! Test to check the new loop closure stop

mod common;
use common::load_test_asset_bytes;
use shared::binfile::{RouteData, BusError};

#[test]
fn test_loop_closure_stop() {
    let data = load_test_asset_bytes("ty225_loop_stop.bin");
    let route_data = match RouteData::load(&data) {
        Ok(data) => data,
        Err(BusError::InvalidVersion) => {
            eprintln!("Skipping test: ty225_loop_stop.bin is VERSION 2, needs to be regenerated to VERSION 3");
            return;
        }
        Err(e) => panic!("Failed to load route data: {:?}", e),
    };

    println!("Route data: {} nodes, {} stops", route_data.node_count, route_data.stop_count);

    // Check the last stop (the one we added)
    let last_idx = route_data.stop_count - 1;
    if let Some(stop) = route_data.get_stop(last_idx) {
        println!("Stop {} (loop closure): progress_cm={}, corridor=[{}, {}]",
            last_idx, stop.progress_cm, stop.corridor_start_cm, stop.corridor_end_cm);
    }

    // Check second-to-last stop
    let second_last_idx = route_data.stop_count - 2;
    if let Some(stop) = route_data.get_stop(second_last_idx) {
        println!("Stop {}: progress_cm={}, corridor=[{}, {}]",
            second_last_idx, stop.progress_cm, stop.corridor_start_cm, stop.corridor_end_cm);
    }

    // Test if GPS at s_cm=1720784 is in any stop's corridor
    let gps_s_cm = 1720784i32;
    println!("\nTesting GPS at s_cm={}:", gps_s_cm);

    let mut found_active = false;
    for i in 0..route_data.stop_count {
        if let Some(stop) = route_data.get_stop(i) {
            if gps_s_cm >= stop.corridor_start_cm && gps_s_cm <= stop.corridor_end_cm {
                println!("  Active Stop {}: progress={}, corridor=[{}, {}], distance={} cm",
                    i, stop.progress_cm, stop.corridor_start_cm, stop.corridor_end_cm,
                    stop.progress_cm as i32 - gps_s_cm);
                found_active = true;
            }
        }
    }

    if !found_active {
        println!("  No active stops found at s_cm={}", gps_s_cm);
    }
}

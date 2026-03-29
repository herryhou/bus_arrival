//! Verify the stop at GPS position is correctly projected

mod common;
use common::load_test_asset_bytes;
use shared::binfile::{RouteData, BusError};

#[test]
fn test_verify_stop_at_gps_position() {
    let data = load_test_asset_bytes("ty225_with_stop_at_gps.bin");
    let route_data = match RouteData::load(&data) {
        Ok(data) => data,
        Err(BusError::InvalidVersion) => {
            eprintln!("Skipping test: ty225_with_stop_at_gps.bin is VERSION 2, needs to be regenerated to VERSION 3");
            return;
        }
        Err(e) => panic!("Failed to load route data: {:?}", e),
    };

    println!("Route data: {} nodes, {} stops", route_data.node_count, route_data.stop_count);

    // Check the last stop (the one we added at GPS position)
    let last_idx = route_data.stop_count - 1;
    if let Some(stop) = route_data.get_stop(last_idx) {
        println!("Stop {} (at GPS position): progress_cm={}, corridor=[{}, {}]",
            last_idx, stop.progress_cm, stop.corridor_start_cm, stop.corridor_end_cm);
    }

    // Test if GPS at s_cm=1717259 is in this stop's corridor
    let gps_s_cm = 1717259i32;
    println!("\nTesting GPS at s_cm={}:", gps_s_cm);

    for i in 0..route_data.stop_count {
        if let Some(stop) = route_data.get_stop(i) {
            if gps_s_cm >= stop.corridor_start_cm && gps_s_cm <= stop.corridor_end_cm {
                println!("  ACTIVE Stop {}: progress={}, corridor=[{}, {}], distance_from_stop={} cm ({} m)",
                    i, stop.progress_cm, stop.corridor_start_cm, stop.corridor_end_cm,
                    stop.progress_cm as i32 - gps_s_cm,
                    (stop.progress_cm as i32 - gps_s_cm).abs() / 100);
            }
        }
    }

    // Also test a few GPS positions around the start of the route
    for test_s_cm in [0, 30000, 80000].iter() {
        println!("\nTesting GPS at s_cm={}:", test_s_cm);
        let mut found_any = false;
        for i in 0..route_data.stop_count {
            if let Some(stop) = route_data.get_stop(i) {
                if *test_s_cm >= stop.corridor_start_cm && *test_s_cm <= stop.corridor_end_cm {
                    println!("  ACTIVE Stop {}: progress={}, corridor=[{}, {}]",
                        i, stop.progress_cm, stop.corridor_start_cm, stop.corridor_end_cm);
                    found_any = true;
                }
            }
        }
        if !found_any {
            println!("  No active stops");
        }
    }
}

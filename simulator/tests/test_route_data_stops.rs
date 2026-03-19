//! Test to check the actual stops in ty225.bin
//!
//! This test loads the actual ty225.bin file and checks the stop data.

mod common;
use common::load_test_asset_bytes;
use shared::binfile::RouteData;

#[test]
fn test_ty225_bin_stops() {
    let data = load_test_asset_bytes("ty225.bin");
    let route_data = RouteData::load(&data).expect("Failed to load route data");

    println!("Route data: {} nodes, {} stops", route_data.node_count, route_data.stop_count);

    for i in 0..route_data.stop_count {
        if let Some(stop) = route_data.get_stop(i) {
            println!("Stop {}: progress_cm={}, corridor_start_cm={}, corridor_end_cm={}",
                i, stop.progress_cm, stop.corridor_start_cm, stop.corridor_end_cm);
        }
    }

    // Check first few GPS readings from ty225.jsonl
    // The first line shows s_cm: 1717209
    // Let's see if any stop corridor contains this value
    let test_s_cm = 1717209i32;
    println!("\nTesting s_cm = {}:", test_s_cm);

    let mut found_any = false;
    for i in 0..route_data.stop_count {
        if let Some(stop) = route_data.get_stop(i) {
            if test_s_cm >= stop.corridor_start_cm && test_s_cm <= stop.corridor_end_cm {
                println!("  s_cm {} is in corridor of Stop {} [{}, {}]",
                    test_s_cm, i, stop.corridor_start_cm, stop.corridor_end_cm);
                found_any = true;
            }
        }
    }

    if !found_any {
        println!("  s_cm {} is NOT in any stop corridor!", test_s_cm);
    }

    // Check progress_cm values relative to s_cm
    println!("\nStop progress_cm values:");
    for i in 0..route_data.stop_count.min(5) {
        if let Some(stop) = route_data.get_stop(i) {
            let diff = stop.progress_cm as i64 - test_s_cm as i64;
            println!("  Stop {}: progress_cm={}, diff_from_s_cm={} cm ({} m)",
                i, stop.progress_cm, diff, diff / 100);
        }
    }
}

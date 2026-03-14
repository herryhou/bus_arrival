//! Test to check the debug route data with 57 stops

use shared::binfile::RouteData;

#[test]
fn test_debug_ty225_bin_stops() {
    let data = std::fs::read("/tmp/ty225_debug.bin")
        .expect("Failed to read /tmp/ty225_debug.bin");
    let route_data = RouteData::load(&data).expect("Failed to load route data");

    println!("Route data: {} nodes, {} stops", route_data.node_count, route_data.stop_count);

    // Show first 5 stops
    for i in 0..route_data.stop_count.min(5) {
        if let Some(stop) = route_data.get_stop(i) {
            println!("Stop {}: progress_cm={}, corridor=[{}, {}]",
                i, stop.progress_cm, stop.corridor_start_cm, stop.corridor_end_cm);
        }
    }

    // Test with GPS position s_cm = 1717209 (from the original jsonl)
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

    // Check progress_cm values around the GPS position
    println!("\nStops around s_cm = {}:", test_s_cm);
    for i in 0..route_data.stop_count {
        if let Some(stop) = route_data.get_stop(i) {
            let diff = stop.progress_cm as i64 - test_s_cm as i64;
            // Show stops within 5km
            if diff.abs() < 500000 {
                println!("  Stop {}: progress_cm={}, diff={} cm ({} m)",
                    i, stop.progress_cm, diff, diff / 100);
            }
        }
    }

    assert_eq!(route_data.stop_count, 57, "Should have 57 stops");
}

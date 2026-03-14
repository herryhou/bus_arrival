//! Test to check active_stops with the new bin file

use shared::binfile::RouteData;

#[test]
fn test_new_ty225_bin_active_stops() {
    let data = std::fs::read("/Users/herry/project/pico2w/bus_arrival/ty225_new.bin")
        .expect("Failed to read ty225_new.bin");
    let route_data = RouteData::load(&data).expect("Failed to load route data");

    println!("Route data: {} nodes, {} stops", route_data.node_count, route_data.stop_count);

    // Check GPS positions from the new jsonl
    let gps_positions = vec![
        (1, 1717259i64),
        (100, 1725259i64),
        (500, 1765259i64),
        (1000, 1805259i64),
    ];

    for (time, s_cm) in gps_positions {
        let stops = route_data.stops();
        let active_stops: Vec<usize> = stops.iter()
            .enumerate()
            .filter(|(_, stop)| {
                let s = s_cm as i32;
                s >= stop.corridor_start_cm && s <= stop.corridor_end_cm
            })
            .map(|(i, _)| i)
            .collect();

        println!("time={}, s_cm={}, active_stops={:?}", time, s_cm, active_stops);
        if !active_stops.is_empty() {
            for &idx in &active_stops {
                if let Some(stop) = route_data.get_stop(idx) {
                    println!("  Stop {}: progress={}, corridor=[{}, {}]",
                        idx, stop.progress_cm, stop.corridor_start_cm, stop.corridor_end_cm);
                }
            }
        }
    }

    // Show all stops with their progress values
    println!("\nAll stops (first 10 and last 5):");
    for i in 0..route_data.stop_count {
        if i < 10 || i >= route_data.stop_count - 5 {
            if let Some(stop) = route_data.get_stop(i) {
                println!("Stop {:2}: progress={:8}, corridor=[{:8}, {:8}]",
                    i, stop.progress_cm, stop.corridor_start_cm, stop.corridor_end_cm);
            }
        } else if i == 10 {
            println!("  ...");
        }
    }
}

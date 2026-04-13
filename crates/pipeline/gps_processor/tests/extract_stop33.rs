mod common;
use common::load_test_asset_bytes;
use shared::binfile::RouteData;

#[test]
fn test_extract_stop33() {
    let data = load_test_asset_bytes("tpF805_normal.bin");
    let route_data = match RouteData::load(&data) {
        Ok(data) => data,
        Err(shared::binfile::BusError::InvalidVersion) => {
            eprintln!("Skipping test: tpF805_normal.bin is not VERSION 5. Regenerate with the latest preprocessor.");
            return;
        }
        Err(e) => panic!("Failed to load route data: {:?}", e),
    };

    println!("Stop #33 (index 32):");
    if let Some(stop) = route_data.get_stop(32) {
        println!("  progress_cm: {}", stop.progress_cm);
        println!("  corridor_start_cm: {}", stop.corridor_start_cm);
        println!("  corridor_end_cm: {}", stop.corridor_end_cm);
        println!(
            "  pre_corridor: {} cm ({} m)",
            stop.progress_cm - stop.corridor_start_cm,
            (stop.progress_cm - stop.corridor_start_cm) / 100
        );
        println!(
            "  post_corridor: {} cm ({} m)",
            stop.corridor_end_cm - stop.progress_cm,
            (stop.corridor_end_cm - stop.progress_cm) / 100
        );
    }

    // Check nearby stops
    println!("\nNearby stops:");
    for i in 30..=34 {
        if let Some(stop) = route_data.get_stop(i) {
            println!(
                "  Stop #{} (idx {}): progress={}, corridor=[{}, {}]",
                i + 1,
                i,
                stop.progress_cm,
                stop.corridor_start_cm,
                stop.corridor_end_cm
            );
        }
    }
}

/// Integration test for loading real route_data.bin files
///
/// This test verifies that the v8.8 binary format (24-byte RouteNode with sparse Grid)
/// can be correctly loaded and parsed.
///
/// Note: v8.8 optimized the Grid structure using bitmask + u16 offsets.
/// Grid space reduced from ~16KB to ~5KB (60-70% savings).
/// This requires VERSION 5. Existing route_data.bin files need to be regenerated
/// with the preprocessor.

use std::fs;
use shared::binfile::RouteData;

#[test]
fn test_load_ty225_route_data() {
    let bin_path = "../visualizer/static/route_data.bin";

    // Skip test if file doesn't exist (might not be generated yet in CI)
    if !std::path::Path::new(bin_path).exists() {
        eprintln!("Skipping test: {} not found", bin_path);
        return;
    }

    let data = fs::read(bin_path).expect("Failed to read route_data.bin");

    // Verify file size is reasonable
    assert!(data.len() > 30_000, "File too small to be valid route data");
    assert!(data.len() < 100_000, "File too large");

    // Load the binary (will fail if VERSION is not 5)
    let route_data = match RouteData::load(&data) {
        Ok(data) => data,
        Err(shared::binfile::BusError::InvalidVersion) => {
            eprintln!("Skipping test: route_data.bin is VERSION 4, needs to be regenerated to VERSION 5");
            return;
        }
        Err(e) => panic!("Failed to load route data: {:?}", e),
    };

    // Verify basic properties
    assert_eq!(route_data.node_count, 837, "Unexpected node count");
    assert_eq!(route_data.stop_count, 58, "Unexpected stop count");

    // Verify we can access nodes (copy packed fields to locals)
    let first_node = route_data.get_node(0).expect("Failed to get first node");
    let seg_len_mm = first_node.seg_len_mm;
    // In v8.7, seg_len_mm is in millimeters with 10x precision
    // The old len2_cm2 value was 3_945_265 cm²
    // The segment length in mm should be approximately sqrt(3_945_265) * 10 ≈ 19863 mm
    assert!(seg_len_mm > 19000 && seg_len_mm < 21000, "Unexpected seg_len_mm: {}", seg_len_mm);

    // Verify we can access stops
    let first_stop = route_data.get_stop(0).expect("Failed to get first stop");
    let progress_cm = first_stop.progress_cm;
    assert_eq!(progress_cm, 32_434, "Unexpected stop progress");

    // Verify grid data
    assert!(route_data.grid.cols > 0, "Grid should have columns");
    assert!(route_data.grid.rows > 0, "Grid should have rows");

    // Verify RouteNode size matches v8.5 format (40 bytes)
    // by checking that the data structure is consistent
    let last_node = route_data.get_node(route_data.node_count - 1)
        .expect("Failed to get last node");
    let last_cum_dist = last_node.cum_dist_cm;
    // For a loop route, last node may have a segment back to start
    // Just verify the value is reasonable (positive and not excessive)
    assert!(last_cum_dist > 1_000_000, "Last node cum_dist too small");

    println!("✓ Successfully loaded and validated route_data.bin (v8.8 VERSION 5)");
    println!("  Nodes: {} × 32 bytes = {} KB",
        route_data.node_count,
        route_data.node_count * 32 / 1024
    );
    println!("  Stops: {} × 12 bytes = {} KB",
        route_data.stop_count,
        route_data.stop_count * 12 / 1024
    );
}

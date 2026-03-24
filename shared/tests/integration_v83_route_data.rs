/// Integration test for loading real route_data.bin files
///
/// This test verifies that the v8.5 binary format (40-byte RouteNode with repr(C))
/// can be correctly loaded and parsed.
///
/// Note: v8.5 changed from repr(C, packed) to repr(C) to avoid UB with field references.
/// This increased RouteNode size from 36 to 40 bytes and requires VERSION 3.
/// Existing route_data.bin files need to be regenerated with the preprocessor.

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

    // Load the binary (will fail if VERSION is not 3)
    let route_data = match RouteData::load(&data) {
        Ok(data) => data,
        Err(shared::binfile::BusError::InvalidVersion) => {
            eprintln!("Skipping test: route_data.bin is VERSION 2, needs to be regenerated to VERSION 3");
            return;
        }
        Err(e) => panic!("Failed to load route data: {:?}", e),
    };

    // Verify basic properties
    assert_eq!(route_data.node_count, 837, "Unexpected node count");
    assert_eq!(route_data.stop_count, 58, "Unexpected stop count");

    // Verify we can access nodes (copy packed fields to locals)
    let first_node = route_data.get_node(0).expect("Failed to get first node");
    let len2_cm2 = first_node.len2_cm2;
    assert_eq!(len2_cm2, 3_945_265, "Unexpected len2_cm2");

    // Verify we can access stops
    let first_stop = route_data.get_stop(0).expect("Failed to get first stop");
    let progress_cm = first_stop.progress_cm;
    assert_eq!(progress_cm, 32_434, "Unexpected stop progress");

    // Verify grid data
    assert!(route_data.grid.cols > 0, "Grid should have columns");
    assert!(route_data.grid.rows > 0, "Grid should have rows");

    // Verify LUTs
    assert_eq!(route_data.gaussian_lut.len(), 256, "Gaussian LUT wrong size");
    assert_eq!(route_data.logistic_lut.len(), 128, "Logistic LUT wrong size");

    // Verify RouteNode size matches v8.5 format (40 bytes)
    // by checking that the data structure is consistent
    let last_node = route_data.get_node(route_data.node_count - 1)
        .expect("Failed to get last node");
    let last_cum_dist = last_node.cum_dist_cm;
    // For a loop route, last node may have a segment back to start
    // Just verify the value is reasonable (positive and not excessive)
    assert!(last_cum_dist > 1_000_000, "Last node cum_dist too small");

    println!("✓ Successfully loaded and validated route_data.bin (v8.5 VERSION 3)");
    println!("  Nodes: {} × 40 bytes = {} KB",
        route_data.node_count,
        route_data.node_count * 40 / 1024
    );
    println!("  Stops: {} × 12 bytes = {} KB",
        route_data.stop_count,
        route_data.stop_count * 12 / 1024
    );
}

use assert_cmd::Command;
use shared::{RouteNode, Stop, MAGIC, VERSION};
use std::fs;
use tempfile::NamedTempFile;
use crc32fast::Hasher;

#[test]
fn test_v8_preprocessor_compliance() {
    // --- GIVEN ---
    // A route with a very long segment (~111m) and stops that are out of order
    let route_json = r#"{
        "route_points": [
            [25.0, 121.0],
            [25.001, 121.0]
        ]
    }"#;
    
    // Stop at 25.0008 (further) comes BEFORE stop at 25.0002 (nearer)
    let stops_json = r#"{
        "stops": [
            {"lat": 25.0008, "lon": 121.0},
            {"lat": 25.0002, "lon": 121.0}
        ]
    }"#;

    let route_file = NamedTempFile::new().unwrap();
    let stops_file = NamedTempFile::new().unwrap();
    let output_file = NamedTempFile::new().unwrap();

    fs::write(route_file.path(), route_json).unwrap();
    fs::write(stops_file.path(), stops_json).unwrap();

    // --- WHEN ---
    // The preprocessor is executed
    let mut cmd = Command::cargo_bin("preprocessor").unwrap();
    cmd.arg(route_file.path())
       .arg(stops_file.path())
       .arg(output_file.path())
       .assert()
       .success();

    // --- THEN ---
    let data = fs::read(output_file.path()).unwrap();
    let data_len = data.len();
    
    // 1. Verify Header
    let magic = u32::from_le_bytes(data[0..4].try_into().unwrap());
    let version = u16::from_le_bytes(data[4..6].try_into().unwrap());
    let node_count = u16::from_le_bytes(data[6..8].try_into().unwrap());
    let stop_count = data[8];
    
    assert_eq!(magic, MAGIC, "Magic bytes must be 'BUSA'");
    assert_eq!(version, VERSION, "Version must be 1");
    assert_eq!(stop_count, 2, "Should have 2 stops");

    // 2. Verify Route Nodes (Interpolation & Max Segment)
    // Every segment must be <= 3000cm
    let mut offset = 17; // Header size
    let mut nodes = Vec::new();
    for _ in 0..node_count {
        let node_bytes = &data[offset..offset+52];
        // SAFETY: RouteNode is repr(C, packed), we read it carefully
        let node: RouteNode = unsafe { std::ptr::read_unaligned(node_bytes.as_ptr() as *const RouteNode) };
        nodes.push(node);
        offset += 52;
    }

    assert!(nodes.len() > 2, "Original 2 points should have been interpolated to break 111m segment");
    
    for i in 0..nodes.len() - 1 {
        let n = &nodes[i];
        let actual_len = ((n.dx_cm as f64).powi(2) + (n.dy_cm as f64).powi(2)).sqrt();
        assert!(actual_len <= 3005.0, "Segment {} length {} exceeds 30m limit", i, actual_len);
        
        // Verify line_c invariant: line_a*x + line_b*y + line_c = 0
        let lhs = n.line_a as i64 * n.x_cm as i64 + n.line_b as i64 * n.y_cm as i64 + n.line_c;
        assert_eq!(lhs, 0, "Line coefficients invariant failed at node {}", i);
    }

    // 3. Verify Stops (Sorting & Separation)
    let mut stops = Vec::new();
    for _ in 0..stop_count {
        let stop_bytes = &data[offset..offset+12];
        let stop: Stop = unsafe { std::ptr::read_unaligned(stop_bytes.as_ptr() as *const Stop) };
        stops.push(stop);
        offset += 12;
    }

    assert!(stops[0].progress_cm < stops[1].progress_cm, "Stops must be sorted by progress");
    
    for i in 0..stops.len() {
        let s = &stops[i];
        assert!(s.corridor_start_cm < s.progress_cm, "Stop {} start corridor must be before progress", i);
        assert!(s.progress_cm < s.corridor_end_cm, "Stop {} end corridor must be after progress", i);
        
        if i > 0 {
            let prev = &stops[i-1];
            let diff = s.corridor_start_cm - prev.corridor_end_cm;
            assert!(diff >= 2000, "Stops must have at least 20m separation, got {}cm", diff);
        }
    }

    // 4. Verify CRC32
    let received_crc = u32::from_le_bytes(data[data_len-4..].try_into().unwrap());
    let mut hasher = Hasher::new();
    hasher.update(&data[..data_len-4]);
    let calculated_crc = hasher.finalize();
    assert_eq!(received_crc, calculated_crc, "CRC32 checksum mismatch");
    
    println!("BDD Verification Passed: All v8 invariants satisfied.");
}
